#!/usr/bin/env python3
"""Smoke-check the C++ producer without treating fixture state as engine parity."""

from __future__ import annotations

import json
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: validate_trace.py trace.json scenario.json")
    trace = json.loads(Path(sys.argv[1]).read_text())
    scenario = json.loads(Path(sys.argv[2]).read_text())
    assert trace["schema"] == "generals.frame_trace.v2"
    assert trace["scenario"] == scenario["scenario"]
    assert trace["producer"] == "generalsmd-cpp-original-randomvalue"
    assert trace["authority"]["rng"].endswith("Common/RandomValue.cpp")
    assert trace["authority"]["objects"].startswith("fixture-only")
    assert len(trace["frames"]) == scenario["final_frame"]
    assert trace["frames"][0]["commands"][0]["command_id"] == 1
    assert trace["frames"][0]["objects"] == scenario["objects"]
    assert trace["frames"][0]["players"] == scenario["players"]
    assert all(len(frame["rng_seed"]) == 6 for frame in trace["frames"])
    assert len({tuple(frame["rng_seed"]) for frame in trace["frames"]}) == scenario["final_frame"]
    assert trace["frames"][0]["rng_seed"] == [3268696253, 3195204591, 604862093, 1270104806, 847062993, 2182320605]
    assert trace["frames"][1]["rng_seed"] == [2778316750, 3804587793, 609383202, 4521108, 3029383598, 2182320606]
    assert [frame["crc"] for frame in trace["frames"]] == [4182470011, 361455992, 1467039468]
    assert all(isinstance(frame["crc"], int) for frame in trace["frames"])
    print(f"validated {len(trace['frames'])} canonical frames; RNG authority is original C++")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
