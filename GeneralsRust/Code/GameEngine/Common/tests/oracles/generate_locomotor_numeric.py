#!/usr/bin/env python3
"""Verify locomotor field provenance and rerun the original numeric callbacks.

This deliberately reuses the original-source callback extractor, not Rust math.
Only the nine fields already converted by Common are in this arithmetic slice.
Other authored fields have a separate raw Common representation and downstream
converters; no complete original locomotor loader or simulation is executed.
"""

import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys

from generate_ini_numeric import PINS


LOCOMOTOR_PINS = {
    "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Locomotor.cpp":
        "d0eb6b4cf7cac16a9d51e1456defc7b63ef97c70f0b356b7df3d77e005974f5a",
    "GeneralsMD/Code/GameEngine/Include/GameLogic/Locomotor.h":
        "298f952d2bc05aceeee10d25424267663164f0e6631f91145705bc91cdaa4c0c",
}

# INI token name, actual C++ callback, actual Real member, oracle column.
FIELDS = [
    ("Speed", "parseVelocityReal", "m_maxSpeed", 4),
    ("SpeedDamaged", "parseVelocityReal", "m_maxSpeedDamaged", 4),
    ("TurnRate", "parseAngularVelocityReal", "m_maxTurnRate", 3),
    ("TurnRateDamaged", "parseAngularVelocityReal", "m_maxTurnRateDamaged", 3),
    ("Acceleration", "parseAccelerationReal", "m_acceleration", 5),
    ("AccelerationDamaged", "parseAccelerationReal", "m_accelerationDamaged", 5),
    ("Lift", "parseAccelerationReal", "m_lift", 5),
    ("LiftDamaged", "parseAccelerationReal", "m_liftDamaged", 5),
    ("Braking", "parseAccelerationReal", "m_braking", 5),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[6])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    sources = {}
    for relative, expected in {**PINS, **LOCOMOTOR_PINS}.items():
        data = (args.repo / relative).read_bytes()
        assert hashlib.sha256(data).hexdigest() == expected, relative
        sources[relative] = data.decode()
    cpp, header = (sources[path] for path in LOCOMOTOR_PINS)
    table = cpp.split("const FieldParse* LocomotorTemplate::getFieldParse() const", 1)[1]
    table = table.split("return TheFieldParse;", 1)[0]
    manifest = []
    for field, callback, member, column in FIELDS:
        pattern = (r'\{\s*"' + field + r'",\s*INI::' + callback
                   + r",\s*NULL,\s*offsetof\(\s*LocomotorTemplate,\s*"
                   + member + r"\s*\)\s*\}")
        assert len(re.findall(pattern, table)) == 1, field
        assert re.search(r"\bReal\s+" + member + r"\s*;", header), member
        manifest.append(f"{field} INI::{callback} {member} column={column}")

    extractor = Path(__file__).with_name("generate_ini_numeric.py")
    command = [sys.executable, str(extractor), "--repo", str(args.repo),
               "--output", str(output)]
    subprocess.run(command, check=True)
    actual = (output / "ini_numeric_original.txt").read_bytes()
    frozen = Path(__file__).parents[1] / "fixtures/ini_numeric_original.txt"
    assert actual == frozen.read_bytes(), "original callback fixture drift"
    rows = [line.split() for line in actual.decode().splitlines() if not line.startswith("#")]
    assert len(rows) == 177
    mapped = ["# token " + " ".join(field for field, *_ in FIELDS)]
    mapped.extend(" ".join([row[0], *(row[column] for *_, column in FIELDS)]) for row in rows)
    (output / "locomotor_original.txt").write_text("\n".join(mapped) + "\n")
    (output / "field_manifest.txt").write_text("\n".join(manifest) + "\n")
    (output / "source_pins.txt").write_text("".join(
        f"{value}  {key}\n" for key, value in {**PINS, **LOCOMOTOR_PINS}.items()))
    print(f"Verified {len(FIELDS)} original Real field callbacks; {len(rows) * len(FIELDS)} mapped outputs")


if __name__ == "__main__":
    main()
