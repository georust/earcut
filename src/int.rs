//! Integer-coordinate variant of [`crate::Earcut`]
//!
//! Uses exact integer arithmetic, avoiding the
//! floating-point rounding errors of [`crate::Earcut`].

use alloc::vec::Vec;
use core::{cmp::Ordering, num::NonZeroU32, ptr};

use crate::node_offset;

#[inline(always)]
fn use_small_path(range_x: i64, range_y: i64) -> bool {
    range_x == 0
        || range_y == 0
        || (range_x <= i32::MAX as i64
            && range_y <= i32::MAX as i64
            && range_x * range_y <= i32::MAX as i64)
}

#[inline(always)]
fn shift_for_z_order(range: i64) -> u32 {
    let used = i64::BITS - range.leading_zeros();
    used.saturating_sub(15)
}

#[inline(always)]
fn wide_sign(v: i64) -> i32 {
    (v > 0) as i32 - (v < 0) as i32
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

/// Returns the absolute integer difference between the polygon area (times 2)
/// and its triangulation area (times 2) for `i32` coordinates. Zero indicates
/// an exact match; non-zero signals a triangulation error.
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
        let v = ((data[a][0] as i64) - (data[c][0] as i64))
            * ((data[b][1] as i64) - (data[a][1] as i64))
            - ((data[a][0] as i64) - (data[b][0] as i64))
                * ((data[c][1] as i64) - (data[a][1] as i64));
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
    let [last_x, last_y] = data[data.len() - 1];
    let [mut bx, mut by] = [last_x as i64, last_y as i64];
    let mut sum = 0;
    for &[ax_r, ay_r] in data {
        let ax = ax_r as i64;
        let ay = ay_r as i64;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    P0 = 0,
    P1 = 1,
    P2 = 2,
}

/// main ear slicing loop which triangulates a polygon (given as a linked list)
#[allow(clippy::too_many_arguments)]
fn earcut_linked<N: Index>(
    nodes: &mut Vec<Node>,
    ear_i: NodeOffset,
    triangles: &mut Vec<N>,
    min_x: i32,
    min_y: i32,
    shift: u32,
    small_path: bool,
    sort_queue: &mut Vec<(i32, NodeOffset)>,
    pass: Pass,
) {
    let mut ear_i = ear_i;

    // interlink polygon nodes in z-order
    if pass == Pass::P0 && shift != NO_HASH {
        index_curve(nodes, ear_i, min_x, min_y, shift, sort_queue);
    }

    let mut stop_i = ear_i;

    loop {
        let ear = node!(nodes, ear_i);
        if ear.prev_i == ear.next_i {
            break;
        }
        let ni = ear.next_i;

        let (is_ear, prev, next) = if shift != NO_HASH {
            is_ear_hashed(nodes, ear, min_x, min_y, shift)
        } else if small_path {
            is_ear_small(nodes, ear)
        } else {
            is_ear(nodes, ear)
        };
        if is_ear {
            let next_i = next.index();
            let next_next_i = next.next_i;

            triangles.extend([
                N::from_usize(prev.index() as usize),
                N::from_usize(ear.index() as usize),
                N::from_usize(next_i as usize),
            ]);

            let ll = ear.link_info();
            remove_node(nodes, ll);

            (ear_i, stop_i) = (next_next_i, next_next_i);
            continue;
        }

        ear_i = ni;
        if ear_i == stop_i {
            if pass == Pass::P0 {
                ear_i = filter_points(nodes, ear_i, None);
                earcut_linked(
                    nodes,
                    ear_i,
                    triangles,
                    min_x,
                    min_y,
                    shift,
                    small_path,
                    sort_queue,
                    Pass::P1,
                );
            } else if pass == Pass::P1 {
                let filtered = filter_points(nodes, ear_i, None);
                ear_i = cure_local_intersections(nodes, filtered, triangles);
                earcut_linked(
                    nodes,
                    ear_i,
                    triangles,
                    min_x,
                    min_y,
                    shift,
                    small_path,
                    sort_queue,
                    Pass::P2,
                );
            } else {
                split_earcut(
                    nodes, ear_i, triangles, min_x, min_y, shift, small_path, sort_queue,
                );
            }
            return;
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

    // look for points inside the triangle in increasing z-order
    //
    // Unlike the float version, we keep the bbox prefilter: the i32 comparisons are
    // cheaper than i64 multiplications.
    let mut o_n = ear.next_z_i.map(|i| node!(nodes, i));
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

    // look for points inside the triangle in decreasing z-order
    let mut o_p = ear.prev_z_i.map(|i| node!(nodes, i));
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
    loop {
        let p = node!(nodes, p_i);
        let p_next_i = p.next_i;
        let p_next = node!(nodes, p_next_i);
        let b_i = p_next.next_i;
        let a = node!(nodes, p.prev_i);
        let b = node!(nodes, b_i);

        if !equals(a, b)
            && intersects(a, p, p_next, b)
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
        } else {
            p_i = p.next_i;
        }

        if p_i == start_i {
            return filter_points(nodes, p_i, None);
        }
    }
}

/// try splitting polygon into two and triangulate them independently
#[allow(clippy::too_many_arguments)]
fn split_earcut<N: Index>(
    nodes: &mut Vec<Node>,
    start_i: NodeOffset,
    triangles: &mut Vec<N>,
    min_x: i32,
    min_y: i32,
    shift: u32,
    small_path: bool,
    sort_queue: &mut Vec<(i32, NodeOffset)>,
) {
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
                ai = filter_points(nodes, ai, end_i);
                let end_i = Some(node!(nodes, ci).next_i);
                ci = filter_points(nodes, ci, end_i);

                // run earcut on each half
                earcut_linked(
                    nodes,
                    ai,
                    triangles,
                    min_x,
                    min_y,
                    shift,
                    small_path,
                    sort_queue,
                    Pass::P0,
                );
                earcut_linked(
                    nodes,
                    ci,
                    triangles,
                    min_x,
                    min_y,
                    shift,
                    small_path,
                    sort_queue,
                    Pass::P0,
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
fn index_curve(
    nodes: &mut [Node],
    start_i: NodeOffset,
    min_x: i32,
    min_y: i32,
    shift: u32,
    order: &mut Vec<(i32, NodeOffset)>,
) {
    order.clear();
    let mut p_i = start_i;
    let mut p = node_mut!(nodes, p_i);

    loop {
        if p.z == 0 {
            p.z = z_order(p.xy, min_x, min_y, shift);
        }
        order.push((p.z, p_i));
        p_i = p.next_i;
        p = node_mut!(nodes, p_i);
        if p_i == start_i {
            break;
        }
    }

    order.sort_unstable_by_key(|&(z, _)| z);

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

/// check if two segments intersect
fn intersects(p1: &Node, q1: &Node, p2: &Node, q2: &Node) -> bool {
    let o1 = wide_sign(area(p1, q1, p2));
    let o2 = wide_sign(area(p1, q1, q2));
    let o3 = wide_sign(area(p2, q2, p1));
    let o4 = wide_sign(area(p2, q2, q1));
    ((o1 != o2) & (o3 != o4)) // general case
        || (o3 == 0 && on_segment(p2, p1, q2)) // p2, q2 and p1 are collinear and p1 lies on p2q2
        || (o4 == 0 && on_segment(p2, q1, q2)) // p2, q2 and q1 are collinear and q1 lies on p2q2
        || (o2 == 0 && on_segment(p1, q2, q1)) // p1, q1 and q2 are collinear and q2 lies on p1q1
        || (o1 == 0 && on_segment(p1, p2, q1)) // p1, q1 and p2 are collinear and p2 lies on p1q1
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
            && intersects(p, p_next, a, b)
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
    let two_px = a.xy[0] as i64 + b.xy[0] as i64;
    let two_py = a.xy[1] as i64 + b.xy[1] as i64;
    let mut p = a;
    let mut inside = false;
    loop {
        let p_next = node!(nodes, p.next_i);
        let py_doubled = p.xy[1] as i64 + p.xy[1] as i64;
        let pn_py_doubled = p_next.xy[1] as i64 + p_next.xy[1] as i64;
        let crosses = (py_doubled > two_py) != (pn_py_doubled > two_py) && p_next.xy[1] != p.xy[1];
        if crosses {
            let dy = p_next.xy[1] as i64 - p.xy[1] as i64;
            let dx = p_next.xy[0] as i64 - p.xy[0] as i64;
            let lhs = (two_px - (p.xy[0] as i64 + p.xy[0] as i64)) * dy;
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

/// find a bridge between vertices that connects hole with an outer ring and and link it
fn eliminate_hole(
    nodes: &mut Vec<Node>,
    hole_i: NodeOffset,
    outer_node_i: NodeOffset,
) -> NodeOffset {
    let Some(bridge_i) = find_hole_bridge(nodes, node!(nodes, hole_i), outer_node_i) else {
        return outer_node_i;
    };
    let bridge_reverse_i = split_polygon(nodes, bridge_i, hole_i);

    // filter collinear points around the cuts
    let end_i = Some(node!(nodes, bridge_reverse_i).next_i);
    filter_points(nodes, bridge_reverse_i, end_i);
    let end_i = Some(node!(nodes, bridge_i).next_i);
    filter_points(nodes, bridge_i, end_i)
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
fn find_hole_bridge(nodes: &[Node], hole: &Node, outer_node_i: NodeOffset) -> Option<NodeOffset> {
    let mut p_i = outer_node_i;
    let mut qx: i64 = i64::MIN;
    let mut m_i: Option<NodeOffset> = None;

    // find a segment intersected by a ray from the hole's leftmost point to the left;
    // segment's endpoint with lesser x will be potential connection point
    // unless they intersect at a vertex, then choose the vertex
    let mut p = node!(nodes, p_i);
    if equals(hole, p) {
        return Some(p_i);
    }
    let hole_x_w = hole.xy[0] as i64;
    let hole_y_w = hole.xy[1] as i64;
    loop {
        let p_next = node!(nodes, p.next_i);
        if equals(hole, p_next) {
            return Some(p.next_i);
        }
        if hole.xy[1] <= p.xy[1] && hole.xy[1] >= p_next.xy[1] && p_next.xy[1] != p.xy[1] {
            // p.y >= hole.y >= p_next.y and p.y != p_next.y, so denom > 0:
            //   x = p.x + (p.y - hole.y) * (p_next.x - p.x) / (p.y - p_next.y)
            let p_x_w = p.xy[0] as i64;
            let p_y_w = p.xy[1] as i64;
            let denom = p_y_w - p_next.xy[1] as i64; // > 0
            let offset_scaled = (p_y_w - hole_y_w) * (p_next.xy[0] as i64 - p_x_w);
            let x = p_x_w + offset_scaled / denom; // Rust's i64/i128 div truncates toward 0
            if x <= hole_x_w && x > qx {
                qx = x;
                m_i = Some(if p.xy[0] < p_next.xy[0] {
                    p_i
                } else {
                    p.next_i
                });
                // Exact "hole lies on segment" check (avoids the precision loss
                // introduced by the integer division above):
                //   (hole.y - p.y) * (p_next.x - p.x) == (hole.x - p.x) * (p_next.y - p.y)
                let lhs = (hole_y_w - p_y_w) * (p_next.xy[0] as i64 - p_x_w);
                let rhs = (hole_x_w - p_x_w) * (p_next.xy[1] as i64 - p_y_w);
                if lhs == rhs {
                    // hole touches outer segment; pick leftmost endpoint
                    return m_i;
                }
            }
        }
        p_i = p.next_i;
        if p_i == outer_node_i {
            break;
        }
        p = p_next;
    }

    let mut m_i = m_i?;

    // look for points inside the triangle of hole point, segment intersection and endpoint;
    // if there are no points found, we have a valid connection;
    // otherwise choose the point of the minimum angle with the ray as connection point

    let stop_i = m_i;
    let mut m = node!(nodes, m_i);
    let mxmy = m.xy;
    // `tan_min` as (|dy|, dx) — represents +infinity while dx == 0.
    let mut tan_min_abs_dy: i64 = 1;
    let mut tan_min_dx: i64 = 0;

    p_i = m_i;
    let mut p = m;

    let qx_t = qx as i32;

    let (tri_a, tri_c) = if hole.xy[1] < mxmy[1] {
        ([hole.xy[0], hole.xy[1]], [qx_t, hole.xy[1]])
    } else {
        ([qx_t, hole.xy[1]], [hole.xy[0], hole.xy[1]])
    };

    loop {
        if (((hole.xy[0] >= p.xy[0]) & (p.xy[0] >= mxmy[0])) && hole.xy[0] != p.xy[0])
            && point_in_triangle(tri_a, mxmy, tri_c, p.xy)
        {
            // tan = |hole.y - p.y| / (hole.x - p.x),  denom > 0 here.
            // Compare via cross-product so we never have to divide:
            //   tan <=> tan_min  ↔  abs_dy * tan_min_dx <=> tan_min_abs_dy * dx
            let abs_dy = i64::abs(hole_y_w - p.xy[1] as i64);
            let dx = hole_x_w - p.xy[0] as i64; // > 0
            let cmp = (abs_dy * tan_min_dx).cmp(&(tan_min_abs_dy * dx));
            if locally_inside(nodes, p, hole)
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

        p_i = p.next_i;
        if p_i == stop_i {
            return Some(m_i);
        }
        p = node!(nodes, p_i);
    }
}

/// whether sector in vertex m contains sector in vertex p in the same coordinates
fn sector_contains_sector(nodes: &[Node], m: &Node, p: &Node) -> bool {
    area(node!(nodes, m.prev_i), m, node!(nodes, p.prev_i)) < 0
        && area(node!(nodes, p.next_i), m, node!(nodes, m.next_i)) < 0
}

/// eliminate colinear or duplicate points
fn filter_points(nodes: &mut [Node], start_i: NodeOffset, end_i: Option<NodeOffset>) -> NodeOffset {
    let mut end_i = end_i.unwrap_or(start_i);

    let mut p_i = start_i;
    let mut p = node!(nodes, p_i);
    loop {
        let p_next = node!(nodes, p.next_i);
        if !p.is_steiner() && (equals(p, p_next) || area(node!(nodes, p.prev_i), p, p_next) == 0) {
            let (prev_i, next_i) = remove_node(nodes, p.link_info());
            (p_i, end_i) = (prev_i, prev_i);
            if p_i == next_i {
                return end_i;
            }
            p = node!(nodes, p_i);
        } else {
            p_i = p.next_i;
            if p_i == end_i {
                return end_i;
            }
            p = p_next;
        };
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
    let (ax, ay) = (a[0] as i64, a[1] as i64);
    let (bx, by) = (b[0] as i64, b[1] as i64);
    let (cx, cy) = (c[0] as i64, c[1] as i64);
    let (px, py) = (p[0] as i64, p[1] as i64);
    ((cx - px) * (ay - py) >= (ax - px) * (cy - py))
        && ((ax - px) * (by - py) >= (bx - px) * (ay - py))
        && ((bx - px) * (cy - py) >= (cx - px) * (by - py))
}

fn point_in_triangle_except_first(a: [i32; 2], b: [i32; 2], c: [i32; 2], p: [i32; 2]) -> bool {
    !(a[0] == p[0] && a[1] == p[1]) && point_in_triangle(a, b, c, p)
}

/// signed area of a triangle (twice the geometric area)
fn area(p: &Node, q: &Node, r: &Node) -> i64 {
    let (px, py) = (p.xy[0] as i64, p.xy[1] as i64);
    let (qx, qy) = (q.xy[0] as i64, q.xy[1] as i64);
    let (rx, ry) = (r.xy[0] as i64, r.xy[1] as i64);
    (qy - py) * (rx - qx) - (qx - px) * (ry - qy)
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
        }
    }

    fn reset(&mut self, capacity: usize) {
        self.nodes.clear();
        self.nodes.reserve(capacity);
        self.nodes.push(Node::new(0, [i32::MAX, i32::MAX]));
    }

    /// Performs earcut triangulation on `i32` coordinates.
    ///
    /// # Panics
    ///
    /// - if `hole_indices` contains a value greater than the number of
    ///   vertices in `data`, or is not monotonically non-decreasing.
    /// - if the input has more than 2^31 vertices
    pub fn earcut<N: Index>(
        &mut self,
        data: impl IntoIterator<Item = [i32; 2]>,
        hole_indices: &[N],
        triangles_out: &mut Vec<N>,
    ) {
        self.data.clear();
        self.data.extend(data);
        triangles_out.clear();
        if self.data.len() < 3 {
            return;
        }
        assert!(self.data.len() <= INDEX_MASK as usize + 1);

        triangles_out.reserve(self.data.len().saturating_mul(3));
        self.reset(self.data.len() / 2 * 3);

        let has_holes = !hole_indices.is_empty();
        let outer_len = if has_holes {
            hole_indices[0].into_usize()
        } else {
            self.data.len()
        };

        let Some(mut outer_node_i) = self.linked_list(0, outer_len, true) else {
            return;
        };
        let outer_node = node!(self.nodes, outer_node_i);
        if outer_node.next_i == outer_node.prev_i {
            return;
        }
        if has_holes {
            outer_node_i = self.eliminate_holes(hole_indices, outer_node_i);
        }

        let mut min_x = 0i32;
        let mut min_y = 0i32;
        let mut shift = NO_HASH;
        let mut small_path = false;
        let need_bbox = self.data.len() > 80 || !has_holes;
        if need_bbox {
            let bbox = input_bbox(&self.data[..outer_len]);
            min_x = bbox.min_x;
            min_y = bbox.min_y;
            let range_x = (bbox.max_x as i64) - (bbox.min_x as i64);
            let range_y = (bbox.max_y as i64) - (bbox.min_y as i64);
            let range = range_x.max(range_y);
            if self.data.len() > 80 && range > 0 {
                shift = shift_for_z_order(range);
            }
            if !has_holes && shift == NO_HASH {
                small_path = use_small_path(range_x, range_y);
            }
        }

        earcut_linked(
            &mut self.nodes,
            outer_node_i,
            triangles_out,
            min_x,
            min_y,
            shift,
            small_path,
            &mut self.sort_queue,
            Pass::P0,
        );
    }

    fn linked_list(&mut self, start: usize, end: usize, clockwise: bool) -> Option<NodeOffset> {
        let data = &self.data[start..end];
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

    fn eliminate_holes<N: Index>(
        &mut self,
        hole_indices: &[N],
        mut outer_node_i: NodeOffset,
    ) -> NodeOffset {
        self.queue.clear();
        for (i, hi) in hole_indices.iter().enumerate() {
            let start = (*hi).into_usize();
            let end = if i < hole_indices.len() - 1 {
                hole_indices[i + 1].into_usize()
            } else {
                self.data.len()
            };
            if let Some(list_i) = self.linked_list(start, end, false) {
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
            let ady = (a_next.xy[1] as i64) - (a.xy[1] as i64);
            let adx = (a_next.xy[0] as i64) - (a.xy[0] as i64);
            let bdy = (b_next.xy[1] as i64) - (b.xy[1] as i64);
            let bdx = (b_next.xy[0] as i64) - (b.xy[0] as i64);
            (ady * bdx).cmp(&(bdy * adx))
        });

        for &(q, _) in &self.queue {
            outer_node_i = eliminate_hole(&mut self.nodes, q, outer_node_i);
        }
        outer_node_i
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
