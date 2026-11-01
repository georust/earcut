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
    ("water-huge3", WATER_HUGE3),
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
    ("water-huge3", WATER_HUGE3),
];

/// A square plate perforated by an `n` x `n` lattice of diamond-shaped holes,
/// with every other row shifted by half a pitch. Stresses hole bridging.
fn perforated_plate(n: usize) -> (Vec<[f64; 2]>, Vec<u32>) {
    let pitch = 12.0;
    let size = pitch * (n as f64 + 1.0);
    let mut data = vec![[0.0, 0.0], [size, 0.0], [size, size], [0.0, size]];
    let mut hole_indices = Vec::with_capacity(n * n);
    for row in 0..n {
        let shift = if row % 2 == 0 { 0.0 } else { pitch / 2.0 };
        for col in 0..n {
            let cx = pitch * (col as f64 + 0.75) + shift;
            let cy = pitch * (row as f64 + 1.0);
            hole_indices.push(data.len() as u32);
            data.extend([
                [cx - 3.0, cy],
                [cx, cy + 3.0],
                [cx + 3.0, cy],
                [cx, cy - 3.0],
            ]);
        }
    }
    (data, hole_indices)
}

fn bench(c: &mut Criterion) {
    let mut earcut = Earcut::new();
    let mut triangles: Vec<u32> = Vec::new();
    for (name, rings) in F64_FIXTURES {
        let (data, hole_indices) = flatten(rings);
        c.bench_function(name, |b| {
            b.iter(|| {
                earcut.earcut_slice(&data, &hole_indices, &mut triangles);
            })
        });
    }

    for n in [8, 16, 32] {
        let (data, hole_indices) = perforated_plate(n);
        c.bench_function(&format!("perforated/{}", n * n), |b| {
            b.iter(|| {
                earcut.earcut_slice(&data, &hole_indices, &mut triangles);
            })
        });
    }

    let mut earcut_int = EarcutI32::new();
    let mut triangles_i: Vec<u32> = Vec::new();
    for (name, rings) in INT_FIXTURES {
        let (data, hole_indices) = flatten_i32(rings).expect("fixture is integer-valued");
        c.bench_function(&format!("int/{name}"), |b| {
            b.iter(|| {
                earcut_int.earcut_slice(&data, &hole_indices, &mut triangles_i);
            })
        });
    }

    for n in [8, 16, 32] {
        let (data, hole_indices) = perforated_plate(n);
        let data: Vec<[i32; 2]> = data.iter().map(|&[x, y]| [x as i32, y as i32]).collect();
        c.bench_function(&format!("int/perforated/{}", n * n), |b| {
            b.iter(|| {
                earcut_int.earcut_slice(&data, &hole_indices, &mut triangles_i);
            })
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
