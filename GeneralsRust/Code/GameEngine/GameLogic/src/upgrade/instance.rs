//! Upgrade Instance System
//!
//! Represents actual upgrade instances in progress or completed.
//! Matches C++ Upgrade from Upgrade.h/.cpp
//!
//! Original C++ Author: Colin Day, March 2002

use super::UpgradeTemplate;
use crate::common::*;
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::Arc;

/// Status of an upgrade instance
/// Matches C++ UpgradeStatusType from Upgrade.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum UpgradeStatus {
    Invalid = 0,
    InProduction = 1,
    Complete = 2,
}

/// An upgrade instance
/// Matches C++ Upgrade from Upgrade.h
#[derive(Debug, Clone)]
pub struct Upgrade {
    /// Template this instance is based on
    template: Arc<UpgradeTemplate>,
    /// Current status
    status: UpgradeStatus,
    /// Progress (0.0 to 1.0)
    progress: Real,
    /// Logic frame when started
    start_frame: u32,
}

impl Upgrade {
    /// Create a new upgrade instance
    /// Matches C++ Upgrade::Upgrade constructor
    pub fn new(template: Arc<UpgradeTemplate>) -> Self {
        Self {
            template,
            status: UpgradeStatus::Invalid,
            progress: 0.0,
            start_frame: 0,
        }
    }

    /// Get the template
    pub fn get_template(&self) -> &UpgradeTemplate {
        &self.template
    }

    /// Get current status
    pub fn get_status(&self) -> UpgradeStatus {
        self.status
    }

    /// Set status
    pub fn set_status(&mut self, status: UpgradeStatus) {
        self.status = status;
    }

    /// Get progress (0.0 to 1.0)
    pub fn get_progress(&self) -> Real {
        self.progress
    }

    /// Set progress
    pub fn set_progress(&mut self, progress: Real) {
        self.progress = progress.clamp(0.0, 1.0);
    }

    /// Get start frame
    pub fn get_start_frame(&self) -> u32 {
        self.start_frame
    }

    /// Set start frame
    pub fn set_start_frame(&mut self, frame: u32) {
        self.start_frame = frame;
    }

    /// Check if upgrade is complete
    pub fn is_complete(&self) -> bool {
        self.status == UpgradeStatus::Complete
    }

    /// Check if upgrade is in production
    pub fn is_in_production(&self) -> bool {
        self.status == UpgradeStatus::InProduction
    }

    /// Update upgrade progress
    /// Returns true if upgrade completed this frame
    pub fn update(&mut self, current_frame: u32, player: &Player) -> bool {
        if self.status != UpgradeStatus::InProduction {
            return false;
        }

        let time_to_build = self.template.calc_time_to_build(player);
        if time_to_build <= 0 {
            self.progress = 1.0;
            self.status = UpgradeStatus::Complete;
            return true;
        }

        let elapsed = current_frame.saturating_sub(self.start_frame);
        self.progress = (elapsed as Real) / (time_to_build as Real);

        if self.progress >= 1.0 {
            self.progress = 1.0;
            self.status = UpgradeStatus::Complete;
            return true;
        }

        false
    }

    /// Begin production of this upgrade
    pub fn begin_production(&mut self, current_frame: u32) {
        self.status = UpgradeStatus::InProduction;
        self.progress = 0.0;
        self.start_frame = current_frame;
    }

    /// Cancel production
    pub fn cancel_production(&mut self) {
        self.status = UpgradeStatus::Invalid;
        self.progress = 0.0;
    }

    /// Force complete (for cheats/debug)
    pub fn force_complete(&mut self) {
        self.status = UpgradeStatus::Complete;
        self.progress = 1.0;
    }
}

impl Snapshotable for Upgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        let mut status_value = self.status as u32;
        xfer.xfer_u32(&mut status_value)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Serialize/deserialize upgrade state
    /// Matches C++ Upgrade::xfer
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version 1
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        // Serialize status
        let mut status_value = self.status as u32;
        xfer.xfer_u32(&mut status_value)
            .map_err(|e| e.to_string())?;

        if xfer.is_reading() {
            self.status = match status_value {
                0 => UpgradeStatus::Invalid,
                1 => UpgradeStatus::InProduction,
                2 => UpgradeStatus::Complete,
                _ => UpgradeStatus::Invalid,
            };
        }

        Ok(())
    }

    /// Post-load processing
    /// Matches C++ Upgrade::loadPostProcess
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_template() -> Arc<UpgradeTemplate> {
        let mut template = UpgradeTemplate::new(AsciiString::from("TestUpgrade"));
        template.set_build_time(10.0); // 10 seconds = 300 frames
        template.set_cost(1000);
        Arc::new(template)
    }

    #[test]
    fn test_upgrade_creation() {
        let template = make_test_template();
        let upgrade = Upgrade::new(template.clone());

        assert_eq!(upgrade.get_status(), UpgradeStatus::Invalid);
        assert_eq!(upgrade.get_progress(), 0.0);
        assert!(!upgrade.is_complete());
    }

    #[test]
    fn test_upgrade_production() {
        let template = make_test_template();
        let mut upgrade = Upgrade::new(template);
        let player = Player::default();

        upgrade.begin_production(0);
        assert_eq!(upgrade.get_status(), UpgradeStatus::InProduction);
        assert!(upgrade.is_in_production());

        // Update halfway through
        let completed = upgrade.update(150, &player);
        assert!(!completed);
        assert_eq!(upgrade.get_progress(), 0.5);

        // Complete
        let completed = upgrade.update(300, &player);
        assert!(completed);
        assert!(upgrade.is_complete());
        assert_eq!(upgrade.get_progress(), 1.0);
    }

    #[test]
    fn test_upgrade_cancel() {
        let template = make_test_template();
        let mut upgrade = Upgrade::new(template);

        upgrade.begin_production(0);
        upgrade.cancel_production();

        assert_eq!(upgrade.get_status(), UpgradeStatus::Invalid);
        assert_eq!(upgrade.get_progress(), 0.0);
    }

    #[test]
    fn test_force_complete() {
        let template = make_test_template();
        let mut upgrade = Upgrade::new(template);

        upgrade.begin_production(0);
        upgrade.force_complete();

        assert!(upgrade.is_complete());
        assert_eq!(upgrade.get_progress(), 1.0);
    }
}
