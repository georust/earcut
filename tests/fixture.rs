use earcut::int::{EarcutI32, deviation as int_deviation};
use earcut::{Earcut, deviation as float_deviation};

mod fixtures;
use fixtures::*;

struct Fixture {
    data_f: Vec<[f64; 2]>,
    hole_indices: Vec<u32>,
}

fn build_fixture(rings: &[&[[f64; 2]]]) -> Fixture {
    let data_f: Vec<[f64; 2]> = rings.iter().flat_map(|r| r.iter().copied()).collect();
    let mut hole_indices = Vec::with_capacity(rings.len().saturating_sub(1));
    let mut sum = 0u32;
    for ring in rings.iter().take(rings.len().saturating_sub(1)) {
        sum += ring.len() as u32;
        hole_indices.push(sum);
    }
    Fixture {
        data_f,
        hole_indices,
    }
}

fn as_i32_points(data: &[[f64; 2]]) -> Option<Vec<[i32; 2]>> {
    let mut points = Vec::with_capacity(data.len());
    for &[x, y] in data {
        if x.fract() != 0.0 || y.fract() != 0.0 {
            return None;
        }
        if !(i32::MIN as f64..=i32::MAX as f64).contains(&x)
            || !(i32::MIN as f64..=i32::MAX as f64).contains(&y)
        {
            return None;
        }
        points.push([x as i32, y as i32]);
    }
    Some(points)
}

fn test_fixture(rings: &[&[[f64; 2]]], num_triangles: usize, expected_deviation: f64) {
    let fixture = build_fixture(rings);

    let mut triangles = vec![];
    let mut earcut = Earcut::new();
    earcut.earcut(
        fixture.data_f.iter().copied(),
        &fixture.hole_indices,
        &mut triangles,
    );

    assert_eq!(triangles.len(), num_triangles * 3);
    let f_deviation = if triangles.is_empty() {
        0.0
    } else {
        float_deviation(
            fixture.data_f.iter().copied(),
            &fixture.hole_indices,
            &triangles,
        )
    };
    assert!(f_deviation <= expected_deviation);

    check_int_fixture_if_applicable(&fixture, triangles.len(), f_deviation);
}

fn check_int_fixture_if_applicable(fixture: &Fixture, f_triangle_indices: usize, f_deviation: f64) {
    let Some(data_i32) = as_i32_points(&fixture.data_f) else {
        return;
    };

    let mut i32_triangles = vec![];
    EarcutI32::new().earcut(
        data_i32.iter().copied(),
        &fixture.hole_indices,
        &mut i32_triangles,
    );

    assert_eq!(
        i32_triangles.len(),
        f_triangle_indices,
        "int index count differs from f64 reference"
    );

    let i_abs_dev = int_deviation(
        data_i32.iter().copied(),
        &fixture.hole_indices,
        &i32_triangles,
    );
    let polygon_area = polygon_area2(&data_i32, &fixture.hole_indices);
    assert!(
        polygon_area >= 0,
        "holes exceeded outer ring area: {polygon_area}"
    );
    if polygon_area == 0 {
        assert_eq!(i_abs_dev, 0, "int deviation was non-zero for zero area");
    } else {
        let i_dev = i_abs_dev as f64 / polygon_area as f64;
        assert!(
            i_dev <= f_deviation + 1e-12,
            "int deviation {i_dev} exceeded f64 deviation {f_deviation}"
        );
    }
}

/// Returns twice the signed area of a polygon ring (shoelace, doubled).
fn signed_area(data: &[[i32; 2]], start: usize, end: usize) -> i64 {
    let mut area = 0i64;
    let mut j = end - 1;
    for i in start..end {
        area += ((data[j][0] as i64) - (data[i][0] as i64))
            * ((data[j][1] as i64) + (data[i][1] as i64));
        j = i;
    }
    area
}

/// Returns twice the polygon area (outer minus holes); matches the scaling of
/// the value returned by `int::deviation`.
fn polygon_area2(data: &[[i32; 2]], hole_indices: &[u32]) -> i64 {
    if data.len() < 3 {
        return 0;
    }
    let outer_len = hole_indices.first().copied().unwrap_or(data.len() as u32) as usize;
    let mut area = signed_area(data, 0, outer_len).abs();
    for (i, &start) in hole_indices.iter().enumerate() {
        let start = start as usize;
        let end = if i + 1 < hole_indices.len() {
            hole_indices[i + 1] as usize
        } else {
            data.len()
        };
        if end - start >= 3 {
            area -= signed_area(data, start, end).abs();
        }
    }
    area
}

#[test]
fn fixture_building() {
    test_fixture(BUILDING, 13, 0.0);
}

#[test]
fn fixture_dude() {
    test_fixture(DUDE, 106, 2e-15);
}

#[test]
fn fixture_water1() {
    test_fixture(WATER, 2482, 0.0008);
}

#[test]
fn fixture_water2() {
    test_fixture(WATER2, 1212, 0.0);
}

#[test]
fn fixture_water3() {
    test_fixture(WATER3, 197, 0.0);
}

#[test]
fn fixture_water3b() {
    test_fixture(WATER3B, 25, 0.0);
}

#[test]
fn fixture_water4() {
    test_fixture(WATER4, 705, 0.0);
}

#[test]
fn fixture_water_huge1() {
    test_fixture(WATER_HUGE, 5176, 0.0011);
}

#[test]
fn fixture_water_huge2() {
    test_fixture(WATER_HUGE2, 4462, 0.004);
}

#[test]
fn fixture_degenerate() {
    test_fixture(DEGENERATE, 0, 0.0);
}

#[test]
fn fixture_bad_hole() {
    test_fixture(BAD_HOLE, 42, 0.019);
}

#[test]
fn fixture_empty_square() {
    test_fixture(EMPTY_SQUARE, 0, 0.0);
}

#[test]
fn fixture_issue16() {
    test_fixture(ISSUE16, 12, 4e-16);
}

#[test]
fn fixture_issue17() {
    test_fixture(ISSUE17, 11, 2e-16);
}

#[test]
fn fixture_steiner() {
    test_fixture(STEINER, 9, 0.0);
}

#[test]
fn fixture_issue29() {
    test_fixture(ISSUE29, 40, 2e-15);
}

#[test]
fn fixture_issue34() {
    test_fixture(ISSUE34, 139, 0.0);
}

#[test]
fn fixture_issue35() {
    test_fixture(ISSUE35, 844, 0.0);
}

#[test]
fn fixture_self_touching() {
    test_fixture(SELF_TOUCHING, 124, 2e-13);
}

#[test]
fn fixture_outside_ring() {
    test_fixture(OUTSIDE_RING, 64, 0.0);
}

#[test]
fn fixture_simplified_us_border() {
    test_fixture(SIMPLIFIED_US_BORDER, 120, 0.0);
}

#[test]
fn fixture_touching_holes() {
    test_fixture(TOUCHING_HOLES, 57, 0.0);
}

#[test]
fn fixture_touching_holes2() {
    test_fixture(TOUCHING_HOLES2, 10, 0.0);
}

#[test]
fn fixture_touching_holes3() {
    test_fixture(TOUCHING_HOLES3, 82, 0.0);
}

#[test]
fn fixture_touching_holes4() {
    test_fixture(TOUCHING_HOLES4, 55, 0.0);
}

#[test]
fn fixture_touching_holes5() {
    test_fixture(TOUCHING_HOLES5, 133, 0.0);
}

#[test]
fn fixture_touching_holes6() {
    test_fixture(TOUCHING_HOLES6, 3098, 0.0);
}

#[test]
fn fixture_hole_touching_outer() {
    test_fixture(HOLE_TOUCHING_OUTER, 77, 0.0);
}

#[test]
fn fixture_hilbert() {
    test_fixture(HILBERT, 1024, 0.0);
}

#[test]
fn fixture_issue45() {
    test_fixture(ISSUE45, 10, 0.0);
}

#[test]
fn fixture_eberly_3() {
    test_fixture(EBERLY_3, 73, 0.0);
}

#[test]
fn fixture_eberly_6() {
    test_fixture(EBERLY_6, 1429, 2e-14);
}

#[test]
fn fixture_issue52() {
    test_fixture(ISSUE52, 109, 0.0);
}

#[test]
fn fixture_shared_points() {
    test_fixture(SHARED_POINTS, 4, 0.0);
}

#[test]
fn fixture_bad_diagonals() {
    test_fixture(BAD_DIAGONALS, 7, 0.0);
}

#[test]
fn fixture_issue83() {
    test_fixture(ISSUE83, 0, 0.0);
}

#[test]
fn fixture_issue107() {
    test_fixture(ISSUE107, 0, 0.0);
}

#[test]
fn fixture_issue111() {
    test_fixture(ISSUE111, 18, 0.0);
}

#[test]
fn fixture_collinear_boxy() {
    test_fixture(BOXY, 58, 0.0);
}

#[test]
fn fixture_collinear_diagonal() {
    test_fixture(COLLINEAR_DIAGONAL, 14, 0.0);
}

#[test]
fn fixture_issue119() {
    test_fixture(ISSUE119, 18, 0.0);
}

#[test]
fn fixture_hourglass() {
    test_fixture(HOURGLASS, 2, 0.0);
}

#[test]
fn fixture_touching2() {
    test_fixture(TOUCHING2, 8, 0.0);
}

#[test]
fn fixture_touching3() {
    test_fixture(TOUCHING3, 15, 0.0);
}

#[test]
fn fixture_touching4() {
    test_fixture(TOUCHING4, 19, 0.0);
}

#[test]
fn fixture_rain() {
    test_fixture(RAIN, 2681, 0.0);
}

#[test]
fn fixture_issue131() {
    test_fixture(ISSUE131, 12, 0.0);
}

#[test]
fn fixture_infinite_loop_jhl() {
    test_fixture(INFINITE_LOOP_JHL, 0, 0.0);
}

#[test]
fn fixture_filtered_bridge_jhl() {
    test_fixture(FILTERED_BRIDGE_JHL, 25, 0.0);
}

#[test]
fn fixture_issue149() {
    test_fixture(ISSUE149, 2, 0.0);
}

#[test]
fn fixture_issue142() {
    test_fixture(ISSUE142, 4, 0.13);
}

#[test]
fn fixture_issue186() {
    test_fixture(ISSUE186, 41, 0.0);
}
