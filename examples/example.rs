use earcut::int::EarcutI32;
use earcut::{Earcut, deviation};

use std::fs;

fn load_fixture(name: &str, num_triangles: usize, expected_deviation: f64) {
    // load JSON
    type Coords = Vec<Vec<[f64; 2]>>;
    let s = fs::read_to_string("./tests/fixtures/".to_string() + name + ".json").unwrap();
    let expected = serde_json::from_str::<Coords>(&s).unwrap();

    // prepare input
    let num_holes = expected.len();
    let vertices = expected.clone().into_iter().flatten().collect::<Vec<_>>();
    let hole_indices: Vec<_> = expected
        .into_iter()
        .map(|x| x.len() as u32)
        .scan(0, |sum, e| {
            *sum += e;
            Some(*sum)
        })
        .take(num_holes - 1)
        .collect();

    // earcut
    let mut triangles = vec![];
    let mut earcut = Earcut::new();
    earcut.earcut(vertices.iter().copied(), &hole_indices, &mut triangles);

    // check
    assert!(triangles.len() == num_triangles * 3);
    if !triangles.is_empty() {
        assert!(
            deviation(vertices.iter().copied(), &hole_indices, &triangles) <= expected_deviation
        );
    }
}

fn main() {
    load_fixture("water", 2482, 0.0008);

    // Force monomorphization of the integer path for asm inspection.
    let mut e = EarcutI32::new();
    let mut tri: Vec<u32> = vec![];
    e.earcut(
        [[0i32, 0], [10, 0], [10, 10], [0, 10]].iter().copied(),
        &[][..],
        &mut tri,
    );
    std::hint::black_box(&tri);
}
