# Rust / earcut.hpp comparison

Run with a local earcut.hpp checkout and a C++17 compiler:

```sh
cargo fetch --manifest-path benches/compare_hpp/Cargo.toml
python3 benches/compare_hpp/run.py /path/to/earcut.hpp --machine "CPU model"
```

Both implementations receive the same prebuilt geometry. Rust uses the public
slice API with reusable buffers; C++ calls the public `mapbox::earcut` function,
including allocation and destruction. Input preparation is excluded.

The runner snapshots sources before building, alternates implementation order
for each case across processes, and keeps every sample. Results are medians of
per-process medians. On macOS, it also records the OS-reported power source,
battery status, and low-power settings before and after measurement. These
observations do not guarantee the physical power source remained constant.

Raw logs and source snapshots go under `target/hpp-comparison/`, or the directory
specified by `--output`. Use `--cases` and `--modes` to narrow a run, and
`--case-sample-ms CASE=MS` to extend individual samples.

`results-m4-pro.json` contains the historical public-API comparison used in the
project README. Its power conditions were not recorded during measurement, so
the timings are provisional. Source hashes identify the code measured; later
correctness fixes are not reflected in those hashes. Checkout and snapshot paths
have been normalized without changing timings or hashes.

The other result files preserve earlier experiments. In particular,
`results-m4-pro.internal-reuse.json` and the earlier archives include C++ internal
API measurements; they are not the public-API comparison used in the README.
