//! C++ `GameLogic::xfer` always `xferSnapshot(TheCampaignManager)` after the
//! object list. GameLogic cannot depend on GameClient, so the live manager
//! is captured/applied through these hooks.

use std::sync::Arc;

use super::super::xfer::{Xfer, XferMode, XferStatus, XferVersion};
use super::super::xfer_load::XferLoad;
use super::super::xfer_save::XferSave;
use std::path::PathBuf;

pub use crate::common::system::skirmish_game_info::{
    SKIRMISH_MAX_SLOTS as CHALLENGE_MAX_SLOTS, SkirmishGameInfo as ChallengeGameInfoXfer,
    SkirmishSlot as ChallengeSlotXfer,
};
const SKIRMISH_GAME_INFO_VERSION: XferVersion = 4;

impl ChallengeGameInfoXfer {
    /// C++ `SkirmishGameInfo::xfer` field order (GameInfo.cpp:1490-1588).
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.transfer(&mut SystemFields(xfer))
    }

    /// C++ `SkirmishGameInfo::xfer` (GameInfo.cpp:1488) version-4 byte stream.
    /// Production save hooks must speak this, not bincode.
    pub fn encode_xfer_bytes(&self) -> Vec<u8> {
        let path = unique_skirmish_xfer_path("enc");
        let mut copy = self.clone();
        {
            let mut xfer = XferSave::new();
            if xfer.open(path.to_string_lossy().into_owned()).is_err() {
                return Vec::new();
            }
            if copy.xfer(&mut xfer).is_err() {
                let _ = xfer.close();
                let _ = std::fs::remove_file(&path);
                return Vec::new();
            }
            let _ = xfer.close();
        }
        let bytes = std::fs::read(&path).unwrap_or_default();
        let _ = std::fs::remove_file(&path);
        bytes
    }

    /// Inverse of `encode_xfer_bytes`. Rejects bincode / length-prefixed blobs.
    pub fn decode_xfer_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.first().copied() != Some(SKIRMISH_GAME_INFO_VERSION) || bytes.len() < 16 {
            return None;
        }
        let path = unique_skirmish_xfer_path("dec");
        std::fs::write(&path, bytes).ok()?;
        let mut info = Self::default();
        let decoded = {
            let mut xfer = XferLoad::new();
            if xfer.open(path.to_string_lossy().into_owned()).is_err() {
                None
            } else {
                let ok = info.xfer(&mut xfer).is_ok();
                let _ = xfer.close();
                ok.then_some(info)
            }
        };
        let _ = std::fs::remove_file(&path);
        decoded
    }
}

struct SystemFields<'a>(&'a mut dyn Xfer);
impl crate::common::system::skirmish_game_info::GameInfoFields for SystemFields<'_> {
    type Error = XferStatus;
    fn loading(&self) -> bool {
        self.0.get_xfer_mode() == XferMode::Load
    }
    fn invalid(&self, _: &str) -> XferStatus {
        XferStatus::InvalidData
    }
    fn xfer_version(&mut self, value: &mut u8, maximum: u8) -> Result<(), XferStatus> {
        self.0.xfer_version(value, maximum)
    }
    fn xfer_int(&mut self, value: &mut i32) -> Result<(), XferStatus> {
        self.0.xfer_int(value)
    }
    fn xfer_unsigned_int(&mut self, value: &mut u32) -> Result<(), XferStatus> {
        self.0.xfer_unsigned_int(value)
    }
    fn xfer_unsigned_short(&mut self, value: &mut u16) -> Result<(), XferStatus> {
        self.0.xfer_unsigned_short(value)
    }
    fn xfer_bool(&mut self, value: &mut bool) -> Result<(), XferStatus> {
        self.0.xfer_bool(value)
    }
    fn xfer_ascii_string(&mut self, value: &mut String) -> Result<(), XferStatus> {
        self.0.xfer_ascii_string(value)
    }
    fn xfer_unicode_string(&mut self, value: &mut String) -> Result<(), XferStatus> {
        self.0.xfer_unicode_string(value)
    }
}

fn unique_skirmish_xfer_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "skirmish_lobby_xfer_{}_{}_{}.bin",
        label,
        std::process::id(),
        stamp
    ))
}

/// C++ `CampaignManager::xfer` version 5 payload (CampaignManager.cpp).
#[derive(Clone, Debug, Default)]
pub struct CampaignManagerXferState {
    pub campaign: String,
    pub mission: String,
    pub rank_points: i32,
    pub difficulty: i32,
    pub is_challenge: bool,
    pub challenge_info: Option<ChallengeGameInfoXfer>,
    pub generals_template: i32,
}

pub type CampaignManagerCaptureHook = Arc<dyn Fn() -> CampaignManagerXferState + Send + Sync>;
pub type CampaignManagerApplyHook = Arc<dyn Fn(CampaignManagerXferState) + Send + Sync>;
