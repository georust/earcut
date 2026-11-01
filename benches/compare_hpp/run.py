#!/usr/bin/env python3
"""Benchmark prebuilt polygon inputs in Rust and C++; keep every raw sample."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import struct
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def command(args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, **kwargs)


def version(args):
    return subprocess.check_output(args, text=True).strip()


def fingerprint(paths):
    return {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}


def power_state():
    if platform.system() != "Darwin":
        return None
    try:
        battery = subprocess.check_output(["pmset", "-g", "batt"], text=True,
                                          stderr=subprocess.DEVNULL)
        custom = subprocess.check_output(["pmset", "-g", "custom"], text=True,
                                         stderr=subprocess.DEVNULL)
    except (OSError, subprocess.CalledProcessError):
        return None
    source = re.search(r"drawing from '([^']+)'", battery)
    status = re.search(r"(\d+)%\s*;\s*([^;]+)", battery)
    modes = {}
    profile = None
    for line in custom.splitlines():
        if line and not line[0].isspace() and line.endswith(":"):
            profile = line[:-1]
        fields = line.split()
        if profile and len(fields) == 2 and fields[0] == "lowpowermode":
            modes[profile] = int(fields[1])
    return {"os_source": source.group(1) if source else None,
            "battery_percent": int(status.group(1)) if status else None,
            "battery_status": status.group(2).strip() if status else None,
            "low_power_mode_by_profile": modes}


def input_cases(path, check_i32):
    """Read the exported manifest, including integer API eligibility if needed."""
    cases = {}
    with path.open("rb") as data:
        def u32():
            return struct.unpack("<I", data.read(4))[0]
        if data.read(8) != b"ECBENCH1":
            raise RuntimeError("invalid input manifest")
        for _ in range(u32()):
            name = data.read(u32()).decode()
            eligible = True
            for _ in range(u32()):
                vertices, holes = u32(), u32()
                if check_i32 and eligible:
                    coordinates = data.read(vertices * 16)
                    eligible = all(v.is_integer() and abs(v) <= 1 << 19
                                   for point in struct.iter_unpack("<dd", coordinates) for v in point)
                else:
                    data.seek(vertices * 16, 1)
                data.seek(holes * 4, 1)
            cases[name] = eligible
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("earcut_hpp", type=Path, help="path to the earcut.hpp checkout")
    parser.add_argument("--machine", required=True, help="machine/CPU name for the results")
    parser.add_argument("--processes", type=int, default=5)
    parser.add_argument("--samples", type=int, default=15)
    parser.add_argument("--sample-ms", type=int, default=100)
    parser.add_argument("--cxx", default="clang++")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--cases", nargs="+", default=[], help="measure only these case names")
    parser.add_argument("--modes", nargs="+", choices=["f64", "i32", "cpp_once"],
                        default=["f64", "cpp_once"])
    parser.add_argument("--case-sample-ms", nargs="+", default=[], metavar="CASE=MS",
                        help="override minimum sample duration for named cases")
    args = parser.parse_args()
    if min(args.processes, args.samples, args.sample_ms) < 1:
        parser.error("processes, samples and sample-ms must be positive")
    case_ms = {}
    for override in args.case_sample_ms:
        try:
            name, value = override.rsplit("=", 1)
            value = int(value)
            if not name or value < 1 or name in case_ms:
                raise ValueError
            case_ms[name] = value
        except ValueError:
            parser.error("case-sample-ms expects unique CASE=MS entries with positive durations")
    hpp = args.earcut_hpp.resolve()
    header = hpp / "include/mapbox/earcut.hpp"
    if not header.is_file():
        parser.error(f"missing {header}")
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    out = (args.output or ROOT / "target/hpp-comparison" / stamp).resolve()
    out.mkdir(parents=True, exist_ok=False)
    paths = [ROOT / "Cargo.toml", ROOT / "src/lib.rs", ROOT / "src/int.rs", ROOT / "src/utils3d.rs",
             ROOT / "benches/tiles-fixture.bin", ROOT / "benches/tiles_fixture.rs",
             ROOT / "benches/benchmark.rs", ROOT / "benches/bench_tiles.rs", header,
             HERE / "main.rs", HERE / "compare.cpp", HERE / "Cargo.toml", HERE / "run.py"]
    if (HERE / "Cargo.lock").is_file():
        paths.append(HERE / "Cargo.lock")
    paths.extend(sorted((ROOT / "tests/fixtures").glob("*.rs")))
    before = fingerprint(paths)
    # Build from immutable copies, rather than reading files while another
    # session may be updating the working tree. Keep them with the raw data.
    snapshot = out / "sources"
    copies = {}
    for path in paths:
        relative = (Path("earcut.hpp/include/mapbox/earcut.hpp") if path == header else
                    Path("earcut") / path.relative_to(ROOT))
        copied = snapshot / relative
        copied.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, copied)
        copies[str(copied)] = before[str(path)]
    if fingerprint(paths) != before or fingerprint(map(Path, copies)) != copies:
        raise RuntimeError("A source changed while creating the benchmark snapshot")
    build_here = snapshot / "earcut/benches/compare_hpp"
    flags = ["-std=c++17", "-O3", "-DNDEBUG", "-ffp-contract=off"]
    source_labels = {str(path): ("earcut.hpp/include/mapbox/earcut.hpp" if path == header else
                                path.relative_to(ROOT).as_posix()) for path in paths}
    metadata = {
        "date": datetime.date.today().isoformat(), "machine": args.machine,
        "rustc": version(["rustc", "-Vv"]), "cxx": version([args.cxx, "--version"]),
        "rust_commit": version(["git", "-C", str(ROOT), "rev-parse", "HEAD"]),
        "cpp_commit": version(["git", "-C", str(hpp), "rev-parse", "HEAD"]),
        "rust_working_tree_modified": bool(version(["git", "-C", str(ROOT), "status", "--short"])),
        "cpp_working_tree_modified": bool(version(["git", "-C", str(hpp), "status", "--short"])),
        "cpp_flags": flags, "rust_profile": "release: opt-level=3, codegen-units=1, lto=fat, panic=abort",
        "processes": args.processes, "samples_per_process": args.samples,
        "minimum_sample_ms": args.sample_ms, "warmup_ms_per_mode": 500,
        "workspace_warmup_calls_per_sample": 3,
        "qos": "QOS_CLASS_USER_INITIATED" if platform.system() == "Darwin" else "unchanged",
        "cases": args.cases, "modes": args.modes, "input_format": "ECBENCH1",
        "apis": {"f64": "public Earcut::earcut_slice, reusable instance and output",
                 "i32": "public EarcutI32::earcut_slice, reusable instance and output",
                 "cpp_once": "public mapbox::earcut<uint32_t>, fresh instance and output per call; allocation and destruction included"},
        "method": "Public APIs only. One case and mode per process; rotate case and mode order. Rust reuses an independent warmed workspace per sample and retains previous workspaces until case end to sample multiple allocation layouts. C++ calls mapbox::earcut for every polygon, including its allocations and destruction. 500ms CPU warm-up and 3 per-sample warm-up calls excluded; input preparation excluded. Median of per-process sample medians; all samples retained. No target-cpu=native/-march=native flags added.",
        "source_sha256": {source_labels[path]: sha for path, sha in before.items()},
        "snapshot_sha256": {Path(path).relative_to(snapshot).as_posix(): sha for path, sha in copies.items()},
    }
    (out / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(f"Results: {out}", flush=True)
    with (out / "build.log").open("w") as log:
        command(["cargo", "build", "--release", "--offline", "--manifest-path", build_here / "Cargo.toml",
                 "--target-dir", out / "rust-target"], stdout=log, stderr=subprocess.STDOUT)
        command([args.cxx, *flags, "-I", snapshot / "earcut.hpp/include", build_here / "compare.cpp", "-o", out / "cpp"],
                stdout=log, stderr=subprocess.STDOUT)
    rust = out / "rust-target/release/compare-earcut-hpp"
    command([rust, "--export", out / "inputs.bin"])
    available = input_cases(out / "inputs.bin", "i32" in args.modes)
    names = args.cases or list(available)
    if len(set(names)) != len(names) or not set(names) <= set(available):
        parser.error("duplicate or unknown case name")
    if len(set(args.modes)) != len(args.modes):
        parser.error("duplicate mode name")
    if not set(case_ms) <= set(names):
        parser.error("case-sample-ms contains an unselected or unknown case")
    metadata["cases"] = names
    case_positions = {name: index for index, name in enumerate(names)}
    metadata["minimum_sample_ms_by_case"] = {name: case_ms.get(name, args.sample_ms) for name in names}
    metadata["inputs_sha256"] = hashlib.sha256((out / "inputs.bin").read_bytes()).hexdigest()
    metadata["power_before"] = power_state()
    (out / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    measurements = {}
    raw = {}
    for process in range(args.processes):
        order = names[process % len(names):] + names[:process % len(names)]
        for name in order:
            modes = args.modes if (process + case_positions[name]) % 2 == 0 else args.modes[::-1]
            for mode in modes:
                if mode == "i32" and not available[name]:
                    continue
                print(f"Process {process + 1}/{args.processes}: {name}/{mode}", flush=True)
                millis = metadata["minimum_sample_ms_by_case"][name]
                cmd = ([rust, args.samples, millis, name] if mode in ("f64", "i32") else
                       [out / "cpp", out / "inputs.bin", args.samples, millis, name])
                path = out / f"{process + 1}-{name}-{mode}.tsv"
                env = {**os.environ, "EARCUT_BENCH_MODE": mode}
                with path.open("w") as log:
                    command(cmd, stdout=log, env=env)
                values = []
                for line in path.read_text().splitlines():
                    got_name, got_mode, got_round, ns = line.split("\t")
                    if (got_name, got_mode, int(got_round)) != (name, mode, len(values)):
                        raise RuntimeError(f"unexpected measurement row: {line}")
                    values.append(float(ns))
                if len(values) != args.samples:
                    raise RuntimeError(f"unexpected number of samples for {name}/{mode}")
                measurements.setdefault(name, {}).setdefault(mode, []).append(statistics.median(values))
                raw.setdefault((name, mode), []).append(values)
    metadata["power_after"] = power_state()
    (out / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    if fingerprint(paths) != before:
        raise RuntimeError("A benchmark source changed during measurement; keep raw logs but do not publish this run")
    if fingerprint(map(Path, copies)) != copies:
        raise RuntimeError("The benchmark source snapshot changed during measurement")
    results = {}
    for name in names:
        modes = measurements.get(name, {})
        expected = set(args.modes) - ({"i32"} if not available[name] else set())
        if set(modes) != expected:
            raise RuntimeError(f"missing measurements for {name}")
        results[name] = {}
        for mode, medians in modes.items():
            if len(medians) != args.processes:
                raise RuntimeError(f"missing process for {name}/{mode}")
            results[name][mode] = {
                "median_ns": statistics.median(medians), "process_medians_ns": medians,
                "process_samples_ns": raw[(name, mode)],
                "process_median_spread_percent": (max(medians) / min(medians) - 1) * 100,
            }
    if args.cases and set(results) != set(args.cases):
        raise RuntimeError("requested cases did not all produce measurements")
    (out / "results.json").write_text(json.dumps({"metadata": metadata, "results": results}, indent=2) + "\n")
    for name, modes in results.items():
        print(name, " ".join(f"{mode}={data['median_ns']:.2f}ns" for mode, data in modes.items()), flush=True)


if __name__ == "__main__":
    main()
