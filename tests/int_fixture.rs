//! Cross-validate `earcut::int::EarcutI32` against the floating-point
//! `earcut::Earcut<f64>` on every integer-valued fixture.

use std::collections::BTreeSet;
use std::fs;

use earcut::int::{EarcutI32, deviation as int_deviation};
use earcut::{Earcut, deviation as float_deviation};

type Coords = Vec<Vec<[f64; 2]>>;

fn load(name: &str) -> Option<(Vec<[f64; 2]>, Vec<[i32; 2]>, Vec<u32>)> {
    let s = fs::read_to_string("./tests/fixtures/".to_string() + name + ".json").unwrap();
    let rings = serde_json::from_str::<Coords>(&s).unwrap();

    // Only accept fixtures whose coords are representable as i32 exactly.
    let flat: Vec<[f64; 2]> = rings.iter().flatten().copied().collect();
    let mut as_i32 = Vec::with_capacity(flat.len());
    for &[x, y] in &flat {
        if x.fract() != 0.0 || y.fract() != 0.0 {
            return None;
        }
        if !(i32::MIN as f64..=i32::MAX as f64).contains(&x)
            || !(i32::MIN as f64..=i32::MAX as f64).contains(&y)
        {
            return None;
        }
        as_i32.push([x as i32, y as i32]);
    }

    let num_holes = rings.len();
    let hole_indices: Vec<u32> = rings
        .into_iter()
        .map(|x| x.len() as u32)
        .scan(0u32, |sum, e| {
            *sum += e;
            Some(*sum)
        })
        .take(num_holes.saturating_sub(1))
        .collect();

    Some((flat, as_i32, hole_indices))
}

fn check(name: &str) {
    let Some((data_f, data_i32, holes)) = load(name) else {
        panic!("fixture {name} contained non-integer coordinates");
    };

    let mut f_tri: Vec<u32> = vec![];
    let mut i32_tri: Vec<u32> = vec![];
    Earcut::new().earcut(data_f.iter().copied(), &holes, &mut f_tri);
    EarcutI32::new().earcut(data_i32.iter().copied(), &holes, &mut i32_tri);

    assert_eq!(
        i32_tri.len(),
        f_tri.len(),
        "{name}: int index count differs from f64 reference"
    );

    let f_dev = if f_tri.is_empty() {
        0.0
    } else {
        float_deviation(data_f.iter().copied(), &holes, &f_tri)
    };
    let i_dev = relative_int_deviation(&data_i32, &holes, &i32_tri);
    assert!(
        i_dev <= f_dev + 1e-12,
        "{name}: int deviation {i_dev} exceeded f64 deviation {f_dev}"
    );
}

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

fn relative_int_deviation(data: &[[i32; 2]], hole_indices: &[u32], triangles: &[u32]) -> f64 {
    let polygon_area = polygon_area2(data, hole_indices);
    if polygon_area == 0 {
        return 0.0;
    }
    int_deviation(data.iter().copied(), hole_indices, triangles) as f64 / polygon_area as f64
}

#[test]
fn integer_fixture_list_matches_fixture_dir() {
    let mut discovered = BTreeSet::new();
    for entry in fs::read_dir("./tests/fixtures").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap();
        if load(name).is_some() {
            discovered.insert(name.to_owned());
        }
    }

    let listed: BTreeSet<String> = listed_fixtures().into_iter().map(str::to_owned).collect();
    assert_eq!(
        listed, discovered,
        "integer fixture list drifted; update tests/int_fixture.rs"
    );
}

macro_rules! for_each_int_fixture {
    ($macro:ident) => {
        $macro! {
            int_fixture_bad_diagonals => "bad-diagonals",
            int_fixture_bad_hole => "bad-hole",
            int_fixture_boxy => "boxy",
            int_fixture_building => "building",
            int_fixture_collinear_diagonal => "collinear-diagonal",
            int_fixture_degenerate => "degenerate",
            int_fixture_eberly_3 => "eberly-3",
            int_fixture_empty_square => "empty-square",
            int_fixture_filtered_bridge_jhl => "filtered-bridge-jhl",
            int_fixture_hilbert => "hilbert",
            int_fixture_hole_touching_outer => "hole-touching-outer",
            int_fixture_hourglass => "hourglass",
            int_fixture_issue111 => "issue111",
            int_fixture_issue119 => "issue119",
            int_fixture_issue131 => "issue131",
            int_fixture_issue149 => "issue149",
            int_fixture_issue186 => "issue186",
            int_fixture_issue34 => "issue34",
            int_fixture_issue35 => "issue35",
            int_fixture_issue45 => "issue45",
            int_fixture_issue52 => "issue52",
            int_fixture_issue83 => "issue83",
            int_fixture_outside_ring => "outside-ring",
            int_fixture_rain => "rain",
            int_fixture_shared_points => "shared-points",
            int_fixture_simplified_us_border => "simplified-us-border",
            int_fixture_steiner => "steiner",
            int_fixture_touching_holes => "touching-holes",
            int_fixture_touching_holes2 => "touching-holes2",
            int_fixture_touching_holes3 => "touching-holes3",
            int_fixture_touching_holes4 => "touching-holes4",
            int_fixture_touching_holes5 => "touching-holes5",
            int_fixture_touching_holes6 => "touching-holes6",
            int_fixture_touching2 => "touching2",
            int_fixture_touching3 => "touching3",
            int_fixture_touching4 => "touching4",
            int_fixture_water => "water",
            int_fixture_water_huge => "water-huge",
            int_fixture_water_huge2 => "water-huge2",
            int_fixture_water2 => "water2",
            int_fixture_water3 => "water3",
            int_fixture_water3b => "water3b",
            int_fixture_water4 => "water4",
        }
    };
}

macro_rules! define_listed_fixtures {
    ($($fn_name:ident => $fixture:literal),* $(,)?) => {
        fn listed_fixtures() -> BTreeSet<&'static str> {
            BTreeSet::from([$($fixture),*])
        }
    };
}

macro_rules! int_fixture_tests {
    ($($fn_name:ident => $fixture:literal),* $(,)?) => {
        $(
            #[test]
            fn $fn_name() { check($fixture); }
        )*
    };
}

for_each_int_fixture!(define_listed_fixtures);
for_each_int_fixture!(int_fixture_tests);
