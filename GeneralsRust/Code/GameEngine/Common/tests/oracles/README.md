# INI numeric arithmetic oracle

`generate_ini_numeric.py` produces `ini_numeric_original.txt` by executing exact
method bodies extracted from the original `INI.cpp`. It includes the complete
original `GameCommon.h`. A portability `Lib/BaseType.h` contains only the original
`Real`, `Int`, `UnsignedInt`, `UnsignedShort`, and `Bool` typedefs and `PI` macro.
The script verifies SHA-256 pins for all three input sources before compiling.
Those sources are unchanged from commit
`b167900544fbe5a2ceb3d4c6e87a0d59b23eea1f`.

This is a host C++17 execution of extracted methods, **not** the complete original
INI loader, original Windows executable/compiler, or a retail asset run.
`getNextToken()` is a single-token test supplier. The original `scanReal` and
`scanPercentToReal` bodies call `sscanf("%f")`; the arithmetic is not rewritten in
the harness. Build flags disable fast math and floating-point contraction.

From `GeneralsRust`, with a C++ compiler on `PATH`:

```sh
python3 Code/GameEngine/Common/tests/oracles/generate_ini_numeric.py --output /tmp/ini-numeric-oracle
cmp /tmp/ini-numeric-oracle/ini_numeric_original.txt Code/GameEngine/Common/tests/fixtures/ini_numeric_original.txt
cargo test --offline --locked -j 1 -p game_engine --test ini_numeric_parity
printf '%s\n' 400.0 999.0 | /tmp/ini-numeric-oracle/ini_numeric_original
cargo test --offline --locked -j 1 -p game_engine --test weapon_numeric_parity
```

The 177 complete finite decimal tokens include authored unit examples, both zero
signs, positive and negative values, adjacent normal/subnormal boundary values,
and large finite values. Each row contains seven hexadecimal IEEE-754 binary32
results: real, angle, angular velocity, velocity, acceleration, duration, percent.
The velocity column tests both the public parser and its public conversion helper.
The real, percent, and duration conversion columns are unchanged controls.

The arithmetic specification is `INI.cpp:559–580,1577–1593,1685–1723` and
`GameCommon.h:37–75`: pre-round `PI / 180.0f`, `1.0f / 30.0f`, its square, and the
angular-velocity product before multiplying the authored value. Reassociating
these expressions changes binary32 values and can introduce an intermediate
overflow. `Real` is `float` and `PI` is `3.14159265359f` in `Lib/BaseType.h`.

The integration target also exercises `INI::parse_current_file` with synthetic
GameData and Weapon blocks. Gravity uses acceleration conversion; authored
WeaponSpeed uses velocity conversion; AcceptableAimDelta uses angle conversion.
ScatterRadius and the weapon constructor's already-per-frame speed stay unscaled,
matching the original `GlobalData.cpp:165` and `Weapon.cpp:156–164,251`.
The separate `weapon_numeric_parity` target checks authored weapon speeds of
400 and 999, whose original converted bits are `41555556` and `42053334`.

Token grammar is deliberately outside this arithmetic change. Regression checks
preserve Rust's existing accepted/rejected token branches, including existing
duration suffix support and negative-duration rejection. Those checks do not
claim C++ grammar parity.
