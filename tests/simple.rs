use earcut::{Earcut, deviation};

#[test]
fn test_default() {
    let mut earcut = Earcut::<f64>::default();
    let data = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
    let hole_indices: &[u32] = &[];
    let mut triangles = vec![];
    earcut.earcut(data.iter().copied(), hole_indices, &mut triangles);
    assert_eq!(triangles, vec![2, 3, 0, 0, 1, 2]);
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
    assert_eq!(triangles.len(), 5 * 3);
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
    assert_eq!(triangles, vec![2, 3, 0, 0, 1, 2]);
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
    assert_eq!(triangles, vec![2, 3, 0, 0, 1, 2]);
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
    assert_eq!(triangles, vec![2, 3, 0, 0, 1, 2]);
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
    assert_eq!(triangles, vec![2, 3, 0, 0, 1, 2]);
    assert_eq!(
        deviation(data.iter().map(|v| [v[0], v[1]]), hole_indices, &triangles),
        0.0
    );
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
            0, 4, 7, 5, 4, 0, 3, 0, 7, 5, 0, 1, 2, 3, 7, 6, 5, 1, 2, 7, 6, 6, 1, 2
        ]
    );
    assert_eq!(
        deviation(data.iter().copied(), hole_indices, &triangles),
        0.0
    );
}
