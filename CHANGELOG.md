# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.12](https://github.com/georust/earcut/compare/v0.4.11...v0.4.12) - 2026-11-02

### Added

- Slice input APIs for floating-point and integer coordinates.
- Reproducible Rust and C++ benchmark tooling.

### Changed

- Faster hole bridging and specialized ear-clipping loops.
- Sync upstream fixes through [mapbox/earcut@934e54d](https://github.com/mapbox/earcut/commit/934e54d).
- Update the visualizer to eframe 0.37.0.

### Fixed

- Detect missing triangulation output for translated and thin polygons.
- Ignore overflowing orientations when choosing refinement winding.
- Align the visualizer's Y axis and vertex dragging with screen coordinates.

## [0.4.11](https://github.com/georust/earcut/compare/v0.4.10...v0.4.11) - 2026-07-25

### Added

- sync with upstream earcut 3.2.3 ([#46](https://github.com/georust/earcut/pull/46))

## [0.4.10](https://github.com/georust/earcut/compare/v0.4.9...v0.4.10) - 2026-05-04

### Added

- *(examples)* Add visualizer ([#44](https://github.com/georust/earcut/pull/44))

### Fixed

- *(ci)* Set GitHub token for release-plz ([#34](https://github.com/georust/earcut/pull/34))

### Other

- restore earcut_impl as deprecated and document utils3d ([#45](https://github.com/georust/earcut/pull/45))
- Refresh README benchmark ([#42](https://github.com/georust/earcut/pull/42))
- bbox prechecks and Vec-based z-order sort ([#40](https://github.com/georust/earcut/pull/40))
- *(test)* consolidate float and int fixture tests ([#39](https://github.com/georust/earcut/pull/39))
- embed test fixtures ([#38](https://github.com/georust/earcut/pull/38))
- consolidate unsafe node access helpers ([#36](https://github.com/georust/earcut/pull/36))
- prepare transfer of `earcut-rs` to GeoRust ([#30](https://github.com/georust/earcut/pull/30))
- Update README CI badge ([#33](https://github.com/georust/earcut/pull/33))
