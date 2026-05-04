use criterion::{Criterion, criterion_group, criterion_main};

use earcut::Earcut;
use earcut::int::EarcutI32;

#[path = "../tests/fixtures/mod.rs"]
mod fixtures;
use fixtures::*;

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

fn flatten_i32(rings: &[&[[f64; 2]]]) -> Option<(Vec<[i32; 2]>, Vec<u32>)> {
    let (data, holes) = flatten(rings);
    let mut out = Vec::with_capacity(data.len());
    for [x, y] in data {
        if x.fract() != 0.0 || y.fract() != 0.0 {
            return None;
        }
        if !(i32::MIN as f64..=i32::MAX as f64).contains(&x)
            || !(i32::MIN as f64..=i32::MAX as f64).contains(&y)
        {
            return None;
        }
        out.push([x as i32, y as i32]);
    }
    Some((out, holes))
}

const F64_FIXTURES: &[(&str, &[&[[f64; 2]]])] = &[
    ("bad-hole", BAD_HOLE),
    ("building", BUILDING),
    ("degenerate", DEGENERATE),
    ("dude", DUDE),
    ("empty-square", EMPTY_SQUARE),
    ("water", WATER),
    ("water2", WATER2),
    ("water3", WATER3),
    ("water3b", WATER3B),
    ("water4", WATER4),
    ("water-huge", WATER_HUGE),
    ("water-huge2", WATER_HUGE2),
];

/// Subset of `F64_FIXTURES` that are integer-representable (every bench we
/// have today except `dude`).
const INT_FIXTURES: &[(&str, &[&[[f64; 2]]])] = &[
    ("bad-hole", BAD_HOLE),
    ("building", BUILDING),
    ("degenerate", DEGENERATE),
    ("empty-square", EMPTY_SQUARE),
    ("water", WATER),
    ("water2", WATER2),
    ("water3", WATER3),
    ("water3b", WATER3B),
    ("water4", WATER4),
    ("water-huge", WATER_HUGE),
    ("water-huge2", WATER_HUGE2),
];

fn bench(c: &mut Criterion) {
    let mut earcut = Earcut::new();
    let mut triangles: Vec<u32> = Vec::new();
    for (name, rings) in F64_FIXTURES {
        let (data, hole_indices) = flatten(rings);
        c.bench_function(name, |b| {
            b.iter(|| {
                earcut.earcut(data.iter().copied(), &hole_indices, &mut triangles);
            })
        });
    }

    let mut earcut_int = EarcutI32::new();
    let mut triangles_i: Vec<u32> = Vec::new();
    for (name, rings) in INT_FIXTURES {
        let (data, hole_indices) = flatten_i32(rings).expect("fixture is integer-valued");
        c.bench_function(&format!("int/{name}"), |b| {
            b.iter(|| {
                earcut_int.earcut(data.iter().copied(), &hole_indices, &mut triangles_i);
            })
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
