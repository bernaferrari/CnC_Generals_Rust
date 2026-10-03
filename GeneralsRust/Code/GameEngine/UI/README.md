# Generals UI model

`generals_ui` owns the original shell transition policies without a renderer, GPU,
live simulation, window registry, audio system, global slot, or synchronization primitive.
The crate uses only `std` and forbids unsafe code.

## Production interface

`AnimateWindowManager<T>` directly owns its animation lists and mutable transition state.
An `AnimationTarget` provides three synchronous geometry operations: position, size, and
position mutation. GameClient supplies its existing GameWindow adapter. Other presentation
backends can supply owned widget state without reproducing animation decisions.

The existing GameClient shell and ControlBar callers use this model through their original
manager methods. The GameClient adapter retains existing Rc/RefCell window ownership;
extracting the model does not claim that the surrounding window tree is instance-isolated.
Frozen simulation presentation values continue to flow toward UI and rendering. This crate
never reads gameplay state or sends gameplay commands.

## Behavioral contract

C++ authorities:

- `GeneralsMD/Code/GameEngine/Source/GameClient/GUI/AnimateWindowManager.cpp`
- `GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ProcessAnimateWindow.cpp`
- `GeneralsMD/Code/GameEngine/Include/GameClient/AnimateWindowManager.h`
- `GeneralsMD/Code/GameEngine/Include/GameClient/ProcessAnimateWindow.h`

The extraction preserves registration order, mandatory-before-optional update order,
mandatory completion accounting, per-update velocity stepping, original speed constants,
delay handling, reverse policy, timed interpolation, reset/rest positions, and spiral sizing.
Vertical slide travel intentionally uses display width as in C++; it is not changed to height.
Each style remains a named source-corresponding module. No easing replacement or frame-rate
conversion is introduced.

`update()` uses the same monotonic clock as the existing port. `update_at()` accepts a supplied
timestamp for deterministic stepping through the production interface. Registration and
reverse initialization still use the existing monotonic clock policy. These are presentation
updates, separate from the simulation's 30 Hz authority.

Existing differences in manager disposal, optional reverse-delay wrapping, and timed
integer rounding remain tracked in hq-x4p78. This structural extraction preserves
the current runtime behavior; it does not claim those discrepancies are fixed.

The regression tests exercise the module with supplied geometry adapters: all slide start
positions, frame stepping, delays, timed completion/reverse, optional completion, spiral size,
constructor inactivity, interleaved independent owners, and reset. A GameClient regression
also drives actual GameWindow instances through this adapter. These tests establish Rust
behavior and C++ source contracts, not original-executable pixel equivalence.

## Scope and verification

This extraction covers existing live shell animations. Layout loading, hit testing, focus,
gadget state, and callback dispatch remain in their original GameClient modules. A complete
replacement UI would also need those contracts, authored asset handling, and platform input.
No parallel UI implementation is added.

Run from `GeneralsRust`:

```sh
cargo test --locked -p generals_ui
cargo test --locked -p game-client-rust --features internal --lib gui::shell::base::tests -- --test-threads=1
cargo check --locked -p generals_main --tests
cargo check --locked -p generals_main --lib --bin generals --target wasm32-unknown-unknown
```
