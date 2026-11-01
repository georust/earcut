use std::collections::HashMap;

use earcut::{
    Earcut, Refiner, deviation,
    int::{EarcutI32, deviation as int_deviation},
    refine,
};

mod fixtures;
use fixtures::{EARCUT, FIXTURES};

// The corpus and its decoder live under `benches/` (its primary consumer is the
// `bench_tiles` benchmark); this correctness test shares the same loader.
#[path = "../benches/tiles_fixture.rs"]
mod tiles_fixture;

fn triangle_perimeter(triangles: &[u32], vertices: &[[f64; 2]]) -> f64 {
    triangles
        .chunks_exact(3)
        .map(|triangle| {
            let a = vertices[triangle[0] as usize];
            let b = vertices[triangle[1] as usize];
            let c = vertices[triangle[2] as usize];
            (a[0] - b[0]).hypot(a[1] - b[1])
                + (b[0] - c[0]).hypot(b[1] - c[1])
                + (c[0] - a[0]).hypot(c[1] - a[1])
        })
        .sum()
}

fn next_half_edge(edge: usize) -> usize {
    edge - edge % 3 + (edge + 1) % 3
}

fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn in_circle(a: [f64; 2], b: [f64; 2], c: [f64; 2], p: [f64; 2]) -> bool {
    let dx = a[0] - p[0];
    let dy = a[1] - p[1];
    let ex = b[0] - p[0];
    let ey = b[1] - p[1];
    let fx = c[0] - p[0];
    let fy = c[1] - p[1];
    let ap = dx * dx + dy * dy;
    let bp = ex * ex + ey * ey;
    let cp = fx * fx + fy * fy;

    dx * (ey * cp - bp * fy) - dy * (ex * cp - bp * fx) + ap * (ex * fy - ey * fx) <= 0.0
}

fn count_illegal_edges(triangles: &[u32], vertices: &[[f64; 2]]) -> usize {
    let mut half_edges = vec![None; triangles.len()];
    let mut edges = HashMap::new();

    for edge in 0..triangles.len() {
        let a = triangles[edge];
        let b = triangles[next_half_edge(edge)];
        let key = (a.min(b), a.max(b));
        if let Some(twin) = edges.remove(&key) {
            half_edges[edge] = Some(twin);
            half_edges[twin] = Some(edge);
        } else {
            edges.insert(key, edge);
        }
    }

    let mut illegal = 0;
    for a in 0..triangles.len() {
        let Some(b) = half_edges[a] else {
            continue;
        };
        if a > b {
            continue;
        }

        let a0 = a - a % 3;
        let b0 = b - b % 3;
        let ar = a0 + (a + 2) % 3;
        let al = a0 + (a + 1) % 3;
        let bl = b0 + (b + 2) % 3;

        let p0 = triangles[ar] as usize;
        let pr = triangles[a] as usize;
        let pl = triangles[al] as usize;
        let p1 = triangles[bl] as usize;
        let convex = orient(vertices[p0], vertices[pr], vertices[p1]) > 0.0
            && orient(vertices[p0], vertices[p1], vertices[pl]) > 0.0;

        if convex && !in_circle(vertices[p0], vertices[pr], vertices[pl], vertices[p1]) {
            illegal += 1;
        }
    }
    illegal
}

fn flatten(rings: &[&[[f64; 2]]]) -> (Vec<[f64; 2]>, Vec<u32>) {
    let vertices = rings.iter().flat_map(|ring| ring.iter().copied()).collect();
    let mut holes = Vec::with_capacity(rings.len().saturating_sub(1));
    let mut offset = 0;
    for ring in rings.iter().take(rings.len().saturating_sub(1)) {
        offset += ring.len() as u32;
        holes.push(offset);
    }
    (vertices, holes)
}

#[test]
fn improves_a_bad_quad_diagonal() {
    let vertices = [[0.0, 0.0], [3.0, 0.0], [10.0, 1.0], [0.0, 2.0]];
    let mut triangles = vec![2_u32, 3, 0, 2, 0, 1];
    let before = triangle_perimeter(&triangles, &vertices);

    refine(&mut triangles, &vertices);

    assert_eq!(triangles, [2, 3, 1, 3, 0, 1]);
    assert!(triangle_perimeter(&triangles, &vertices) < before * 0.7);
    assert_eq!(deviation(vertices, &[], &triangles), 0.0);
}

#[test]
fn improves_a_clockwise_quad_diagonal_keeping_the_winding() {
    let vertices = [[0.0, 0.0], [3.0, 0.0], [10.0, 1.0], [0.0, 2.0]];
    let mut triangles = vec![2_u32, 0, 3, 2, 1, 0];

    refine(&mut triangles, &vertices);

    assert_eq!(triangles, [1, 0, 3, 2, 1, 3]);
    assert_eq!(deviation(vertices, &[], &triangles), 0.0);
}

#[test]
fn leaves_a_good_quad_diagonal_alone_and_reuses_scratch() {
    let mut refiner = Refiner::new();
    let bad_vertices = [[0.0, 0.0], [3.0, 0.0], [10.0, 1.0], [0.0, 2.0]];
    let mut bad_triangles = vec![2_u16, 3, 0, 2, 0, 1];
    refiner.refine(&mut bad_triangles, &bad_vertices);

    let vertices = [[0.0, 0.0], [5.0, 0.0], [4.0, 1.0], [0.0, 4.0]];
    let mut triangles = vec![2_u16, 3, 0, 2, 0, 1];
    refiner.refine(&mut triangles, &vertices);

    assert_eq!(triangles, [2, 3, 0, 2, 0, 1]);
    assert_eq!(deviation(vertices, &[] as &[u16], &triangles), 0.0);
}

#[test]
fn preserves_and_improves_a_concave_polygon() {
    let vertices = [
        [0.0, 0.0],
        [4.0, 0.0],
        [4.0, 1.0],
        [1.0, 1.0],
        [1.0, 4.0],
        [0.0, 4.0],
    ];
    let mut triangles = Vec::new();
    Earcut::new().earcut(vertices, &[] as &[u32], &mut triangles);
    let length = triangles.len();
    let before = triangle_perimeter(&triangles, &vertices);

    refine(&mut triangles, &vertices);

    assert_eq!(triangles.len(), length);
    assert!(triangle_perimeter(&triangles, &vertices) < before * 0.9);
    assert_eq!(deviation(vertices, &[] as &[u32], &triangles), 0.0);
}

#[test]
fn terminates_on_near_cocircular_points() {
    let vertices = [
        [127.65906365022843, 9.336137742499535],
        [124.21725103117963, 30.888097161477972],
        [91.35514946628345, 89.65621376119454],
        [40.10446780041529, 121.5550560957686],
        [-110.83205604043928, 64.03323632184248],
        [-127.20394987965459, -14.253249980770189],
        [61.074962259031416, -112.48932831632469],
        [127.37846573978545, -12.598669206638515],
        [127.77010311801033, -7.668164657400608],
    ];
    let mut triangles = Vec::new();
    Earcut::new().earcut(vertices, &[] as &[u32], &mut triangles);
    let length = triangles.len();

    refine(&mut triangles, &vertices);

    assert_eq!(triangles.len(), length);
    assert!(deviation(vertices, &[] as &[u32], &triangles) < 1e-15);

    let vertices_f32 = vertices.map(|[x, y]| [x as f32, y as f32]);
    let mut triangles_f32 = Vec::new();
    Earcut::new().earcut(vertices_f32, &[] as &[u32], &mut triangles_f32);
    let length_f32 = triangles_f32.len();
    refine(&mut triangles_f32, &vertices_f32);
    assert_eq!(triangles_f32.len(), length_f32);
    assert!(deviation(vertices_f32, &[] as &[u32], &triangles_f32) < 1e-6);

    // Squaring distances at this scale overflows f32. Non-finite predicates
    // must leave the affected edges unchanged instead of re-flipping forever.
    let large_vertices_f32 = vertices.map(|[x, y]| [x as f32 * 1e8, y as f32 * 1e8]);
    let mut large_triangles_f32 = Vec::new();
    Earcut::new().earcut(large_vertices_f32, &[] as &[u32], &mut large_triangles_f32);
    let large_length_f32 = large_triangles_f32.len();
    refine(&mut large_triangles_f32, &large_vertices_f32);
    assert_eq!(large_triangles_f32.len(), large_length_f32);
    assert!(deviation(large_vertices_f32, &[] as &[u32], &large_triangles_f32).is_finite());
}

#[test]
fn legalizes_all_convex_interior_edges_in_earcut_fixture() {
    let (vertices, holes) = flatten(EARCUT);
    let mut triangles = Vec::new();
    Earcut::new().earcut(vertices.iter().copied(), &holes, &mut triangles);

    refine(&mut triangles, &vertices);

    assert_eq!(count_illegal_edges(&triangles, &vertices), 0);
    assert_eq!(deviation(vertices.iter().copied(), &holes, &triangles), 0.0);
}

#[test]
fn preserves_triangle_count_and_coverage_across_all_fixtures() {
    let mut refiner = Refiner::new();

    for &(name, rings) in FIXTURES {
        let (vertices, holes) = flatten(rings);
        let mut triangles = Vec::new();
        Earcut::new().earcut(vertices.iter().copied(), &holes, &mut triangles);
        let length = triangles.len();
        let before = deviation(vertices.iter().copied(), &holes, &triangles);

        refiner.refine(&mut triangles, &vertices);

        let after = deviation(vertices.iter().copied(), &holes, &triangles);
        assert_eq!(triangles.len(), length, "{name}");
        assert!(
            (after - before).abs() <= 1e-12,
            "{name}: deviation changed from {before} to {after}"
        );
    }
}

#[derive(Default)]
struct DeviationStats {
    nonzero: usize,
    first: Option<(usize, f64)>,
    worst: Option<(usize, f64)>,
    sum: f64,
}

impl DeviationStats {
    fn record(&mut self, polygon: usize, deviation: f64) {
        if deviation == 0.0 {
            return;
        }
        self.nonzero += 1;
        self.first.get_or_insert((polygon, deviation));
        if self.worst.is_none_or(|(_, worst)| deviation > worst) {
            self.worst = Some((polygon, deviation));
        }
        self.sum += deviation;
    }
}

#[test]
fn mvt_corpus_has_exact_triangulations_and_refined_quality() {
    let mut earcut = Earcut::new();
    let mut int_earcut = EarcutI32::new();
    let mut refiner = Refiner::new();
    let mut vertices_i32 = Vec::new();
    let mut triangles = Vec::new();
    let mut int_triangles = Vec::new();
    let mut base_deviation = DeviationStats::default();
    let mut refined_deviation = DeviationStats::default();
    let mut length_changed = 0;
    let mut base_perimeter = 0.0;
    let mut refined_perimeter = 0.0;
    let mut polygon_index = 0;

    let polygon_count = tiles_fixture::for_each_polygon(|vertices, holes| {
        vertices_i32.clear();
        vertices_i32.extend(vertices.iter().map(|&[x, y]| {
            assert!(x.fract() == 0.0 && y.fract() == 0.0);
            let point = [x as i32, y as i32];
            assert!(point.into_iter().all(|v| v.abs() <= 1 << 19));
            point
        }));
        int_earcut.earcut(vertices_i32.iter().copied(), holes, &mut int_triangles);
        assert_eq!(
            int_deviation(vertices_i32.iter().copied(), holes, &int_triangles),
            0,
            "integer triangulation deviation at polygon {polygon_index}"
        );

        earcut.earcut(vertices.iter().copied(), holes, &mut triangles);
        let triangle_indices = triangles.len();
        base_perimeter += triangle_perimeter(&triangles, vertices);
        base_deviation.record(
            polygon_index,
            deviation(vertices.iter().copied(), holes, &triangles),
        );

        refiner.refine(&mut triangles, vertices);
        refined_perimeter += triangle_perimeter(&triangles, vertices);
        length_changed += usize::from(triangles.len() != triangle_indices);
        refined_deviation.record(
            polygon_index,
            deviation(vertices.iter().copied(), holes, &triangles),
        );
        polygon_index += 1;
    });

    assert_eq!(polygon_count, 119_680);
    assert_eq!(polygon_index, polygon_count);
    assert_eq!(
        base_deviation.nonzero, 0,
        "base triangulation: first {:?}, worst {:?}, sum {}",
        base_deviation.first, base_deviation.worst, base_deviation.sum
    );
    assert_eq!(length_changed, 0);
    assert_eq!(
        refined_deviation.nonzero, 0,
        "refined triangulation: first {:?}, worst {:?}, sum {}",
        refined_deviation.first, refined_deviation.worst, refined_deviation.sum
    );
    assert!(
        refined_perimeter < base_perimeter * 0.72,
        "refined perimeter ratio {} is not below 0.72",
        refined_perimeter / base_perimeter
    );
}
