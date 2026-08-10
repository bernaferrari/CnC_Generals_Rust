//! Early residual honesty packs (waves 72–400). No playable_claim flip.

mod types;
mod waves_72_120;
mod waves_121_160;
mod waves_161_200;
mod waves_201_240;
mod waves_241_280;
mod waves_281_320;
mod waves_321_360;
mod waves_361_400;

pub(super) use types::EarlyHonesty;

pub(super) fn evaluate_early_honesty(pres: &crate::presentation_frame::PresentationFrame, presentation_ok: bool) -> EarlyHonesty {
    let waves_72_120 = waves_72_120::evaluate(pres, presentation_ok);
    let waves_121_160 = waves_121_160::evaluate(pres, presentation_ok);
    let waves_161_200 = waves_161_200::evaluate(pres, presentation_ok);
    let waves_201_240 = waves_201_240::evaluate(pres, presentation_ok);
    let waves_241_280 = waves_241_280::evaluate(pres, presentation_ok);
    let waves_281_320 = waves_281_320::evaluate(pres, presentation_ok);
    let waves_321_360 = waves_321_360::evaluate(pres, presentation_ok);
    let waves_361_400 = waves_361_400::evaluate(pres, presentation_ok);
    EarlyHonesty::from_parts(waves_72_120, waves_121_160, waves_161_200, waves_201_240, waves_241_280, waves_281_320, waves_321_360, waves_361_400)
}
