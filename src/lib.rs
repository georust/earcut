//! A Rust port of the [Earcut](https://github.com/mapbox/earcut) polygon triangulation library.

#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use core::{cmp::Ordering, num::NonZeroU32, ptr};

use num_traits::float::Float;

macro_rules! node {
    ($self:ident.$nodes:ident, $offset:expr) => {
        // SAFETY: all `NodeOffset`s used by this crate follow the invariant
        // documented on `node_at`.
        unsafe { $crate::node_at(&$self.$nodes, $offset) }
    };
    ($nodes:ident, $offset:expr) => {
        // SAFETY: all `NodeOffset`s used by this crate follow the invariant
        // documented on `node_at`.
        unsafe { $crate::node_at($nodes, $offset) }
    };
}

macro_rules! node_mut {
    ($self:ident.$nodes:ident, $offset:expr) => {
        // SAFETY: all `NodeOffset`s used by this crate follow the invariant
        // documented on `node_at`.
        unsafe { $crate::node_at_mut(&mut $self.$nodes, $offset) }
    };
    ($nodes:ident, $offset:expr) => {
        // SAFETY: all `NodeOffset`s used by this crate follow the invariant
        // documented on `node_at`.
        unsafe { $crate::node_at_mut($nodes, $offset) }
    };
}

pub mod int;
pub mod utils3d;

/// Index of a vertex
pub trait Index: Copy {
    fn into_usize(self) -> usize;
    fn from_usize(v: usize) -> Self;
}
impl Index for u32 {
    fn into_usize(self) -> usize {
        self as usize
    }
    fn from_usize(v: usize) -> Self {
        v as Self
    }
}
impl Index for u16 {
    fn into_usize(self) -> usize {
        self as usize
    }
    fn from_usize(v: usize) -> Self {
        v as Self
    }
}
impl Index for usize {
    fn into_usize(self) -> usize {
        self
    }
    fn from_usize(v: usize) -> Self {
        v as Self
    }
}

/// Byte offset of a node in the nodes Vec.
///
/// Non-zero so `Option<NodeOffset>` keeps `Node` small.
type NodeOffset = NonZeroU32;

/// # Safety
///
/// `offset` must point to a valid element in `nodes`.
///
/// In this crate, this holds because:
///
/// - each offset is created with `node_offset::<N>` and is only used after its
///   node has been appended.
/// - nodes are only appended to the Vec and are never removed or reordered, so
///   each offset keeps identifying the same node.
/// - offsets are byte offsets, not raw pointers. Each access adds the offset to
///   the current `nodes.as_ptr()`, so Vec reallocation does not make offsets
///   invalid.
/// - `node_offset` checks that the byte offset fits in `u32`.
#[inline(always)]
unsafe fn node_at<N>(nodes: &[N], offset: NodeOffset) -> &N {
    let off = offset.get() as usize;
    let stride = core::mem::size_of::<N>();
    debug_assert!(stride > 0);
    debug_assert!(off.is_multiple_of(stride));
    debug_assert!(off / stride < nodes.len());
    // SAFETY: the caller guarantees that `offset` is valid for `nodes`;
    // see the safety contract above.
    unsafe { &*nodes.as_ptr().byte_add(off) }
}

/// # Safety
///
/// `offset` must point to a valid element in `nodes`. See `node_at` for the
/// full invariant.
#[inline(always)]
unsafe fn node_at_mut<N>(nodes: &mut [N], offset: NodeOffset) -> &mut N {
    let off = offset.get() as usize;
    let stride = core::mem::size_of::<N>();
    debug_assert!(stride > 0);
    debug_assert!(off.is_multiple_of(stride));
    debug_assert!(off / stride < nodes.len());
    // SAFETY: the caller guarantees that `offset` is valid for `nodes`;
    // see the safety contract above.
    unsafe { &mut *nodes.as_mut_ptr().byte_add(off) }
}

/// Creates a byte offset for a real node.
///
/// Index 0 is the dummy node, so real node offsets are non-zero.
#[inline(always)]
fn node_offset<N>(index: usize) -> NodeOffset {
    let stride = core::mem::size_of::<N>();
    assert!(index > 0 && index <= u32::MAX as usize / stride);
    let byte_offset = (index * stride) as u32;
    NodeOffset::new(byte_offset).unwrap()
}

const STEINER_BIT: u32 = 1 << 31;
const INDEX_MASK: u32 = !STEINER_BIT;

struct Node<T: Float> {
    /// vertex index in coordinates array (lower 31 bits) + steiner flag (bit 31)
    i_steiner: u32,
    /// z-order curve value
    z: i32,
    /// vertex coordinates
    xy: [T; 2],
    /// previous vertex nodes in a polygon ring
    prev_i: NodeOffset,
    /// next vertex nodes in a polygon ring
    next_i: NodeOffset,
    /// previous nodes in z-order
    prev_z_i: Option<NodeOffset>,
    /// next nodes in z-order
    next_z_i: Option<NodeOffset>,
}

struct LinkInfo {
    prev_i: NodeOffset,
    next_i: NodeOffset,
    prev_z_i: Option<NodeOffset>,
    next_z_i: Option<NodeOffset>,
}

const HOLE_BLOCK_SIZE: usize = 16;

/// Bounding-box blocks used while holes are merged into the outer ring.
struct HoleBlockIndex<T: Float> {
    bboxes: Vec<[T; 4]>,
    heads: Vec<NodeOffset>,
    stops: Vec<NodeOffset>,
}

impl<T: Float> HoleBlockIndex<T> {
    fn new() -> Self {
        Self {
            bboxes: Vec::new(),
            heads: Vec::new(),
            stops: Vec::new(),
        }
    }

    fn reset(&mut self, max_nodes: usize, num_holes: usize) {
        self.bboxes.clear();
        self.heads.clear();
        self.stops.clear();

        let indexed_nodes = max_nodes.saturating_add(num_holes.saturating_mul(2));
        let max_blocks = indexed_nodes
            .div_ceil(HOLE_BLOCK_SIZE)
            .saturating_add(num_holes)
            .saturating_add(2);
        self.bboxes.reserve(max_blocks);
        self.heads.reserve(max_blocks);
        self.stops.reserve(max_blocks);
    }

    /// Index the ring run `head..stop`; `head == stop` means the whole ring.
    fn index_segment(&mut self, nodes: &mut [Node<T>], head_i: NodeOffset, stop_i: NodeOffset) {
        let mut p_i = head_i;
        loop {
            let block = self.bboxes.len();
            self.heads.push(p_i);

            let mut min_x = T::infinity();
            let mut min_y = T::infinity();
            let mut max_x = T::neg_infinity();
            let mut max_y = T::neg_infinity();
            let mut count = 0;

            loop {
                let p = node!(nodes, p_i);
                let (next_i, [x, y]) = (p.next_i, p.xy);
                let [next_x, next_y] = node!(nodes, next_i).xy;
                node_mut!(nodes, p_i).z = i32::try_from(block).expect("too many hole index blocks");

                min_x = min_x.min(x).min(next_x);
                min_y = min_y.min(y).min(next_y);
                max_x = max_x.max(x).max(next_x);
                max_y = max_y.max(y).max(next_y);

                p_i = next_i;
                count += 1;
                if count == HOLE_BLOCK_SIZE || p_i == stop_i {
                    break;
                }
            }

            self.bboxes.push([min_x, min_y, max_x, max_y]);
            self.stops.push(p_i);
            if p_i == stop_i {
                break;
            }
        }
    }

    fn grow(&mut self, block: usize, [x, y]: [T; 2]) {
        let bbox = &mut self.bboxes[block];
        bbox[0] = bbox[0].min(x);
        bbox[1] = bbox[1].min(y);
        bbox[2] = bbox[2].max(x);
        bbox[3] = bbox[3].max(y);
    }

    fn live_head(&mut self, nodes: &[Node<T>], block: usize) -> NodeOffset {
        let mut head_i = self.heads[block];
        let mut prev_i = node!(nodes, head_i).prev_i;
        while node!(nodes, prev_i).next_i != head_i {
            head_i = node!(nodes, head_i).next_i;
            prev_i = node!(nodes, head_i).prev_i;
        }
        self.heads[block] = head_i;
        head_i
    }

    fn live_stop(&mut self, nodes: &[Node<T>], block: usize) -> NodeOffset {
        let mut stop_i = self.stops[block];
        let mut prev_i = node!(nodes, stop_i).prev_i;
        while node!(nodes, prev_i).next_i != stop_i {
            stop_i = node!(nodes, stop_i).next_i;
            prev_i = node!(nodes, stop_i).prev_i;
        }
        self.stops[block] = stop_i;
        stop_i
    }
}

impl<T: Float> Node<T> {
    const PLACEHOLDER_OFFSET: NodeOffset =
        NodeOffset::new(core::mem::size_of::<Self>() as u32).unwrap();

    fn new(i: u32, xy: [T; 2]) -> Self {
        debug_assert!(i & STEINER_BIT == 0);
        Self {
            i_steiner: i,
            xy,
            prev_i: Self::PLACEHOLDER_OFFSET,
            next_i: Self::PLACEHOLDER_OFFSET,
            z: 0,
            prev_z_i: None,
            next_z_i: None,
        }
    }

    #[inline(always)]
    fn index(&self) -> u32 {
        self.i_steiner & INDEX_MASK
    }

    #[inline(always)]
    fn is_steiner(&self) -> bool {
        self.i_steiner & STEINER_BIT != 0
    }

    #[inline(always)]
    fn set_steiner(&mut self) {
        self.i_steiner |= STEINER_BIT;
    }

    fn link_info(&self) -> LinkInfo {
        LinkInfo {
            prev_i: self.prev_i,
            next_i: self.next_i,
            prev_z_i: self.prev_z_i,
            next_z_i: self.next_z_i,
        }
    }
}

/// Instance of the earcut algorithm.
pub struct Earcut<T: Float> {
    data: Vec<[T; 2]>,
    nodes: Vec<Node<T>>,
    queue: Vec<(NodeOffset, T)>,
    sort_queue: Vec<(i32, NodeOffset)>,
    sort_scratch: Vec<(i32, NodeOffset)>,
    hole_blocks: HoleBlockIndex<T>,
}

impl<T: Float> Default for Earcut<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Float> Earcut<T> {
    /// Creates a new instance of the earcut algorithm.
    ///
    /// You can reuse a single instance for multiple triangulations to reduce memory allocations.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            nodes: Vec::new(),
            queue: Vec::new(),
            sort_queue: Vec::new(),
            sort_scratch: Vec::new(),
            hole_blocks: HoleBlockIndex::new(),
        }
    }

    fn reset(&mut self, capacity: usize) {
        self.nodes.clear();
        self.nodes.reserve(capacity);
        self.nodes
            .push(Node::new(0, [T::infinity(), T::infinity()])); // dummy node
    }

    /// Performs the earcut triangulation on a polygon.
    ///
    /// The API is similar to the original JavaScript implementation, except you can provide a vector for the output indices.
    ///
    /// # Panics
    ///
    /// - if `hole_indices` contains a value greater than the number of
    ///   vertices in `data`, or is not monotonically non-decreasing.
    /// - if the internal node storage exceeds the `u32` byte-offset capacity
    pub fn earcut<N: Index>(
        &mut self,
        data: impl IntoIterator<Item = [T; 2]>,
        hole_indices: &[N],
        triangles_out: &mut Vec<N>,
    ) {
        self.data.clear();
        self.data.extend(data);
        let data = core::mem::take(&mut self.data);
        self.earcut_slice(&data, hole_indices, triangles_out);
        self.data = data;
    }

    /// Same as [`earcut`](Self::earcut), but takes the vertices as a slice, which is
    /// read in place instead of being copied into an internal buffer first.
    ///
    /// # Panics
    ///
    /// Same as [`earcut`](Self::earcut).
    pub fn earcut_slice<N: Index>(
        &mut self,
        data: &[[T; 2]],
        hole_indices: &[N],
        triangles_out: &mut Vec<N>,
    ) {
        triangles_out.clear();
        self.run(data, hole_indices, triangles_out);
    }

    #[deprecated(since = "0.4.10", note = "internal entry point; call `earcut` instead")]
    pub fn earcut_impl<N: Index>(&mut self, hole_indices: &[N], triangles_out: &mut Vec<N>) {
        let data = core::mem::take(&mut self.data);
        self.run(&data, hole_indices, triangles_out);
        self.data = data;
    }

    fn run<N: Index>(&mut self, data: &[[T; 2]], hole_indices: &[N], triangles_out: &mut Vec<N>) {
        if data.len() < 3 {
            return;
        }
        assert!(data.len() <= INDEX_MASK as usize + 1);

        triangles_out.reserve(data.len().saturating_mul(3));
        self.reset(data.len() / 2 * 3);

        let has_holes = !hole_indices.is_empty();
        let outer_len: usize = if has_holes {
            hole_indices[0].into_usize()
        } else {
            data.len()
        };

        // create nodes
        let Some(mut outer_node_i) = self.linked_list(data, 0, outer_len, true) else {
            return;
        };
        let outer_node = node!(self.nodes, outer_node_i);
        if outer_node.next_i == outer_node.prev_i {
            return;
        }
        if has_holes {
            outer_node_i = self.eliminate_holes(data, hole_indices, outer_node_i);
        }

        let mut min_x = T::zero();
        let mut min_y = T::zero();
        let mut inv_size = T::zero();

        // if the shape is not too simple, we'll use z-order curve hash later; calculate polygon bbox
        if data.len() > 80 {
            let [x0, y0] = data[0];
            let (mut mnx, mut mny, mut mxx, mut mxy) = (x0, y0, x0, y0);
            for &[x, y] in &data[1..outer_len] {
                mnx = T::min(mnx, x);
                mny = T::min(mny, y);
                mxx = T::max(mxx, x);
                mxy = T::max(mxy, y);
            }
            min_x = mnx;
            min_y = mny;
            // minX, minY and invSize are later used to transform coords into integers for z-order calculation
            inv_size = (mxx - mnx).max(mxy - mny);
            if inv_size != T::zero() {
                inv_size = T::from(32767.0).unwrap() / inv_size;
            }
        }

        earcut_linked(
            &mut self.nodes,
            outer_node_i,
            triangles_out,
            min_x,
            min_y,
            inv_size,
            &mut self.sort_queue,
            &mut self.sort_scratch,
        );
    }

    /// create a circular doubly linked list from polygon points in the specified winding order
    fn linked_list(
        &mut self,
        data: &[[T; 2]],
        start: usize,
        end: usize,
        clockwise: bool,
    ) -> Option<NodeOffset> {
        let data = &data[start..end];
        if data.is_empty() {
            return None;
        }

        let mut last_i: Option<NodeOffset> = None;
        let iter = data.iter().enumerate();

        if clockwise == (signed_area(data) > T::zero()) {
            for (i, &xy) in iter {
                let idx = start + i;
                last_i = Some(insert_node(&mut self.nodes, idx as u32, xy, last_i));
            }
        } else {
            for (i, &xy) in iter.rev() {
                let idx = start + i;
                last_i = Some(insert_node(&mut self.nodes, idx as u32, xy, last_i));
            }
        };

        if let Some(li) = last_i {
            let last = node!(self.nodes, li);
            if equals(last, node!(self.nodes, last.next_i)) {
                let ll = last.link_info();
                let (_, next_i) = remove_node(&mut self.nodes, ll);
                last_i = Some(next_i);
            }
        }

        last_i
    }

    /// link every hole into the outer loop, producing a single-ring polygon without holes
    fn eliminate_holes<N: Index>(
        &mut self,
        data: &[[T; 2]],
        hole_indices: &[N],
        mut outer_node_i: NodeOffset,
    ) -> NodeOffset {
        self.queue.clear();
        for (i, hi) in hole_indices.iter().enumerate() {
            let start = (*hi).into_usize();
            let end = if i < hole_indices.len() - 1 {
                hole_indices[i + 1].into_usize()
            } else {
                data.len()
            };
            if let Some(list_i) = self.linked_list(data, start, end, false) {
                let list = &mut node_mut!(self.nodes, list_i);
                if list_i == list.next_i {
                    list.set_steiner();
                }
                let (leftmost_i, leftmost) = get_leftmost(&self.nodes, list_i);
                self.queue.push((leftmost_i, leftmost.xy[0]));
            }
        }

        self.queue.sort_by(|(ai, ax), (bi, bx)| {
            // compareXYSlope
            match ax.partial_cmp(bx) {
                Some(Ordering::Equal) => {}
                Some(ordering) => return ordering,
                None => return Ordering::Equal,
            }
            // when the left-most point of 2 holes meet at a vertex, sort the holes counterclockwise so that when we find
            // the bridge to the outer shell is always the point that they meet at.
            let a = node!(self.nodes, *ai);
            let b = node!(self.nodes, *bi);
            match a.xy[1].partial_cmp(&b.xy[1]) {
                Some(Ordering::Equal) => {}
                Some(ordering) => return ordering,
                None => return Ordering::Equal,
            };
            let a_next = node!(self.nodes, a.next_i);
            let b_next = node!(self.nodes, b.next_i);
            let a_slope = (a_next.xy[1] - a.xy[1]) / (a_next.xy[0] - a.xy[0]);
            let b_slope = (b_next.xy[1] - b.xy[1]) / (b_next.xy[0] - b.xy[0]);
            a_slope.partial_cmp(&b_slope).unwrap_or(Ordering::Equal)
        });

        self.hole_blocks.reset(data.len(), hole_indices.len());
        self.hole_blocks
            .index_segment(&mut self.nodes, outer_node_i, outer_node_i);

        // Process holes from left to right. Each merged hole extends the
        // append-only block index used by subsequent bridge searches.
        for i in 0..self.queue.len() {
            let q = self.queue[i].0;
            outer_node_i = eliminate_hole(&mut self.nodes, q, outer_node_i, &mut self.hole_blocks);
        }

        // Collapse collinear/coincident points across the whole merged ring
        // once before clipping.
        filter_points(&mut self.nodes, outer_node_i, None, None).0
    }
}

/// main ear slicing loop which triangulates a polygon (given as a linked list)
#[allow(clippy::too_many_arguments)]
fn earcut_linked<T: Float, N: Index>(
    nodes: &mut Vec<Node<T>>,
    ear_i: NodeOffset,
    triangles: &mut Vec<N>,
    min_x: T,
    min_y: T,
    inv_size: T,
    sort_queue: &mut Vec<(i32, NodeOffset)>,
    sort_scratch: &mut Vec<(i32, NodeOffset)>,
) {
    let mut ear_i = ear_i;

    // interlink polygon nodes in z-order
    if inv_size != T::zero() {
        index_curve(
            nodes,
            ear_i,
            min_x,
            min_y,
            inv_size,
            sort_queue,
            sort_scratch,
        );
    }

    let mut stop_i = ear_i;
    let mut cured = false;

    // iterate through ears, slicing them one by one
    loop {
        let ear = node!(nodes, ear_i);
        if ear.prev_i == ear.next_i {
            break;
        }
        let ni = ear.next_i;

        let (is_ear, prev, next) = if inv_size != T::zero() {
            is_ear_hashed(nodes, ear, min_x, min_y, inv_size)
        } else {
            is_ear(nodes, ear)
        };
        if is_ear {
            let next_i = next.index();
            let next_node_i = ear.next_i;

            // cut off the triangle
            triangles.extend([
                N::from_usize(prev.index() as usize),
                N::from_usize(ear.index() as usize),
                N::from_usize(next_i as usize),
            ]);

            let ll = ear.link_info();
            remove_node(nodes, ll);

            (ear_i, stop_i) = (next_node_i, next_node_i);

            continue;
        }

        ear_i = ni;

        // if we looped through the whole remaining polygon and can't find any more ears
        if ear_i == stop_i {
            // Filtering a point can expose another removable point or a new
            // ear, so keep clipping while filtering makes progress.
            let (filtered_i, filtered_out) = filter_points(nodes, ear_i, None, None);
            ear_i = filtered_i;
            if filtered_out {
                stop_i = ear_i;
                continue;
            }

            // Cure local self-intersections once before falling back to a
            // polygon split.
            if !cured {
                ear_i = cure_local_intersections(nodes, ear_i, triangles);
                stop_i = ear_i;
                cured = true;
                continue;
            }

            split_earcut(
                nodes,
                ear_i,
                triangles,
                min_x,
                min_y,
                inv_size,
                sort_queue,
                sort_scratch,
            );
            break;
        }
    }
}

/// check whether a polygon node forms a valid ear with adjacent nodes
fn is_ear<'a, T: Float>(
    nodes: &'a [Node<T>],
    ear: &'a Node<T>,
) -> (bool, &'a Node<T>, &'a Node<T>) {
    let b = ear;
    let a = node!(nodes, b.prev_i);
    let c = node!(nodes, b.next_i);

    if area(a, b, c) >= T::zero() {
        // reflex, can't be an ear
        return (false, a, c);
    }

    // now make sure we don't have other points inside the potential ear

    // triangle bbox for z-order range
    let x0 = a.xy[0].min(b.xy[0].min(c.xy[0]));
    let y0 = a.xy[1].min(b.xy[1].min(c.xy[1]));
    let x1 = a.xy[0].max(b.xy[0].max(c.xy[0]));
    let y1 = a.xy[1].max(b.xy[1].max(c.xy[1]));

    let mut p = node!(nodes, c.next_i);
    let mut p_prev = node!(nodes, p.prev_i);
    while !ptr::eq(p, a) {
        let p_next = node!(nodes, p.next_i);
        if (p.xy[0] >= x0 && p.xy[0] <= x1 && p.xy[1] >= y0 && p.xy[1] <= y1)
            && point_in_triangle_except_first(a.xy, b.xy, c.xy, p.xy)
            && area(p_prev, p, p_next) >= T::zero()
        {
            return (false, a, c);
        }
        (p_prev, p) = (p, p_next);
    }
    (true, a, c)
}

fn is_ear_hashed<'a, T: Float>(
    nodes: &'a [Node<T>],
    ear: &'a Node<T>,
    min_x: T,
    min_y: T,
    inv_size: T,
) -> (bool, &'a Node<T>, &'a Node<T>) {
    let b = ear;
    let a = node!(nodes, b.prev_i);
    let c = node!(nodes, b.next_i);

    if area(a, b, c) >= T::zero() {
        // reflex, can't be an ear
        return (false, a, c);
    }

    // Cache vertex coordinates in stack locals so the inner loops don't
    // reload them through the (potentially aliased) node references.
    let a_xy = a.xy;
    let b_xy = b.xy;
    let c_xy = c.xy;

    // triangle bbox
    let xy_min = [
        a_xy[0].min(b_xy[0].min(c_xy[0])),
        a_xy[1].min(b_xy[1].min(c_xy[1])),
    ];
    let xy_max = [
        a_xy[0].max(b_xy[0].max(c_xy[0])),
        a_xy[1].max(b_xy[1].max(c_xy[1])),
    ];

    // z-order range for the current triangle bbox;
    let min_z = z_order(xy_min, min_x, min_y, inv_size);
    let max_z = z_order(xy_max, min_x, min_y, inv_size);

    // Walk both z-order directions interleaved: the two pointer chases are
    // independent, so the CPU keeps two loads in flight instead of one on
    // this latency-bound traversal. Any hit returns immediately, so the visit
    // order does not change the result.
    //
    // `point_in_triangle_except_first` already rejects points coinciding with
    // `a`, so only the shared `c` vertex needs an identity check (matches the
    // C++ reference).
    let mut o_n = ear.next_z_i.map(|i| node!(nodes, i));
    let mut o_p = ear.prev_z_i.map(|i| node!(nodes, i));
    while let (Some(n), Some(p)) = (o_n, o_p) {
        if n.z > max_z || p.z < min_z {
            break;
        }
        if !ptr::eq(n, c)
            && point_in_triangle_except_first(a_xy, b_xy, c_xy, n.xy)
            && area(node!(nodes, n.prev_i), n, node!(nodes, n.next_i)) >= T::zero()
        {
            return (false, a, c);
        }
        if !ptr::eq(p, c)
            && point_in_triangle_except_first(a_xy, b_xy, c_xy, p.xy)
            && area(node!(nodes, p.prev_i), p, node!(nodes, p.next_i)) >= T::zero()
        {
            return (false, a, c);
        }
        o_n = n.next_z_i.map(|i| node!(nodes, i));
        o_p = p.prev_z_i.map(|i| node!(nodes, i));
    }

    // drain the remaining increasing z-order side
    while let Some(n) = o_n {
        if n.z > max_z {
            break;
        };
        if !ptr::eq(n, c)
            && point_in_triangle_except_first(a_xy, b_xy, c_xy, n.xy)
            && area(node!(nodes, n.prev_i), n, node!(nodes, n.next_i)) >= T::zero()
        {
            return (false, a, c);
        }
        o_n = n.next_z_i.map(|i| node!(nodes, i));
    }

    // drain the remaining decreasing z-order side
    while let Some(p) = o_p {
        if p.z < min_z {
            break;
        };
        if !ptr::eq(p, c)
            && point_in_triangle_except_first(a_xy, b_xy, c_xy, p.xy)
            && area(node!(nodes, p.prev_i), p, node!(nodes, p.next_i)) >= T::zero()
        {
            return (false, a, c);
        }
        o_p = p.prev_z_i.map(|i| node!(nodes, i));
    }

    (true, a, c)
}

/// go through all polygon nodes and cure small local self-intersections
fn cure_local_intersections<T: Float, N: Index>(
    nodes: &mut [Node<T>],
    mut start_i: NodeOffset,
    triangles: &mut Vec<N>,
) -> NodeOffset {
    let mut p_i = start_i;
    let mut cured = false;
    loop {
        let p = node!(nodes, p_i);
        let p_next_i = p.next_i;
        let p_next = node!(nodes, p_next_i);
        let b_i = p_next.next_i;
        let a = node!(nodes, p.prev_i);
        let b = node!(nodes, b_i);

        if !equals(a, b)
            && intersects(a, p, p_next, b, false)
            && locally_inside(nodes, a, b)
            && locally_inside(nodes, b, a)
        {
            triangles.extend([
                N::from_usize(a.index() as usize),
                N::from_usize(p.index() as usize),
                N::from_usize(b.index() as usize),
            ]);

            let b_next_i = b.next_i;
            remove_node(nodes, p.link_info());
            let pnl = node!(nodes, p_next_i).link_info();
            remove_node(nodes, pnl);

            (p_i, start_i) = (b_next_i, b_i);
            cured = true;
        } else {
            p_i = p.next_i;
        }

        if p_i == start_i {
            return if cured {
                filter_points(nodes, p_i, None, None).0
            } else {
                p_i
            };
        }
    }
}

/// try splitting polygon into two and triangulate them independently
#[allow(clippy::too_many_arguments)]
fn split_earcut<T: Float, N: Index>(
    nodes: &mut Vec<Node<T>>,
    start_i: NodeOffset,
    triangles: &mut Vec<N>,
    min_x: T,
    min_y: T,
    inv_size: T,
    sort_queue: &mut Vec<(i32, NodeOffset)>,
    sort_scratch: &mut Vec<(i32, NodeOffset)>,
) {
    // look for a valid diagonal that divides the polygon into two
    let mut ai = start_i;
    let mut a = node!(nodes, ai);
    loop {
        let a_next = node!(nodes, a.next_i);
        let a_prev = node!(nodes, a.prev_i);
        let mut bi = a_next.next_i;

        while bi != a.prev_i {
            let b = node!(nodes, bi);
            if a.index() != b.index() && is_valid_diagonal(nodes, a, b, a_next, a_prev) {
                // split the polygon in two by the diagonal
                let mut ci = split_polygon(nodes, ai, bi);

                // filter colinear points around the cuts
                let end_i = Some(node!(nodes, ai).next_i);
                ai = filter_points(nodes, ai, end_i, None).0;
                let end_i = Some(node!(nodes, ci).next_i);
                ci = filter_points(nodes, ci, end_i, None).0;

                // run earcut on each half
                earcut_linked(
                    nodes,
                    ai,
                    triangles,
                    min_x,
                    min_y,
                    inv_size,
                    sort_queue,
                    sort_scratch,
                );
                earcut_linked(
                    nodes,
                    ci,
                    triangles,
                    min_x,
                    min_y,
                    inv_size,
                    sort_queue,
                    sort_scratch,
                );
                return;
            }
            bi = b.next_i;
        }

        ai = a.next_i;
        if ai == start_i {
            return;
        }
        a = a_next;
    }
}

/// interlink polygon nodes in z-order
fn index_curve<T: Float>(
    nodes: &mut [Node<T>],
    start_i: NodeOffset,
    min_x: T,
    min_y: T,
    inv_size: T,
    order: &mut Vec<(i32, NodeOffset)>,
    scratch: &mut Vec<(i32, NodeOffset)>,
) {
    order.clear();
    let mut p_i = start_i;
    let mut p = node_mut!(nodes, p_i);

    loop {
        // `z` temporarily holds the hole block id during hole elimination.
        p.z = z_order(p.xy, min_x, min_y, inv_size);
        order.push((p.z, p_i));
        p_i = p.next_i;
        p = node_mut!(nodes, p_i);
        if p_i == start_i {
            break;
        }
    }

    sort_by_z(order, scratch);

    for idx in 0..order.len() {
        let prev_z_i = if idx > 0 {
            Some(order[idx - 1].1)
        } else {
            None
        };
        let next_z_i = order.get(idx + 1).map(|&(_, i)| i);
        let p = node_mut!(nodes, order[idx].1);
        p.prev_z_i = prev_z_i;
        p.next_z_i = next_z_i;
    }
}

/// Sort a large z-order queue with four stable byte-wise radix passes.
fn sort_by_z(order: &mut [(i32, NodeOffset)], scratch: &mut Vec<(i32, NodeOffset)>) {
    const RADIX_MIN_LEN: usize = 512;
    let n = order.len();
    if n < RADIX_MIN_LEN {
        order.sort_unstable_by_key(|&(z, _)| z);
        return;
    }

    #[inline(always)]
    fn scatter(
        src: &[(i32, NodeOffset)],
        dst: &mut [(i32, NodeOffset)],
        shift: u32,
        offsets: &mut [u32; 256],
    ) {
        for &item in src {
            let bucket = (item.0 as u32 >> shift & 0xff) as usize;
            dst[offsets[bucket] as usize] = item;
            offsets[bucket] += 1;
        }
    }

    let mut counts = [[0u32; 256]; 4];
    for &(z, _) in order.iter() {
        let key = z as u32;
        counts[0][(key & 0xff) as usize] += 1;
        counts[1][(key >> 8 & 0xff) as usize] += 1;
        counts[2][(key >> 16 & 0xff) as usize] += 1;
        counts[3][(key >> 24 & 0xff) as usize] += 1;
    }

    scratch.resize(n, order[0]);
    let mut in_order = true;
    for (pass, counts) in counts.iter().enumerate() {
        if counts.iter().any(|&count| count as usize == n) {
            continue;
        }
        let mut offsets = [0u32; 256];
        let mut sum = 0;
        for (offset, &count) in offsets.iter_mut().zip(counts.iter()) {
            *offset = sum;
            sum += count;
        }
        let shift = pass as u32 * 8;
        if in_order {
            scatter(order, scratch, shift, &mut offsets);
        } else {
            scatter(scratch, order, shift, &mut offsets);
        }
        in_order = !in_order;
    }
    if !in_order {
        order.copy_from_slice(scratch);
    }
}

/// find the leftmost node of a polygon ring
fn get_leftmost<T: Float>(nodes: &[Node<T>], start_i: NodeOffset) -> (NodeOffset, &Node<T>) {
    let mut p_i = start_i;
    let mut p = node!(nodes, p_i);
    let mut leftmost_i = start_i;
    let mut leftmost = p;

    loop {
        if p.xy[0] < leftmost.xy[0] || (p.xy[0] == leftmost.xy[0] && p.xy[1] < leftmost.xy[1]) {
            (leftmost_i, leftmost) = (p_i, p);
        }
        p_i = p.next_i;
        if p_i == start_i {
            return (leftmost_i, leftmost);
        }
        p = node!(nodes, p_i);
    }
}

/// check if a diagonal between two polygon nodes is valid (lies in polygon interior)
fn is_valid_diagonal<T: Float>(
    nodes: &[Node<T>],
    a: &Node<T>,
    b: &Node<T>,
    a_next: &Node<T>,
    a_prev: &Node<T>,
) -> bool {
    if a_next.index() == b.index() || a_prev.index() == b.index() {
        return false;
    }

    let b_next = node!(nodes, b.next_i);
    let b_prev = node!(nodes, b.prev_i);

    let zero_length_valid =
        equals(a, b) && area(a_prev, a, a_next) > T::zero() && area(b_prev, b, b_next) > T::zero();

    (zero_length_valid
        || (locally_inside(nodes, a, b)
            && locally_inside(nodes, b, a)
            // does not create opposite-facing sectors
            && (area(a_prev, a, b_prev) != T::zero() || area(a, b_prev, b) != T::zero())))
        && !intersects_polygon(nodes, a, b)
        && (zero_length_valid || middle_inside(nodes, a, b))
}

/// check if two segments intersect; optionally include collinear boundary touches
fn intersects<T: Float>(
    p1: &Node<T>,
    q1: &Node<T>,
    p2: &Node<T>,
    q2: &Node<T>,
    include_boundary: bool,
) -> bool {
    let o1 = area(p1, q1, p2);
    let o2 = area(p1, q1, q2);
    let o3 = area(p2, q2, p1);
    let o4 = area(p2, q2, q1);
    let proper = ((o1 > T::zero() && o2 < T::zero()) || (o1 < T::zero() && o2 > T::zero()))
        && ((o3 > T::zero() && o4 < T::zero()) || (o3 < T::zero() && o4 > T::zero()));
    proper
        || include_boundary
            && ((o3 == T::zero() && on_segment(p2, p1, q2))
                || (o4 == T::zero() && on_segment(p2, q1, q2))
                || (o2 == T::zero() && on_segment(p1, q2, q1))
                || (o1 == T::zero() && on_segment(p1, p2, q1)))
}

/// check if a polygon diagonal intersects any polygon segments
fn intersects_polygon<T: Float>(nodes: &[Node<T>], a: &Node<T>, b: &Node<T>) -> bool {
    let ai = a.index();
    let bi = b.index();
    let x0 = a.xy[0].min(b.xy[0]);
    let y0 = a.xy[1].min(b.xy[1]);
    let x1 = a.xy[0].max(b.xy[0]);
    let y1 = a.xy[1].max(b.xy[1]);
    let mut p = a;
    loop {
        let p_next = node!(nodes, p.next_i);
        let pi = p.index();
        let pni = p_next.index();
        let px0 = p.xy[0].min(p_next.xy[0]);
        let py0 = p.xy[1].min(p_next.xy[1]);
        let px1 = p.xy[0].max(p_next.xy[0]);
        let py1 = p.xy[1].max(p_next.xy[1]);
        if (((pi != ai) && (pi != bi)) && ((pni != ai) && (pni != bi)))
            && px0 <= x1
            && px1 >= x0
            && py0 <= y1
            && py1 >= y0
            && intersects(p, p_next, a, b, true)
        {
            return true;
        }
        p = p_next;
        if ptr::eq(p, a) {
            return false;
        }
    }
}

/// check if the middle point of a polygon diagonal is inside the polygon
fn middle_inside<T: Float>(nodes: &[Node<T>], a: &Node<T>, b: &Node<T>) -> bool {
    let mut p = a;
    let mut inside = false;
    let two = T::one() + T::one();
    let (px, py) = ((a.xy[0] + b.xy[0]) / two, (a.xy[1] + b.xy[1]) / two);
    loop {
        let p_next = node!(nodes, p.next_i);
        inside ^= (p.xy[1] > py) != (p_next.xy[1] > py)
            && p_next.xy[1] != p.xy[1]
            && (px
                < (p_next.xy[0] - p.xy[0]) * (py - p.xy[1]) / (p_next.xy[1] - p.xy[1]) + p.xy[0]);
        p = p_next;
        if ptr::eq(p, a) {
            return inside;
        }
    }
}

/// find a bridge between vertices that connects hole with an outer ring and and link it
fn eliminate_hole<T: Float>(
    nodes: &mut Vec<Node<T>>,
    hole_i: NodeOffset,
    outer_node_i: NodeOffset,
    blocks: &mut HoleBlockIndex<T>,
) -> NodeOffset {
    let Some(bridge_i) = find_hole_bridge(nodes, node!(nodes, hole_i), outer_node_i, blocks) else {
        return outer_node_i;
    };
    let bridge_reverse_i = split_polygon(nodes, bridge_i, hole_i);

    // Index the merged hole and its two slit edges before filtering. Removed
    // collinear nodes leave conservative bboxes behind, so they cannot cause
    // a false skip in later searches.
    let bridge2_i = node!(nodes, bridge_reverse_i).next_i;
    let stop_i = node!(nodes, bridge2_i).next_i;
    blocks.index_segment(nodes, bridge_i, stop_i);

    // filter collinear points around the cuts
    let end_i = Some(node!(nodes, bridge_reverse_i).next_i);
    filter_points(nodes, bridge_reverse_i, end_i, Some(blocks));
    let end_i = Some(node!(nodes, bridge_i).next_i);
    filter_points(nodes, bridge_i, end_i, Some(blocks)).0
}

/// check if a polygon diagonal is locally inside the polygon
fn locally_inside<T: Float>(nodes: &[Node<T>], a: &Node<T>, b: &Node<T>) -> bool {
    let a_prev = node!(nodes, a.prev_i);
    let a_next = node!(nodes, a.next_i);
    if area(a_prev, a, a_next) < T::zero() {
        area(a, b, a_next) >= T::zero() && area(a, a_prev, b) >= T::zero()
    } else {
        area(a, b, a_prev) < T::zero() || area(a, a_next, b) < T::zero()
    }
}

/// David Eberly's algorithm for finding a bridge between hole and outer polygon
fn find_hole_bridge<T: Float>(
    nodes: &[Node<T>],
    hole: &Node<T>,
    outer_node_i: NodeOffset,
    blocks: &mut HoleBlockIndex<T>,
) -> Option<NodeOffset> {
    let mut qx = T::neg_infinity();
    let mut m_i: Option<NodeOffset> = None;
    let [hx, hy] = hole.xy;

    // find a segment intersected by a ray from the hole's leftmost point to the left;
    // segment's endpoint with lesser x will be potential connection point
    // unless they intersect at a vertex, then choose the vertex
    if equals(hole, node!(nodes, outer_node_i)) {
        return Some(outer_node_i);
    }

    for block in 0..blocks.bboxes.len() {
        let [min_x, min_y, max_x, max_y] = blocks.bboxes[block];
        if hy < min_y || hy > max_y || min_x > hx || max_x <= qx {
            continue;
        }

        let stop_i = blocks.live_stop(nodes, block);
        let mut p_i = blocks.live_head(nodes, block);
        loop {
            let p = node!(nodes, p_i);
            let next_i = p.next_i;
            let p_next = node!(nodes, next_i);
            let live = node!(nodes, p.prev_i).next_i == p_i;
            if live {
                if equals(hole, p_next) {
                    return Some(next_i);
                }
                if hy <= p.xy[1] && hy >= p_next.xy[1] && p_next.xy[1] != p.xy[1] {
                    let x = p.xy[0]
                        + (hy - p.xy[1]) * (p_next.xy[0] - p.xy[0]) / (p_next.xy[1] - p.xy[1]);
                    if x <= hx && x > qx {
                        qx = x;
                        m_i = Some(if p.xy[0] < p_next.xy[0] { p_i } else { next_i });
                        if x == hx {
                            return m_i;
                        }
                    }
                }
            }
            p_i = next_i;
            if p_i == stop_i {
                break;
            }
        }
    }

    let mut m_i = m_i?;

    // look for points inside the triangle of hole point, segment intersection and endpoint;
    // if there are no points found, we have a valid connection;
    // otherwise choose the point of the minimum angle with the ray as connection point

    let mut m = node!(nodes, m_i);
    let mxmy = m.xy;
    let mut tan_min = T::infinity();
    let triangle_min_y = hy.min(mxmy[1]);
    let triangle_max_y = hy.max(mxmy[1]);

    let (tri_a, tri_c) = if hy < mxmy[1] {
        ([hx, hy], [qx, hy])
    } else {
        ([qx, hy], [hx, hy])
    };

    for block in 0..blocks.bboxes.len() {
        let [min_x, min_y, max_x, max_y] = blocks.bboxes[block];
        if max_x < mxmy[0] || min_x > hx || max_y < triangle_min_y || min_y > triangle_max_y {
            continue;
        }

        let stop_i = blocks.live_stop(nodes, block);
        let mut p_i = blocks.live_head(nodes, block);
        loop {
            let p = node!(nodes, p_i);
            let next_i = p.next_i;
            let live = node!(nodes, p.prev_i).next_i == p_i;
            if live
                && hx >= p.xy[0]
                && p.xy[0] >= mxmy[0]
                && hx != p.xy[0]
                && point_in_triangle(tri_a, mxmy, tri_c, p.xy)
            {
                let tan = (hy - p.xy[1]).abs() / (hx - p.xy[0]);
                let p_next = node!(nodes, next_i);
                if (locally_inside(nodes, p, hole)
                    || (p.xy[1] == hy && p_next.xy[1] == hy && p_next.xy[0] > hx))
                    && (tan < tan_min
                        || (tan == tan_min
                            && (p.xy[0] > m.xy[0]
                                || (p.xy[0] == m.xy[0] && sector_contains_sector(nodes, m, p)))))
                {
                    (m_i, m) = (p_i, p);
                    tan_min = tan;
                }
            }
            p_i = next_i;
            if p_i == stop_i {
                break;
            }
        }
    }

    Some(m_i)
}

/// whether sector in vertex m contains sector in vertex p in the same coordinates
fn sector_contains_sector<T: Float>(nodes: &[Node<T>], m: &Node<T>, p: &Node<T>) -> bool {
    area(node!(nodes, m.prev_i), m, node!(nodes, p.prev_i)) < T::zero()
        && area(node!(nodes, p.next_i), m, node!(nodes, m.next_i)) < T::zero()
}

/// eliminate colinear or duplicate points
fn filter_points<T: Float>(
    nodes: &mut [Node<T>],
    start_i: NodeOffset,
    end_i: Option<NodeOffset>,
    mut blocks: Option<&mut HoleBlockIndex<T>>,
) -> (NodeOffset, bool) {
    let full = end_i.is_none();
    let mut end_i = end_i.unwrap_or(start_i);
    let mut filtered_out = false;

    let mut p_i = start_i;
    loop {
        let p = node!(nodes, p_i);
        let p_next = node!(nodes, p.next_i);
        let mut again = false;
        if p_i != p.next_i
            && !p.is_steiner()
            && (equals(p, p_next) || area(node!(nodes, p.prev_i), p, p_next) == T::zero())
        {
            if full || p_i == end_i {
                end_i = p.prev_i;
            }
            filtered_out = true;
            if let Some(blocks) = blocks.as_deref_mut() {
                let prev = node!(nodes, p.prev_i);
                blocks.grow(prev.z as usize, p_next.xy);
            }
            let (prev_i, _) = remove_node(nodes, p.link_info());
            p_i = prev_i;
            again = true;
        } else if full || p_i != end_i {
            p_i = p.next_i;
            again = !full;
        }

        if !again && p_i == end_i {
            return (end_i, filtered_out);
        }
    }
}

/// link two polygon vertices with a bridge; if the vertices belong to the same ring, it splits polygon into two;
/// if one belongs to the outer ring and another to a hole, it merges it into a single ring
fn split_polygon<T: Float>(
    nodes: &mut Vec<Node<T>>,
    a_i: NodeOffset,
    b_i: NodeOffset,
) -> NodeOffset {
    debug_assert!(!nodes.is_empty());
    let a2_i = node_offset::<Node<T>>(nodes.len());
    let b2_i = node_offset::<Node<T>>(nodes.len() + 1);

    let a = node_mut!(nodes, a_i);
    let mut a2 = Node::new(a.index(), a.xy);
    let an_i = a.next_i;
    a.next_i = b_i;
    a2.prev_i = b2_i;
    a2.next_i = an_i;

    let b = node_mut!(nodes, b_i);
    let mut b2 = Node::new(b.index(), b.xy);
    let bp_i = b.prev_i;
    b.prev_i = a_i;
    b2.next_i = a2_i;
    b2.prev_i = bp_i;

    node_mut!(nodes, an_i).prev_i = a2_i;
    node_mut!(nodes, bp_i).next_i = b2_i;

    nodes.extend([a2, b2]);

    b2_i
}

/// create a node and optionally link it with previous one (in a circular doubly linked list)
fn insert_node<T: Float>(
    nodes: &mut Vec<Node<T>>,
    i: u32,
    xy: [T; 2],
    last: Option<NodeOffset>,
) -> NodeOffset {
    let mut p = Node::new(i, xy);
    let p_i = node_offset::<Node<T>>(nodes.len());
    match last {
        Some(last_i) => {
            let last = node_mut!(nodes, last_i);
            let last_next_i = last.next_i;
            (p.next_i, last.next_i) = (last_next_i, p_i);
            p.prev_i = last_i;
            node_mut!(nodes, last_next_i).prev_i = p_i;
        }
        None => {
            (p.prev_i, p.next_i) = (p_i, p_i);
        }
    }
    nodes.push(p);
    p_i
}

fn remove_node<T: Float>(nodes: &mut [Node<T>], pl: LinkInfo) -> (NodeOffset, NodeOffset) {
    // Each `node_mut!` is its own statement, so the `&mut` never outlives it.
    // That means the z-neighbor writes are sound even when a z-neighbor
    // coincides with the ring neighbor (sequential, non-overlapping borrows),
    // so no equality branch is needed (matches the C++ reference).
    node_mut!(nodes, pl.prev_i).next_i = pl.next_i;
    node_mut!(nodes, pl.next_i).prev_i = pl.prev_i;
    if let Some(prev_z_i) = pl.prev_z_i {
        node_mut!(nodes, prev_z_i).next_z_i = pl.next_z_i;
    }
    if let Some(next_z_i) = pl.next_z_i {
        node_mut!(nodes, next_z_i).prev_z_i = pl.prev_z_i;
    }

    (pl.prev_i, pl.next_i)
}

/// Returns a percentage difference between the polygon area and its triangulation area;
/// used to verify correctness of triangulation
///
/// # Panics
///
/// Panics if a hole index or triangle vertex index is out of bounds, or hole
/// indices are not monotonically non-decreasing.
pub fn deviation<T: Float, N: Index>(
    data: impl IntoIterator<Item = [T; 2]>,
    hole_indices: &[N],
    triangles: &[N],
) -> T {
    let data = data.into_iter().collect::<Vec<[T; 2]>>();
    let has_holes = !hole_indices.is_empty();
    let outer_len = match has_holes {
        true => hole_indices[0].into_usize(),
        false => data.len(),
    };
    let polygon_area = if data.len() < 3 || outer_len < 3 {
        T::zero()
    } else {
        let mut polygon_area = signed_area(&data[..outer_len]).abs();
        if has_holes {
            for i in 0..hole_indices.len() {
                let start = hole_indices[i].into_usize();
                let end = if i < hole_indices.len() - 1 {
                    hole_indices[i + 1].into_usize()
                } else {
                    data.len()
                };
                if end - start >= 3 {
                    polygon_area = polygon_area - signed_area(&data[start..end]).abs();
                }
            }
        }
        polygon_area
    };

    let mut triangles_area = T::zero();
    for [a, b, c] in triangles
        .chunks_exact(3)
        .map(|idxs| [idxs[0], idxs[1], idxs[2]])
    {
        let a = a.into_usize();
        let b = b.into_usize();
        let c = c.into_usize();
        triangles_area = triangles_area
            + ((data[a][0] - data[c][0]) * (data[b][1] - data[a][1])
                - (data[a][0] - data[b][0]) * (data[c][1] - data[a][1]))
                .abs();
    }
    if polygon_area == T::zero() && triangles_area == T::zero() {
        T::zero()
    } else {
        ((polygon_area - triangles_area) / polygon_area).abs()
    }
}

/// signed area of a polygon ring (twice the geometric area)
fn signed_area<T: Float>(data: &[[T; 2]]) -> T {
    debug_assert!(!data.is_empty());
    let [mut bx, mut by] = data[data.len() - 1];
    let mut sum = T::zero();
    for &[ax, ay] in data {
        sum = sum + (bx - ax) * (ay + by);
        (bx, by) = (ax, ay);
    }
    sum
}

/// z-order of a point given coords and inverse of the longer side of data bbox
fn z_order<T: Float>(xy: [T; 2], min_x: T, min_y: T, inv_size: T) -> i32 {
    // coords are transformed into non-negative 15-bit integer range
    let x_scaled = (xy[0] - min_x) * inv_size;
    let y_scaled = (xy[1] - min_y) * inv_size;
    let mut x = x_scaled.to_f64().unwrap() as u32;
    let mut y = y_scaled.to_f64().unwrap() as u32;

    x = (x | (x << 8)) & 0x00FF00FF;
    x = (x | (x << 4)) & 0x0F0F0F0F;
    x = (x | (x << 2)) & 0x33333333;
    x = (x | (x << 1)) & 0x55555555;

    y = (y | (y << 8)) & 0x00FF00FF;
    y = (y | (y << 4)) & 0x0F0F0F0F;
    y = (y | (y << 2)) & 0x33333333;
    y = (y | (y << 1)) & 0x55555555;

    (x | (y << 1)) as i32
}

fn point_in_triangle<T: Float>(a: [T; 2], b: [T; 2], c: [T; 2], p: [T; 2]) -> bool {
    ((c[0] - p[0]) * (a[1] - p[1]) >= (a[0] - p[0]) * (c[1] - p[1]))
        && ((a[0] - p[0]) * (b[1] - p[1]) >= (b[0] - p[0]) * (a[1] - p[1]))
        && ((b[0] - p[0]) * (c[1] - p[1]) >= (c[0] - p[0]) * (b[1] - p[1]))
}

fn point_in_triangle_except_first<T: Float>(a: [T; 2], b: [T; 2], c: [T; 2], p: [T; 2]) -> bool {
    !(a[0] == p[0] && a[1] == p[1]) && point_in_triangle(a, b, c, p)
}

/// signed area of a triangle
fn area<T: Float>(p: &Node<T>, q: &Node<T>, r: &Node<T>) -> T {
    (q.xy[1] - p.xy[1]) * (r.xy[0] - q.xy[0]) - (q.xy[0] - p.xy[0]) * (r.xy[1] - q.xy[1])
}

/// check if two points are equal
fn equals<T: Float>(p1: &Node<T>, p2: &Node<T>) -> bool {
    p1.xy == p2.xy
}

/// for collinear points p, q, r, check if point q lies on segment pr
fn on_segment<T: Float>(p: &Node<T>, q: &Node<T>, r: &Node<T>) -> bool {
    ((q.xy[0] <= p.xy[0].max(r.xy[0])) & (q.xy[1] <= p.xy[1].max(r.xy[1])))
        && ((q.xy[0] >= p.xy[0].min(r.xy[0])) & (q.xy[1] >= p.xy[1].min(r.xy[1])))
}

const NO_HALF_EDGE: usize = usize::MAX;

/// Reusable scratch space for Delaunay refinement.
///
/// [`Refiner::refine`] applies Lawson edge flips to an existing triangulation.
/// It preserves the boundary and the number of triangles while improving the
/// quality of interior edges. Keeping a `Refiner` around avoids reallocating
/// the half-edge and hash-table buffers between calls.
pub struct Refiner {
    edge_stack: Vec<usize>,
    half_edges: Vec<usize>,
    hash_table: Vec<usize>,
    hash_stamp: Vec<u32>,
    edge_stamp: Vec<u8>,
    generation: u32,
    hash_mask: usize,
}

impl Refiner {
    /// Creates an empty refiner.
    pub fn new() -> Self {
        Self {
            edge_stack: Vec::new(),
            half_edges: Vec::new(),
            hash_table: Vec::new(),
            hash_stamp: Vec::new(),
            edge_stamp: Vec::new(),
            generation: 0,
            hash_mask: 0,
        }
    }

    /// Refines a triangulation in place using Lawson edge flips.
    ///
    /// `triangles` must contain counter-clockwise triples of valid indices into
    /// `vertices` and describe a manifold triangulation. Boundary edges are
    /// never flipped. The predicate intentionally includes a small
    /// floating-point tolerance so nearly cocircular inputs terminate instead
    /// of flipping forever. If the predicate overflows, the affected edge is
    /// conservatively left unchanged; prefer `f64` for large coordinate ranges.
    ///
    /// # Panics
    ///
    /// Panics if the index count is not a multiple of three, an index is out of
    /// bounds for `vertices`, or the triangulation is too large for the scratch
    /// buffers.
    pub fn refine<T: Float, N: Index>(&mut self, triangles: &mut [N], vertices: &[[T; 2]]) {
        assert_eq!(
            triangles.len() % 3,
            0,
            "triangle index count must be a multiple of 3"
        );

        let edge_count = triangles.len();
        if edge_count < 6 {
            return;
        }

        self.prepare(edge_count);
        let generation = self.generation;

        for edge in 0..edge_count {
            let start = triangles[edge].into_usize();
            let end = triangles[next_half_edge(edge)].into_usize();
            let (lo, hi) = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            let mut slot = edge_hash(lo, hi) & self.hash_mask;

            while self.hash_stamp[slot] == generation {
                let other = self.hash_table[slot];
                if other != NO_HALF_EDGE {
                    let other_start = triangles[other].into_usize();
                    let other_end = triangles[next_half_edge(other)].into_usize();
                    if lo == other_start.min(other_end) && hi == other_start.max(other_end) {
                        self.half_edges[edge] = other;
                        self.half_edges[other] = edge;
                        self.hash_table[slot] = NO_HALF_EDGE;
                        self.edge_stamp[other] = 1;
                        self.edge_stack.push(other);
                        break;
                    }
                }
                slot = (slot + 1) & self.hash_mask;
            }

            if self.hash_stamp[slot] != generation {
                self.hash_stamp[slot] = generation;
                self.hash_table[slot] = edge;
            }
        }

        while let Some(a) = self.edge_stack.pop() {
            self.edge_stamp[a] = 0;

            let b = self.half_edges[a];
            if b == NO_HALF_EDGE {
                continue;
            }

            let a0 = a - a % 3;
            let b0 = b - b % 3;
            let ar = a0 + (a + 2) % 3;
            let al = a0 + (a + 1) % 3;
            let bl = b0 + (b + 2) % 3;
            let br = b0 + (b + 1) % 3;

            let p0 = triangles[ar].into_usize();
            let pr = triangles[a].into_usize();
            let pl = triangles[al].into_usize();
            let p1 = triangles[bl].into_usize();

            let v0 = vertices[p0];
            let vr = vertices[pr];
            let vl = vertices[pl];
            let v1 = vertices[p1];

            if !refine_edge_is_legal(v0, vr, vl, v1)
                && refine_orient(v0, vr, v1) > T::zero()
                && refine_orient(v0, v1, vl) > T::zero()
            {
                triangles[a] = N::from_usize(p1);
                triangles[b] = N::from_usize(p0);

                let hbl = self.half_edges[bl];
                let har = self.half_edges[ar];

                self.half_edges[a] = hbl;
                if hbl != NO_HALF_EDGE {
                    self.half_edges[hbl] = a;
                }

                self.half_edges[b] = har;
                if har != NO_HALF_EDGE {
                    self.half_edges[har] = b;
                }

                self.half_edges[ar] = bl;
                self.half_edges[bl] = ar;

                self.queue_edge(a);
                self.queue_edge(b);
                self.queue_edge(al);
                self.queue_edge(br);
            }
        }
    }

    fn prepare(&mut self, edge_count: usize) {
        self.edge_stack.clear();
        self.edge_stack.reserve(edge_count);

        self.half_edges.resize(edge_count, NO_HALF_EDGE);
        self.half_edges.fill(NO_HALF_EDGE);
        self.edge_stamp.resize(edge_count, 0);
        self.edge_stamp.fill(0);

        let required_slots = edge_count
            .checked_mul(4)
            .and_then(usize::checked_next_power_of_two)
            .expect("triangulation is too large to refine");
        if self.hash_table.len() < required_slots {
            self.hash_table.resize(required_slots, NO_HALF_EDGE);
            self.hash_stamp.resize(required_slots, 0);
        }
        self.hash_mask = required_slots - 1;

        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.hash_stamp.fill(0);
            self.generation = 1;
        }
    }

    fn queue_edge(&mut self, edge: usize) {
        if self.half_edges[edge] != NO_HALF_EDGE && self.edge_stamp[edge] == 0 {
            self.edge_stamp[edge] = 1;
            self.edge_stack.push(edge);
        }
    }
}

impl Default for Refiner {
    fn default() -> Self {
        Self::new()
    }
}

/// Refines a triangulation in place using Lawson edge flips.
///
/// `triangles` has the same input requirements as [`Refiner::refine`],
/// including counter-clockwise triangle winding.
///
/// Use [`Refiner`] instead when refining repeatedly to reuse its scratch
/// buffers.
///
/// # Panics
///
/// Panics under the conditions documented by [`Refiner::refine`].
pub fn refine<T: Float, N: Index>(triangles: &mut [N], vertices: &[[T; 2]]) {
    Refiner::new().refine(triangles, vertices);
}

fn next_half_edge(edge: usize) -> usize {
    edge - edge % 3 + (edge + 1) % 3
}

fn edge_hash(lo: usize, hi: usize) -> usize {
    lo.wrapping_mul(0x9e37_79b1) ^ hi.wrapping_mul(0x85eb_ca6b)
}

fn refine_orient<T: Float>(a: [T; 2], b: [T; 2], c: [T; 2]) -> T {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn refine_edge_is_legal<T: Float>(a: [T; 2], b: [T; 2], c: [T; 2], p: [T; 2]) -> bool {
    let dx = a[0] - p[0];
    let dy = a[1] - p[1];
    let ex = b[0] - p[0];
    let ey = b[1] - p[1];
    let fx = c[0] - p[0];
    let fy = c[1] - p[1];

    let ap = dx * dx + dy * dy;
    let bp = ex * ex + ey * ey;
    let cp = fx * fx + fy * fy;
    let scale = ap + bp + cp;
    let determinant =
        dx * (ey * cp - bp * fy) - dy * (ex * cp - bp * fx) + ap * (ex * fy - ey * fx);

    let tolerance = T::from(1e-13)
        .unwrap()
        .max(T::epsilon() * T::from(450.0).unwrap());
    let limit = tolerance * scale * scale;
    if !determinant.is_finite() || !limit.is_finite() {
        return true;
    }
    determinant <= limit
}
