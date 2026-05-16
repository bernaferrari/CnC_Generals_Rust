// FILE: upgrade.rs ////////////////////////////////////////////////////////////
// Game upgrade system
// C++ Reference: /GeneralsMD/Code/GameEngine/Source/Common/System/Upgrade.cpp
// C++ Header:   /GeneralsMD/Code/GameEngine/Include/Common/Upgrade.h
///////////////////////////////////////////////////////////////////////////////

use crate::common::system::{Snapshotable, Xfer, XferVersion};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Upgrade {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cost: u32,
    pub prerequisites: Vec<String>,
    pub unlocks: Vec<String>,
}

pub struct UpgradeSystem {
    upgrades: HashMap<String, Upgrade>,
    purchased_upgrades: Vec<String>,
}

impl UpgradeSystem {
    pub fn new() -> Self {
        Self {
            upgrades: HashMap::new(),
            purchased_upgrades: Vec::new(),
        }
    }

    pub fn register_upgrade(&mut self, upgrade: Upgrade) {
        self.upgrades.insert(upgrade.id.clone(), upgrade);
    }

    pub fn can_purchase(&self, upgrade_id: &str) -> bool {
        if let Some(upgrade) = self.upgrades.get(upgrade_id) {
            upgrade
                .prerequisites
                .iter()
                .all(|prereq| self.purchased_upgrades.contains(prereq))
        } else {
            false
        }
    }

    pub fn purchase_upgrade(&mut self, upgrade_id: &str) -> bool {
        if self.can_purchase(upgrade_id)
            && !self.purchased_upgrades.contains(&upgrade_id.to_string())
        {
            self.purchased_upgrades.push(upgrade_id.to_string());
            true
        } else {
            false
        }
    }

    pub fn has_upgrade(&self, upgrade_id: &str) -> bool {
        self.purchased_upgrades.contains(&upgrade_id.to_string())
    }
}

impl Default for UpgradeSystem {
    fn default() -> Self {
        Self::new()
    }
}

// =====================================================================
// C++ Upgrade class - matches C++ Upgrade from Upgrade.h/Upgrade.cpp
// This is the per-object upgrade instance with status tracking,
// separate from the UpgradeSystem shop above.
// C++ Reference: Upgrade.h lines 83-120, Upgrade.cpp lines 53-82
// =====================================================================

/// Upgrade status type - matches C++ UpgradeStatusType (Upgrade.h lines 25-30)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UpgradeStatusType {
    Invalid = 0,
    InProduction = 1,
    Complete = 2,
}

impl Default for UpgradeStatusType {
    fn default() -> Self {
        UpgradeStatusType::Invalid
    }
}

/// C++ Upgrade class - per-object upgrade instance
/// C++ Reference: Upgrade.h lines 83-120
/// Mirrors m_template (UpgradeTemplate*), m_status (UpgradeStatusType)
#[derive(Debug, Clone)]
pub struct CppUpgrade {
    pub template_name: String,
    pub status: UpgradeStatusType,
}

impl CppUpgrade {
    pub fn new(template_name: String) -> Self {
        CppUpgrade {
            template_name,
            status: UpgradeStatusType::Invalid,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.status == UpgradeStatusType::Complete
    }

    pub fn is_in_production(&self) -> bool {
        self.status == UpgradeStatusType::InProduction
    }
}

impl Default for CppUpgrade {
    fn default() -> Self {
        CppUpgrade {
            template_name: String::new(),
            status: UpgradeStatusType::Invalid,
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Snapshotable implementation for CppUpgrade
// C++ Reference: Upgrade.cpp lines 53-82
// ------------------------------------------------------------------------------------------------

impl Snapshotable for CppUpgrade {
    /// CRC - matches C++ Upgrade::crc() (Upgrade.cpp line 53)
    /// C++ implementation is empty.
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    /// Save/Load transfer - matches C++ Upgrade::xfer() (Upgrade.cpp lines 63-74)
    ///
    /// Version Info:
    /// 1: Initial version
    ///
    /// Fields xfer'd (Upgrade.cpp lines 63-74):
    ///   1. status (UpgradeStatusType via xferUser, sizeof=1 byte)
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version: XferVersion = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("CppUpgrade::xfer version error: {}", e))?;

        // C++ line 72: xferUser(&m_status, sizeof(UpgradeStatusType))
        let mut status_byte = self.status as u8;
        xfer.xfer_unsigned_byte(&mut status_byte)
            .map_err(|e| format!("CppUpgrade::xfer status error: {}", e))?;
        self.status = match status_byte {
            0 => UpgradeStatusType::Invalid,
            1 => UpgradeStatusType::InProduction,
            2 => UpgradeStatusType::Complete,
            _ => UpgradeStatusType::Invalid,
        };

        Ok(())
    }

    /// Load post process - matches C++ Upgrade::loadPostProcess() (Upgrade.cpp line 79)
    /// C++ implementation is empty.
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
