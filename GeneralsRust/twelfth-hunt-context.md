# Goal
Find player-visible C++ vs Rust leftover/live-host gaps for epic hq-5hpgc.

# Constraints
- C++: GeneralsMD/Code
- Live host: GeneralsRust/Code/Main
- Leftover crate: GeneralsRust/Code/GameEngine
- Dual-world: leftover can be correct while live empty-gates. Report the LIVE path.
- Allowed: wgpu vs DX; Rust safety; enum names if values match.
- Do NOT report GameNetwork.
- Do NOT report unwrap/clippy/docs/tests-only or residual honesty packs.
- Do NOT report already-closed beads (water 4-corner, slope Impassable, detector relationship, GenericBridge, waveguide, DAMAGE_WATER, rebuild GLA holes, radar brownout, GLA energy, Flame DoT, clicked science, SMALL_MISSILE flares, bounty gates, parking keep-stall/heal/reservedForExit/runway-column, Jarmen range, Lotus 2D+LOS, SA facing/FX/laser/infil, campaign ScoreScreen, BriefingVoice silent, science host drain/SPP/capable).
- playable_claim stays false.
- Skip formatters/linters/project-wide tests.

# Output
JSON only:
{"slice":"...","findings":[{"title":"[Area] ...","priority":1,"cpp":"File.cpp:lines","leftover":"file.rs:lines","live":"file.rs:lines","player_visible":"...","not_duplicate_of":"..."}]}
Empty findings array if live-host parity. Max 4 findings. Highest player-visible first.
