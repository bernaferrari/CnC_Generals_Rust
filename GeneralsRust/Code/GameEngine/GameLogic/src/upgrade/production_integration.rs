//! Production System Integration
//!
//! Integrates upgrades with the production/build queue system.
//! Handles upgrade research as a production queue item.
//!
//! Original C++ reference: ProductionUpdate.cpp, BuildListInfo.cpp

use super::{PlayerUpgradeManager, UpgradeError, UpgradeResult, UpgradeTemplate};
use crate::common::*;
use std::sync::{Arc, RwLock};

/// Production queue item type for upgrades
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionItemType {
    Unit,
    Building,
    Upgrade,
}

/// Production queue item for an upgrade
#[derive(Debug, Clone)]
pub struct UpgradeProductionItem {
    /// Upgrade template being researched
    pub template: Arc<UpgradeTemplate>,
    /// Start frame
    pub start_frame: u32,
    /// Progress (0.0 to 1.0)
    pub progress: Real,
    /// Whether production is paused
    pub paused: bool,
}

impl UpgradeProductionItem {
    pub fn new(template: Arc<UpgradeTemplate>, start_frame: u32) -> Self {
        Self {
            template,
            start_frame,
            progress: 0.0,
            paused: false,
        }
    }

    /// Update progress based on current frame
    pub fn update(&mut self, current_frame: u32, player: &Player) -> bool {
        if self.paused {
            return false;
        }

        let time_to_build = self.template.calc_time_to_build(player);
        if time_to_build <= 0 {
            self.progress = 1.0;
            return true;
        }

        let elapsed = current_frame.saturating_sub(self.start_frame);
        self.progress = (elapsed as Real) / (time_to_build as Real);

        self.progress >= 1.0
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self, current_frame: u32) {
        if self.paused {
            self.paused = false;
            // Adjust start frame to maintain progress
            let time_to_build = self.template.calc_time_to_build(&Player::default());
            let frames_completed = (self.progress * time_to_build as Real) as u32;
            self.start_frame = current_frame.saturating_sub(frames_completed);
        }
    }

    pub fn get_progress(&self) -> Real {
        self.progress
    }

    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0
    }
}

/// Upgrade production queue
/// Integrates with building production system
pub struct UpgradeProductionQueue {
    /// Queue of upgrades being researched
    queue: Vec<UpgradeProductionItem>,
    /// Maximum queue size (typically 1 for upgrades)
    max_queue_size: usize,
    /// Building ID this queue belongs to
    building_id: ObjectID,
}

impl UpgradeProductionQueue {
    pub fn new(building_id: ObjectID, max_queue_size: usize) -> Self {
        Self {
            queue: Vec::new(),
            max_queue_size,
            building_id,
        }
    }

    /// Add upgrade to production queue
    pub fn enqueue_upgrade(
        &mut self,
        template: Arc<UpgradeTemplate>,
        current_frame: u32,
    ) -> UpgradeResult<()> {
        if self.queue.len() >= self.max_queue_size {
            return Err(UpgradeError::InvalidType);
        }

        let item = UpgradeProductionItem::new(template, current_frame);
        self.queue.push(item);

        Ok(())
    }

    /// Cancel upgrade at index
    pub fn cancel_upgrade(&mut self, index: usize) -> Option<UpgradeProductionItem> {
        if index < self.queue.len() {
            Some(self.queue.remove(index))
        } else {
            None
        }
    }

    /// Update all queued upgrades
    pub fn update(&mut self, current_frame: u32, player: &Player) -> Vec<Arc<UpgradeTemplate>> {
        let mut completed = Vec::new();

        // Only process first item in queue
        if let Some(item) = self.queue.first_mut() {
            if item.update(current_frame, player) {
                completed.push(item.template.clone());
                self.queue.remove(0);
            }
        }

        completed
    }

    /// Get current upgrade being researched
    pub fn get_current(&self) -> Option<&UpgradeProductionItem> {
        self.queue.first()
    }

    /// Get all queued upgrades
    pub fn get_queue(&self) -> &[UpgradeProductionItem] {
        &self.queue
    }

    /// Pause current upgrade
    pub fn pause_current(&mut self) {
        if let Some(item) = self.queue.first_mut() {
            item.pause();
        }
    }

    /// Resume current upgrade
    pub fn resume_current(&mut self, current_frame: u32) {
        if let Some(item) = self.queue.first_mut() {
            item.resume(current_frame);
        }
    }

    /// Clear all queued upgrades
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Get queue size
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// Integration helper for production system
pub struct UpgradeProductionIntegration;

impl UpgradeProductionIntegration {
    /// Queue an upgrade for research at a building
    /// Matches C++ ProductionUpdate handling of upgrade research
    pub fn queue_upgrade_research(
        building_id: ObjectID,
        template: Arc<UpgradeTemplate>,
        player: &mut Player,
        upgrade_manager: &mut PlayerUpgradeManager,
        current_frame: u32,
    ) -> UpgradeResult<()> {
        // Check prerequisites
        if !Self::check_upgrade_prerequisites(&template, player) {
            return Err(UpgradeError::PrerequisitesNotMet(
                template.get_name().to_string(),
            ));
        }

        // Begin upgrade research
        upgrade_manager.begin_upgrade(template, player, current_frame)?;

        log::info!(
            "Building {} queued upgrade research: {}",
            building_id,
            upgrade_manager.get_in_progress_upgrades().len()
        );

        Ok(())
    }

    /// Check if upgrade prerequisites are met
    /// Matches C++ BuildListInfo prerequisite checking
    fn check_upgrade_prerequisites(template: &UpgradeTemplate, player: &Player) -> bool {
        if let Some(tree_guard) = crate::upgrade::prerequisites::get_tech_tree() {
            return tree_guard.can_research(template.get_name_key(), player);
        }

        true
    }

    /// Complete an upgrade research
    /// Matches C++ ProductionUpdate completion handling
    pub fn complete_upgrade_research(
        template: Arc<UpgradeTemplate>,
        player: &mut Player,
        upgrade_manager: &mut PlayerUpgradeManager,
    ) {
        // Grant upgrade to player
        upgrade_manager.grant_upgrade(template.clone(), player);

        log::info!(
            "Completed upgrade research: {} for player {}",
            template.get_name(),
            player.get_id()
        );
    }

    /// Cancel an upgrade research
    /// Matches C++ ProductionUpdate cancellation handling
    pub fn cancel_upgrade_research(
        template: &UpgradeTemplate,
        player: &mut Player,
        upgrade_manager: &mut PlayerUpgradeManager,
        refund_percentage: Real,
    ) -> UpgradeResult<()> {
        upgrade_manager.cancel_upgrade(template.get_name_key(), player, refund_percentage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_template() -> Arc<UpgradeTemplate> {
        let mut template = UpgradeTemplate::new(AsciiString::from("TestUpgrade"));
        template.set_build_time(10.0); // 300 frames
        template.set_cost(1000);
        Arc::new(template)
    }

    #[test]
    fn test_production_item_creation() {
        let template = make_test_template();
        let item = UpgradeProductionItem::new(template, 0);

        assert_eq!(item.progress, 0.0);
        assert!(!item.paused);
        assert!(!item.is_complete());
    }

    #[test]
    fn test_production_item_update() {
        let template = make_test_template();
        let mut item = UpgradeProductionItem::new(template, 0);
        let player = Player::default();

        // Update at half completion
        let completed = item.update(150, &player);
        assert!(!completed);
        assert_eq!(item.progress, 0.5);

        // Update at full completion
        let completed = item.update(300, &player);
        assert!(completed);
        assert!(item.is_complete());
    }

    #[test]
    fn test_production_item_pause_resume() {
        let template = make_test_template();
        let mut item = UpgradeProductionItem::new(template, 0);
        let player = Player::default();

        // Progress to 50%
        item.update(150, &player);
        assert_eq!(item.progress, 0.5);

        // Pause
        item.pause();
        assert!(item.paused);

        // Update while paused (should not progress)
        item.update(200, &player);
        assert_eq!(item.progress, 0.5);

        // Resume and continue
        item.resume(200);
        assert!(!item.paused);
        item.update(350, &player);
        assert!(item.is_complete());
    }

    #[test]
    fn test_production_queue() {
        let mut queue = UpgradeProductionQueue::new(100, 5);
        let template = make_test_template();

        assert!(queue.is_empty());

        let result = queue.enqueue_upgrade(template.clone(), 0);
        assert!(result.is_ok());
        assert_eq!(queue.len(), 1);

        let current = queue.get_current();
        assert!(current.is_some());
    }

    #[test]
    fn test_production_queue_completion() {
        let mut queue = UpgradeProductionQueue::new(100, 5);
        let template = make_test_template();
        let player = Player::default();

        queue.enqueue_upgrade(template, 0).unwrap();

        // Update to completion
        let completed = queue.update(300, &player);
        assert_eq!(completed.len(), 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_production_queue_cancel() {
        let mut queue = UpgradeProductionQueue::new(100, 5);
        let template = make_test_template();

        queue.enqueue_upgrade(template, 0).unwrap();
        assert_eq!(queue.len(), 1);

        let cancelled = queue.cancel_upgrade(0);
        assert!(cancelled.is_some());
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_max_size() {
        let mut queue = UpgradeProductionQueue::new(100, 1);
        let template1 = make_test_template();
        let template2 = make_test_template();

        assert!(queue.enqueue_upgrade(template1, 0).is_ok());
        assert!(queue.enqueue_upgrade(template2, 0).is_err());
    }
}
