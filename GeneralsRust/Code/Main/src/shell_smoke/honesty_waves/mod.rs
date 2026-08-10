//! Later residual honesty packs (waves 401+). No playable_claim flip.

mod types;
mod waves_401_440;
mod waves_441_480;
mod waves_481_520;
mod waves_521_560;
mod waves_561_600;
mod waves_601_640;
mod waves_641_680;
mod waves_681_720;
mod waves_721_760;
mod waves_761_800;
mod waves_801_840;
mod waves_841_880;
mod waves_881_920;
mod waves_921_941;

pub(super) use types::WaveHonesty;

pub(super) fn evaluate_honesty_waves() -> WaveHonesty {
    let waves_401_440 = waves_401_440::evaluate();
    let waves_441_480 = waves_441_480::evaluate();
    let waves_481_520 = waves_481_520::evaluate();
    let waves_521_560 = waves_521_560::evaluate();
    let waves_561_600 = waves_561_600::evaluate();
    let waves_601_640 = waves_601_640::evaluate();
    let waves_641_680 = waves_641_680::evaluate();
    let waves_681_720 = waves_681_720::evaluate();
    let waves_721_760 = waves_721_760::evaluate();
    let waves_761_800 = waves_761_800::evaluate();
    let waves_801_840 = waves_801_840::evaluate();
    let waves_841_880 = waves_841_880::evaluate();
    let waves_881_920 = waves_881_920::evaluate();
    let waves_921_941 = waves_921_941::evaluate();
    WaveHonesty::from_parts(waves_401_440, waves_441_480, waves_481_520, waves_521_560, waves_561_600, waves_601_640, waves_641_680, waves_681_720, waves_721_760, waves_761_800, waves_801_840, waves_841_880, waves_881_920, waves_921_941)
}
