# hq-0f6bh CLEARED spot-check (seed=20261103, n=15)
Against C++ Xfer / behavior — NOT "name absent ⇒ fixed".

| id | clear reason | verdict | notes |
|----|--------------|---------|-------|
| hq-oboav | theme_mismatch | KEEP CLEAR | rustfmt chore; not Xfer |
| hq-jp7wp | symbol_absent UPGRADE_MASK_REGISTRY | KEEP CLEAR | registry gone; mask_for_name → UpgradeCenter only; tests unknown/known |
| hq-pzmt6 | theme_mismatch | KEEP CLEAR | OBJECTSHROUD_FOGGED derives Fogged enum (not PartialClear rename-only) |
| hq-31tsu1 | theme_mismatch | KEEP CLEAR | wrapping_add landed + integration tests (notes) |
| hq-1ycsxx | theme_mismatch | KEEP CLEAR | single-tick parachute path landed + fixtures |
| hq-3q15nz | theme_mismatch | KEEP CLEAR | event-owned goals landed (968ebd3f2) + 21 tests |
| hq-7oe2r | theme_mismatch | REOPEN | TeamPrototypePersist still omits production_condition_always_false; C++ TeamPrototype::xfer saves it |
| hq-uqcvb | theme_mismatch | REOPEN | fogged Ghost PartitionData retention still an ownership/lifecycle gap |
| hq-4jy7m | theme_mismatch | REOPEN | Main camo phase still may not bridge StealthUpdate Xfer phase (init/save) |
| hq-q6sza | theme_mismatch | REOPEN | debris frames/FX vs W3DDebrisDraw Xfer still a presentation/Xfer gap |
| hq-3xdoao | theme_mismatch | REOPEN | CostModifierUpgrade still derive-by-name vs authored module (save/latch) |
| hq-4xzj7c | theme_mismatch | KEEP CLEAR* | AI clock ownership — verify briefly; if still presentation-tied REOPEN |
| hq-j0wz0 | theme_mismatch | KEEP CLEAR* | collision owner — not Client Xfer; residual ≠ name-change |
| hq-o7zx9 | theme_mismatch | KEEP CLEAR* | physics visual timing — not save field gap |
| hq-qpg313 | theme_mismatch | REOPEN | global turret fallback for unadmitted templates still named |

* marginal: not "name changed"; theme_mismatch OK for non-Xfer, but not proof of fix.

Reopen set: hq-7oe2r, hq-uqcvb, hq-4jy7m, hq-q6sza, hq-3xdoao, hq-qpg313
