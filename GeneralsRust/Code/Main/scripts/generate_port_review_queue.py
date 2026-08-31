#!/usr/bin/env python3
"""Generate bounded, deterministic C++ -> Rust provenance review packets."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import tempfile
from collections import defaultdict
from pathlib import Path
from typing import Any

from generate_port_provenance import extract_cpp_symbols


SCHEMA_VERSION = 1
DEFAULT_PACKET_SIZE = 15
MAX_PACKET_SIZE = 20
OUTPUT_NAME = "PORT_PROVENANCE_REVIEW_QUEUE.json"


def subsystem(source: str) -> str:
    parts = Path(source).parts
    for marker in ("Source", "Include", "src", "include"):
        if marker in parts:
            tail = parts[parts.index(marker) + 1 : -1]
            return "/".join(tail[:2]) if tail else "root"
    code_index = parts.index("Code") if "Code" in parts else -1
    tail = parts[code_index + 1 : -1]
    return "/".join(tail[:2]) if tail else "root"


def slug(value: str) -> str:
    result = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return result or "root"


def review_units(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        entry
        for entry in manifest["entries"]
        if entry["unit_kind"] == "translation_unit"
        and entry["inventory_class"] != "deferred_network"
        and entry["mapping"]["review_state"] != "reviewed"
    ]


def unit_record(repo: Path, entry: dict[str, Any]) -> dict[str, Any]:
    source = entry["source"]
    source_path = repo / source["path"]
    return {
        "entry_id": entry["id"],
        "source": {
            "path": source["path"],
            "sha256": source["sha256"],
            "symbols": extract_cpp_symbols(source_path) if source_path.is_file() else [],
        },
        "candidate_destinations": [
            {
                "path": destination["path"],
                "classification": destination["classification"],
                "cargo_reachable": destination["cargo_reachable"],
            }
            for destination in entry["mapping"]["destinations"]
        ],
        "blockers": entry["blockers"],
    }


def build_queue(
    repo: Path, manifest: dict[str, Any], packet_size: int = DEFAULT_PACKET_SIZE
) -> dict[str, Any]:
    if packet_size < 1 or packet_size > MAX_PACKET_SIZE:
        raise ValueError(f"packet_size must be in 1..={MAX_PACKET_SIZE}")
    groups: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for entry in review_units(manifest):
        source = entry["source"]["path"]
        groups[(entry["scope"], subsystem(source))].append(entry)

    packets: list[dict[str, Any]] = []
    for (scope, area), entries in sorted(groups.items()):
        ordered = sorted(entries, key=lambda entry: entry["source"]["path"])
        for offset in range(0, len(ordered), packet_size):
            chunk = ordered[offset : offset + packet_size]
            paths = [entry["source"]["path"] for entry in chunk]
            digest = hashlib.sha256("\n".join(paths).encode("utf-8")).hexdigest()[:12]
            packets.append(
                {
                    "id": f"{scope}-{slug(area)}-{digest}",
                    "scope": scope,
                    "subsystem": area,
                    "unit_count": len(chunk),
                    "units": [unit_record(repo, entry) for entry in chunk],
                    "acceptance": {
                        "review_input": "PORT_PROVENANCE_REVIEWED.json",
                        "requirements": [
                            "Inspect every listed C++ unit and candidate; never approve by stem alone.",
                            "Record only reachable production Rust destinations that own the behavior.",
                            "Do not treat tests, shims, telemetry, or deferred GameNetwork as implementation.",
                            "Keep symbol/range and behavioral proof claims separate from path review.",
                            "Ownership blockers only clear with complete valid ownership records: every symbol occurrence assigned, ranges exact, hashes current.",
                        ],
                        "commands": [
                            [
                                "python3",
                                "GeneralsRust/Code/Main/scripts/generate_port_tracking.py",
                                "--repo-root",
                                ".",
                            ],
                            [
                                "python3",
                                "GeneralsRust/Code/Main/scripts/generate_port_provenance.py",
                                "--repo-root",
                                ".",
                                "--verify-generated",
                            ],
                            [
                                "python3",
                                "GeneralsRust/Code/Main/scripts/generate_port_review_queue.py",
                                "--repo-root",
                                ".",
                                "--verify-generated",
                            ],
                            ["git", "diff", "--check"],
                        ],
                    },
                }
            )

    unit_paths = [
        unit["source"]["path"] for packet in packets for unit in packet["units"]
    ]
    return {
        "schema_version": SCHEMA_VERSION,
        "generator": "GeneralsRust/Code/Main/scripts/generate_port_review_queue.py",
        "provenance_input_digest": manifest["input_digest"],
        "packet_size": packet_size,
        "summary": {
            "packets": len(packets),
            "unreviewed_required_translation_units": len(unit_paths),
            "unique_units": len(set(unit_paths)),
            "maximum_packet_size": max((packet["unit_count"] for packet in packets), default=0),
        },
        "packets": packets,
    }


def generate(
    repo: Path,
    output: Path,
    packet_size: int,
    manifest: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if manifest is None:
        manifest = json.loads(
            (repo / "PORT_PROVENANCE_MANIFEST.json").read_text(encoding="utf-8")
        )
    queue = build_queue(repo, manifest, packet_size)
    output.write_text(json.dumps(queue, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return queue


def verify_generated(repo: Path, output: Path, packet_size: int) -> bool:
    if not output.is_file():
        return False
    with tempfile.TemporaryDirectory() as temporary:
        candidate = Path(temporary) / OUTPUT_NAME
        generate(repo, candidate, packet_size)
        return candidate.read_bytes() == output.read_bytes()


def select_packet(queue: dict[str, Any], packet_id: str) -> dict[str, Any]:
    matches = [packet for packet in queue["packets"] if packet["id"] == packet_id]
    if len(matches) != 1:
        raise ValueError(f"unknown provenance packet: {packet_id}")
    return matches[0]


def parse_args() -> argparse.Namespace:
    script = Path(__file__).resolve()
    repo = script.parents[4]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=repo)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--packet-size", type=int, default=DEFAULT_PACKET_SIZE)
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--verify-generated", action="store_true")
    action.add_argument("--list-packets", action="store_true")
    action.add_argument("--packet")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo = args.repo_root.resolve()
    output = args.output or repo / OUTPUT_NAME
    if args.list_packets or args.packet:
        queue = json.loads(output.read_text(encoding="utf-8"))
        if args.packet:
            print(json.dumps(select_packet(queue, args.packet), indent=2, sort_keys=True))
        else:
            for packet in queue["packets"]:
                print(
                    f"{packet['id']}\t{packet['unit_count']}\t"
                    f"{packet['scope']}\t{packet['subsystem']}"
                )
        return 0
    if args.verify_generated:
        passed = verify_generated(repo, output, args.packet_size)
        print(f"provenance review queue current={str(passed).lower()} path={output}")
        return 0 if passed else 1
    queue = generate(repo, output, args.packet_size)
    print(
        f"review queue: {queue['summary']['packets']} packets, "
        f"{queue['summary']['unique_units']} unique unreviewed units, "
        f"max packet {queue['summary']['maximum_packet_size']}"
    )
    print(f"Generated: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
