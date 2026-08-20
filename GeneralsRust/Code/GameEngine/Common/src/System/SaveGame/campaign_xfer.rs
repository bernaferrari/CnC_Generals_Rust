//! C++ `GameLogic::xfer` always `xferSnapshot(TheCampaignManager)` after the
//! object list. GameLogic cannot depend on GameClient, so the live manager
//! is captured/applied through these hooks.

use std::sync::Arc;

/// C++ `CampaignManager::xfer` version 5 payload (CampaignManager.cpp).
#[derive(Clone, Debug, Default)]
pub struct CampaignManagerXferState {
    pub campaign: String,
    pub mission: String,
    pub rank_points: i32,
    pub difficulty: i32,
    pub is_challenge: bool,
    pub challenge_map: String,
    pub challenge_template: i32,
    pub generals_template: i32,
}

pub type CampaignManagerCaptureHook = Arc<dyn Fn() -> CampaignManagerXferState + Send + Sync>;
pub type CampaignManagerApplyHook = Arc<dyn Fn(CampaignManagerXferState) + Send + Sync>;
