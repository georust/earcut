use earcut::int::{EarcutI32, deviation as int_deviation};
use earcut::{Earcut, deviation};

#[test]
fn test_default() {
    let mut earcut = Earcut::<f64>::default();
    let data = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
    let hole_indices: &[u32] = &[];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles, vec![2, 3, 0, 2, 0, 1]);
}

#[test]
fn test_empty() {
    let mut earcut = Earcut::new();
    let data: [[f64; 2]; 0] = [];
    let hole_indices: &[u32] = &[];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 0);
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_invalid_point() {
    let mut earcut = Earcut::new();
    let data = [[100.0, 200.0]];
    let hole_indices: &[u32] = &[];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 0);
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_invalid_line() {
    let mut earcut = Earcut::new();
    let data = [[0.0, 0.0], [100.0, 200.0]];
    let hole_indices: &[u32] = &[];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 0);
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_invalid_empty_hole() {
    let mut earcut = Earcut::new();
    let data = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0]];
    let hole_indices: &[u32] = &[3];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 3);
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_steiner_point_hole() {
    let mut earcut = Earcut::new();
    let data = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [50.0, 30.0]];
    let hole_indices: &[u32] = &[3];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 3 * 3);
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_steiner_line_hole() {
    let mut earcut = Earcut::new();
    let data = [[0., 0.], [100., 0.], [100., 100.], [50., 30.], [60., 30.]];
    let hole_indices: &[u32] = &[3];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles, vec![2, 0, 1]);
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_square() {
    let mut earcut = Earcut::new();
    let data = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
    let hole_indices: &[u32] = &[];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles, vec![2, 3, 0, 2, 0, 1]);
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_square_u16() {
    let mut earcut = Earcut::new();
    let data = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
    let hole_indices: &[u16] = &[];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles, vec![2, 3, 0, 2, 0, 1]);
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_square_usize() {
    let mut earcut = Earcut::new();
    let data = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
    let hole_indices: &[usize] = &[];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles, vec![2, 3, 0, 2, 0, 1]);
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_map_3d_to_2d() {
    let mut earcut = Earcut::new();
    #[allow(clippy::useless_vec)]
    let data = vec![
        [0.0, 0.0, 1.0],
        [100.0, 0.0, 1.0],
        [100.0, 100.0, 1.0],
        [0.0, 100.0, 1.0],
    ];
    let hole_indices: &[usize] = &[];
    let mut triangles = vec![];
    earcut.earcut(
        data.iter().map(|v| [v[0], v[1]]),
        hole_indices,
        &mut triangles,
    );
    assert_eq!(triangles, vec![2, 3, 0, 2, 0, 1]);
    assert_eq!(
        deviation(data.iter().map(|v| [v[0], v[1]]), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_int_empty() {
    let mut earcut = EarcutI32::new();
    let data: [[i32; 2]; 0] = [];
    let hole_indices: &[u32] = &[];
    let mut triangles: Vec<u32> = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 0);
}

#[test]
fn test_int_invalid_point() {
    let mut earcut = EarcutI32::new();
    let data = [[100, 200]];
    let hole_indices: &[u32] = &[];
    let mut triangles: Vec<u32> = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 0);
}

#[test]
fn test_int_invalid_line() {
    let mut earcut = EarcutI32::new();
    let data = [[0, 0], [100, 200]];
    let hole_indices: &[u32] = &[];
    let mut triangles: Vec<u32> = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 0);
}

#[test]
fn test_int_empty_outer_ring() {
    // hole_indices[0] == 0 means the outer ring is empty: matches JS behavior
    let mut earcut = EarcutI32::new();
    let data = [[0, 0], [100, 0], [100, 100], [0, 100]];
    let hole_indices: &[u32] = &[0];
    let mut triangles: Vec<u32> = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 0);
    assert_eq!(
        int_deviation(data.iter().copied(), hole_indices, &triangles),
        0
    );
}

#[test]
fn test_int_degenerate_outer_ring() {
    // outer ring with 2 vertices: linked list builds a degenerate ring where
    // `prev_i == next_i`, exercising the degenerate-ring early return.
    let mut earcut = EarcutI32::new();
    let data = [[0, 0], [100, 0], [10, 10], [90, 10], [10, 90]];
    let hole_indices: &[u32] = &[2];
    let mut triangles: Vec<u32> = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 0);
    assert_eq!(
        int_deviation(data.iter().copied(), hole_indices, &triangles),
        0
    );
}

#[test]
fn test_int_square() {
    let mut earcut = EarcutI32::new();
    let data = [[0, 0], [100, 0], [100, 100], [0, 100]];
    let hole_indices: &[u32] = &[];
    let mut triangles: Vec<u32> = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles, vec![2, 3, 0, 2, 0, 1]);
}

// Largest supported absolute coordinate (`EarcutI32` domain limit: |coord| <= 2^19).
const LIM: i32 = 1 << 19;

#[test]
fn test_int_max_supported_range() {
    // A square spanning the full supported coordinate box.
    let data = [[-LIM, -LIM], [LIM, -LIM], [LIM, LIM], [-LIM, LIM]];
    let mut triangles = Vec::<u32>::new();

    EarcutI32::new().earcut(data, &[], &mut triangles);

    assert_eq!(triangles.len(), 6);
    assert_eq!(int_deviation(data, &[] as &[u32], &triangles), 0);
}

#[test]
fn test_int_max_supported_range_with_hole() {
    let data = [
        [-LIM, -LIM],
        [LIM, -LIM],
        [LIM, LIM],
        [-LIM, LIM],
        [-LIM / 2, -LIM / 2],
        [LIM / 2, -LIM / 2],
        [LIM / 2, LIM / 2],
        [-LIM / 2, LIM / 2],
    ];
    let mut triangles = Vec::<u32>::new();

    EarcutI32::new().earcut(data, &[4], &mut triangles);

    assert_eq!(triangles.len(), 8 * 3);
    assert_eq!(int_deviation(data, &[4], &triangles), 0);
}

#[test]
#[should_panic(expected = "out of range")]
fn test_int_range_over_limit_panics() {
    // Spanning the full i32 range exceeds the documented [-2^19, 2^19] domain.
    let data = [
        [i32::MIN, i32::MIN],
        [i32::MAX, i32::MIN],
        [i32::MAX, i32::MAX],
        [i32::MIN, i32::MAX],
    ];
    let mut triangles = Vec::<u32>::new();
    EarcutI32::new().earcut(data, &[], &mut triangles);
}

#[test]
fn test_int_degenerate_line_in_domain() {
    let data = [[-LIM, 0], [0, 0], [LIM, 0]];
    let mut triangles = Vec::<u32>::new();

    EarcutI32::new().earcut(data, &[], &mut triangles);

    assert!(triangles.is_empty());
}

#[test]
fn test_int_hole_bridge_preserves_rational_intersection() {
    let outer = [
        [0, 0],
        [4, 0],
        [4, 8],
        [6, 8],
        [6, 0],
        [10, 0],
        [10, 10],
        [0, 10],
    ];
    let hole = [[7, 3], [7, 6], [9, 6], [9, 3]];
    let transform = |[x, y]: [i32; 2]| [-3 * x + 5 * y, 4 * x - 7 * y];
    let data = outer
        .into_iter()
        .chain(hole)
        .map(transform)
        .collect::<Vec<_>>();
    let mut triangles = Vec::<u32>::new();

    EarcutI32::new().earcut(data.iter().copied(), &[8], &mut triangles);

    assert_eq!(triangles.len(), 12 * 3);
    assert_eq!(int_deviation(data, &[8], &triangles), 0);
}

#[test]
fn test_square_with_square_hole() {
    let mut earcut = Earcut::new();
    let data = [
        [0.0, 0.0],
        [100.0, 0.0],
        [100.0, 100.0],
        [0.0, 100.0],
        [10.0, 10.0],
        [90.0, 10.0],
        [90.0, 90.0],
        [10.0, 90.0],
    ];
    let hole_indices: &[u32] = &[4];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(
        triangles,
        vec![
            0, 4, 7, 5, 4, 0, 5, 0, 1, 5, 1, 2, 3, 0, 7, 3, 7, 6, 6, 5, 2, 6, 2, 3
        ]
    );
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_slice_matches_iterator() {
    let data = [
        [0.0, 0.0],
        [100.0, 0.0],
        [100.0, 100.0],
        [0.0, 100.0],
        [20.0, 20.0],
        [80.0, 20.0],
        [80.0, 80.0],
        [20.0, 80.0],
    ];
    let hole_indices: &[u32] = &[4];
    let mut earcut = Earcut::new();
    let (mut from_iter, mut from_slice) = (vec![], vec![]);
    earcut.earcut(data.iter().copied(), hole_indices, &mut from_iter);
    earcut.earcut_slice(&data, hole_indices, &mut from_slice);
    assert_eq!(from_slice, from_iter);

    let data_i: Vec<[i32; 2]> = data.iter().map(|&[x, y]| [x as i32, y as i32]).collect();
    let mut earcut_i = EarcutI32::new();
    earcut_i.earcut(data_i.iter().copied(), hole_indices, &mut from_iter);
    earcut_i.earcut_slice(&data_i, hole_indices, &mut from_slice);
    assert_eq!(from_slice, from_iter);
}

#[test]
fn test_collinear_polygon_has_zero_deviation() {
    // on the line y = 2x; the shoelace sum of these points is not exactly zero
    let data = [[0.1, 0.2], [1.3, 2.6], [2.5, 5.0], [3.7, 7.4]];
    let hole_indices: &[u32] = &[];
    let mut triangles = vec![];
    Earcut::new().earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles.len(), 0);
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}

#[test]
fn test_deviation_rejects_missing_output_f64() {
    let no_holes: &[u32] = &[];
    let empty: &[u32] = &[];
    for offset in [0.0, 1e8] {
        let square = [
            [offset, offset],
            [offset + 1.0, offset],
            [offset + 1.0, offset + 1.0],
            [offset, offset + 1.0],
        ];
        assert_eq!(deviation(square, no_holes, empty), 1.0);
        assert_eq!(deviation(square, no_holes, &[0, 0, 0]), 1.0);
        assert_eq!(deviation(square, no_holes, &[0, 1, 2]), 0.5);
        assert_eq!(deviation(square, no_holes, &[0, 1, 2, 0, 2, 3]), 0.0);

        let mut with_hole = square.to_vec();
        with_hole.extend([
            [offset + 0.25, offset + 0.25],
            [offset + 0.75, offset + 0.25],
            [offset + 0.75, offset + 0.75],
            [offset + 0.25, offset + 0.75],
        ]);
        assert_eq!(deviation(with_hole, &[4], empty), 1.0);
    }

    // A small positive area must not be treated as numerical degeneracy.
    let thin = [[0.0, 0.0], [1.0, 0.0], [1.0, 1e-20], [0.0, 1e-20]];
    assert_eq!(deviation(thin, no_holes, empty), 1.0);
}

#[test]
fn test_deviation_rejects_missing_output_f32() {
    let no_holes: &[u32] = &[];
    let empty: &[u32] = &[];
    for offset in [0.0_f32, 4000.0] {
        let square = [
            [offset, offset],
            [offset + 1.0, offset],
            [offset + 1.0, offset + 1.0],
            [offset, offset + 1.0],
        ];
        assert_eq!(deviation(square, no_holes, empty), 1.0);
        assert_eq!(deviation(square, no_holes, &[0, 0, 0]), 1.0);
        assert_eq!(deviation(square, no_holes, &[0, 1, 2]), 0.5);
        assert_eq!(deviation(square, no_holes, &[0, 1, 2, 0, 2, 3]), 0.0);

        let mut with_hole = square.to_vec();
        with_hole.extend([
            [offset + 0.25, offset + 0.25],
            [offset + 0.75, offset + 0.25],
            [offset + 0.75, offset + 0.75],
            [offset + 0.25, offset + 0.75],
        ]);
        assert_eq!(deviation(with_hole, &[4], empty), 1.0);
    }

    let thin = [[0.0_f32, 0.0], [1.0, 0.0], [1.0, 1e-8], [0.0, 1e-8]];
    assert_eq!(deviation(thin, no_holes, empty), 1.0);
}

#[test]
fn test_large_convex_polygon() {
    // a strictly convex ring skips the point checks of ear slicing; with them, each
    // hashed check of its fan of long triangles scans a large part of the ring
    let n = 100_000;
    let data: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let t = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            [t.cos(), t.sin()]
        })
        .collect();
    let mut triangles: Vec<u32> = vec![];
    Earcut::new().earcut(data.iter().copied(), &[] as &[u32], &mut triangles);
    assert_eq!(triangles.len() / 3, n - 2);
    assert!(deviation(data.iter().copied(), &[] as &[u32], &triangles) < 1e-12);

    let data_i: Vec<[i32; 2]> = (0..n)
        .map(|i| {
            let t = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            [(t.cos() * 400_000.0) as i32, (t.sin() * 400_000.0) as i32]
        })
        .collect();
    EarcutI32::new().earcut(data_i.iter().copied(), &[] as &[u32], &mut triangles);
    assert_eq!(
        int_deviation(data_i.iter().copied(), &[] as &[u32], &triangles),
        0
    );
}

#[test]
fn test_nested_splits_do_not_overflow_a_small_stack() {
    // a rectangle with k small triangular holes, each one step right and down from the
    // previous; every hole adds a level of nested splits
    let k = 600;
    let mut data = vec![
        [-1.0, -(k as f64)],
        [2.0 * k as f64 + 1.0, -(k as f64)],
        [2.0 * k as f64 + 1.0, 7.0],
        [-1.0, 7.0],
    ];
    let mut hole_indices: Vec<u32> = vec![];
    for i in 0..k {
        let i = i as f64;
        hole_indices.push(data.len() as u32);
        data.extend([
            [1.0 + 2.0 * i, -i],
            [2.0 * i, 6.0 - i],
            [2.0 + 2.0 * i, 6.0 - i],
        ]);
    }
    let data_i: Vec<[i32; 2]> = data.iter().map(|&[x, y]| [x as i32, y as i32]).collect();
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            let mut triangles: Vec<u32> = vec![];
            Earcut::new().earcut(data.iter().copied(), &hole_indices, &mut triangles);
            assert_eq!(triangles.len() / 3, 3002);
            assert_eq!(
                deviation(data.iter().copied(), &hole_indices, &triangles),
                0.0
            );
            EarcutI32::new().earcut(data_i.iter().copied(), &hole_indices, &mut triangles);
            assert_eq!(triangles.len() / 3, 3002);
            assert_eq!(
                int_deviation(data_i.iter().copied(), &hole_indices, &triangles),
                0
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
