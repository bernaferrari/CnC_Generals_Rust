//! Tunnel Tracker System - GLA faction tunnel network management
//!
//! C++ Reference: /GeneralsMD/Code/GameEngine/Source/Common/RTS/TunnelTracker.cpp
//! C++ Header:   /GeneralsMD/Code/GameEngine/Include/Common/TunnelTracker.h
//!
//! The part of a Player's brain that holds the communal passenger list of all tunnels.
//! This has a similar interface to a ContainModule, naturally, but players can't have modules.
//!
//! Author: Graham Smallwood, March 2002 (C++ version)

use crate::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

/// Object ID type alias matching C++ ObjectID (u32)
pub type ObjectID = u32;

/// Invalid object ID constant matching C++ INVALID_ID
pub const INVALID_ID: ObjectID = 0;

/// Standard game logic frames per second (30 FPS)
/// Matches C++ LOGICFRAMES_PER_SECOND
pub const LOGICFRAMES_PER_SECOND: u32 = 30;

// ------------------------------------------------------------------------------------------------
// TunnelTracker - Manages all tunnel networks for a single player
// ------------------------------------------------------------------------------------------------
// C++ class hierarchy: TunnelTracker : public MemoryPoolObject, public Snapshot
//
// Field ordering and defaults match C++ TunnelTracker.h lines 57-64:
//   std::list<ObjectID>    m_tunnelIDs          - tunnel entrance object IDs
//   ContainedItemsList     m_containList         - contained object pointers (Rust: IDs for parity)
//   std::list<ObjectID>    m_xferContainList     - for loading during post-processing
//   Int                    m_containListSize      - size of contain list
//   UnsignedInt            m_tunnelCount          - number of registered tunnels
//   ObjectID               m_curNemesisID         - current enemy target ID
//   UnsignedInt            m_nemesisTimestamp     - frame when nemesis was last updated
// ------------------------------------------------------------------------------------------------

/// Tunnel tracker for managing GLA tunnel networks.
///
/// In C++, the contain list holds raw `Object*` pointers. In Rust, we store `ObjectID`s
/// in `contain_list` for the Common crate (no direct Object dependency). The GameLogic
/// crate's TunnelTracker wraps this with actual object references.
///
/// The `xfer_contain_list` field is only used during save/load and maps directly to
/// C++ `m_xferContainList` which stores ObjectIDs during the load phase before
/// `load_post_process()` resolves them to real pointers.
#[derive(Debug, Clone)]
pub struct TunnelTracker {
    /// I have to try to keep track of these because Caves need to iterate on them.
    /// C++ field: m_tunnelIDs (std::list<ObjectID>)
    tunnel_ids: Vec<ObjectID>,

    /// The contained object ID list.
    /// C++ field: m_containList (ContainedItemsList = std::list<Object*>)
    /// In this Common-crate layer we store IDs; the GameLogic layer resolves to Arc<Object>.
    contain_list: Vec<ObjectID>,

    /// For loading of contain_list during post processing.
    /// C++ field: m_xferContainList (std::list<ObjectID>)
    xfer_contain_list: Vec<ObjectID>,

    /// Size of the contain list (maintained separately for save/load parity).
    /// C++ field: m_containListSize (Int)
    contain_list_size: i32,

    /// How many tunnels have registered so we know when we should kill our contain list.
    /// C++ field: m_tunnelCount (UnsignedInt)
    tunnel_count: u32,

    /// If we have team(s) guarding a tunnel network system, this is one of the current targets.
    /// C++ field: m_curNemesisID (ObjectID)
    cur_nemesis_id: ObjectID,

    /// We only keep nemesis for a couple of seconds.
    /// C++ field: m_nemesisTimestamp (UnsignedInt)
    nemesis_timestamp: u32,
}

impl TunnelTracker {
    /// Create a new tunnel tracker.
    /// Matches C++ TunnelTracker::TunnelTracker() (TunnelTracker.cpp lines 27-33)
    pub fn new() -> Self {
        Self {
            tunnel_ids: Vec::new(),
            contain_list: Vec::new(),
            xfer_contain_list: Vec::new(),
            contain_list_size: 0,
            tunnel_count: 0,
            cur_nemesis_id: INVALID_ID,
            nemesis_timestamp: 0,
        }
    }

    // --------------------------------------------------------------------------------------------
    // Contain list access
    // --------------------------------------------------------------------------------------------

    /// Get the number of contained objects.
    /// Matches C++ getContainCount() (header line 25)
    pub fn get_contain_count(&self) -> u32 {
        self.contain_list_size as u32
    }

    /// Get the maximum tunnel capacity.
    /// Matches C++ TunnelTracker::getContainMax() (TunnelTracker.cpp lines 81-84)
    ///
    /// In C++ this reads `TheGlobalData->m_maxTunnelCapacity`. In the Common crate
    /// we don't have direct access to the singleton, so this returns a default that
    /// callers should override via TheGlobalData when available.
    pub fn get_contain_max(&self) -> i32 {
        // PARITY_NOTE: C++ reads TheGlobalData->m_maxTunnelCapacity (default 0 in GlobalData).
        // The GameLogic wrapper should override this with the singleton value.
        // Returning 0 here matches the C++ default when GlobalData hasn't been initialized.
        0
    }

    /// Get a reference to the contained items ID list.
    /// Matches C++ getContainedItemsList() (header line 27)
    pub fn get_contained_items_list(&self) -> &Vec<ObjectID> {
        &self.contain_list
    }

    /// Check if an object type can use tunnels.
    /// Matches C++ TunnelTracker::isValidContainerFor() (TunnelTracker.cpp lines 132-150)
    ///
    /// `is_aircraft` should be the result of `obj->isKindOf(KINDOF_AIRCRAFT)`.
    /// October 11, 2002 -- Kris: Dustin wants ALL units to be able to use tunnels!
    /// srj sez: um, except aircraft.
    pub fn is_valid_container_for(&self, is_aircraft: bool, check_capacity: bool) -> bool {
        if is_aircraft {
            return false;
        }

        if check_capacity {
            let contain_max = self.get_contain_max();
            let contain_count = self.get_contain_count() as i32;
            contain_count < contain_max
        } else {
            true
        }
    }

    /// Add an object ID to the contain list.
    /// Matches C++ TunnelTracker::addToContainList() (TunnelTracker.cpp lines 153-157)
    pub fn add_to_contain_list(&mut self, object_id: ObjectID) {
        self.contain_list.push(object_id);
        self.contain_list_size += 1;
    }

    /// Remove an object ID from the contain list.
    /// Matches C++ TunnelTracker::removeFromContain() (TunnelTracker.cpp lines 160-171)
    ///
    /// `expose_stealth_units` parameter is preserved for parity but unused in C++ implementation.
    pub fn remove_from_contain(&mut self, object_id: ObjectID, _expose_stealth_units: bool) {
        if let Some(pos) = self.contain_list.iter().position(|&id| id == object_id) {
            self.contain_list.remove(pos);
            self.contain_list_size -= 1;
        }
    }

    /// Check whether an object ID is in the contain list.
    /// Matches C++ TunnelTracker::isInContainer() (TunnelTracker.cpp lines 174-177)
    pub fn is_in_container(&self, object_id: ObjectID) -> bool {
        self.contain_list.contains(&object_id)
    }

    // --------------------------------------------------------------------------------------------
    // Tunnel lifecycle
    // --------------------------------------------------------------------------------------------

    /// Register that a tunnel was created.
    /// Matches C++ TunnelTracker::onTunnelCreated() (TunnelTracker.cpp lines 180-184)
    pub fn on_tunnel_created(&mut self, new_tunnel_id: ObjectID) {
        self.tunnel_count += 1;
        self.tunnel_ids.push(new_tunnel_id);
    }

    /// Register that a tunnel was destroyed.
    /// Matches C++ TunnelTracker::onTunnelDestroyed() (TunnelTracker.cpp lines 187-212)
    ///
    /// Returns a list of object IDs that need to be destroyed (cave-in) if this was the last tunnel,
    /// and optionally the valid tunnel ID that contained objects should be re-assigned to.
    ///
    /// In C++ this directly manipulates objects. In the Common crate, we return the information
    /// so the GameLogic layer can perform the actual object operations.
    pub fn on_tunnel_destroyed(&mut self, dead_tunnel_id: ObjectID) -> TunnelDestroyResult {
        self.tunnel_count = self.tunnel_count.saturating_sub(1);
        self.tunnel_ids.retain(|&id| id != dead_tunnel_id);

        if self.tunnel_count == 0 {
            // Kill everyone in our contain list. Cave in!
            // Matches C++ lines 192-198
            let objects_to_destroy: Vec<ObjectID> = self.contain_list.drain(..).collect();
            self.contain_list_size = 0;

            TunnelDestroyResult::CaveIn {
                objects_to_destroy,
            }
        } else {
            // Otherwise, make sure nobody inside remembers the dead tunnel as the one they entered
            // (scripts need to use so there must be something valid here)
            // Matches C++ lines 200-211
            let valid_tunnel_id = self.tunnel_ids.first().copied();

            // Collect object IDs that need reassignment (those contained by the dead tunnel)
            let objects_to_reassign: Vec<ObjectID> = self
                .contain_list
                .iter()
                .copied()
                .collect();

            TunnelDestroyResult::Reassign {
                dead_tunnel_id,
                valid_tunnel_id,
                objects_to_reassign,
            }
        }
    }

    // --------------------------------------------------------------------------------------------
    // Nemesis tracking
    // --------------------------------------------------------------------------------------------

    /// Update the current nemesis (enemy unit being targeted).
    /// Matches C++ TunnelTracker::updateNemesis() (TunnelTracker.cpp lines 87-100)
    ///
    /// In the Common crate we work with IDs and frame numbers directly.
    /// The GameLogic layer resolves the Object references and calls this with the IDs.
    ///
    /// `target_id` and `target_kind_of` are None if target is null.
    /// `target_is_vehicle/structure/infantry/aircraft` correspond to KindOf checks.
    /// `current_frame` is TheGameLogic->getFrame().
    pub fn update_nemesis(
        &mut self,
        target_id: Option<ObjectID>,
        target_is_vehicle: bool,
        target_is_structure: bool,
        target_is_infantry: bool,
        target_is_aircraft: bool,
        current_frame: u32,
    ) {
        // C++ line 89: if (getCurNemesis()==NULL)
        // We check if nemesis is currently invalid
        if self.get_cur_nemesis_raw(current_frame).is_none() {
            // C++ line 90: if (target)
            if let Some(tid) = target_id {
                // C++ lines 91-93: kindof checks
                if target_is_vehicle || target_is_structure || target_is_infantry || target_is_aircraft
                {
                    self.cur_nemesis_id = tid;
                    self.nemesis_timestamp = current_frame;
                }
            }
        } else {
            // C++ line 97: else if (getCurNemesis()==target)
            if let Some(tid) = target_id {
                if self.cur_nemesis_id == tid {
                    self.nemesis_timestamp = current_frame;
                }
            }
        }
    }

    /// Get current nemesis ID without object resolution.
    /// Matches C++ TunnelTracker::getCurNemesis() (TunnelTracker.cpp lines 103-129)
    ///
    /// In the Common crate, we handle the timestamp expiry and return the ID.
    /// The GameLogic layer handles object lookup and status checks (stealthed, dead, etc.).
    /// Returns None if nemesis is invalid, expired, or should be cleared.
    pub fn get_cur_nemesis(&mut self, current_frame: u32) -> Option<ObjectID> {
        self.get_cur_nemesis_raw(current_frame)
    }

    /// Internal nemesis check with timestamp expiry.
    /// C++ lines 105-111: check INVALID_ID and 4-second timeout
    fn get_cur_nemesis_raw(&mut self, current_frame: u32) -> Option<ObjectID> {
        if self.cur_nemesis_id == INVALID_ID {
            return None;
        }

        // C++ line 108: 4 * LOGICFRAMES_PER_SECOND timeout
        if self.nemesis_timestamp + 4 * LOGICFRAMES_PER_SECOND < current_frame {
            self.cur_nemesis_id = INVALID_ID;
            return None;
        }

        Some(self.cur_nemesis_id)
    }

    /// Mark the current nemesis as invalid (e.g., target is stealthed or dead).
    /// Called by GameLogic layer when object-level checks fail.
    pub fn clear_nemesis(&mut self) {
        self.cur_nemesis_id = INVALID_ID;
    }

    // --------------------------------------------------------------------------------------------
    // Tunnel access
    // --------------------------------------------------------------------------------------------

    /// Get the tunnel count. Used by TunnelContains to check if they are the last one
    /// ahead of deletion time.
    /// Matches C++ friend_getTunnelCount() (header line 42)
    pub fn get_tunnel_count(&self) -> u32 {
        self.tunnel_count
    }

    /// Get reference to the tunnel ID list.
    /// Matches C++ getContainerList() (header line 44)
    pub fn get_tunnel_ids(&self) -> &Vec<ObjectID> {
        &self.tunnel_ids
    }

    // --------------------------------------------------------------------------------------------
    // Healing
    // --------------------------------------------------------------------------------------------

    /// Calculate healing amounts for all contained objects.
    /// Matches C++ TunnelTracker::healObjects() + healObject() (TunnelTracker.cpp lines 224-271)
    ///
    /// Returns a list of (object_id, heal_amount) tuples for the GameLogic layer to apply.
    /// The GameLogic layer performs the actual BodyModule::attemptHealing() calls.
    ///
    /// `get_max_health` closure should return the max health for a given object ID.
    /// `get_contained_by_frame` closure should return the frame when the object entered.
    /// `current_frame` is TheGameLogic->getFrame().
    pub fn calculate_healing<F, G>(
        &self,
        current_frame: u32,
        frames_for_full_heal: f32,
        get_max_health: F,
        get_contained_by_frame: G,
    ) -> Vec<(ObjectID, f32)>
    where
        F: Fn(ObjectID) -> f32,
        G: Fn(ObjectID) -> u32,
    {
        let mut results = Vec::new();

        for &obj_id in &self.contain_list {
            let max_health = get_max_health(obj_id);
            let contained_by_frame = get_contained_by_frame(obj_id);
            let frames_contained = current_frame.saturating_sub(contained_by_frame) as f32;

            let heal_amount = if frames_contained >= frames_for_full_heal {
                // C++ lines 248-256: been in long enough, set to max health
                max_health
            } else {
                // C++ lines 258-269: gradual healing - pretend at zero health and
                // give a sliver as if fully healing over framesForFullHeal frames
                max_health / frames_for_full_heal
            };

            results.push((obj_id, heal_amount));
        }

        results
    }
}

impl Default for TunnelTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------------------------------------------
// Result type for on_tunnel_destroyed
// ------------------------------------------------------------------------------------------------

/// Result of destroying a tunnel, returned by `on_tunnel_destroyed`.
/// The GameLogic layer uses this to perform actual object operations.
#[derive(Debug, Clone)]
pub enum TunnelDestroyResult {
    /// Last tunnel destroyed - cave in! All contained objects must be destroyed.
    CaveIn {
        objects_to_destroy: Vec<ObjectID>,
    },
    /// A tunnel was destroyed but others remain. Objects referencing the dead tunnel
    /// need to be reassigned to a valid one.
    Reassign {
        dead_tunnel_id: ObjectID,
        valid_tunnel_id: Option<ObjectID>,
        objects_to_reassign: Vec<ObjectID>,
    },
}

// ------------------------------------------------------------------------------------------------
// Snapshotable trait implementation
// Matches C++ Snapshot override: crc(), xfer(), loadPostProcess()
// C++ Reference: TunnelTracker.cpp lines 274-388
// ------------------------------------------------------------------------------------------------

impl Snapshotable for TunnelTracker {
    /// CRC check - matches C++ TunnelTracker::crc() (TunnelTracker.cpp lines 276-279)
    /// C++ implementation is empty (no CRC data for tunnel tracker).
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    /// Save/Load transfer - matches C++ TunnelTracker::xfer() (TunnelTracker.cpp lines 286-331)
    ///
    /// Version Info:
    /// 1: Initial version
    ///
    /// Save order:
    ///   1. XferVersion (1 byte)
    ///   2. STL ObjectID list (tunnel IDs) via xferSTLObjectIDList
    ///   3. containListSize (Int)
    ///   4. contain list ObjectIDs (one per entry)
    ///   5. tunnelCount (UnsignedInt)
    ///
    /// Load order is the same, but contain list IDs go to xfer_contain_list
    /// for later resolution in load_post_process().
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // version
        const CURRENT_VERSION: XferVersion = 1;
        let mut version: XferVersion = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("TunnelTracker::xfer version error: {}", e))?;

        // tunnel object id list - matches C++ xfer->xferSTLObjectIDList(&m_tunnelIDs)
        xfer.xfer_stl_object_id_list(&mut self.tunnel_ids)
            .map_err(|e| format!("TunnelTracker::xfer tunnel IDs error: {}", e))?;

        // contain list count - matches C++ xfer->xferInt(&m_containListSize)
        xfer.xfer_int(&mut self.contain_list_size)
            .map_err(|e| format!("TunnelTracker::xfer contain list size error: {}", e))?;

        // contain list data
        // C++ lines 301-326: save writes ObjectIDs from m_containList pointers,
        // load reads ObjectIDs into m_xferContainList for post-processing
        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for &obj_id in &self.contain_list {
                    let mut id = obj_id;
                    xfer.xfer_object_id(&mut id)
                        .map_err(|e| format!("TunnelTracker::xfer contain object ID error: {}", e))?;
                }
            }
            XferMode::Load => {
                // C++ lines 317-325: read into m_xferContainList
                for _ in 0..self.contain_list_size {
                    let mut object_id: ObjectID = 0;
                    xfer.xfer_object_id(&mut object_id)
                        .map_err(|e| format!("TunnelTracker::xfer load contain ID error: {}", e))?;
                    self.xfer_contain_list.push(object_id);
                }
            }
            _ => {
                return Err(format!(
                    "TunnelTracker::xfer - unknown xfer mode {:?}",
                    xfer.get_xfer_mode()
                ));
            }
        }

        // tunnel count - matches C++ xfer->xferUnsignedInt(&m_tunnelCount)
        xfer.xfer_unsigned_int(&mut self.tunnel_count)
            .map_err(|e| format!("TunnelTracker::xfer tunnel count error: {}", e))?;

        Ok(())
    }

    /// Load post-process - matches C++ TunnelTracker::loadPostProcess() (TunnelTracker.cpp lines 336-387)
    ///
    /// Translates the ObjectIDs in xfer_contain_list into entries in contain_list.
    /// In C++, this resolves pointers via TheGameLogic->findObjectByID().
    /// In the Common crate, we just move the IDs from xfer_contain_list to contain_list.
    /// The GameLogic layer should then resolve these IDs to actual objects.
    ///
    /// Returns the list of loaded ObjectIDs that need to be resolved to real objects.
    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ line 340: sanity - contain list should be empty until we post process
        if !self.contain_list.is_empty() {
            return Err(
                "TunnelTracker::loadPostProcess - contain_list should be empty but is not"
                    .to_string(),
            );
        }

        // Move xfer_contain_list contents into contain_list
        // C++ lines 350-383: iterate m_xferContainList, find each object, push to m_containList
        // In Common crate we just transfer the IDs; GameLogic resolves to objects
        self.contain_list = std::mem::take(&mut self.xfer_contain_list);
        self.contain_list_size = self.contain_list.len() as i32;

        // C++ line 386: clear the xfer contain list (done via take above)
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_tracker_creation() {
        let tracker = TunnelTracker::new();
        assert_eq!(tracker.tunnel_count, 0);
        assert_eq!(tracker.contain_list_size, 0);
        assert_eq!(tracker.cur_nemesis_id, INVALID_ID);
        assert_eq!(tracker.nemesis_timestamp, 0);
        assert!(tracker.tunnel_ids.is_empty());
        assert!(tracker.contain_list.is_empty());
        assert!(tracker.xfer_contain_list.is_empty());
    }

    #[test]
    fn test_default_matches_new() {
        let default = TunnelTracker::default();
        let new = TunnelTracker::new();
        assert_eq!(default.tunnel_count, new.tunnel_count);
        assert_eq!(default.contain_list_size, new.contain_list_size);
        assert_eq!(default.cur_nemesis_id, new.cur_nemesis_id);
    }

    #[test]
    fn test_tunnel_lifecycle() {
        let mut tracker = TunnelTracker::new();

        // Create tunnels
        tracker.on_tunnel_created(100);
        assert_eq!(tracker.tunnel_count, 1);
        assert!(tracker.tunnel_ids.contains(&100));

        tracker.on_tunnel_created(200);
        assert_eq!(tracker.tunnel_count, 2);
        assert!(tracker.tunnel_ids.contains(&200));

        // Destroy one tunnel - should reassign
        let result = tracker.on_tunnel_destroyed(100);
        match result {
            TunnelDestroyResult::Reassign {
                dead_tunnel_id,
                valid_tunnel_id,
                ..
            } => {
                assert_eq!(dead_tunnel_id, 100);
                assert_eq!(valid_tunnel_id, Some(200));
            }
            TunnelDestroyResult::CaveIn { .. } => panic!("Expected Reassign, got CaveIn"),
        }
        assert_eq!(tracker.tunnel_count, 1);
        assert!(!tracker.tunnel_ids.contains(&100));

        // Destroy last tunnel - should cave in
        // Add some contained objects first
        tracker.add_to_contain_list(50);
        tracker.add_to_contain_list(51);
        assert_eq!(tracker.get_contain_count(), 2);

        let result = tracker.on_tunnel_destroyed(200);
        match result {
            TunnelDestroyResult::CaveIn {
                objects_to_destroy,
            } => {
                assert_eq!(objects_to_destroy.len(), 2);
                assert!(objects_to_destroy.contains(&50));
                assert!(objects_to_destroy.contains(&51));
            }
            TunnelDestroyResult::Reassign { .. } => panic!("Expected CaveIn, got Reassign"),
        }
        assert_eq!(tracker.tunnel_count, 0);
        assert_eq!(tracker.get_contain_count(), 0);
    }

    #[test]
    fn test_contain_list_operations() {
        let mut tracker = TunnelTracker::new();

        tracker.add_to_contain_list(10);
        tracker.add_to_contain_list(20);
        tracker.add_to_contain_list(30);
        assert_eq!(tracker.get_contain_count(), 3);
        assert!(tracker.is_in_container(20));
        assert!(!tracker.is_in_container(99));

        tracker.remove_from_contain(20, false);
        assert_eq!(tracker.get_contain_count(), 2);
        assert!(!tracker.is_in_container(20));

        // Removing non-existent should be no-op
        tracker.remove_from_contain(999, false);
        assert_eq!(tracker.get_contain_count(), 2);
    }

    #[test]
    fn test_is_valid_container_for() {
        let tracker = TunnelTracker::new();

        // Aircraft can't use tunnels
        assert!(!tracker.is_valid_container_for(true, false));
        assert!(!tracker.is_valid_container_for(true, true));

        // Non-aircraft without capacity check
        assert!(tracker.is_valid_container_for(false, false));
    }

    #[test]
    fn test_nemesis_tracking() {
        let mut tracker = TunnelTracker::new();

        // No nemesis initially
        assert!(tracker.get_cur_nemesis(0).is_none());

        // Set nemesis (vehicle target at frame 100)
        tracker.update_nemesis(Some(42), true, false, false, false, 100);
        assert_eq!(tracker.get_cur_nemesis(100), Some(42));

        // Update timestamp when same target is seen (frame 150)
        tracker.update_nemesis(Some(42), true, false, false, false, 150);
        assert_eq!(tracker.get_cur_nemesis(150), Some(42));

        // Nemesis expires after 4 seconds (120 frames at 30 FPS)
        // Last update at frame 150, so expires after frame 150 + 120 = 270
        assert_eq!(tracker.get_cur_nemesis(269), Some(42)); // still valid
        assert_eq!(tracker.get_cur_nemesis(271), None); // expired (271 > 270)

        // Infantry target
        tracker.update_nemesis(Some(55), false, false, true, false, 300);
        assert_eq!(tracker.get_cur_nemesis(300), Some(55));

        // Structure target
        tracker.update_nemesis(Some(66), false, true, false, false, 400);
        // Previous nemesis expired, so new one should be set
        assert_eq!(tracker.get_cur_nemesis(400), Some(66));

        // Aircraft target
        tracker.cur_nemesis_id = INVALID_ID; // clear for test
        tracker.update_nemesis(Some(77), false, false, false, true, 500);
        assert_eq!(tracker.get_cur_nemesis(500), Some(77));

        // No kindof match - should not set nemesis
        tracker.cur_nemesis_id = INVALID_ID;
        tracker.update_nemesis(Some(88), false, false, false, false, 600);
        assert!(tracker.get_cur_nemesis(600).is_none());

        // None target - should not set nemesis
        tracker.update_nemesis(None, false, false, false, false, 700);
        assert!(tracker.get_cur_nemesis(700).is_none());
    }

    #[test]
    fn test_clear_nemesis() {
        let mut tracker = TunnelTracker::new();
        tracker.update_nemesis(Some(42), true, false, false, false, 100);
        assert_eq!(tracker.get_cur_nemesis(100), Some(42));

        tracker.clear_nemesis();
        assert!(tracker.get_cur_nemesis(100).is_none());
    }

    #[test]
    fn test_calculate_healing() {
        let mut tracker = TunnelTracker::new();
        tracker.add_to_contain_list(10);
        tracker.add_to_contain_list(20);

        let frames_for_full_heal = 300.0; // 10 seconds at 30 FPS
        let current_frame = 400;

        // Object 10: entered at frame 100, been in 300 frames = full heal time
        // Object 20: entered at frame 300, been in 100 frames = partial
        let results = tracker.calculate_healing(
            current_frame,
            frames_for_full_heal,
            |id| if id == 10 { 100.0 } else { 200.0 },
            |id| if id == 10 { 100 } else { 300 },
        );

        assert_eq!(results.len(), 2);

        // Object 10: 400 - 100 = 300 frames, exactly frames_for_full_heal -> max health
        let (id10, heal10) = results.iter().find(|(id, _)| *id == 10).unwrap();
        assert_eq!(*id10, 10);
        assert_eq!(*heal10, 100.0); // max health

        // Object 20: 400 - 300 = 100 frames, partial -> max_health / frames_for_full_heal
        let (id20, heal20) = results.iter().find(|(id, _)| *id == 20).unwrap();
        assert_eq!(*id20, 20);
        assert!((*heal20 - 200.0 / 300.0).abs() < 0.001); // gradual healing
    }

    #[test]
    fn test_load_post_process() {
        let mut tracker = TunnelTracker::new();

        // Simulate xfer load state
        tracker.xfer_contain_list = vec![10, 20, 30];
        tracker.contain_list_size = 3;

        tracker.load_post_process().unwrap();
        assert_eq!(tracker.contain_list, vec![10, 20, 30]);
        assert!(tracker.xfer_contain_list.is_empty());
    }

    #[test]
    fn test_load_post_process_nonempty_contain_fails() {
        let mut tracker = TunnelTracker::new();
        tracker.contain_list = vec![99];
        tracker.xfer_contain_list = vec![10];

        let result = tracker.load_post_process();
        assert!(result.is_err());
    }

    #[test]
    fn test_get_tunnel_ids() {
        let mut tracker = TunnelTracker::new();
        tracker.on_tunnel_created(100);
        tracker.on_tunnel_created(200);

        assert_eq!(tracker.get_tunnel_ids(), &vec![100, 200]);
        assert_eq!(tracker.get_tunnel_count(), 2);
    }
}
