use earcut::int::EarcutI32;
use earcut::{Earcut, deviation};

/// A square plate perforated by an `n` x `n` lattice of diamond-shaped holes.
/// With `shift`, every other row is offset so holes bridge into the middle of
/// edges merged by earlier holes rather than straight into the outer ring.
fn perforated_plate(n: usize, shift: f64) -> (Vec<[f64; 2]>, Vec<u32>) {
    let pitch = 12.0;
    let size = pitch * (n as f64 + 1.0);
    let mut data = vec![[0.0, 0.0], [size, 0.0], [size, size], [0.0, size]];
    let mut hole_indices = Vec::with_capacity(n * n);
    for row in 0..n {
        let dx = if row % 2 == 0 { 0.0 } else { shift };
        for col in 0..n {
            let cx = pitch * (col as f64 + 0.75) + dx;
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

#[test]
fn perforated_plates_are_fully_covered() {
    // one instance throughout, so the hole index is also reused across sizes
    let mut earcut = Earcut::new();
    let mut earcut_int = EarcutI32::new();
    let mut triangles = Vec::new();
    let mut triangles_int = Vec::new();

    for n in [1, 3, 12, 40, 5] {
        for shift in [0.0, 6.0] {
            let (plate, hole_indices) = perforated_plate(n, shift);
            for quarter_turns in 0..4 {
                let data: Vec<[f64; 2]> = plate
                    .iter()
                    .map(|&[x, y]| match quarter_turns {
                        0 => [x, y],
                        1 => [y, -x],
                        2 => [-x, -y],
                        _ => [-y, x],
                    })
                    .collect();
                let label = format!("n={n} shift={shift} quarter_turns={quarter_turns}");

                earcut.earcut(data.iter().copied(), &hole_indices, &mut triangles);
                // at most (4 + 6 n^2) - 2: each hole adds 4 vertices and 2 bridge vertices,
                // fewer when a bridge runs collinear with a hole edge and gets filtered
                assert!(triangles.len() / 3 <= 2 + 6 * n * n, "{label}");
                assert_eq!(
                    deviation(data.iter().copied(), &hole_indices, &triangles),
                    0.0,
                    "{label}"
                );

                let data_int: Vec<[i32; 2]> =
                    data.iter().map(|&[x, y]| [x as i32, y as i32]).collect();
                earcut_int.earcut(data_int.iter().copied(), &hole_indices, &mut triangles_int);
                assert_eq!(triangles_int.len(), triangles.len(), "{label}");
                assert_eq!(
                    earcut::int::deviation(data_int.iter().copied(), &hole_indices, &triangles_int),
                    0,
                    "{label}"
                );
            }
        }
    }
}
