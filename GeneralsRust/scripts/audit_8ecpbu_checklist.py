#!/usr/bin/env python3
"""Audit hq-8ecpbu checklist items for absent process globals / ambient paths.

Extracts THE_*/PROCESS_*/ambient phrases from each checklist child bead, rg-scans
GeneralsRust/Code (skipping // comment-only hits). Prints CLEAR vs KEEP.

Usage (from GeneralsRust):
  python3 scripts/audit_xbhvkd_checklist.py [/tmp/xbhvkd_checklist.txt]
"""
from __future__ import annotations
import json, re, subprocess, sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
CODE = ROOT / "Code"

AMBIENT_MAP = [
    (r"process GlobalData|THE_GLOBAL_DATA", ["THE_GLOBAL_DATA", "GLOBAL_DATA"]),
    (r"process ThingFactory|THE_THING_FACTORY", ["THE_THING_FACTORY", "THING_FACTORY_SLOT"]),
    (r"TheTerrainLogic|THE_TERRAIN_LOGIC", ["THE_TERRAIN_LOGIC"]),
    (r"OBJECT_REGISTRY", ["OBJECT_REGISTRY"]),
    (r"AUDIO_EVENT_OWNER_RESOLVER", ["AUDIO_EVENT_OWNER_RESOLVER"]),
    (r"THE_TERRAIN_VISUAL|terrain visual singleton", ["THE_TERRAIN_VISUAL"]),
    (r"PROCESS_LIFETIME", ["PROCESS_LIFETIME"]),
    (r"FILE_LENGTH_CACHE", ["FILE_LENGTH_CACHE"]),
    (r"LIVE_GAME_CLIENT_SLOT", ["LIVE_GAME_CLIENT_SLOT"]),
    (r"NAME_KEY_STATE", ["NAME_KEY_STATE"]),
    (r"CHALLENGE_MENU_STATE", ["CHALLENGE_MENU_STATE"]),
    (r"MISSION_SCRIPT_RUNTIME|SCRIPT_RUNTIME", ["MISSION_SCRIPT_RUNTIME", "SCRIPT_RUNTIME"]),
    (r"THE_PLAYER\b", ["THE_PLAYER"]),
    (r"THE_TEAM_FACTORY", ["THE_TEAM_FACTORY"]),
]
SYM = re.compile(
    r"\b(THE_[A-Z0-9_]{3,}|PROCESS_[A-Z0-9_]{3,}|FALLBACK_[A-Z0-9_]{3,}|"
    r"LIVE_[A-Z0-9_]{3,}|[A-Z][A-Z0-9_]{4,}_(?:STATE|SLOT|BAG|CACHE|REGISTRY|RESOLVER)|"
    r"FILE_LENGTH_CACHE|PROCESS_LIFETIME|OBJECT_REGISTRY|GLOBAL_DATA|AUDIO_EVENT_OWNER_RESOLVER)\b"
)
STRICT_TITLE = re.compile(
    r"(?i)\b(process|ambient|singleton|thread.?local|EngineStores|WorldServices|"
    r"CommonServices|GlobalData|PROCESS_LIFETIME|live GameClient|whole-operation|"
    r"active_services|with_world_services|global pending|Xfer|xfer|save|load|snapshot|CHUNK)\b"
)
OWN_SCOPE = re.compile(r"(?i)\b(per (client|session|world|instance)|instance-owned|receiving client)\b")
ALWAYS = set()  # xfer survivor: no forced KEEP


def bd_show(iid: str):
    try:
        out = subprocess.check_output(
            ["bd", "show", iid, "--json"], cwd=REPO, text=True, stderr=subprocess.DEVNULL, timeout=30
        )
        d = json.loads(out)
        return d[0] if isinstance(d, list) else d
    except Exception:
        return None


def code_hits(sym: str) -> list[str]:
    r = subprocess.run(
        ["rg", "-n", "--glob", "*.rs", "-w", sym, str(CODE)],
        capture_output=True, text=True, timeout=60,
    )
    hits = []
    for ln in r.stdout.splitlines():
        parts = ln.split(":", 2)
        if len(parts) < 3:
            continue
        if parts[2].lstrip().startswith("//"):
            continue
        hits.append(f"{parts[0]}:{parts[1]}")
    return hits


def main() -> int:
    checklist = Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/xbhvkd_checklist.txt")
    items = []
    for line in checklist.read_text().splitlines():
        m = re.match(r"- (hq-\S+) — (.+?) — (.+)$", line.strip())
        if m:
            items.append({"id": m.group(1), "title": m.group(2), "cref": m.group(3)})
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    with ThreadPoolExecutor(max_workers=8) as ex:
        futs = {ex.submit(bd_show, it["id"]): it for it in items}
        for fut in as_completed(futs):
            futs[fut]["bead"] = fut.result()
    all_syms = set()
    for it in items:
        bead = it.get("bead") or {}
        blob = "\n".join([it["title"], it["cref"], bead.get("description") or "", bead.get("notes") or ""])
        it["blob"] = blob
        all_syms |= set(SYM.findall(blob))
        for pat, syms in AMBIENT_MAP:
            if re.search(pat, blob, re.I):
                all_syms.update(syms)
    cache = {}
    with ThreadPoolExecutor(max_workers=10) as ex:
        futs = {ex.submit(code_hits, s): s for s in all_syms}
        for fut in as_completed(futs):
            cache[futs[fut]] = fut.result()
    clear, keep = [], []
    for it in items:
        targets = set(SYM.findall(it["blob"]))
        for pat, syms in AMBIENT_MAP:
            if re.search(pat, it["blob"], re.I):
                targets.update(syms)
        targets -= {"ACTIVE_STACK", "SCOPED_ACTIVE"}
        present, absent = [], []
        for s in sorted(targets):
            hits = cache.get(s) or []
            (present if hits else absent).append(s if not hits else (s, hits[:2]))
        present_syms = [p[0] if isinstance(p, tuple) else p for p in present]
        if targets and not present_syms:
            clear.append((it["id"], f"symbol_absent@{head}:{absent}", it["title"]))
        elif it["id"] in ALWAYS or present_syms or STRICT_TITLE.search(it["title"]) or OWN_SCOPE.search(it["title"]):
            keep.append((it["id"], present_syms, it["title"]))
        elif re.search(r"(?i)path|cell|layer|bridge|obstacle|destination|locomotor|AIPathfind|snap|guard|move", it["title"]):
            # Behavior-level: never bulk-clear pathfinder gaps on theme alone.
            keep.append((it["id"], present_syms or ["behavior_keep"], it["title"]))
        else:
            # Still avoid theme_mismatch CLEAR for this survivor — keep for manual C++ compare.
            keep.append((it["id"], present_syms or ["manual_review"], it["title"]))
    print(f"head={head} clear={len(clear)} keep={len(keep)}")
    for iid, reason, title in clear:
        print(f"CLEAR {iid} {reason} | {title[:60]}")
    for iid, present, title in keep:
        print(f"KEEP {iid} {present} | {title[:60]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
