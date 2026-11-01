use earcut::{Earcut, deviation, int::EarcutI32};

fn closed_hole_grid(side: usize) -> (Vec<[f64; 2]>, Vec<u32>) {
    let size = ((side + 1) * 8) as f64;
    let mut vertices = vec![
        [0.0, 0.0],
        [size, 0.0],
        [size, size],
        [0.0, size],
        [0.0, 0.0],
    ];
    let mut holes = Vec::new();
    for row in 0..side {
        for column in 0..side {
            let x = (column * 8 + 2) as f64;
            let y = (row * 8 + 2) as f64;
            holes.push(vertices.len() as u32);
            vertices.extend([
                [x, y],
                [x, y + 2.0],
                [x + 2.0, y + 2.0],
                [x + 2.0, y],
                [x, y],
            ]);
        }
    }
    (vertices, holes)
}

#[test]
fn bridge_scan_reuses_buffers_across_small_large_and_hole_free_inputs() {
    let mut earcut = Earcut::new();
    let mut earcut_int = EarcutI32::new();
    let mut triangles = Vec::<u32>::new();
    let mut triangles_int = Vec::<u32>::new();
    for side in [2, 8, 0, 1, 12, 2, 3, 1] {
        let (vertices, holes) = closed_hole_grid(side);
        for rotation in 0..4 {
            // Repeated closing vertices exercise removal of the initial ring node.
            let data: Vec<_> = vertices
                .iter()
                .map(|&[x, y]| match rotation {
                    0 => [x, y],
                    1 => [y, -x],
                    2 => [-x, -y],
                    _ => [-y, x],
                })
                .collect();
            let data_int: Vec<_> = data.iter().map(|&[x, y]| [x as i32, y as i32]).collect();
            earcut.earcut_slice(&data, &holes, &mut triangles);
            earcut_int.earcut_slice(&data_int, &holes, &mut triangles_int);
            let mut fresh = Vec::<u32>::new();
            Earcut::new().earcut_slice(&data, &holes, &mut fresh);
            assert_eq!(triangles, fresh, "side={side}, rotation={rotation}");
            EarcutI32::new().earcut_slice(&data_int, &holes, &mut fresh);
            assert_eq!(triangles_int, fresh, "side={side}, rotation={rotation}");
            assert_eq!(deviation(data, &holes, &triangles), 0.0);
            assert_eq!(earcut::int::deviation(data_int, &holes, &triangles_int), 0);
        }
    }
}
