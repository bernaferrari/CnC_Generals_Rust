#!/usr/bin/env python3
"""Compare the executed original health kernel with a built Main test binary.

Only original internalChangeHealth/calcDamageState execute on the C++ side.
The driver supplies bounded status/visual stubs and retail or custom0.8/0.4
thresholds. This does not verify particles, rendering, arbitrary GameData, or
complete gameplay.
Build/source provenance for the supplied test binary remains the caller's
responsibility; its hash and the inspected source hashes are recorded.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[4]
CPP = "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/ActiveBody.cpp"
CPP_SHA = "1bf2a867ac27febb5947720cc7e54beb1b5e2afb1311ea7b0109356062589054"
PROFILES = {
    "retail": ("ordinary_health_callback_component_trace", 13),
    "receiving-custom": ("receiving_custom_threshold_health_callback_component_trace", 7),
}
FIELDS = ("scenario", "phase", "hp_bits", "previous_bits", "state",
          "effectively_dead", "under_construction", "visual_calls")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def extract(data, signature):
    start = data.index(signature)
    position = data.index(b"{", start) + 1
    depth = 1
    while depth:
        depth += (data[position] == ord("{")) - (data[position] == ord("}"))
        position += 1
    return data[start:position]


def run(command, cwd):
    process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, text=True,
                               start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=60)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.communicate()
        raise RuntimeError(f"component command timed out: {command}") from None
    if process.returncode:
        raise RuntimeError(f"component command failed ({process.returncode}): "
                           f"{command}\n{stdout}\n{stderr}")
    return stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=ROOT)
    parser.add_argument("--test-binary", type=Path, required=True)
    parser.add_argument("--expected-binary-sha256", required=True)
    parser.add_argument("--cxx", default="c++")
    parser.add_argument("--profile", choices=PROFILES, default="retail")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root, binary = args.repo.resolve(), args.test_binary.resolve()
    test, scenario_count = PROFILES[args.profile]
    before = digest(binary.read_bytes())
    if before != args.expected_binary_sha256:
        raise RuntimeError("test binary differs from the supplied build receipt")
    original = (root / CPP).read_bytes()
    if digest(original) != CPP_SHA:
        raise RuntimeError("original C++ changed; review the extraction and scope")
    driver = Path(__file__).with_name("cpp_oracles") / "active_body_health.cpp"
    extractions = {}
    with tempfile.TemporaryDirectory(prefix="generals-body-cpp-") as name:
        work = Path(name)
        for filename, signature in (
            ("original_internal_change_health.inc",
             b"void ActiveBody::internalChangeHealth( Real delta )"),
            ("original_calc_damage_state.inc",
             b"static BodyDamageType calcDamageState(Real health, Real maxHealth)"),
        ):
            data = extract(original, signature)
            (work / filename).write_bytes(data)
            extractions[filename] = digest(data)
        (work / "driver.cpp").write_bytes(driver.read_bytes())
        compiler = run([args.cxx, "--version"], work)
        run([args.cxx, "-std=c++17", "-Wall", "-Wextra", "-pedantic",
             "driver.cpp", "-o", "body-component"], work)
        cpp_command = [str(work / "body-component")]
        if args.profile != "retail":
            cpp_command.append(args.profile)
        cpp_output = run(cpp_command, work)
        names = [line.removesuffix(": test") for line in
                 run([str(binary), "--list"], work).splitlines()
                 if line.endswith(f"::{test}: test")]
        if len(names) != 1:
            raise RuntimeError(f"expected one actual health-kernel test, got {names}")
        rust_output = run([str(binary), names[0], "--exact",
                           "--test-threads=1", "--nocapture"], work)
    cpp = [json.loads(line) for line in cpp_output.splitlines()]
    rust = [json.loads(line.split("BODY_CPP_PARITY ", 1)[1])
            for line in rust_output.splitlines() if "BODY_CPP_PARITY " in line]
    expected = [{key: row[key] for key in FIELDS} for row in cpp
                if row["phase"] in ("visual", "complete")]
    actual = [{key: row[key] for key in FIELDS} for row in rust]
    if not expected or sum(row["phase"] == "complete" for row in expected) != scenario_count:
        raise RuntimeError("original component did not execute all health scenarios")
    result = dict(status="pass" if actual == expected else "different",
                  scope=__doc__, profile=args.profile, binary_sha256=before, original_sha256=CPP_SHA,
                  driver_sha256=digest(driver.read_bytes()), extractions=extractions,
                  compiler=compiler, observations=len(expected),
                  original=expected, rust=actual)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    if digest(binary.read_bytes()) != before:
        raise RuntimeError("test binary changed during comparison")
    if actual != expected:
        raise RuntimeError(f"health traces differ; see {args.output}")
    print(f"Original C++ health component: {len(expected)} exact observations")


if __name__ == "__main__":
    main()
