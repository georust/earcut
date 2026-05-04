use earcut::Earcut;
use std::env;

#[path = "../tests/fixtures/mod.rs"]
mod fixtures;
use fixtures::FIXTURES;

fn lookup(name: &str) -> &'static [&'static [[f64; 2]]] {
    FIXTURES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("unknown fixture: {name}"))
}

fn flatten(rings: &[&[[f64; 2]]]) -> (Vec<[f64; 2]>, Vec<u32>) {
    let data: Vec<[f64; 2]> = rings.iter().flat_map(|r| r.iter().copied()).collect();
    let hole_indices: Vec<u32> = rings
        .iter()
        .take(rings.len().saturating_sub(1))
        .scan(0u32, |s, r| {
            *s += r.len() as u32;
            Some(*s)
        })
        .collect();
    (data, hole_indices)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let fixture = args.get(1).map(String::as_str).unwrap_or("water-huge2");
    let iterations: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);

    let (vertices, hole_indices) = flatten(lookup(fixture));
    let mut earcut = Earcut::new();
    let mut triangles: Vec<u32> = Vec::new();

    eprintln!(
        "fixture={fixture} vertices={} holes={} iterations={iterations}",
        vertices.len(),
        hole_indices.len()
    );

    for _ in 0..iterations {
        earcut.earcut(vertices.iter().copied(), &hole_indices, &mut triangles);
        std::hint::black_box(&triangles);
    }

    eprintln!("triangles={}", triangles.len());
}
