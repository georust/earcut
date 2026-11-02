use earcut::{Earcut, int::EarcutI32};
use std::{
    fs::File,
    hint::black_box,
    io::{BufWriter, Write},
    time::{Duration, Instant},
};

#[path = "../../tests/fixtures/mod.rs"]
mod fixtures;
#[path = "../tiles_fixture.rs"]
mod tiles_fixture;

type Polygon = (Vec<[f64; 2]>, Vec<u32>);
struct Case {
    name: String,
    polygons: Vec<Polygon>,
}

fn flatten(rings: &[&[[f64; 2]]]) -> Polygon {
    let mut vertices = Vec::new();
    let mut holes = Vec::new();
    for (i, ring) in rings.iter().enumerate() {
        if i != 0 {
            holes.push(vertices.len() as u32);
        }
        vertices.extend_from_slice(ring);
    }
    (vertices, holes)
}

fn plate(side: usize) -> Polygon {
    let size = ((side + 1) * 12) as f64;
    let mut vertices = vec![[0., 0.], [size, 0.], [size, size], [0., size]];
    let mut holes = Vec::new();
    for row in 0..side {
        for col in 0..side {
            let x = (col * 12 + 9 + if row % 2 == 1 { 6 } else { 0 }) as f64;
            let y = ((row + 1) * 12) as f64;
            holes.push(vertices.len() as u32);
            vertices.extend([[x - 3., y], [x, y + 3.], [x + 3., y], [x, y - 3.]]);
        }
    }
    (vertices, holes)
}

fn cases(filters: &[String]) -> Vec<Case> {
    use fixtures::*;
    let mut cases: Vec<_> = [
        ("bad_hole", BAD_HOLE),
        ("building", BUILDING),
        ("degenerate", DEGENERATE),
        ("dude", DUDE),
        ("empty_square", EMPTY_SQUARE),
        ("water", WATER),
        ("water2", WATER2),
        ("water3", WATER3),
        ("water3b", WATER3B),
        ("water4", WATER4),
        ("water_huge", WATER_HUGE),
        ("water_huge2", WATER_HUGE2),
        ("water_huge3", WATER_HUGE3),
    ]
    .into_iter()
    .filter(|(name, _)| filters.is_empty() || filters.iter().any(|f| f == name))
    .map(|(name, rings)| Case {
        name: name.into(),
        polygons: vec![flatten(rings)],
    })
    .collect();
    for n in [8, 16, 32] {
        let name = format!("holes_{}", n * n);
        if !filters.is_empty() && !filters.contains(&name) {
            continue;
        }
        cases.push(Case {
            name,
            polygons: vec![plate(n)],
        });
    }
    if filters.is_empty() || filters.iter().any(|name| name == "MVT_corpus") {
        let mut polygons = Vec::new();
        tiles_fixture::for_each_polygon(|v, h| polygons.push((v.to_vec(), h.to_vec())));
        assert_eq!(polygons.len(), 119_680);
        cases.push(Case {
            name: "MVT_corpus".into(),
            polygons,
        });
    }
    cases
}

fn export(cases: &[Case], path: &str) {
    let mut file = BufWriter::new(File::create(path).unwrap());
    file.write_all(b"ECBENCH1").unwrap();
    fn u32(w: &mut impl Write, x: usize) {
        w.write_all(&u32::try_from(x).unwrap().to_le_bytes())
            .unwrap();
    }
    u32(&mut file, cases.len());
    for case in cases {
        u32(&mut file, case.name.len());
        file.write_all(case.name.as_bytes()).unwrap();
        u32(&mut file, case.polygons.len());
        for (vertices, holes) in &case.polygons {
            u32(&mut file, vertices.len());
            u32(&mut file, holes.len());
            for p in vertices {
                for v in p {
                    file.write_all(&v.to_le_bytes()).unwrap();
                }
            }
            for h in holes {
                file.write_all(&h.to_le_bytes()).unwrap();
            }
        }
    }
    file.flush().unwrap();
}

fn sample(run: &mut impl FnMut(), duration: Duration) -> f64 {
    let probe = Instant::now();
    run();
    let batch = if probe.elapsed() >= Duration::from_millis(1) {
        1
    } else {
        64
    };
    let start = Instant::now();
    let mut calls = 0;
    loop {
        for _ in 0..batch {
            run();
        }
        calls += batch;
        if start.elapsed() >= duration {
            break;
        }
    }
    start.elapsed().as_secs_f64() * 1e9 / calls as f64
}

fn configure_thread() {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn pthread_set_qos_class_self_np(class: u32, priority: i32) -> i32;
        }
        // QOS_CLASS_USER_INITIATED from the macOS SDK's sys/qos.h.
        assert_eq!(unsafe { pthread_set_qos_class_self_np(0x19, 0) }, 0);
    }
}

fn measure<T>(
    name: &str,
    mode: &str,
    rounds: usize,
    duration: Duration,
    mut create: impl FnMut() -> T,
    mut run: impl FnMut(&mut T),
) {
    // Each sample gets an independently allocated, warmed workspace. Retaining
    // previous ones prevents the allocator handing every sample the same arena.
    // Construction and warming remain outside the timed region.
    let mut retained = Vec::with_capacity(rounds + 1);
    let mut warm = create();
    sample(&mut || run(&mut warm), Duration::from_millis(500));
    retained.push(warm);
    for round in 0..rounds {
        let mut workspace = create();
        for _ in 0..3 {
            run(&mut workspace);
        }
        let ns = sample(&mut || run(&mut workspace), duration);
        println!("{name}\t{mode}\t{round}\t{ns:.6}");
        retained.push(workspace);
    }
    black_box(&retained);
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|s| s == "--export") {
        export(&cases(&[]), &args[2]);
        return;
    }
    configure_thread();
    let rounds: usize = args.get(1).unwrap().parse().unwrap();
    assert!(rounds > 0);
    let millis: u64 = args.get(2).unwrap().parse().unwrap();
    let duration = Duration::from_millis(millis);
    let filters = &args[3..];
    let mode = std::env::var("EARCUT_BENCH_MODE").unwrap_or_else(|_| "all".into());
    assert!(matches!(mode.as_str(), "all" | "f64" | "i32"));
    for case in &cases(filters) {
        let int_data: Option<Vec<_>> = if mode == "f64" {
            None
        } else {
            case.polygons
                .iter()
                .map(|(v, h)| {
                    if v.iter()
                        .flatten()
                        .any(|v| v.fract() != 0. || v.abs() > (1 << 19) as f64)
                    {
                        return None;
                    }
                    Some((
                        v.iter()
                            .map(|&[x, y]| [x as i32, y as i32])
                            .collect::<Vec<_>>(),
                        h.clone(),
                    ))
                })
                .collect()
        };
        if mode == "all" || mode == "f64" {
            measure(
                &case.name,
                "f64",
                rounds,
                duration,
                || (Earcut::new(), Vec::<u32>::new()),
                |(earcut, triangles)| {
                    for (v, h) in &case.polygons {
                        earcut.earcut_slice(black_box(v), h, triangles);
                        black_box(triangles.as_slice());
                    }
                },
            );
        }
        if (mode == "all" || mode == "i32") && int_data.is_some() {
            measure(
                &case.name,
                "i32",
                rounds,
                duration,
                || (EarcutI32::new(), Vec::<u32>::new()),
                |(earcut, triangles)| {
                    for (v, h) in int_data.as_ref().unwrap() {
                        earcut.earcut_slice(black_box(v), h, triangles);
                        black_box(triangles.as_slice());
                    }
                },
            );
        }
    }
}
