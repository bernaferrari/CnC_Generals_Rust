//! Build queue system for production
//!
//! Manages the queue of units/upgrades to be produced by a building,
//! with support for priorities, cancellation, and progress tracking.

use crate::common::*;
use std::collections::VecDeque;

/// Priority level for build queue entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuildPriority {
    /// Low priority - background production
    Low = 0,
    /// Normal priority - standard production
    Normal = 1,
    /// High priority - rush production
    High = 2,
    /// Urgent priority - immediate production
    Urgent = 3,
}

impl Default for BuildPriority {
    fn default() -> Self {
        BuildPriority::Normal
    }
}

/// Type of production entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionType {
    /// Producing a unit
    Unit,
    /// Researching an upgrade
    Upgrade,
    /// Special ability or power
    SpecialPower,
}

/// Entry in the build queue
#[derive(Debug, Clone)]
pub struct BuildQueueEntry {
    /// Template name of the unit/upgrade to produce
    pub template_name: String,
    /// Type of production
    pub production_type: ProductionType,
    /// Priority level
    pub priority: BuildPriority,
    /// Cost in credits
    pub cost: i32,
    /// Build time in game frames
    pub build_time: u32,
    /// Time already spent building (in frames)
    pub time_spent: u32,
    /// Player who ordered the production
    pub player_id: ObjectID,
    /// Whether this is a repeat order
    pub is_repeat: bool,
    /// Index in the visual queue (for UI)
    pub queue_index: usize,
}

impl BuildQueueEntry {
    /// Create a new build queue entry
    pub fn new(
        template_name: String,
        production_type: ProductionType,
        cost: i32,
        build_time: u32,
        player_id: ObjectID,
    ) -> Self {
        Self {
            template_name,
            production_type,
            priority: BuildPriority::Normal,
            cost,
            build_time,
            time_spent: 0,
            player_id,
            is_repeat: false,
            queue_index: 0,
        }
    }

    /// Get progress as a percentage (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if self.build_time == 0 {
            return 1.0;
        }
        (self.time_spent as f32) / (self.build_time as f32)
    }

    /// Check if production is complete
    pub fn is_complete(&self) -> bool {
        self.time_spent >= self.build_time
    }

    /// Get remaining time in frames
    pub fn remaining_time(&self) -> u32 {
        self.build_time.saturating_sub(self.time_spent)
    }

    /// Calculate refund amount if cancelled
    pub fn calculate_refund(&self) -> i32 {
        // Refund based on progress - no refund if >50% complete
        if self.progress() > 0.5 {
            0
        } else {
            (self.cost as f32 * (1.0 - self.progress())).round() as i32
        }
    }
}

/// Build queue manager for a production facility
#[derive(Debug)]
pub struct BuildQueue {
    /// Maximum queue size (0 = unlimited)
    max_size: usize,
    /// Current queue of production entries
    queue: VecDeque<BuildQueueEntry>,
    /// Whether production is paused
    paused: bool,
    /// Whether the queue auto-starts production
    auto_start: bool,
}

impl BuildQueue {
    /// Create a new build queue
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            queue: VecDeque::new(),
            paused: false,
            auto_start: true,
        }
    }

    /// Add an entry to the queue
    pub fn enqueue(&mut self, mut entry: BuildQueueEntry) -> Result<(), String> {
        // Check queue size limit
        if self.max_size > 0 && self.queue.len() >= self.max_size {
            return Err("Build queue is full".to_string());
        }

        // Set queue index
        entry.queue_index = self.queue.len();

        // Insert based on priority
        let insert_pos = self
            .queue
            .iter()
            .position(|e| e.priority < entry.priority)
            .unwrap_or(self.queue.len());

        self.queue.insert(insert_pos, entry);

        // Update queue indices
        self.update_indices();

        Ok(())
    }

    /// Remove and return the front entry
    pub fn dequeue(&mut self) -> Option<BuildQueueEntry> {
        let entry = self.queue.pop_front();
        self.update_indices();
        entry
    }

    /// Get the current production item (front of queue) without removing it
    pub fn current(&self) -> Option<&BuildQueueEntry> {
        self.queue.front()
    }

    /// Get mutable reference to current production
    pub fn current_mut(&mut self) -> Option<&mut BuildQueueEntry> {
        self.queue.front_mut()
    }

    /// Cancel a specific queue entry by index
    pub fn cancel(&mut self, index: usize) -> Option<BuildQueueEntry> {
        if index < self.queue.len() {
            let entry = self.queue.remove(index);
            self.update_indices();
            entry
        } else {
            None
        }
    }

    /// Cancel all entries in the queue
    pub fn cancel_all(&mut self) -> Vec<BuildQueueEntry> {
        let entries: Vec<_> = self.queue.drain(..).collect();
        entries
    }

    /// Check if the queue contains an entry matching the production type and template name.
    pub fn contains_template(&self, production_type: ProductionType, name: &str) -> bool {
        self.queue
            .iter()
            .any(|entry| entry.production_type == production_type && entry.template_name == name)
    }

    /// Find the queue index for an entry matching the production type and template name.
    pub fn find_by_template_and_type(
        &self,
        production_type: ProductionType,
        name: &str,
    ) -> Option<usize> {
        self.queue.iter().position(|entry| {
            entry.production_type == production_type && entry.template_name == name
        })
    }

    pub fn cancel_by_template_and_type(
        &mut self,
        production_type: ProductionType,
        name: &str,
    ) -> Option<BuildQueueEntry> {
        let index = self.find_by_template_and_type(production_type, name)?;
        self.cancel(index)
    }

    /// Check if the queue contains any entries of the specified production type.
    pub fn has_production_type(&self, production_type: ProductionType) -> bool {
        self.queue
            .iter()
            .any(|entry| entry.production_type == production_type)
    }

    /// Check if the queue contains any entry of the given production type.
    pub fn contains_type(&self, production_type: ProductionType) -> bool {
        self.queue
            .iter()
            .any(|entry| entry.production_type == production_type)
    }

    /// Get queue size
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Check if queue is full
    pub fn is_full(&self) -> bool {
        self.max_size > 0 && self.queue.len() >= self.max_size
    }

    /// Get all queue entries
    pub fn entries(&self) -> &VecDeque<BuildQueueEntry> {
        &self.queue
    }

    /// Pause production
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume production
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Check if paused
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Toggle auto-start
    pub fn set_auto_start(&mut self, auto_start: bool) {
        self.auto_start = auto_start;
    }

    /// Check auto-start setting
    pub fn is_auto_start(&self) -> bool {
        self.auto_start
    }

    /// Update the current production entry with time spent
    pub fn update_current(&mut self, frames: u32) -> bool {
        if self.paused {
            return false;
        }

        if let Some(entry) = self.current_mut() {
            entry.time_spent = entry.time_spent.saturating_add(frames);
            true
        } else {
            false
        }
    }

    /// Move an entry to a different position in the queue
    pub fn reorder(&mut self, from_index: usize, to_index: usize) -> Result<(), String> {
        if from_index >= self.queue.len() {
            return Err("Invalid source index".to_string());
        }
        if to_index >= self.queue.len() {
            return Err("Invalid destination index".to_string());
        }

        let entry = self.queue.remove(from_index).unwrap();
        self.queue.insert(to_index, entry);
        self.update_indices();

        Ok(())
    }

    /// Get entry by index
    pub fn get(&self, index: usize) -> Option<&BuildQueueEntry> {
        self.queue.get(index)
    }

    /// Get mutable entry by index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut BuildQueueEntry> {
        self.queue.get_mut(index)
    }

    /// Clear the entire queue
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Update queue indices after modifications
    fn update_indices(&mut self) {
        for (i, entry) in self.queue.iter_mut().enumerate() {
            entry.queue_index = i;
        }
    }

    /// Calculate total cost of all queued items
    pub fn total_cost(&self) -> i32 {
        self.queue.iter().map(|e| e.cost).sum()
    }

    /// Calculate total build time remaining
    pub fn total_build_time(&self) -> u32 {
        self.queue.iter().map(|e| e.remaining_time()).sum()
    }

    /// Count entries by type
    pub fn count_by_type(&self, production_type: ProductionType) -> usize {
        self.queue
            .iter()
            .filter(|e| e.production_type == production_type)
            .count()
    }

    /// Find first entry matching template name
    pub fn find_by_template(&self, template_name: &str) -> Option<usize> {
        self.queue
            .iter()
            .position(|e| e.template_name == template_name)
    }
}

impl Default for BuildQueue {
    fn default() -> Self {
        Self::new(0) // Unlimited by default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_queue_basic() {
        let mut queue = BuildQueue::new(5);

        let entry = BuildQueueEntry::new("Tank".to_string(), ProductionType::Unit, 1000, 300, 1);

        assert!(queue.enqueue(entry).is_ok());
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
    }

    #[test]
    fn test_queue_priority() {
        let mut queue = BuildQueue::new(10);

        let mut low = BuildQueueEntry::new("UnitA".to_string(), ProductionType::Unit, 100, 100, 1);
        low.priority = BuildPriority::Low;

        let mut high = BuildQueueEntry::new("UnitB".to_string(), ProductionType::Unit, 200, 200, 1);
        high.priority = BuildPriority::High;

        let mut normal =
            BuildQueueEntry::new("UnitC".to_string(), ProductionType::Unit, 150, 150, 1);
        normal.priority = BuildPriority::Normal;

        queue.enqueue(low).unwrap();
        queue.enqueue(normal).unwrap();
        queue.enqueue(high).unwrap();

        // High priority should be first
        assert_eq!(queue.current().unwrap().template_name, "UnitB");
        queue.dequeue();
        // Normal should be second
        assert_eq!(queue.current().unwrap().template_name, "UnitC");
        queue.dequeue();
        // Low should be last
        assert_eq!(queue.current().unwrap().template_name, "UnitA");
    }

    #[test]
    fn test_queue_cancel() {
        let mut queue = BuildQueue::new(5);

        for i in 0..3 {
            let entry =
                BuildQueueEntry::new(format!("Unit{}", i), ProductionType::Unit, 100, 100, 1);
            queue.enqueue(entry).unwrap();
        }

        assert_eq!(queue.len(), 3);

        let cancelled = queue.cancel(1);
        assert!(cancelled.is_some());
        assert_eq!(cancelled.unwrap().template_name, "Unit1");
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_progress_tracking() {
        let mut entry =
            BuildQueueEntry::new("Tank".to_string(), ProductionType::Unit, 1000, 100, 1);

        assert_eq!(entry.progress(), 0.0);
        assert_eq!(entry.remaining_time(), 100);

        entry.time_spent = 50;
        assert_eq!(entry.progress(), 0.5);
        assert_eq!(entry.remaining_time(), 50);

        entry.time_spent = 100;
        assert_eq!(entry.progress(), 1.0);
        assert!(entry.is_complete());
        assert_eq!(entry.remaining_time(), 0);
    }

    #[test]
    fn test_refund_calculation() {
        let mut entry =
            BuildQueueEntry::new("Tank".to_string(), ProductionType::Unit, 1000, 100, 1);

        // At 0% progress, full refund
        assert_eq!(entry.calculate_refund(), 1000);

        // At 25% progress, 75% refund
        entry.time_spent = 25;
        assert_eq!(entry.calculate_refund(), 750);

        // At 50% progress, 50% refund
        entry.time_spent = 50;
        assert_eq!(entry.calculate_refund(), 500);

        // At 51% progress, no refund
        entry.time_spent = 51;
        assert_eq!(entry.calculate_refund(), 0);
    }

    #[test]
    fn test_queue_max_size() {
        let mut queue = BuildQueue::new(2);

        let entry1 = BuildQueueEntry::new("Unit1".to_string(), ProductionType::Unit, 100, 100, 1);
        let entry2 = BuildQueueEntry::new("Unit2".to_string(), ProductionType::Unit, 100, 100, 1);
        let entry3 = BuildQueueEntry::new("Unit3".to_string(), ProductionType::Unit, 100, 100, 1);

        assert!(queue.enqueue(entry1).is_ok());
        assert!(queue.enqueue(entry2).is_ok());
        assert!(queue.is_full());
        assert!(queue.enqueue(entry3).is_err());
    }

    #[test]
    fn test_pause_resume() {
        let mut queue = BuildQueue::new(5);

        let entry = BuildQueueEntry::new("Tank".to_string(), ProductionType::Unit, 1000, 100, 1);
        queue.enqueue(entry).unwrap();

        assert!(!queue.is_paused());
        assert!(queue.update_current(10));

        queue.pause();
        assert!(queue.is_paused());
        assert!(!queue.update_current(10)); // Should not update when paused

        queue.resume();
        assert!(!queue.is_paused());
    }
}
