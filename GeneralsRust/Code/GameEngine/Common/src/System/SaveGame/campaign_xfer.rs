//! C++ `GameLogic::xfer` always `xferSnapshot(TheCampaignManager)` after the
//! object list. GameLogic cannot depend on GameClient, so the live manager
//! is captured/applied through these hooks.

use std::sync::Arc;

use super::super::xfer::{Xfer, XferMode, XferStatus, XferVersion};
use super::super::xfer_load::XferLoad;
use super::super::xfer_save::XferSave;
use std::path::PathBuf;

/// C++ `GameInfo.h` / `NetworkDefs.h`: `MAX_SLOTS = 8`.
pub const CHALLENGE_MAX_SLOTS: usize = 8;
const SKIRMISH_GAME_INFO_VERSION: XferVersion = 4;

/// One slot of C++ `SkirmishGameInfo::xfer` (GameInfo.cpp:1488-1588).
#[derive(Clone, Debug, Default)]
pub struct ChallengeSlotXfer {
    pub state: i32,
    pub name: String,
    pub is_accepted: bool,
    pub is_muted: bool,
    pub color: i32,
    pub start_pos: i32,
    pub player_template: i32,
    pub team_number: i32,
    pub orig_color: i32,
    pub orig_start_pos: i32,
    pub orig_player_template: i32,
}

/// C++ `SkirmishGameInfo::xfer` payload used as `TheChallengeGameInfo`.
#[derive(Clone, Debug)]
pub struct ChallengeGameInfoXfer {
    pub preorder_mask: i32,
    pub crc_interval: i32,
    pub in_game: bool,
    pub in_progress: bool,
    pub surrendered: bool,
    pub game_id: i32,
    pub slots: [ChallengeSlotXfer; CHALLENGE_MAX_SLOTS],
    pub local_ip: u32,
    pub map_name: String,
    pub map_crc: u32,
    pub map_size: u32,
    pub map_mask: i32,
    pub seed: i32,
    pub superweapon_restriction: u16,
    pub starting_cash: u32,
}

impl Default for ChallengeGameInfoXfer {
    fn default() -> Self {
        Self {
            preorder_mask: 0,
            crc_interval: 0,
            in_game: false,
            in_progress: false,
            surrendered: false,
            game_id: 0,
            slots: Default::default(),
            local_ip: 0,
            map_name: String::new(),
            map_crc: 0,
            map_size: 0,
            map_mask: 0,
            seed: 0,
            superweapon_restriction: 0,
            starting_cash: 0,
        }
    }
}

impl ChallengeGameInfoXfer {
    /// C++ `SkirmishGameInfo::xfer` field order (GameInfo.cpp:1490-1588).
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut version = SKIRMISH_GAME_INFO_VERSION;
        xfer.xfer_version(&mut version, SKIRMISH_GAME_INFO_VERSION)?;

        xfer.xfer_int(&mut self.preorder_mask)?;
        xfer.xfer_int(&mut self.crc_interval)?;
        xfer.xfer_bool(&mut self.in_game)?;
        xfer.xfer_bool(&mut self.in_progress)?;
        xfer.xfer_bool(&mut self.surrendered)?;
        xfer.xfer_int(&mut self.game_id)?;

        let mut slot_count = CHALLENGE_MAX_SLOTS as i32;
        xfer.xfer_int(&mut slot_count)?;
        let slots_to_xfer = slot_count.clamp(0, CHALLENGE_MAX_SLOTS as i32) as usize;
        for slot in self.slots.iter_mut().take(slots_to_xfer) {
            xfer.xfer_int(&mut slot.state)?;
            if version >= 2 {
                xfer.xfer_unicode_string(&mut slot.name)?;
            }
            xfer.xfer_bool(&mut slot.is_accepted)?;
            xfer.xfer_bool(&mut slot.is_muted)?;
            xfer.xfer_int(&mut slot.color)?;
            xfer.xfer_int(&mut slot.start_pos)?;
            xfer.xfer_int(&mut slot.player_template)?;
            xfer.xfer_int(&mut slot.team_number)?;
            xfer.xfer_int(&mut slot.orig_color)?;
            xfer.xfer_int(&mut slot.orig_start_pos)?;
            xfer.xfer_int(&mut slot.orig_player_template)?;
        }

        xfer.xfer_unsigned_int(&mut self.local_ip)?;
        xfer.xfer_ascii_string(&mut self.map_name)?;
        xfer.xfer_unsigned_int(&mut self.map_crc)?;
        xfer.xfer_unsigned_int(&mut self.map_size)?;
        xfer.xfer_int(&mut self.map_mask)?;
        xfer.xfer_int(&mut self.seed)?;

        if version >= 3 {
            xfer.xfer_unsigned_short(&mut self.superweapon_restriction)?;
            if version == 3 {
                let mut obsolete = false;
                xfer.xfer_bool(&mut obsolete)?;
            }
            let mut money_version: XferVersion = 1;
            xfer.xfer_version(&mut money_version, 1)?;
            xfer.xfer_unsigned_int(&mut self.starting_cash)?;
        } else if xfer.get_xfer_mode() == XferMode::Load {
            self.superweapon_restriction = 0;
            self.starting_cash = 0;
        }
        Ok(())
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
