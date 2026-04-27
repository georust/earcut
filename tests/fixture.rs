use earcut::{Earcut, deviation};

fn test_fixture(coords: &str, num_triangles: usize, expected_deviation: f64) {
    // load JSON
    type Coords = Vec<Vec<[f64; 2]>>;
    let expected = serde_json::from_str::<Coords>(coords).unwrap();

    // prepare input
    let num_holes = expected.len();
    let data: Vec<[f64; 2]> = expected.clone().into_iter().flatten().collect();
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
    earcut.earcut(data.iter().copied(), &hole_indices, &mut triangles);

    // check
    assert_eq!(
        triangles.len(),
        num_triangles * 3,
        "{} {}",
        triangles.len(),
        num_triangles * 3
    );
    if !triangles.is_empty() {
        assert!(deviation(data.iter().copied(), &hole_indices, &triangles) <= expected_deviation);
    }
}

#[test]
fn fixture_building() {
    test_fixture(include_str!("fixtures/building.json"), 13, 0.0);
}

#[test]
fn fixture_dude() {
    test_fixture(include_str!("fixtures/dude.json"), 106, 2e-15);
}

#[test]
fn fixture_water1() {
    test_fixture(include_str!("fixtures/water.json"), 2482, 0.0008);
}

#[test]
fn fixture_water2() {
    test_fixture(include_str!("fixtures/water2.json"), 1212, 0.0);
}

#[test]
fn fixture_water3() {
    test_fixture(include_str!("fixtures/water3.json"), 197, 0.0);
}

#[test]
fn fixture_water3b() {
    test_fixture(include_str!("fixtures/water3b.json"), 25, 0.0);
}

#[test]
fn fixture_water4() {
    test_fixture(include_str!("fixtures/water4.json"), 705, 0.0);
}

#[test]
fn fixture_water_huge1() {
    test_fixture(include_str!("fixtures/water-huge.json"), 5176, 0.0011);
}

#[test]
fn fixture_water_huge2() {
    test_fixture(include_str!("fixtures/water-huge2.json"), 4462, 0.004);
}

#[test]
fn fixture_degenerate() {
    test_fixture(include_str!("fixtures/degenerate.json"), 0, 0.0);
}

#[test]
fn fixture_bad_hole() {
    test_fixture(include_str!("fixtures/bad-hole.json"), 42, 0.019);
}

#[test]
fn fixture_empty_square() {
    test_fixture(include_str!("fixtures/empty-square.json"), 0, 0.0);
}

#[test]
fn fixture_issue16() {
    test_fixture(include_str!("fixtures/issue16.json"), 12, 4e-16);
}

#[test]
fn fixture_issue17() {
    test_fixture(include_str!("fixtures/issue17.json"), 11, 2e-16);
}

#[test]
fn fixture_steiner() {
    test_fixture(include_str!("fixtures/steiner.json"), 9, 0.0);
}

#[test]
fn fixture_issue29() {
    test_fixture(include_str!("fixtures/issue29.json"), 40, 2e-15);
}

#[test]
fn fixture_issue34() {
    test_fixture(include_str!("fixtures/issue34.json"), 139, 0.0);
}

#[test]
fn fixture_issue35() {
    test_fixture(include_str!("fixtures/issue35.json"), 844, 0.0);
}

#[test]
fn fixture_self_touching() {
    test_fixture(include_str!("fixtures/self-touching.json"), 124, 2e-13);
}

#[test]
fn fixture_outside_ring() {
    test_fixture(include_str!("fixtures/outside-ring.json"), 64, 0.0);
}

#[test]
fn fixture_simplified_us_border() {
    test_fixture(include_str!("fixtures/simplified-us-border.json"), 120, 0.0);
}

#[test]
fn fixture_touching_holes() {
    test_fixture(include_str!("fixtures/touching-holes.json"), 57, 0.0);
}

#[test]
fn fixture_touching_holes2() {
    test_fixture(include_str!("fixtures/touching-holes2.json"), 10, 0.0);
}

#[test]
fn fixture_touching_holes3() {
    test_fixture(include_str!("fixtures/touching-holes3.json"), 82, 0.0);
}

#[test]
fn fixture_touching_holes4() {
    test_fixture(include_str!("fixtures/touching-holes4.json"), 55, 0.0);
}

#[test]
fn fixture_touching_holes5() {
    test_fixture(include_str!("fixtures/touching-holes5.json"), 133, 0.0);
}

#[test]
fn fixture_touching_holes6() {
    test_fixture(include_str!("fixtures/touching-holes6.json"), 3098, 0.0);
}

#[test]
fn fixture_hole_touching_outer() {
    test_fixture(include_str!("fixtures/hole-touching-outer.json"), 77, 0.0);
}

#[test]
fn fixture_hilbert() {
    test_fixture(include_str!("fixtures/hilbert.json"), 1024, 0.0);
}

#[test]
fn fixture_issue45() {
    test_fixture(include_str!("fixtures/issue45.json"), 10, 0.0);
}

#[test]
fn fixture_eberly_3() {
    test_fixture(include_str!("fixtures/eberly-3.json"), 73, 0.0);
}

#[test]
fn fixture_eberly_6() {
    test_fixture(include_str!("fixtures/eberly-6.json"), 1429, 2e-14);
}

#[test]
fn fixture_issue52() {
    test_fixture(include_str!("fixtures/issue52.json"), 109, 0.0);
}

#[test]
fn fixture_shared_points() {
    test_fixture(include_str!("fixtures/shared-points.json"), 4, 0.0);
}

#[test]
fn fixture_bad_diagonals() {
    test_fixture(include_str!("fixtures/bad-diagonals.json"), 7, 0.0);
}

#[test]
fn fixture_issue83() {
    test_fixture(include_str!("fixtures/issue83.json"), 0, 0.0);
}

#[test]
fn fixture_issue107() {
    test_fixture(include_str!("fixtures/issue107.json"), 0, 0.0);
}

#[test]
fn fixture_issue111() {
    test_fixture(include_str!("fixtures/issue111.json"), 18, 0.0);
}

#[test]
fn fixture_collinear_boxy() {
    test_fixture(include_str!("fixtures/boxy.json"), 58, 0.0);
}

#[test]
fn fixture_collinear_diagonal() {
    test_fixture(include_str!("fixtures/collinear-diagonal.json"), 14, 0.0);
}

#[test]
fn fixture_issue119() {
    test_fixture(include_str!("fixtures/issue119.json"), 18, 0.0);
}

#[test]
fn fixture_hourglass() {
    test_fixture(include_str!("fixtures/hourglass.json"), 2, 0.0);
}

#[test]
fn fixture_touching2() {
    test_fixture(include_str!("fixtures/touching2.json"), 8, 0.0);
}

#[test]
fn fixture_touching3() {
    test_fixture(include_str!("fixtures/touching3.json"), 15, 0.0);
}

#[test]
fn fixture_touching4() {
    test_fixture(include_str!("fixtures/touching4.json"), 19, 0.0);
}

#[test]
fn fixture_rain() {
    test_fixture(include_str!("fixtures/rain.json"), 2681, 0.0);
}

#[test]
fn fixture_issue131() {
    test_fixture(include_str!("fixtures/issue131.json"), 12, 0.0);
}

#[test]
fn fixture_infinite_loop_jhl() {
    test_fixture(include_str!("fixtures/infinite-loop-jhl.json"), 0, 0.0);
}

#[test]
fn fixture_filtered_bridge_jhl() {
    test_fixture(include_str!("fixtures/filtered-bridge-jhl.json"), 25, 0.0);
}

#[test]
fn fixture_issue149() {
    test_fixture(include_str!("fixtures/issue149.json"), 2, 0.0);
}

#[test]
fn fixture_issue142() {
    test_fixture(include_str!("fixtures/issue142.json"), 4, 0.13);
}

#[test]
fn fixture_issue186() {
    test_fixture(include_str!("fixtures/issue186.json"), 41, 0.0);
}
