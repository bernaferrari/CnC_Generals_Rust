# Original unnamed weapon defaults

`weapon_damage_default.rs` uses the public template/admission/AttackObject/update
route. Its expected independent `Explosive` / `Normal` pair is grounded in the
original `WeaponTemplate` constructor and `dealDamageInternal` copies preserved
by this partial extraction.

`generate_weapon_damage_default.py` checks the three complete original file hashes, selects twelve
unique enum/default/getter/copy spans, and writes a small C++ harness plus
`extraction.json`. `output.txt` is the captured output: speed, radius, DamageType
ordinal, DeathType ordinal. All six rows are EXPLOSION (0), NORMAL (0).

Expected rows and pins are in `../fixtures/weapon_damage_default/`.

Reproduce from this directory with an explicit repository root and output folder:

```sh
python3 generate_weapon_damage_default.py --root /path/to/repository --out /tmp/weapon-default-oracle
c++ -std=c++17 -O0 -g0 /tmp/weapon-default-oracle/weapon_default_original.cpp -o /tmp/weapon-default-oracle/oracle
/tmp/weapon-default-oracle/oracle
```

The extractor's default root records the original cloud checkout; use `--root`
for another checkout. The generated C++ substitutes the surrounding structs and
changes speed/radius fields in the harness. This is not the complete constructor,
original INI parser, original engine, armor table, delivery, or save protocol.
In particular, speed zero's immediate delivery is the current Rust convention;
it is not validated against the original C++ division-by-speed path.

The Rust controls use unique fresh definitions in Main's real INI loader and
verify parsed types before ordinary admission. All roster relationships and
weapon stats are declared before admission. Named LASER uses neutral armor,
since the current named target-legality check rejects zero-coefficient vehicle
armor. The unnamed speed-zero vehicle witness retains that armor and records
an actual discharge before checking damage.

The named finite controls deliberately retain the existing resolver policy:
omitted named DeathType becomes Exploded for Explosion and Lasered for LASER.
Those outcomes are scope-preservation guards, not original death-default parity;
the original defaults DeathType to NORMAL independently for named definitions too.
The queued-shot snapshot proves the sampled pending fields, not save restoration
or lethal callback behavior.

## Recorded verification

On the unchanged production tree of bridge `9fdc511d5f18` (byte-identical to
accepted `b7b3b4456dbf`), the frozen public regression has 9 passing controls
and 6 intended unnamed-default failures. The paired-default repair passes all
15 tests with the exact same regression SHA-256:
`6226dcb733e462046f2ebf7f8bb67daec7db6f07ec5a49d7d0982bfbef90ce8e`.
The real speed-zero vehicle shot has the same durable discharge marker but
240 HP before versus 215 HP after in post-update trace frame 2.

The existing unchanged deterministic-frame-trace suite remains 6/8 both before
and after; this repair does not fix those two HP assertions. Historical smoke
results receive no repair credit. Targeted Rustfmt, whitespace, LOC and unsafe
ratchets pass, with zero ratchet violations and no allowlist edits. These are
focused native public-integration checks, not whole-game or whole-package parity.
