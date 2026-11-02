//! Integer-coordinate variant of [`crate::Earcut`]
//!
//! Uses exact integer arithmetic, avoiding the
//! floating-point rounding errors of [`crate::Earcut`].

use alloc::vec::Vec;
use core::{cmp::Ordering, num::NonZeroU32, ptr};

use crate::node_offset;

/// Largest permitted absolute coordinate value on each axis.
const MAX_ABS_COORD: i32 = 1 << 19;

#[inline(always)]
fn cmp_products(a: i64, b: i64, c: i64, d: i64) -> Ordering {
    (a * b).cmp(&(c * d))
}

#[inline(always)]
fn use_small_path(range_x: i64, range_y: i64) -> bool {
    range_x <= i32::MAX as i64 && range_y <= i32::MAX as i64 && range_x * range_y <= i32::MAX as i64
}

#[inline(always)]
fn shift_for_z_order(range: i64) -> u32 {
    let used = i64::BITS - range.leading_zeros();
    used.saturating_sub(15)
}

#[inline(always)]
fn small_area_non_negative(p: [i32; 2], q: [i32; 2], r: [i32; 2]) -> bool {
    let lhs = (q[1] - p[1]) * (r[0] - q[0]);
    let rhs = (q[0] - p[0]) * (r[1] - q[1]);
    lhs >= rhs
}

#[inline(always)]
fn small_point_in_triangle(a: [i32; 2], b: [i32; 2], c: [i32; 2], p: [i32; 2]) -> bool {
    ((c[0] - p[0]) * (a[1] - p[1]) >= (a[0] - p[0]) * (c[1] - p[1]))
        && ((a[0] - p[0]) * (b[1] - p[1]) >= (b[0] - p[0]) * (a[1] - p[1]))
        && ((b[0] - p[0]) * (c[1] - p[1]) >= (c[0] - p[0]) * (b[1] - p[1]))
}

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
        Self::try_from(v).expect("EarcutI32 vertex index does not fit in u16")
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

/// Returns the absolute integer difference between the polygon area (times 2)
/// and its triangulation area (times 2) for `i32` coordinates. Zero indicates
/// an exact match; non-zero signals a triangulation error.
///
/// Coordinates must lie within the same domain as [`EarcutI32::earcut`]
/// (`[-2^19, 2^19]`), which keeps the `i64` area accumulation exact for any
/// valid triangulation.
///
/// # Panics
///
/// Panics if a hole index or triangle vertex index is out of bounds, or hole
/// indices are not monotonically non-decreasing.
pub fn deviation<N: Index>(
    data: impl IntoIterator<Item = [i32; 2]>,
    hole_indices: &[N],
    triangles: &[N],
) -> i64 {
    let data = data.into_iter().collect::<Vec<[i32; 2]>>();
    let has_holes = !hole_indices.is_empty();
    let outer_len = match has_holes {
        true => hole_indices[0].into_usize(),
        false => data.len(),
    };
    let polygon_area = if data.len() < 3 || outer_len < 3 {
        0
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
                    polygon_area -= signed_area(&data[start..end]).abs();
                }
            }
        }
        polygon_area
    };

    let mut triangles_area = 0;
    for [a, b, c] in triangles
        .chunks_exact(3)
        .map(|idxs| [idxs[0], idxs[1], idxs[2]])
    {
        let a = a.into_usize();
        let b = b.into_usize();
        let c = c.into_usize();
        let v = (i64::from(data[a][0]) - i64::from(data[c][0]))
            * (i64::from(data[b][1]) - i64::from(data[a][1]))
            - (i64::from(data[a][0]) - i64::from(data[b][0]))
                * (i64::from(data[c][1]) - i64::from(data[a][1]));
        triangles_area += v.abs();
    }
    if polygon_area < triangles_area {
        triangles_area - polygon_area
    } else {
        polygon_area - triangles_area
    }
}

/// signed area of a polygon ring (twice the geometric area)
fn signed_area(data: &[[i32; 2]]) -> i64 {
    debug_assert!(!data.is_empty());
    let [ox, oy] = data[0];
    let [last_x, last_y] = data[data.len() - 1];
    let [mut bx, mut by] = [
        i64::from(last_x) - i64::from(ox),
        i64::from(last_y) - i64::from(oy),
    ];
    let mut sum = 0;
    for &[ax_r, ay_r] in data {
        let ax = i64::from(ax_r) - i64::from(ox);
        let ay = i64::from(ay_r) - i64::from(oy);
        sum += (bx - ax) * (ay + by);
        (bx, by) = (ax, ay);
    }
    sum
}

/// Sentinel `shift` value signaling "no z-order hashing" for this run.
const NO_HASH: u32 = u32::MAX;

/// Byte offset (from `nodes` base pointer) of a `Node` in the `nodes` Vec.
type NodeOffset = NonZeroU32;

const STEINER_BIT: u32 = 1 << 31;
const INDEX_MASK: u32 = !STEINER_BIT;

struct Node {
    /// vertex index in coordinates array (lower 31 bits) + steiner flag (bit 31)
    i_steiner: u32,
    /// z-order curve value
    z: i32,
    /// vertex coordinates x
    xy: [i32; 2],
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
// For a few short rings, index construction costs more than direct traversal.
// Limit both dimensions: even short inputs can have many holes to bridge.
const DIRECT_BRIDGE_MAX_VERTICES: usize = 4 * HOLE_BLOCK_SIZE;
const DIRECT_BRIDGE_MAX_HOLES: usize = 4;
/// Member slots per chunk, and how many blocks a freshly filled chunk takes, which
/// leaves room for merged holes to be inserted without splitting right away.
const CHUNK_CAPACITY: usize = 32;
const CHUNK_FILL: usize = 16;

/// Bounding-box blocks used while holes are merged into the outer ring.
///
/// Every indexed node records its owning block in `z`, and a block is the run
/// of consecutive ring nodes carrying its id, starting at its head. Walking a
/// block therefore stops as soon as the ring leaves it, so a hole spliced into
/// the middle of an older block is never re-walked through that block.
///
/// Blocks are grouped into chunks of consecutive blocks with a combined bbox, so
/// a bridge search can skip a whole chunk at once. Chunks are listed in ring
/// order (`chunk_order`) and keep their blocks in ring order, so scanning visits
/// edges in the same order as walking the merged ring from the outer ring's
/// head; this keeps tie-breaking in `find_hole_bridge` identical to an
/// unindexed scan. All storage is flat and reused across triangulations.
struct HoleBlockIndex {
    /// per block: bbox of its edges, first node, and the chunk holding it
    bboxes: Vec<[i32; 4]>,
    heads: Vec<NodeOffset>,
    block_chunks: Vec<u32>,
    /// per chunk: combined bbox, member count, and `CHUNK_CAPACITY` member slots;
    /// the slots are kept across triangulations and only read up to the member count
    chunk_bboxes: Vec<[i32; 4]>,
    chunk_lens: Vec<u32>,
    chunk_members: Vec<u32>,
    /// chunks in ring order
    chunk_order: Vec<u32>,
    /// blocks waiting to be placed by `insert_after`
    pending: Vec<u32>,
}

fn empty_bbox() -> [i32; 4] {
    [i32::MAX, i32::MAX, i32::MIN, i32::MIN]
}

fn union_bbox(bbox: &mut [i32; 4], other: [i32; 4]) {
    bbox[0] = bbox[0].min(other[0]);
    bbox[1] = bbox[1].min(other[1]);
    bbox[2] = bbox[2].max(other[2]);
    bbox[3] = bbox[3].max(other[3]);
}

impl HoleBlockIndex {
    fn new() -> Self {
        Self {
            bboxes: Vec::new(),
            heads: Vec::new(),
            block_chunks: Vec::new(),
            chunk_bboxes: Vec::new(),
            chunk_lens: Vec::new(),
            chunk_members: Vec::new(),
            chunk_order: Vec::new(),
            pending: Vec::new(),
        }
    }

    fn reset(&mut self, max_nodes: usize, num_holes: usize) {
        self.bboxes.clear();
        self.heads.clear();
        self.block_chunks.clear();
        self.chunk_bboxes.clear();
        self.chunk_lens.clear();
        self.chunk_order.clear();

        let indexed_nodes = max_nodes.saturating_add(num_holes.saturating_mul(2));
        // one partial block per indexed segment, plus one detached tail per merge
        let max_blocks = indexed_nodes
            .div_ceil(HOLE_BLOCK_SIZE)
            .saturating_add(num_holes.saturating_mul(2))
            .saturating_add(2);
        self.bboxes.reserve(max_blocks);
        self.heads.reserve(max_blocks);
        self.block_chunks.reserve(max_blocks);
    }

    /// Index the ring run `head..stop`; `head == stop` means the whole ring.
    /// Returns the last new block; the caller places the new blocks in chunks.
    fn index_segment(
        &mut self,
        nodes: &mut [Node],
        head_i: NodeOffset,
        stop_i: NodeOffset,
    ) -> usize {
        let mut p_i = head_i;
        loop {
            let block = self.bboxes.len();
            self.heads.push(p_i);
            self.block_chunks.push(u32::MAX);

            let mut min_x = i32::MAX;
            let mut min_y = i32::MAX;
            let mut max_x = i32::MIN;
            let mut max_y = i32::MIN;
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
            if p_i == stop_i {
                return block;
            }
        }
    }

    fn index_outer(&mut self, nodes: &mut [Node], outer_i: NodeOffset) {
        let last = self.index_segment(nodes, outer_i, outer_i);
        for block in 0..=last {
            if block % CHUNK_FILL == 0 {
                let chunk = self.new_chunk();
                self.chunk_order.push(chunk as u32);
            }
            self.push_member(self.chunk_lens.len() - 1, block as u32);
        }
    }

    /// Index a hole just merged at `bridge` (the ring run `bridge..stop`) and place its
    /// blocks right after the block that owned `bridge`, where the hole now sits in the ring.
    fn index_merged_hole(&mut self, nodes: &mut [Node], bridge_i: NodeOffset, stop_i: NodeOffset) {
        let old_block = node!(nodes, bridge_i).z;
        let first = self.bboxes.len() as u32;
        let last = self.index_segment(nodes, bridge_i, stop_i) as u32;
        let tail = self.detach_tail(nodes, old_block, stop_i).map(|b| b as u32);
        let replace = Self::live_head(&mut self.heads, nodes, old_block as usize).is_none();
        self.insert_after(old_block as usize, first..=last, tail, replace);
    }

    fn new_chunk(&mut self) -> usize {
        let chunk = self.chunk_lens.len();
        self.chunk_lens.push(0);
        self.chunk_bboxes.push(empty_bbox());
        let slots_end = (chunk + 1) * CHUNK_CAPACITY;
        if self.chunk_members.len() < slots_end {
            self.chunk_members.resize(slots_end, 0);
        }
        chunk
    }

    fn push_member(&mut self, chunk: usize, block: u32) {
        let len = self.chunk_lens[chunk] as usize;
        self.chunk_members[chunk * CHUNK_CAPACITY + len] = block;
        self.chunk_lens[chunk] += 1;
        self.block_chunks[block as usize] = chunk as u32;
        union_bbox(&mut self.chunk_bboxes[chunk], self.bboxes[block as usize]);
    }

    fn chunk_range(&self, chunk: usize) -> core::ops::Range<usize> {
        let start = chunk * CHUNK_CAPACITY;
        start..start + self.chunk_lens[chunk] as usize
    }

    /// Place the new blocks `new` followed by `tail` (in ring order) right after `block`.
    /// If its chunk overflows, the chunk keeps the blocks before the insertion point
    /// and the rest spill into new chunks that follow it in ring order.
    fn insert_after(
        &mut self,
        block: usize,
        new: core::ops::RangeInclusive<u32>,
        tail: Option<u32>,
        replace: bool,
    ) {
        let chunk = self.block_chunks[block] as usize;
        let range = self.chunk_range(chunk);
        let at = range.start
            + self.chunk_members[range.clone()]
                .iter()
                .position(|&b| b as usize == block)
                .expect("block belongs to its chunk")
            + usize::from(!replace);
        let removed = usize::from(replace);
        let n = (new.end() - new.start() + 1) as usize + usize::from(tail.is_some());

        if range.len() + n - removed <= CHUNK_CAPACITY {
            self.chunk_members
                .copy_within(at + removed..range.end, at + n);
            for (slot, b) in (at..).zip(new.chain(tail)) {
                self.chunk_members[slot] = b;
                self.block_chunks[b as usize] = chunk as u32;
                union_bbox(&mut self.chunk_bboxes[chunk], self.bboxes[b as usize]);
            }
            self.chunk_lens[chunk] += (n - removed) as u32;
            return;
        }

        self.pending.clear();
        self.pending.extend(new.chain(tail));
        self.pending
            .extend_from_slice(&self.chunk_members[at + removed..range.end]);
        let kept = at - range.start;
        self.chunk_lens[chunk] = 0;
        self.chunk_bboxes[chunk] = empty_bbox();
        for k in range.start..at {
            let b = self.chunk_members[k];
            self.push_member(chunk, b);
        }

        let first_new = self.chunk_lens.len();
        let mut target = chunk;
        let mut room = CHUNK_FILL.saturating_sub(kept);
        for k in 0..self.pending.len() {
            if room == 0 {
                target = self.new_chunk();
                room = CHUNK_FILL;
            }
            let b = self.pending[k];
            self.push_member(target, b);
            room -= 1;
        }

        let order_at = self
            .chunk_order
            .iter()
            .position(|&c| c as usize == chunk)
            .expect("chunk is listed in ring order")
            + 1;
        self.chunk_order.splice(
            order_at..order_at,
            first_new as u32..self.chunk_lens.len() as u32,
        );
    }

    fn grow(&mut self, block: usize, [x, y]: [i32; 2]) {
        let point = [x, y, x, y];
        union_bbox(&mut self.bboxes[block], point);
        union_bbox(
            &mut self.chunk_bboxes[self.block_chunks[block] as usize],
            point,
        );
    }

    /// First live node of the block, or `None` once none of its nodes are left.
    ///
    /// Removed nodes keep their links, so skipping forward from a removed head
    /// reaches either the block's next survivor or a node owned elsewhere. Takes
    /// only `heads`, so a scan can keep iterating a chunk's members meanwhile.
    fn live_head(heads: &mut [NodeOffset], nodes: &[Node], block: usize) -> Option<NodeOffset> {
        let mut head_i = heads[block];
        let mut prev_i = node!(nodes, head_i).prev_i;
        while node!(nodes, prev_i).next_i != head_i {
            head_i = node!(nodes, head_i).next_i;
            prev_i = node!(nodes, head_i).prev_i;
        }
        heads[block] = head_i;
        (node!(nodes, head_i).z == block as i32).then_some(head_i)
    }

    /// Merging a hole at `bridge` re-owns the bridge node, which cuts its former
    /// block in two. Give the part after the bridge (starting at `tail`) a block
    /// of its own so it stays reachable; it inherits the old bbox, which still
    /// covers all of its edges.
    fn detach_tail(
        &mut self,
        nodes: &mut [Node],
        old_block: i32,
        tail_i: NodeOffset,
    ) -> Option<usize> {
        if node!(nodes, tail_i).z != old_block {
            return None;
        }
        let block = self.bboxes.len();
        let block_z = i32::try_from(block).expect("too many hole index blocks");
        self.bboxes.push(self.bboxes[old_block as usize]);
        self.heads.push(tail_i);
        self.block_chunks.push(u32::MAX);
        let mut p_i = tail_i;
        while node!(nodes, p_i).z == old_block {
            let p = node_mut!(nodes, p_i);
            p.z = block_z;
            p_i = p.next_i;
        }
        Some(block)
    }
}

#[derive(Clone, Copy)]
struct InputBbox {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
}

impl InputBbox {
    #[inline(always)]
    fn new([x, y]: [i32; 2]) -> Self {
        Self {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        }
    }

    #[inline(always)]
    fn update(&mut self, [x, y]: [i32; 2]) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }
}

impl Node {
    const PLACEHOLDER_OFFSET: NodeOffset =
        NodeOffset::new(core::mem::size_of::<Self>() as u32).unwrap();

    fn new(i: u32, xy: [i32; 2]) -> Self {
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

/// main ear slicing loop which triangulates a polygon (given as a linked list)
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn earcut_linked<N: Index>(
    nodes: &mut Vec<Node>,
    ear_i: NodeOffset,
    triangles: &mut Vec<N>,
    min_x: i32,
    min_y: i32,
    shift: u32,
    small_path: bool,
    sort_queue: &mut Vec<(i32, NodeOffset)>,
    sort_scratch: &mut Vec<(i32, NodeOffset)>,
    split_stack: &mut Vec<NodeOffset>,
) {
    if shift != NO_HASH {
        earcut_linked_impl::<N, true>(
            nodes,
            ear_i,
            triangles,
            min_x,
            min_y,
            shift,
            small_path,
            sort_queue,
            sort_scratch,
            split_stack,
        );
    } else {
        earcut_linked_impl::<N, false>(
            nodes,
            ear_i,
            triangles,
            min_x,
            min_y,
            shift,
            small_path,
            sort_queue,
            sort_scratch,
            split_stack,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn earcut_linked_impl<N: Index, const HASHED: bool>(
    nodes: &mut Vec<Node>,
    ear_i: NodeOffset,
    triangles: &mut Vec<N>,
    min_x: i32,
    min_y: i32,
    shift: u32,
    small_path: bool,
    sort_queue: &mut Vec<(i32, NodeOffset)>,
    sort_scratch: &mut Vec<(i32, NodeOffset)>,
    split_stack: &mut Vec<NodeOffset>,
) {
    let mut ear_i = ear_i;

    // interlink polygon nodes in z-order
    if HASHED {
        index_curve(nodes, ear_i, min_x, min_y, shift, sort_queue, sort_scratch);
    }

    let mut stop_i = ear_i;
    let mut cured = false;
    // a strictly convex ring has no vertex that could block an ear, so skip the point
    // checks until a candidate fails; without this, a big convex ring is cut as one fan
    // of long triangles and each hashed check scans O(n) points
    let mut convex = HASHED && is_convex(nodes, ear_i);

    loop {
        let ear = node!(nodes, ear_i);
        if ear.prev_i == ear.next_i {
            break;
        }
        let ni = ear.next_i;

        let (is_ear, prev, next) = if HASHED && convex {
            let (a, c) = (node!(nodes, ear.prev_i), node!(nodes, ear.next_i));
            (area(a, ear, c) < 0, a, c)
        } else if HASHED {
            is_ear_hashed(nodes, ear, min_x, min_y, shift)
        } else if small_path {
            is_ear_small(nodes, ear)
        } else {
            is_ear(nodes, ear)
        };
        if is_ear {
            let next_i = next.index();
            let next_node_i = ear.next_i;

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

        convex = false;
        ear_i = ni;
        if ear_i == stop_i {
            let (filtered_i, filtered_out) = filter_points(nodes, ear_i, None, None);
            ear_i = filtered_i;
            if filtered_out {
                stop_i = ear_i;
                continue;
            }

            if !cured {
                ear_i = cure_local_intersections::<N>(nodes, ear_i, triangles);
                stop_i = ear_i;
                cured = true;
                continue;
            }

            split_earcut(nodes, ear_i, split_stack);
            break;
        }
    }
}

/// check whether every vertex of a ring is strictly convex
fn is_convex(nodes: &[Node], start_i: NodeOffset) -> bool {
    let mut p_i = start_i;
    loop {
        let p = node!(nodes, p_i);
        if area(node!(nodes, p.prev_i), p, node!(nodes, p.next_i)) >= 0 {
            return false;
        }
        p_i = p.next_i;
        if p_i == start_i {
            return true;
        }
    }
}
/// check whether a polygon node forms a valid ear with adjacent nodes
fn is_ear<'a>(nodes: &'a [Node], ear: &'a Node) -> (bool, &'a Node, &'a Node) {
    let b = ear;
    let a = node!(nodes, b.prev_i);
    let c = node!(nodes, b.next_i);

    if area(a, b, c) >= 0 {
        // reflex, can't be an ear
        return (false, a, c);
    }

    // now make sure we don't have other points inside the potential ear

    // triangle bbox
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
            && area(p_prev, p, p_next) >= 0
        {
            return (false, a, c);
        }
        (p_prev, p) = (p, p_next);
    }
    (true, a, c)
}

/// Exact small-polygon fast path for integer types that can prove narrow arithmetic is safe.
fn is_ear_small<'a>(nodes: &'a [Node], ear: &'a Node) -> (bool, &'a Node, &'a Node) {
    let b = ear;
    let a_i = b.prev_i;
    let c_i = b.next_i;
    let a = node!(nodes, a_i);
    let c = node!(nodes, c_i);
    let a_xy = a.xy;
    let b_xy = b.xy;
    let c_xy = c.xy;

    if small_area_non_negative(a_xy, b_xy, c_xy) {
        return (false, a, c);
    }

    let x0 = a_xy[0].min(b_xy[0].min(c_xy[0]));
    let y0 = a_xy[1].min(b_xy[1].min(c_xy[1]));
    let x1 = a_xy[0].max(b_xy[0].max(c_xy[0]));
    let y1 = a_xy[1].max(b_xy[1].max(c_xy[1]));

    let mut p_i = c.next_i;
    let mut p_prev_xy = c_xy;
    while p_i != a_i {
        let p = node!(nodes, p_i);
        let p_xy = p.xy;
        let p_next_i = p.next_i;
        let p_next_xy = node!(nodes, p_next_i).xy;
        if (p_xy[0] >= x0 && p_xy[0] <= x1 && p_xy[1] >= y0 && p_xy[1] <= y1)
            && !(a_xy[0] == p_xy[0] && a_xy[1] == p_xy[1])
            && small_area_non_negative(p_prev_xy, p_xy, p_next_xy)
            && small_point_in_triangle(a_xy, b_xy, c_xy, p_xy)
        {
            return (false, a, c);
        }
        p_prev_xy = p_xy;
        p_i = p_next_i;
    }
    (true, a, c)
}

fn is_ear_hashed<'a>(
    nodes: &'a [Node],
    ear: &'a Node,
    min_x: i32,
    min_y: i32,
    shift: u32,
) -> (bool, &'a Node, &'a Node) {
    let b = ear;
    let a = node!(nodes, b.prev_i);
    let c = node!(nodes, b.next_i);

    if area(a, b, c) >= 0 {
        // reflex, can't be an ear
        return (false, a, c);
    }

    // triangle bbox
    let xy_min = [
        a.xy[0].min(b.xy[0].min(c.xy[0])),
        a.xy[1].min(b.xy[1].min(c.xy[1])),
    ];
    let xy_max = [
        a.xy[0].max(b.xy[0].max(c.xy[0])),
        a.xy[1].max(b.xy[1].max(c.xy[1])),
    ];

    // z-order range for the current triangle bbox;
    let min_z = z_order(xy_min, min_x, min_y, shift);
    let max_z = z_order(xy_max, min_x, min_y, shift);

    // Walk both z-order directions interleaved: the two pointer chases are
    // independent, so the CPU keeps two loads in flight instead of one on
    // this latency-bound traversal. Any hit returns immediately, so the visit
    // order does not change the result.
    //
    // Unlike the float version, we keep the bbox prefilter: the i32 comparisons are
    // cheaper than i64 multiplications.
    let mut o_n = ear.next_z_i.map(|i| node!(nodes, i));
    let mut o_p = ear.prev_z_i.map(|i| node!(nodes, i));
    while let (Some(n), Some(p)) = (o_n, o_p) {
        if n.z > max_z || p.z < min_z {
            break;
        }
        if ((n.xy[0] >= xy_min[0])
            & (n.xy[0] <= xy_max[0])
            & (n.xy[1] >= xy_min[1])
            & (n.xy[1] <= xy_max[1]))
            && (!ptr::eq(n, a) && !ptr::eq(n, c))
            && point_in_triangle_except_first(a.xy, b.xy, c.xy, n.xy)
            && area(node!(nodes, n.prev_i), n, node!(nodes, n.next_i)) >= 0
        {
            return (false, a, c);
        }
        if ((p.xy[0] >= xy_min[0])
            & (p.xy[0] <= xy_max[0])
            & (p.xy[1] >= xy_min[1])
            & (p.xy[1] <= xy_max[1]))
            && (!ptr::eq(p, a) && !ptr::eq(p, c))
            && point_in_triangle_except_first(a.xy, b.xy, c.xy, p.xy)
            && area(node!(nodes, p.prev_i), p, node!(nodes, p.next_i)) >= 0
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
        if ((n.xy[0] >= xy_min[0])
            & (n.xy[0] <= xy_max[0])
            & (n.xy[1] >= xy_min[1])
            & (n.xy[1] <= xy_max[1]))
            && (!ptr::eq(n, a) && !ptr::eq(n, c))
            && point_in_triangle_except_first(a.xy, b.xy, c.xy, n.xy)
            && area(node!(nodes, n.prev_i), n, node!(nodes, n.next_i)) >= 0
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
        if ((p.xy[0] >= xy_min[0])
            & (p.xy[0] <= xy_max[0])
            & (p.xy[1] >= xy_min[1])
            & (p.xy[1] <= xy_max[1]))
            && (!ptr::eq(p, a) && !ptr::eq(p, c))
            && point_in_triangle_except_first(a.xy, b.xy, c.xy, p.xy)
            && area(node!(nodes, p.prev_i), p, node!(nodes, p.next_i)) >= 0
        {
            return (false, a, c);
        }
        o_p = p.prev_z_i.map(|i| node!(nodes, i));
    }

    (true, a, c)
}

/// go through all polygon nodes and cure small local self-intersections
fn cure_local_intersections<N: Index>(
    nodes: &mut [Node],
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

/// try splitting polygon into two; both halves are queued on `split_stack` to be
/// triangulated independently, `a` on top so it is triangulated first
fn split_earcut(nodes: &mut Vec<Node>, start_i: NodeOffset, split_stack: &mut Vec<NodeOffset>) {
    // look for a valid diagonal that divides the polygon into two
    let mut ai = start_i;
    let mut a = node!(nodes, ai);
    loop {
        let a_next = node!(nodes, a.next_i);
        let a_prev = node!(nodes, a.prev_i);
        let a_index = a.index();
        let mut bi = a_next.next_i;

        while bi != a.prev_i {
            let b = node!(nodes, bi);
            if a_index != b.index() && is_valid_diagonal(nodes, a, b, a_next, a_prev) {
                // split the polygon in two by the diagonal
                let mut ci = split_polygon(nodes, ai, bi);

                // filter colinear points around the cuts
                let end_i = Some(node!(nodes, ai).next_i);
                ai = filter_points(nodes, ai, end_i, None).0;
                let end_i = Some(node!(nodes, ci).next_i);
                ci = filter_points(nodes, ci, end_i, None).0;

                // queue both halves for earcut, a on top so it's triangulated first
                split_stack.extend([ci, ai]);
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
fn index_curve(
    nodes: &mut [Node],
    start_i: NodeOffset,
    min_x: i32,
    min_y: i32,
    shift: u32,
    order: &mut Vec<(i32, NodeOffset)>,
    scratch: &mut Vec<(i32, NodeOffset)>,
) {
    order.clear();
    let mut p_i = start_i;
    let mut p = node_mut!(nodes, p_i);

    loop {
        // `z` temporarily holds the hole block id during hole elimination.
        p.z = z_order(p.xy, min_x, min_y, shift);
        order.push((p.z, p_i));
        p_i = p.next_i;
        p = node_mut!(nodes, p_i);
        if p_i == start_i {
            break;
        }
    }

    crate::sort_by_z(order, scratch);

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

/// find the leftmost node of a polygon ring
fn get_leftmost(nodes: &[Node], start_i: NodeOffset) -> (NodeOffset, &Node) {
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
fn is_valid_diagonal(nodes: &[Node], a: &Node, b: &Node, a_next: &Node, a_prev: &Node) -> bool {
    let b_next = node!(nodes, b.next_i);
    let b_prev = node!(nodes, b.prev_i);
    // dones't intersect other edges
    (((a_next.index() != b.index()) && (a_prev.index() != b.index())) && !intersects_polygon(nodes, a, b))
        // locally visible
        && ((locally_inside(nodes, a, b) && locally_inside(nodes, b, a) && middle_inside(nodes, a, b))
            // does not create opposite-facing sectors
            && (area(a_prev, a, b_prev) != 0 || area(a, b_prev, b) != 0)
            // special zero-length case
            || equals(a, b)
                && area(a_prev, a, a_next) > 0
                && area(b_prev, b, b_next) > 0)
}

/// check if two segments intersect; optionally include collinear boundary touches
fn intersects(p1: &Node, q1: &Node, p2: &Node, q2: &Node, include_boundary: bool) -> bool {
    let o1 = area(p1, q1, p2);
    let o2 = area(p1, q1, q2);
    let o3 = area(p2, q2, p1);
    let o4 = area(p2, q2, q1);
    let proper =
        ((o1 > 0 && o2 < 0) || (o1 < 0 && o2 > 0)) && ((o3 > 0 && o4 < 0) || (o3 < 0 && o4 > 0));
    proper
        || include_boundary
            && ((o3 == 0 && on_segment(p2, p1, q2))
                || (o4 == 0 && on_segment(p2, q1, q2))
                || (o2 == 0 && on_segment(p1, q2, q1))
                || (o1 == 0 && on_segment(p1, p2, q1)))
}

/// check if a polygon diagonal intersects any polygon segments
fn intersects_polygon(nodes: &[Node], a: &Node, b: &Node) -> bool {
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
fn middle_inside(nodes: &[Node], a: &Node, b: &Node) -> bool {
    // Every product below combines coordinates with coefficients that sum to
    // zero (e.g. `a.x + b.x - 2*p.x`), so each factor is bounded by the
    // coordinate range and the products stay exact in `i64` (see the domain
    // note on `EarcutI32::earcut`).
    let two_px = i64::from(a.xy[0]) + i64::from(b.xy[0]);
    let two_py = i64::from(a.xy[1]) + i64::from(b.xy[1]);
    let mut p = a;
    let mut inside = false;
    loop {
        let p_next = node!(nodes, p.next_i);
        let py_doubled = i64::from(p.xy[1]) * 2;
        let pn_py_doubled = i64::from(p_next.xy[1]) * 2;
        let crosses = (py_doubled > two_py) != (pn_py_doubled > two_py) && p_next.xy[1] != p.xy[1];
        if crosses {
            let dy = i64::from(p_next.xy[1]) - i64::from(p.xy[1]);
            let dx = i64::from(p_next.xy[0]) - i64::from(p.xy[0]);
            let lhs = (two_px - i64::from(p.xy[0]) * 2) * dy;
            let rhs = dx * (two_py - py_doubled);
            // px < intersect ↔ lhs < rhs (if dy > 0) or lhs > rhs (if dy < 0)
            let px_lt_intersect = if dy > 0 { lhs < rhs } else { lhs > rhs };
            if px_lt_intersect {
                inside = !inside;
            }
        }
        p = p_next;
        if ptr::eq(p, a) {
            return inside;
        }
    }
}

#[inline(always)]
fn ray_intersection_x(p: [i32; 2], next: [i32; 2], y: i32) -> (i64, i64) {
    let den = i64::from(p[1]) - i64::from(next[1]);
    debug_assert!(den > 0);
    let num = i64::from(p[0]) * den
        + (i64::from(p[1]) - i64::from(y)) * (i64::from(next[0]) - i64::from(p[0]));
    (num, den)
}

/// find a bridge between vertices that connects hole with an outer ring and and link it
fn eliminate_hole(
    nodes: &mut Vec<Node>,
    hole_i: NodeOffset,
    outer_node_i: NodeOffset,
    blocks: &mut HoleBlockIndex,
) -> NodeOffset {
    let Some(bridge_i) = find_hole_bridge(nodes, node!(nodes, hole_i), outer_node_i, blocks) else {
        return outer_node_i;
    };
    let bridge_reverse_i = split_polygon(nodes, bridge_i, hole_i);

    if blocks.chunk_order.is_empty() {
        return heal_bridge(nodes, bridge_i, bridge_reverse_i, hole_i, None);
    }

    let bridge2_i = node!(nodes, bridge_reverse_i).next_i;
    let stop_i = node!(nodes, bridge2_i).next_i;
    blocks.index_merged_hole(nodes, bridge_i, stop_i);

    heal_bridge(nodes, bridge_i, bridge_reverse_i, hole_i, Some(blocks))
}

/// heal collinear/coincident points around the two new slit edges of a merged hole
fn heal_bridge(
    nodes: &mut [Node],
    bridge_i: NodeOffset,
    bridge_reverse_i: NodeOffset,
    hole_i: NodeOffset,
    mut blocks: Option<&mut HoleBlockIndex>,
) -> NodeOffset {
    // leave bridge2 alone: dropping it as collinear would leave bridge lying on the
    // healed edge, where a later hole could bridge into it from outside its sector
    let start_i = node!(nodes, bridge_reverse_i).prev_i;
    filter_points(
        nodes,
        start_i,
        Some(bridge_reverse_i),
        blocks.as_deref_mut(),
    );

    // likewise, if bridge lies strictly between its prev and hole, healing would drop it
    // and leave bridge2 on the healed edge; skip the heal then (a spike at bridge, e.g.
    // a hole touching an outer edge, still needs it)
    let bridge = node!(nodes, bridge_i);
    let a = node!(nodes, bridge.prev_i);
    let hole = node!(nodes, hole_i);
    if area(a, bridge, hole) == 0
        && (i64::from(a.xy[0]) - i64::from(bridge.xy[0]))
            * (i64::from(hole.xy[0]) - i64::from(bridge.xy[0]))
            + (i64::from(a.xy[1]) - i64::from(bridge.xy[1]))
                * (i64::from(hole.xy[1]) - i64::from(bridge.xy[1]))
            < 0
    {
        return bridge_i;
    }

    let end_i = Some(bridge.next_i);
    filter_points(nodes, bridge_i, end_i, blocks).0
}

/// check if a polygon diagonal is locally inside the polygon
fn locally_inside(nodes: &[Node], a: &Node, b: &Node) -> bool {
    let a_prev = node!(nodes, a.prev_i);
    let a_next = node!(nodes, a.next_i);
    if area(a_prev, a, a_next) < 0 {
        area(a, b, a_next) >= 0 && area(a, a_prev, b) >= 0
    } else {
        area(a, b, a_prev) < 0 || area(a, a_next, b) < 0
    }
}

/// David Eberly's algorithm for finding a bridge between hole and outer polygon
fn find_hole_bridge(
    nodes: &[Node],
    hole: &Node,
    outer_node_i: NodeOffset,
    blocks: &mut HoleBlockIndex,
) -> Option<NodeOffset> {
    if blocks.chunk_order.is_empty() {
        let mut head_i = blocks.heads[0];
        loop {
            let prev_i = node!(nodes, head_i).prev_i;
            if node!(nodes, prev_i).next_i == head_i {
                break;
            }
            head_i = node!(nodes, head_i).next_i;
        }
        blocks.heads[0] = head_i;
        return find_hole_bridge_linear(nodes, hole, outer_node_i, head_i);
    }

    // Best ray/segment crossing as an exact fraction `(num, den)`, `den > 0`.
    let mut qx: Option<(i64, i64)> = None;
    let mut m_i: Option<NodeOffset> = None;
    let [hx, hy] = hole.xy;

    // find a segment intersected by a ray from the hole's leftmost point to the left;
    // segment's endpoint with lesser x will be potential connection point
    // unless they intersect at a vertex, then choose the vertex
    if equals(hole, node!(nodes, outer_node_i)) {
        return Some(outer_node_i);
    }
    let single_chunk = blocks.chunk_order.len() == 1;
    for ci in 0..blocks.chunk_order.len() {
        let chunk = blocks.chunk_order[ci] as usize;
        let [min_x, min_y, max_x, max_y] = blocks.chunk_bboxes[chunk];
        if !single_chunk
            && (hy < min_y
                || hy > max_y
                || min_x > hx
                || qx.is_some_and(|(n, d)| n >= i64::from(max_x) * d))
        {
            continue;
        }
        let members = blocks.chunk_range(chunk);
        for &block in &blocks.chunk_members[members] {
            let block = block as usize;
            let [min_x, min_y, max_x, max_y] = blocks.bboxes[block];
            if hy < min_y
                || hy > max_y
                || min_x > hx
                || qx.is_some_and(|(n, d)| n >= i64::from(max_x) * d)
            {
                continue;
            }

            let Some(head_i) = HoleBlockIndex::live_head(&mut blocks.heads, nodes, block) else {
                continue;
            };
            // walking `next` from a live node only visits live nodes
            let mut p_i = head_i;
            loop {
                let p = node!(nodes, p_i);
                let next_i = p.next_i;
                let p_next = node!(nodes, next_i);
                if equals(hole, p_next) {
                    return Some(next_i);
                }
                if hy <= p.xy[1] && hy >= p_next.xy[1] && p_next.xy[1] != p.xy[1] {
                    // Exact fractional ray/segment intersection avoids the rounding
                    // that a truncating integer division would introduce, which
                    // can otherwise pick a non-visible bridge on sheared inputs.
                    let (x_num, x_den) = ray_intersection_x(p.xy, p_next.xy, hy);
                    let hx_scaled = i64::from(hx) * x_den;
                    if x_num <= hx_scaled && qx.is_none_or(|(n, d)| x_num * d > n * x_den) {
                        qx = Some((x_num, x_den));
                        m_i = Some(if p.xy[0] < p_next.xy[0] { p_i } else { next_i });
                        if x_num == hx_scaled {
                            // hole touches outer segment; pick leftmost endpoint
                            return m_i;
                        }
                    }
                }
                p_i = next_i;
                if p_i == head_i || p_next.z != p.z {
                    break;
                }
            }
        }
    }

    let mut m_i = m_i?;
    let (qx_num, qx_den) = qx.expect("a bridge endpoint requires a ray intersection");

    // look for points inside the triangle of hole point, segment intersection and endpoint;
    // if there are no points found, we have a valid connection;
    // otherwise choose the point of the minimum angle with the ray as connection point

    let mut m = node!(nodes, m_i);
    let mxmy = m.xy;
    // `tan_min` as (|dy|, dx) — represents +infinity while dx == 0.
    let mut tan_min_abs_dy: i64 = 1;
    let mut tan_min_dx: i64 = 0;

    let triangle_min_y = hy.min(mxmy[1]);
    let triangle_max_y = hy.max(mxmy[1]);

    let single_chunk = blocks.chunk_order.len() == 1;
    for ci in 0..blocks.chunk_order.len() {
        let chunk = blocks.chunk_order[ci] as usize;
        let [min_x, min_y, max_x, max_y] = blocks.chunk_bboxes[chunk];
        if !single_chunk
            && (max_x < mxmy[0] || min_x > hx || max_y < triangle_min_y || min_y > triangle_max_y)
        {
            continue;
        }
        let members = blocks.chunk_range(chunk);
        for &block in &blocks.chunk_members[members] {
            let block = block as usize;
            let [min_x, min_y, max_x, max_y] = blocks.bboxes[block];
            if max_x < mxmy[0] || min_x > hx || max_y < triangle_min_y || min_y > triangle_max_y {
                continue;
            }

            let Some(head_i) = HoleBlockIndex::live_head(&mut blocks.heads, nodes, block) else {
                continue;
            };
            let mut p_i = head_i;
            loop {
                let p = node!(nodes, p_i);
                if p.z as usize != block {
                    break;
                }
                let next_i = p.next_i;
                if hx >= p.xy[0]
                    && p.xy[0] >= mxmy[0]
                    && hx != p.xy[0]
                    && point_in_bridge_triangle([hx, hy], mxmy, qx_num, qx_den, p.xy)
                {
                    // `tan` cross products are differences bounded by the 2^30 range,
                    // so this comparison stays exact in i64.
                    let abs_dy = i64::from(hy).abs_diff(i64::from(p.xy[1])) as i64;
                    let dx = i64::from(hx) - i64::from(p.xy[0]);
                    let cmp = (abs_dy * tan_min_dx).cmp(&(tan_min_abs_dy * dx));
                    let p_next = node!(nodes, next_i);
                    if (locally_inside(nodes, p, hole)
                        || (p.xy[1] == hy && p_next.xy[1] == hy && p_next.xy[0] > hx))
                        && (cmp.is_lt()
                            || (cmp.is_eq()
                                && (p.xy[0] > m.xy[0]
                                    || (p.xy[0] == m.xy[0]
                                        && sector_contains_sector(nodes, m, p)))))
                    {
                        (m_i, m) = (p_i, p);
                        tan_min_abs_dy = abs_dy;
                        tan_min_dx = dx;
                    }
                }
                p_i = next_i;
                if p_i == head_i {
                    break;
                }
            }
        }
    }

    Some(m_i)
}

// Direct ring traversal, derived from this module's bridge search.
fn find_hole_bridge_linear(
    nodes: &[Node],
    hole: &Node,
    outer_node_i: NodeOffset,
    head_i: NodeOffset,
) -> Option<NodeOffset> {
    // Best ray/segment crossing as an exact fraction `(num, den)`, `den > 0`.
    let mut qx: Option<(i64, i64)> = None;
    let mut m_i: Option<NodeOffset> = None;
    let [hx, hy] = hole.xy;

    // find a segment intersected by a ray from the hole's leftmost point to the left;
    // segment's endpoint with lesser x will be potential connection point
    // unless they intersect at a vertex, then choose the vertex
    if equals(hole, node!(nodes, outer_node_i)) {
        return Some(outer_node_i);
    }
    let mut p_i = head_i;
    loop {
        let p = node!(nodes, p_i);
        let next_i = p.next_i;
        let p_next = node!(nodes, next_i);
        if equals(hole, p_next) {
            return Some(next_i);
        }
        if hy <= p.xy[1] && hy >= p_next.xy[1] && p_next.xy[1] != p.xy[1] {
            // Exact fractional ray/segment intersection avoids the rounding
            // that a truncating integer division would introduce, which
            // can otherwise pick a non-visible bridge on sheared inputs.
            let (x_num, x_den) = ray_intersection_x(p.xy, p_next.xy, hy);
            let hx_scaled = i64::from(hx) * x_den;
            if x_num <= hx_scaled && qx.is_none_or(|(n, d)| x_num * d > n * x_den) {
                qx = Some((x_num, x_den));
                m_i = Some(if p.xy[0] < p_next.xy[0] { p_i } else { next_i });
                if x_num == hx_scaled {
                    // hole touches outer segment; pick leftmost endpoint
                    return m_i;
                }
            }
        }
        p_i = next_i;
        if p_i == head_i {
            break;
        }
    }

    let mut m_i = m_i?;
    let (qx_num, qx_den) = qx.expect("a bridge endpoint requires a ray intersection");

    // look for points inside the triangle of hole point, segment intersection and endpoint;
    // if there are no points found, we have a valid connection;
    // otherwise choose the point of the minimum angle with the ray as connection point

    let mut m = node!(nodes, m_i);
    let mxmy = m.xy;
    // `tan_min` as (|dy|, dx) — represents +infinity while dx == 0.
    let mut tan_min_abs_dy: i64 = 1;
    let mut tan_min_dx: i64 = 0;

    let mut p_i = head_i;
    loop {
        let p = node!(nodes, p_i);
        let next_i = p.next_i;
        if hx >= p.xy[0]
            && p.xy[0] >= mxmy[0]
            && hx != p.xy[0]
            && point_in_bridge_triangle([hx, hy], mxmy, qx_num, qx_den, p.xy)
        {
            // `tan` cross products are differences bounded by the 2^30 range,
            // so this comparison stays exact in i64.
            let abs_dy = i64::from(hy).abs_diff(i64::from(p.xy[1])) as i64;
            let dx = i64::from(hx) - i64::from(p.xy[0]);
            let cmp = (abs_dy * tan_min_dx).cmp(&(tan_min_abs_dy * dx));
            let p_next = node!(nodes, next_i);
            if (locally_inside(nodes, p, hole)
                || (p.xy[1] == hy && p_next.xy[1] == hy && p_next.xy[0] > hx))
                && (cmp.is_lt()
                    || (cmp.is_eq()
                        && (p.xy[0] > m.xy[0]
                            || (p.xy[0] == m.xy[0] && sector_contains_sector(nodes, m, p)))))
            {
                (m_i, m) = (p_i, p);
                tan_min_abs_dy = abs_dy;
                tan_min_dx = dx;
            }
        }
        p_i = next_i;
        if p_i == head_i {
            break;
        }
    }

    Some(m_i)
}

/// whether sector in vertex m contains sector in vertex p in the same coordinates
fn sector_contains_sector(nodes: &[Node], m: &Node, p: &Node) -> bool {
    area(node!(nodes, m.prev_i), m, node!(nodes, p.prev_i)) < 0
        && area(node!(nodes, p.next_i), m, node!(nodes, m.next_i)) < 0
}

/// eliminate colinear or duplicate points
fn filter_points(
    nodes: &mut [Node],
    start_i: NodeOffset,
    end_i: Option<NodeOffset>,
    mut blocks: Option<&mut HoleBlockIndex>,
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
            && (equals(p, p_next) || area(node!(nodes, p.prev_i), p, p_next) == 0)
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
fn split_polygon(nodes: &mut Vec<Node>, a_i: NodeOffset, b_i: NodeOffset) -> NodeOffset {
    debug_assert!(!nodes.is_empty());
    let a2_i = node_offset::<Node>(nodes.len());
    let b2_i = node_offset::<Node>(nodes.len() + 1);

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
fn insert_node(
    nodes: &mut Vec<Node>,
    i: u32,
    xy: [i32; 2],
    last: Option<NodeOffset>,
) -> NodeOffset {
    let mut p = Node::new(i, xy);
    let p_i = node_offset::<Node>(nodes.len());
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

fn remove_node(nodes: &mut [Node], pl: LinkInfo) -> (NodeOffset, NodeOffset) {
    let prev = node_mut!(nodes, pl.prev_i);
    prev.next_i = pl.next_i;
    if let Some(prev_z_i) = pl.prev_z_i {
        if prev_z_i == pl.prev_i {
            prev.next_z_i = pl.next_z_i;
        } else {
            node_mut!(nodes, prev_z_i).next_z_i = pl.next_z_i;
        }
    }

    let next = node_mut!(nodes, pl.next_i);
    next.prev_i = pl.prev_i;
    if let Some(next_z_i) = pl.next_z_i {
        if next_z_i == pl.next_i {
            next.prev_z_i = pl.prev_z_i;
        } else {
            node_mut!(nodes, next_z_i).prev_z_i = pl.prev_z_i;
        }
    }

    (pl.prev_i, pl.next_i)
}

#[inline]
fn input_bbox(data: &[[i32; 2]]) -> InputBbox {
    let mut bbox = InputBbox::new(data[0]);
    for &xy in &data[1..] {
        bbox.update(xy);
    }
    bbox
}

/// z-order of a point given coords, origin, and a right-shift to normalize
/// coordinates into the 15-bit z-order range.
fn z_order(xy: [i32; 2], min_x: i32, min_y: i32, shift: u32) -> i32 {
    // coords are transformed into non-negative 15-bit integer range
    let mut x = (((xy[0] as i64) - (min_x as i64)) >> shift) as u32 & 0x7FFF;
    let mut y = (((xy[1] as i64) - (min_y as i64)) >> shift) as u32 & 0x7FFF;

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

fn point_in_triangle(a: [i32; 2], b: [i32; 2], c: [i32; 2], p: [i32; 2]) -> bool {
    let (ax, ay) = (i64::from(a[0]), i64::from(a[1]));
    let (bx, by) = (i64::from(b[0]), i64::from(b[1]));
    let (cx, cy) = (i64::from(c[0]), i64::from(c[1]));
    let (px, py) = (i64::from(p[0]), i64::from(p[1]));
    cmp_products(cx - px, ay - py, ax - px, cy - py).is_ge()
        && cmp_products(ax - px, by - py, bx - px, ay - py).is_ge()
        && cmp_products(bx - px, cy - py, cx - px, by - py).is_ge()
}

/// Point-in-triangle test where one triangle vertex has an exact fractional
/// x-coordinate `num / den` (the ray/segment intersection). Used only by
/// [`find_hole_bridge`]. Within the `[-2^19, 2^19]` domain the fractional
/// orientations stay exact in `i64`.
fn point_in_bridge_triangle(
    hole: [i32; 2],
    bridge: [i32; 2],
    num: i64,
    den: i64,
    p: [i32; 2],
) -> bool {
    if hole[1] < bridge[1] {
        orient_i32(hole, bridge, p) >= 0
            && orient_i32_to_frac(bridge, num, den, hole[1], p) >= 0
            && orient_frac_to_i32(num, den, hole[1], hole, p) >= 0
    } else {
        orient_frac_to_i32(num, den, hole[1], bridge, p) >= 0
            && orient_i32(bridge, hole, p) >= 0
            && orient_i32_to_frac(hole, num, den, hole[1], p) >= 0
    }
}

fn orient_i32(a: [i32; 2], b: [i32; 2], p: [i32; 2]) -> i64 {
    (i64::from(b[0]) - i64::from(a[0])) * (i64::from(p[1]) - i64::from(a[1]))
        - (i64::from(b[1]) - i64::from(a[1])) * (i64::from(p[0]) - i64::from(a[0]))
}

/// Orientation of triangle `(a, (num/den, b_y), p)`, scaled by `den > 0`
/// (which preserves sign).
fn orient_i32_to_frac(a: [i32; 2], num: i64, den: i64, b_y: i32, p: [i32; 2]) -> i64 {
    let dx_b = num - i64::from(a[0]) * den;
    let dx_p = i64::from(p[0]) - i64::from(a[0]);
    dx_b * (i64::from(p[1]) - i64::from(a[1])) - (i64::from(b_y) - i64::from(a[1])) * dx_p * den
}

/// Orientation of triangle `((num/den, a_y), b, p)`, scaled by `den > 0`
/// (which preserves sign).
fn orient_frac_to_i32(num: i64, den: i64, a_y: i32, b: [i32; 2], p: [i32; 2]) -> i64 {
    let dx_b = i64::from(b[0]) * den - num;
    let dx_p = i64::from(p[0]) * den - num;
    dx_b * (i64::from(p[1]) - i64::from(a_y)) - (i64::from(b[1]) - i64::from(a_y)) * dx_p
}

fn point_in_triangle_except_first(a: [i32; 2], b: [i32; 2], c: [i32; 2], p: [i32; 2]) -> bool {
    !(a[0] == p[0] && a[1] == p[1]) && point_in_triangle(a, b, c, p)
}

/// signed area of a triangle (twice the geometric area)
fn area(p: &Node, q: &Node, r: &Node) -> i8 {
    let (px, py) = (i64::from(p.xy[0]), i64::from(p.xy[1]));
    let (qx, qy) = (i64::from(q.xy[0]), i64::from(q.xy[1]));
    let (rx, ry) = (i64::from(r.xy[0]), i64::from(r.xy[1]));
    match cmp_products(qy - py, rx - qx, qx - px, ry - qy) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// check if two points are equal
fn equals(p1: &Node, p2: &Node) -> bool {
    p1.xy == p2.xy
}

/// for collinear points p, q, r, check if point q lies on segment pr
fn on_segment(p: &Node, q: &Node, r: &Node) -> bool {
    ((q.xy[0] <= p.xy[0].max(r.xy[0])) & (q.xy[1] <= p.xy[1].max(r.xy[1])))
        && ((q.xy[0] >= p.xy[0].min(r.xy[0])) & (q.xy[1] >= p.xy[1].min(r.xy[1])))
}

/// Integer earcut specialized for `i32` coordinates.
pub struct EarcutI32 {
    data: Vec<[i32; 2]>,
    nodes: Vec<Node>,
    queue: Vec<(NodeOffset, i32)>,
    sort_queue: Vec<(i32, NodeOffset)>,
    sort_scratch: Vec<(i32, NodeOffset)>,
    /// halves of split polygons waiting to be triangulated, so deeply nested
    /// splits don't grow the call stack
    split_stack: Vec<NodeOffset>,
    hole_blocks: HoleBlockIndex,
}

impl Default for EarcutI32 {
    fn default() -> Self {
        Self::new()
    }
}

impl EarcutI32 {
    /// Creates a reusable `i32` earcut instance.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            nodes: Vec::new(),
            queue: Vec::new(),
            sort_queue: Vec::new(),
            sort_scratch: Vec::new(),
            split_stack: Vec::new(),
            hole_blocks: HoleBlockIndex::new(),
        }
    }

    fn reset(&mut self, capacity: usize) {
        self.nodes.clear();
        self.nodes.reserve(capacity);
        self.nodes.push(Node::new(0, [i32::MAX, i32::MAX]));
    }

    /// Performs earcut triangulation on `i32` coordinates.
    ///
    /// # Compatibility
    ///
    /// The triangulation and index order are not guaranteed to match
    /// mapbox/earcut (upstream main at `934e54d`). Ties between candidates
    /// for a hole bridge may be resolved differently.
    ///
    /// # Domain
    ///
    /// Every coordinate must lie within `[-2^19, 2^19]` on both axes.
    ///
    /// # Panics
    ///
    /// - if any coordinate lies outside `[-2^19, 2^19]`.
    /// - if `hole_indices` contains a value greater than the number of
    ///   vertices in `data`, or is not monotonically non-decreasing.
    /// - if a vertex index cannot be represented by the output index type.
    /// - if the internal node storage exceeds the `u32` byte-offset capacity
    pub fn earcut<N: Index>(
        &mut self,
        data: impl IntoIterator<Item = [i32; 2]>,
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
    /// # Compatibility
    ///
    /// Same as [`earcut`](Self::earcut).
    ///
    /// # Panics
    ///
    /// Same as [`earcut`](Self::earcut).
    #[inline(always)]
    pub fn earcut_slice<N: Index>(
        &mut self,
        data: &[[i32; 2]],
        hole_indices: &[N],
        triangles_out: &mut Vec<N>,
    ) {
        triangles_out.clear();

        let mut previous_hole = 0;
        for &hole in hole_indices {
            let hole = hole.into_usize();
            assert!(
                hole <= data.len(),
                "EarcutI32 hole index {hole} exceeds vertex count {}",
                data.len()
            );
            assert!(
                hole >= previous_hole,
                "EarcutI32 hole indices are not monotonically non-decreasing"
            );
            previous_hole = hole;
        }

        if data.is_empty() {
            return;
        }
        let bbox = input_bbox(data);
        assert!(
            bbox.min_x >= -MAX_ABS_COORD
                && bbox.max_x <= MAX_ABS_COORD
                && bbox.min_y >= -MAX_ABS_COORD
                && bbox.max_y <= MAX_ABS_COORD,
            "EarcutI32 coordinate out of range: bbox ({}, {})..=({}, {}) exceeds [-2^19, 2^19]",
            bbox.min_x,
            bbox.min_y,
            bbox.max_x,
            bbox.max_y
        );
        if data.len() < 3 {
            return;
        }
        assert!(data.len() <= INDEX_MASK as usize + 1);

        triangles_out.reserve(data.len().saturating_mul(3));
        self.reset(data.len() / 2 * 3);
        self.run::<N>(data, hole_indices, triangles_out, bbox);
    }

    #[inline]
    fn run<N: Index>(
        &mut self,
        data: &[[i32; 2]],
        hole_indices: &[N],
        triangles_out: &mut Vec<N>,
        full_bbox: InputBbox,
    ) {
        let has_holes = !hole_indices.is_empty();
        let outer_len = if has_holes {
            hole_indices[0].into_usize()
        } else {
            data.len()
        };

        let Some(mut outer_node_i) = self.linked_list(data, 0, outer_len, true) else {
            return;
        };
        let outer_node = node!(self.nodes, outer_node_i);
        if outer_node.next_i == outer_node.prev_i {
            return;
        }
        if has_holes {
            outer_node_i = self.eliminate_holes::<N>(data, hole_indices, outer_node_i);
        }

        let mut min_x = 0i32;
        let mut min_y = 0i32;
        let mut shift = NO_HASH;
        let mut small_path = false;
        let need_bbox = data.len() > 80 || !has_holes;
        if need_bbox {
            let bbox = if has_holes {
                input_bbox(&data[..outer_len])
            } else {
                full_bbox
            };
            min_x = bbox.min_x;
            min_y = bbox.min_y;
            let range_x = i64::from(bbox.max_x) - i64::from(bbox.min_x);
            let range_y = i64::from(bbox.max_y) - i64::from(bbox.min_y);
            let range = range_x.max(range_y);
            if data.len() > 80 && range > 0 {
                shift = shift_for_z_order(range);
            }
            if !has_holes && shift == NO_HASH {
                small_path = use_small_path(range_x, range_y);
            }
        }

        self.split_stack.clear();
        let mut start_i = Some(outer_node_i);
        while let Some(ear_i) = start_i {
            earcut_linked::<N>(
                &mut self.nodes,
                ear_i,
                triangles_out,
                min_x,
                min_y,
                shift,
                small_path,
                &mut self.sort_queue,
                &mut self.sort_scratch,
                &mut self.split_stack,
            );
            start_i = self.split_stack.pop();
        }
    }

    fn linked_list(
        &mut self,
        data: &[[i32; 2]],
        start: usize,
        end: usize,
        clockwise: bool,
    ) -> Option<NodeOffset> {
        let data = &data[start..end];
        if data.is_empty() {
            return None;
        }

        let mut last_i = None;
        let iter = data.iter().enumerate();
        if clockwise == (signed_area(data) > 0) {
            for (i, &xy) in iter {
                last_i = Some(insert_node(&mut self.nodes, (start + i) as u32, xy, last_i));
            }
        } else {
            for (i, &xy) in iter.rev() {
                last_i = Some(insert_node(&mut self.nodes, (start + i) as u32, xy, last_i));
            }
        }
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

    #[inline(never)]
    fn eliminate_holes<N: Index>(
        &mut self,
        data: &[[i32; 2]],
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
            match ax.cmp(bx) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
            let a = node!(self.nodes, *ai);
            let b = node!(self.nodes, *bi);
            match a.xy[1].cmp(&b.xy[1]) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
            let a_next = node!(self.nodes, a.next_i);
            let b_next = node!(self.nodes, b.next_i);
            let ady = i64::from(a_next.xy[1]) - i64::from(a.xy[1]);
            let adx = i64::from(a_next.xy[0]) - i64::from(a.xy[0]);
            let bdy = i64::from(b_next.xy[1]) - i64::from(b.xy[1]);
            let bdx = i64::from(b_next.xy[0]) - i64::from(b.xy[0]);
            (ady * bdx).cmp(&(bdy * adx))
        });

        // Keep the initial ring anchor on the direct path so candidate ties
        // use the same traversal order as the indexed path.
        if data.len() <= DIRECT_BRIDGE_MAX_VERTICES && hole_indices.len() <= DIRECT_BRIDGE_MAX_HOLES
        {
            self.hole_blocks.chunk_order.clear();
            self.hole_blocks.heads.clear();
            self.hole_blocks.heads.push(outer_node_i);
        } else {
            self.hole_blocks.reset(data.len(), hole_indices.len());
            self.hole_blocks.index_outer(&mut self.nodes, outer_node_i);
        }

        for i in 0..self.queue.len() {
            let q = self.queue[i].0;
            outer_node_i = eliminate_hole(&mut self.nodes, q, outer_node_i, &mut self.hole_blocks);
        }
        filter_points(&mut self.nodes, outer_node_i, None, None).0
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{EarcutI32, deviation};

    #[test]
    fn deviation_is_zero_for_too_short_input() {
        let data = [[0, 0], [10, 0]];

        assert_eq!(deviation(data, &[] as &[u32], &[] as &[u32]), 0);
    }

    #[test]
    fn clears_output_for_too_short_input() {
        let data = [[0, 0], [10, 0]];
        let mut earcut = EarcutI32::default();
        let mut triangles = vec![99u32, 100, 101];

        earcut.earcut(data, &[] as &[u32], &mut triangles);

        assert!(triangles.is_empty());
    }

    #[test]
    fn supports_u16_indices() {
        let data = [[0, 0], [10, 0], [10, 10], [0, 10]];
        let mut earcut = EarcutI32::new();
        let mut triangles = Vec::<u16>::new();

        earcut.earcut(data, &[] as &[u16], &mut triangles);

        assert_eq!(triangles.len(), 6);
        assert_eq!(deviation(data, &[] as &[u16], &triangles), 0);
    }

    #[test]
    fn supports_usize_indices() {
        let data = [[0, 0], [10, 0], [10, 10], [0, 10]];
        let mut earcut = EarcutI32::new();
        let mut triangles = Vec::<usize>::new();

        earcut.earcut(data, &[] as &[usize], &mut triangles);

        assert_eq!(triangles.len(), 6);
        assert_eq!(deviation(data, &[] as &[usize], &triangles), 0);
    }

    #[test]
    fn returns_empty_for_collapsed_outer_ring() {
        let data = [[0, 0], [1, 0], [1, 0]];
        let mut earcut = EarcutI32::new();
        let mut triangles = Vec::<u32>::new();

        earcut.earcut(data, &[] as &[u32], &mut triangles);

        assert!(triangles.is_empty());
    }

    #[test]
    fn ignores_empty_hole_ring() {
        let data = [[0, 0], [10, 0], [0, 10]];
        let mut earcut = EarcutI32::new();
        let mut triangles = Vec::<u32>::new();

        earcut.earcut(data, &[3u32], &mut triangles);

        assert_eq!(triangles.len(), 3);
        assert_eq!(deviation(data, &[] as &[u32], &triangles), 0);
    }
}
