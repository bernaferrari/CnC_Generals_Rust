#!/usr/bin/env python3
"""Emit a stable GeneralsMD INI data dump for Rust parity comparison."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_INI_ROOT = REPO_ROOT / "windows_game/extracted_big_files_v2/INIZH/Data/INI"


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Dump GeneralsMD object, weapon, and armor INI data as stable JSON"
    )
    parser.add_argument("--ini-root", type=Path, default=DEFAULT_INI_ROOT)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--min-objects", type=int, default=0)
    parser.add_argument("--min-weapons", type=int, default=0)
    parser.add_argument("--min-armors", type=int, default=0)
    args = parser.parse_args()

    ini_root = args.ini_root
    object_files = sorted((ini_root / "Object").rglob("*.ini"))
    objects: dict[str, dict[str, Any]] = {}

    for path in object_files:
        objects.update(parse_object_file(path.read_text(encoding="utf-8", errors="replace")))

    weapons = parse_named_sections(ini_root / "Weapon.ini", "Weapon", ini_root)
    armors = parse_named_sections(ini_root / "Armor.ini", "Armor", ini_root)

    assert_minimum("object template", len(objects), args.min_objects)
    assert_minimum("weapon template", len(weapons), args.min_weapons)
    assert_minimum("armor template", len(armors), args.min_armors)

    dump = {
        "source_root": str(ini_root),
        "counts": {
            "object_files": len(object_files),
            "object_templates": len(objects),
            "weapon_templates": len(weapons),
            "armor_templates": len(armors),
        },
        "objects": {name: objects[name] for name in sorted(objects)},
        "weapons": {name: weapons[name] for name in sorted(weapons)},
        "armors": {name: armors[name] for name in sorted(armors)},
    }

    encoded = json.dumps(dump, indent=2, sort_keys=False) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")


def assert_minimum(label: str, actual: int, expected: int) -> None:
    if actual < expected:
        raise SystemExit(f"{label} coverage too low: {actual} < {expected}")


def parse_object_file(content: str) -> dict[str, dict[str, Any]]:
    lines = content.splitlines()
    definitions: dict[str, dict[str, Any]] = {}
    current: dict[str, Any] | None = None
    current_condition_state = ""

    for index, raw_line in enumerate(lines):
        trimmed = strip_inline_comment(raw_line.strip()).strip()
        if not trimmed or is_comment(trimmed) or is_bracket_header(trimmed):
            continue

        header = parse_object_header(trimmed)
        if header is not None:
            if current is not None:
                definitions[current["name"]] = object_dump(current)
            name, parent_name = header
            current = new_object_definition(name, parent_name)
            current_condition_state = ""
            continue

        if trimmed.lower() == "end":
            if current is not None and is_object_terminator(lines, index + 1):
                definitions[current["name"]] = object_dump(current)
                current = None
            current_condition_state = ""
            continue

        if current is None:
            continue

        if trimmed.lower() == "defaultconditionstate":
            current_condition_state = "default"
            continue

        if "=" not in trimmed:
            continue

        key, value = trimmed.split("=", 1)
        key = key.strip()
        value = unquote(strip_inline_comment(value.strip()).strip())
        lower_key = key.lower()

        if lower_key == "type":
            current["object_type"] = value
        elif lower_key == "displayname":
            current["display_name"] = value
        elif lower_key == "conditionstate":
            current_condition_state = value.lower()
        elif lower_key in {"model", "modelname", "w3dmodel"}:
            assign_model_name(current, value, current_condition_state)
        elif lower_key in {"drawmodule", "draw"}:
            current["draw_module"] = value
        elif lower_key == "armortype":
            current["armor_type"] = value
        elif lower_key in {"hitpoints", "health", "maxhealth"}:
            try:
                current["hit_points"] = int(value)
            except ValueError:
                pass
        elif lower_key == "scale":
            try:
                current["scale"] = float(value)
            except ValueError:
                current["scale"] = 1.0
        elif lower_key == "owner":
            current["owner"] = value
        elif "texture" in lower_key:
            current["textures"][key] = value
        else:
            current["attributes"][key] = value

    if current is not None:
        definitions[current["name"]] = object_dump(current)

    return definitions


def new_object_definition(name: str, parent_name: str | None) -> dict[str, Any]:
    return {
        "name": name,
        "parent_name": parent_name,
        "object_type": "",
        "display_name": "",
        "model_name": None,
        "textures": {},
        "draw_module": None,
        "armor_type": None,
        "hit_points": None,
        "scale": 1.0,
        "owner": None,
        "attributes": {},
    }


def object_dump(definition: dict[str, Any]) -> dict[str, Any]:
    return {
        "parent_name": definition["parent_name"],
        "object_type": definition["object_type"],
        "display_name": definition["display_name"],
        "model_name": definition["model_name"],
        "textures": dict(sorted(definition["textures"].items())),
        "draw_module": definition["draw_module"],
        "armor_type": definition["armor_type"],
        "hit_points": definition["hit_points"],
        "scale": definition["scale"],
        "owner": definition["owner"],
        "attributes": dict(sorted(definition["attributes"].items())),
    }


def parse_object_header(line: str) -> tuple[str, str | None] | None:
    if "=" in line:
        return None

    tokens = line.split()
    if not tokens:
        return None

    if tokens[0] == "Object" and len(tokens) >= 2:
        return tokens[1], None
    if tokens[0] in {"ChildObject", "ObjectReskin"} and len(tokens) >= 2:
        return tokens[1], tokens[2] if len(tokens) >= 3 else None
    return None


def is_object_terminator(lines: list[str], start_index: int) -> bool:
    for raw_line in lines[start_index:]:
        trimmed = strip_inline_comment(raw_line.strip()).strip()
        if not trimmed or is_comment(trimmed):
            continue
        return parse_object_header(trimmed) is not None
    return True


def assign_model_name(
    definition: dict[str, Any], value: str, _condition_state: str
) -> None:
    if not value or value.lower() == "none":
        return
    if definition["model_name"] is None:
        definition["model_name"] = value


def parse_named_sections(
    path: Path, expected_type: str, ini_root: Path
) -> dict[str, dict[str, Any]]:
    content = path.read_text(encoding="utf-8", errors="replace")
    source_file = path.relative_to(ini_root).as_posix()
    sections: dict[str, dict[str, Any]] = {}
    current: tuple[str, list[dict[str, str]]] | None = None

    for raw_line in content.splitlines():
        line = strip_inline_comment(raw_line.strip()).strip()
        if not line:
            continue

        header = parse_section_header(line)
        if header is not None:
            if current is not None:
                name, properties = current
                sections[name] = {"source_file": source_file, "properties": properties}
            section_type, name = header
            current = (name, []) if section_type.lower() == expected_type.lower() else None
            continue

        if line.lower() == "end":
            if current is not None:
                name, properties = current
                sections[name] = {"source_file": source_file, "properties": properties}
                current = None
            continue

        if current is not None and "=" in line:
            _, properties = current
            key, value = line.split("=", 1)
            properties.append({"key": key.strip(), "value": unquote(value.strip())})

    if current is not None:
        name, properties = current
        sections[name] = {"source_file": source_file, "properties": properties}

    return sections


def parse_section_header(line: str) -> tuple[str, str] | None:
    if "=" in line:
        return None

    tokens = line.split()
    if len(tokens) >= 2 and tokens[0] in {"Weapon", "Armor"}:
        return tokens[0], tokens[1]
    return None


def strip_inline_comment(value: str) -> str:
    in_single = False
    in_double = False
    index = 0

    while index < len(value):
        char = value[index]
        if char == "'" and not in_double:
            in_single = not in_single
        elif char == '"' and not in_single:
            in_double = not in_double
        elif char in {";", "#"} and not in_single and not in_double:
            return value[:index].rstrip()
        elif (
            char == "/"
            and not in_single
            and not in_double
            and index + 1 < len(value)
            and value[index + 1] == "/"
        ):
            return value[:index].rstrip()
        index += 1

    return value


def is_comment(value: str) -> bool:
    return value.startswith(";") or value.startswith("//") or value.startswith("#")


def is_bracket_header(value: str) -> bool:
    return value.startswith("[") and value.endswith("]")


def unquote(value: str) -> str:
    if len(value) >= 2 and (
        (value.startswith('"') and value.endswith('"'))
        or (value.startswith("'") and value.endswith("'"))
    ):
        return value[1:-1]
    return value


if __name__ == "__main__":
    main()
