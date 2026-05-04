use earcut::int::EarcutI32;
use earcut::{Earcut, deviation};

#[path = "../tests/fixtures/mod.rs"]
mod fixtures;

fn run_fixture(rings: &[&[[f64; 2]]], num_triangles: usize, expected_deviation: f64) {
    let num_rings = rings.len();
    let vertices: Vec<[f64; 2]> = rings.iter().flat_map(|r| r.iter().copied()).collect();
    let hole_indices: Vec<u32> = rings
        .iter()
        .take(num_rings.saturating_sub(1))
        .scan(0u32, |s, r| {
            *s += r.len() as u32;
            Some(*s)
        })
        .collect();

    let mut triangles = vec![];
    let mut earcut = Earcut::new();
    earcut.earcut(vertices.iter().copied(), &hole_indices, &mut triangles);

    assert_eq!(triangles.len(), num_triangles * 3);
    if !triangles.is_empty() {
        assert!(
            deviation(vertices.iter().copied(), &hole_indices, &triangles) <= expected_deviation
        );
    }
}

fn main() {
    run_fixture(fixtures::WATER, 2482, 0.0008);

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
