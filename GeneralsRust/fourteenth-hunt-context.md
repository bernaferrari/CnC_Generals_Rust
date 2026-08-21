# Goal
Find remaining player-visible C++ vs live-host Rust gaps. File beads. Do not implement.

# Constraints
Repo: /Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main
C++: GeneralsMD. Live: GeneralsRust/Code/Main. Leftover: GeneralsRust/Code/GameEngine.
Do not port GameNetwork. wgpu vs DX allowed. Rust safety allowed if behavior matches.
Find-only. No source edits except `bd create`.
Skip formatters/linters/project-wide tests.

# Allowed
wgpu vs DX; Rust enums if values match.

# How to file
bd create "..." --description="..." -t bug -p 1 --deps discovered-from:hq-cev5c --json
Use P1 only if player-visible live-path mismatch vs C++. P2 if leftover-only or rare.
Do not file leftovers that already have live host parity.
Do not file GameNetwork.
Do not file unwrap/safety cleanup.
