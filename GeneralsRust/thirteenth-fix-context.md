# Goal
Fix remaining open/in-progress beads on the LIVE host (GeneralsRust/Code/Main). C++ is source of truth. Leftover crate ports exist and may be reused; they are not the live tick unless Main calls them.

# Constraints
- Repo root: /Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main
- C++: GeneralsMD. Live host: GeneralsRust/Code/Main. Leftover crate: GeneralsRust/Code/GameEngine
- Do not port GameNetwork. Do not touch wgpu architecture. playable_claim stays false.
- Skip formatters, linters, and project-wide test suites. Compile only your crate if you must (`cargo check -p generals_main --offline` is enough).
- Close your bead with `bd close <id> --reason "..." --json` only after the live path actually implements the C++ behavior.
- If a leftover API already matches C++, call it from Main. Do not reimplement in a second copy.
- Host coordinates are Y-up, ground plane XZ. C++ is Z-up, ground plane XY. Convert axes when copying C++ vectors.

# Already fixed this drain (do not redo)
- hq-acax3 Battle Bus throw → +Y, no stop_moving wipe, hop integrates Y
- hq-cwobf Attack range → FROM_BOUNDINGSPHERE_2D on XZ minus both radii (`object/rtb.rs`)
- hq-t53k3 DemoTrap slot lock → PRIMARY detonate / SECONDARY proximity / TERTIARY manual (`unit_command_set_weapon_lock`)
- hq-rl2pe ApproachTarget → standoff (min+max)/2 inside min-range, else 0.9*max (`world_tick/attack.rs`)
