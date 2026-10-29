#!/usr/bin/env python3
"""Compile and execute the exact unsigned frame-gap expression from retail C++."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import re
import subprocess
import tempfile


SOURCE_RELATIVE = Path(
    "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/"
    "ParticleUplinkCannonUpdate.cpp"
)
SOURCE_SHA256 = "e43a7cf0984cc70507c0801e96ae34a929626b9334eed3d5329fa7048e9b785c"
BRANCH = re.compile(
    r"if\s*\(\s*(m_scriptedWaypointMode\s*\|\|\s*"
    r"m_lastDrivingClickFrame\s*-\s*m_2ndLastDrivingClickFrame\s*<\s*"
    r"data->m_doubleClickToFastDriveDelay)\s*\)"
)


def main() -> None:
    repo_root = Path(__file__).resolve().parents[6]
    source_path = repo_root / SOURCE_RELATIVE
    source_bytes = source_path.read_bytes()
    actual_sha = hashlib.sha256(source_bytes).hexdigest()
    if actual_sha != SOURCE_SHA256:
        raise SystemExit(
            f"C++ source hash mismatch: expected {SOURCE_SHA256}, got {actual_sha}"
        )

    source = source_bytes.decode("utf-8")
    match = BRANCH.search(source)
    if match is None:
        raise SystemExit("pinned source no longer contains the expected branch")
    expression = match.group(1)

    harness = f"""#include <cstdint>
#include <cstdio>

using UnsignedInt = std::uint32_t;
struct ParticleUplinkCannonUpdateModuleData {{
    UnsignedInt m_doubleClickToFastDriveDelay;
}};

static bool source_branch(UnsignedInt last, UnsignedInt previous,
                          UnsignedInt delay) {{
    bool m_scriptedWaypointMode = false;
    UnsignedInt m_lastDrivingClickFrame = last;
    UnsignedInt m_2ndLastDrivingClickFrame = previous;
    ParticleUplinkCannonUpdateModuleData data_value{{delay}};
    ParticleUplinkCannonUpdateModuleData* data = &data_value;
    return ({expression});
}}

int main() {{
    constexpr UnsignedInt delay = (500u * 30u) / 1000u;
    struct Case {{ const char* name; UnsignedInt last; UnsignedInt previous; bool expected; }};
    const Case cases[] = {{
        {{"finite gap 14", 100u, 86u, true}},
        {{"finite gap 15", 100u, 85u, false}},
        {{"finite gap 31", 100u, 69u, false}},
        {{"wrap gap 14", 7u, UINT32_MAX - 6u, true}},
        {{"wrap gap 15", 7u, UINT32_MAX - 7u, false}},
        {{"wrap gap 31", 7u, UINT32_MAX - 23u, false}},
    }};
    for (const Case& test : cases) {{
        const bool actual = source_branch(test.last, test.previous, delay);
        if (actual != test.expected) {{
            std::fprintf(stderr, "%s: expected %d, got %d\\n",
                         test.name, test.expected, actual);
            return 1;
        }}
        std::printf("PASS %s\\n", test.name);
    }}
    return 0;
}}
"""

    compiler = os.environ.get("CXX", "c++")
    with tempfile.TemporaryDirectory(prefix="particle-click-wrap-cpp-") as temp_dir:
        cpp_path = Path(temp_dir) / "witness.cpp"
        binary_path = Path(temp_dir) / "witness"
        cpp_path.write_text(harness, encoding="utf-8")
        subprocess.run(
            [compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", str(cpp_path), "-o", str(binary_path)],
            check=True,
        )
        subprocess.run([str(binary_path)], check=True)


if __name__ == "__main__":
    main()
