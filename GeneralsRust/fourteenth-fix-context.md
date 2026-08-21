# Goal
Fix remaining P1 live-host beads from hq-cev5c. C++ is source of truth.

# Constraints
Repo: /Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main
Live: GeneralsRust/Code/Main. Leftover: GeneralsRust/Code/GameEngine. C++: GeneralsMD
Do not port GameNetwork. Skip formatters/linters/project-wide tests.
Close bead with bd close <id> --reason "..." --json only after live path implements C++.
Host Y-up (XZ ground). Reuse leftover ports.
Unexpected dirty files are other work — adapt, do not revert them.
