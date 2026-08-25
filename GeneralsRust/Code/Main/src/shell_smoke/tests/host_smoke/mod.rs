//! Host smoke residual assertions (full ShellSmokeResult pack), split by theme.
//!
//! `run_shell_smoke` is invoked once; helpers only assert the shared result.

pub use super::*;

mod core;
mod presentation;
mod waves_113_182;
mod waves_183_243;
mod waves_244_304;
mod waves_305_365;
mod waves_366_425;
mod waves_426_487;
mod waves_488_547;
mod waves_548_608;
mod waves_609_669;
mod waves_670_730;
mod waves_72_112;
mod waves_731_788;

use self::core::assert_core;
use self::presentation::assert_presentation;
use self::waves_72_112::assert_waves_72_112;
use self::waves_113_182::assert_waves_113_182;
use self::waves_183_243::assert_waves_183_243;
use self::waves_244_304::assert_waves_244_304;
use self::waves_305_365::assert_waves_305_365;
use self::waves_366_425::assert_waves_366_425;
use self::waves_426_487::assert_waves_426_487;
use self::waves_488_547::assert_waves_488_547;
use self::waves_548_608::assert_waves_548_608;
use self::waves_609_669::assert_waves_609_669;
use self::waves_670_730::assert_waves_670_730;
use self::waves_731_788::assert_waves_731_788;

#[test]
fn host_smoke_applies_skirmish_and_advances_frames() {
    let r = run_shell_smoke(8);
    assert_core(&r);
    assert_presentation(&r);
    assert_waves_72_112(&r);
    assert_waves_113_182(&r);
    assert_waves_183_243(&r);
    assert_waves_244_304(&r);
    assert_waves_305_365(&r);
    assert_waves_366_425(&r);
    assert_waves_426_487(&r);
    assert_waves_488_547(&r);
    assert_waves_548_608(&r);
    assert_waves_609_669(&r);
    assert_waves_670_730(&r);
    assert_waves_731_788(&r);
    assert!(
        !r.playable_claim,
        "shell_host_playable_ok must never flip playable_claim"
    );
}
