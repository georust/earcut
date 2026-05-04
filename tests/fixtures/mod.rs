#![allow(dead_code, clippy::approx_constant)]
#![cfg_attr(rustfmt, rustfmt::skip)]

mod bad_diagonals;
mod bad_hole;
mod boxy;
mod building;
mod collinear_diagonal;
mod degenerate;
mod dude;
mod eberly_3;
mod eberly_6;
mod empty_square;
mod filtered_bridge_jhl;
mod hilbert;
mod hole_touching_outer;
mod hourglass;
mod infinite_loop_jhl;
mod issue107;
mod issue111;
mod issue119;
mod issue131;
mod issue142;
mod issue149;
mod issue16;
mod issue17;
mod issue186;
mod issue29;
mod issue34;
mod issue35;
mod issue45;
mod issue52;
mod issue83;
mod outside_ring;
mod rain;
mod self_touching;
mod shared_points;
mod simplified_us_border;
mod steiner;
mod touching2;
mod touching3;
mod touching4;
mod touching_holes;
mod touching_holes2;
mod touching_holes3;
mod touching_holes4;
mod touching_holes5;
mod touching_holes6;
mod water;
mod water2;
mod water3;
mod water3b;
mod water4;
mod water_huge;
mod water_huge2;

pub use bad_diagonals::BAD_DIAGONALS;
pub use bad_hole::BAD_HOLE;
pub use boxy::BOXY;
pub use building::BUILDING;
pub use collinear_diagonal::COLLINEAR_DIAGONAL;
pub use degenerate::DEGENERATE;
pub use dude::DUDE;
pub use eberly_3::EBERLY_3;
pub use eberly_6::EBERLY_6;
pub use empty_square::EMPTY_SQUARE;
pub use filtered_bridge_jhl::FILTERED_BRIDGE_JHL;
pub use hilbert::HILBERT;
pub use hole_touching_outer::HOLE_TOUCHING_OUTER;
pub use hourglass::HOURGLASS;
pub use infinite_loop_jhl::INFINITE_LOOP_JHL;
pub use issue107::ISSUE107;
pub use issue111::ISSUE111;
pub use issue119::ISSUE119;
pub use issue131::ISSUE131;
pub use issue142::ISSUE142;
pub use issue149::ISSUE149;
pub use issue16::ISSUE16;
pub use issue17::ISSUE17;
pub use issue186::ISSUE186;
pub use issue29::ISSUE29;
pub use issue34::ISSUE34;
pub use issue35::ISSUE35;
pub use issue45::ISSUE45;
pub use issue52::ISSUE52;
pub use issue83::ISSUE83;
pub use outside_ring::OUTSIDE_RING;
pub use rain::RAIN;
pub use self_touching::SELF_TOUCHING;
pub use shared_points::SHARED_POINTS;
pub use simplified_us_border::SIMPLIFIED_US_BORDER;
pub use steiner::STEINER;
pub use touching2::TOUCHING2;
pub use touching3::TOUCHING3;
pub use touching4::TOUCHING4;
pub use touching_holes::TOUCHING_HOLES;
pub use touching_holes2::TOUCHING_HOLES2;
pub use touching_holes3::TOUCHING_HOLES3;
pub use touching_holes4::TOUCHING_HOLES4;
pub use touching_holes5::TOUCHING_HOLES5;
pub use touching_holes6::TOUCHING_HOLES6;
pub use water::WATER;
pub use water2::WATER2;
pub use water3::WATER3;
pub use water3b::WATER3B;
pub use water4::WATER4;
pub use water_huge::WATER_HUGE;
pub use water_huge2::WATER_HUGE2;

/// All fixtures as `(name, rings)` pairs, sorted by name.
pub static FIXTURES: &[(&str, &[&[[f64; 2]]])] = &[
    ("bad-diagonals", BAD_DIAGONALS),
    ("bad-hole", BAD_HOLE),
    ("boxy", BOXY),
    ("building", BUILDING),
    ("collinear-diagonal", COLLINEAR_DIAGONAL),
    ("degenerate", DEGENERATE),
    ("dude", DUDE),
    ("eberly-3", EBERLY_3),
    ("eberly-6", EBERLY_6),
    ("empty-square", EMPTY_SQUARE),
    ("filtered-bridge-jhl", FILTERED_BRIDGE_JHL),
    ("hilbert", HILBERT),
    ("hole-touching-outer", HOLE_TOUCHING_OUTER),
    ("hourglass", HOURGLASS),
    ("infinite-loop-jhl", INFINITE_LOOP_JHL),
    ("issue107", ISSUE107),
    ("issue111", ISSUE111),
    ("issue119", ISSUE119),
    ("issue131", ISSUE131),
    ("issue142", ISSUE142),
    ("issue149", ISSUE149),
    ("issue16", ISSUE16),
    ("issue17", ISSUE17),
    ("issue186", ISSUE186),
    ("issue29", ISSUE29),
    ("issue34", ISSUE34),
    ("issue35", ISSUE35),
    ("issue45", ISSUE45),
    ("issue52", ISSUE52),
    ("issue83", ISSUE83),
    ("outside-ring", OUTSIDE_RING),
    ("rain", RAIN),
    ("self-touching", SELF_TOUCHING),
    ("shared-points", SHARED_POINTS),
    ("simplified-us-border", SIMPLIFIED_US_BORDER),
    ("steiner", STEINER),
    ("touching2", TOUCHING2),
    ("touching3", TOUCHING3),
    ("touching4", TOUCHING4),
    ("touching-holes", TOUCHING_HOLES),
    ("touching-holes2", TOUCHING_HOLES2),
    ("touching-holes3", TOUCHING_HOLES3),
    ("touching-holes4", TOUCHING_HOLES4),
    ("touching-holes5", TOUCHING_HOLES5),
    ("touching-holes6", TOUCHING_HOLES6),
    ("water", WATER),
    ("water2", WATER2),
    ("water3", WATER3),
    ("water3b", WATER3B),
    ("water4", WATER4),
    ("water-huge", WATER_HUGE),
    ("water-huge2", WATER_HUGE2),
];
