//! Compare float earcut, integer earcut, and float earcut + refine over a realistic MVT corpus.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use earcut::{Earcut, Refiner, int::EarcutI32};

#[path = "tiles_fixture.rs"]
mod tiles_fixture;

type Polygon<T> = (Vec<[T; 2]>, Vec<u32>);

fn load_polygons() -> (Vec<Polygon<f64>>, Vec<Polygon<i32>>) {
    let mut polygons_f64 = Vec::new();
    let mut polygons_i32 = Vec::new();
    tiles_fixture::for_each_polygon(|vertices, holes| {
        let vertices_i32 = vertices
            .iter()
            .map(|&[x, y]| {
                assert!(x.fract() == 0.0 && y.fract() == 0.0);
                let point = [x as i32, y as i32];
                assert!(point.into_iter().all(|v| v.abs() <= 1 << 19));
                point
            })
            .collect();
        polygons_f64.push((vertices.to_vec(), holes.to_vec()));
        polygons_i32.push((vertices_i32, holes.to_vec()));
    });
    (polygons_f64, polygons_i32)
}

fn bench_tiles(c: &mut Criterion) {
    let (polygons_f64, polygons_i32) = load_polygons();
    let total_vertices: u64 = polygons_f64.iter().map(|(v, _)| v.len() as u64).sum();

    let mut group = c.benchmark_group("tiles");
    group.throughput(Throughput::Elements(total_vertices));

    let mut earcut = Earcut::new();
    let mut triangles: Vec<u32> = Vec::new();
    group.bench_function("earcut", |b| {
        b.iter(|| {
            for (vertices, holes) in &polygons_f64 {
                earcut.earcut_slice(vertices, holes, &mut triangles);
                black_box(&triangles);
            }
        })
    });

    let mut earcut_int = EarcutI32::new();
    let mut triangles_int: Vec<u32> = Vec::new();
    group.bench_function("earcut-int", |b| {
        b.iter(|| {
            for (vertices, holes) in &polygons_i32 {
                earcut_int.earcut_slice(vertices, holes, &mut triangles_int);
                black_box(&triangles_int);
            }
        })
    });

    let mut refiner = Refiner::new();
    group.bench_function("earcut+refine", |b| {
        b.iter(|| {
            for (vertices, holes) in &polygons_f64 {
                earcut.earcut_slice(vertices, holes, &mut triangles);
                refiner.refine(&mut triangles, vertices);
                black_box(&triangles);
            }
        })
    });

    group.finish();
}

criterion_group!(benches, bench_tiles);
criterion_main!(benches);
