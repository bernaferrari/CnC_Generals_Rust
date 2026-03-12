//! Production prerequisite system
//!
//! This module manages the requirements that must be met before
//! a unit or building can be produced, including required buildings
//! and technologies.
//!
//! ## C++ Reference
//!
//! This is a port of:
//! - `/GeneralsMD/Code/GameEngine/Source/Common/RTS/ProductionPrerequisite.cpp`
//! - `/GeneralsMD/Code/GameEngine/Include/Common/ProductionPrerequisite.h`
//!
//! ## Implementation Status
//!
//! ✅ **Completed:**
//! - Core prerequisite data structures (PrereqUnitRec, PrereqUnitFlags)
//! - Template resolution system (resolveNames)
//! - Object counting by type (calcNumPrereqUnitsOwned)
//! - Science requirement checking (is_satisfied)
//! - Prerequisite validation logic (is_satisfied)
//! - Add/remove prerequisite methods
//! - Build facility template queries
//! - Requirements list generation (getRequiresList)
//!
//! ⏳ **Pending Dependencies (wired via callbacks where possible):**
//! - Player object counting integration (set via `set_prereq_object_counter`)
//! - GameText system integration for localized error messages
//!
//! ## Architecture Notes
//!
//! Prerequisites use a combination of AND/OR logic:
//! - Units without OR flag = AND (all required)
//! - Units with UNIT_OR_WITH_PREV = OR (grouped with previous)
//!
//! Example: (Barracks OR WarFactory) AND TechCenter
//! - Entry 0: Barracks (no flag)
//! - Entry 1: WarFactory (OR flag set)
//! - Entry 2: TechCenter (no flag)

use super::{
    handles::ThingTemplateHandle,
    player::Player,
    science::{ScienceAccess, ScienceType},
};
use crate::common::{
    system::{Snapshotable, Xfer},
    thing::thing_factory::get_thing_factory,
};
use std::sync::OnceLock;

/// Maximum number of prerequisites that can be efficiently processed
///
/// Matches C++ ProductionPrerequisite.h line 84: MAX_PREREQ = 32
/// This is used for static array allocation to avoid heap allocations
/// during prerequisite checking (performance critical operation)
pub const MAX_PREREQ: usize = 32;

/// Flags for unit prerequisites
///
/// Matches C++ ProductionPrerequisite.h lines 72-75: enum flags
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrereqUnitFlags {
    flags: u32,
}

impl PrereqUnitFlags {
    /// No special flags
    pub const NONE: u32 = 0;

    /// If set, unit is "or-ed" with previous unit, so that either one's presence satisfies
    /// Matches C++ line 74: UNIT_OR_WITH_PREV = 0x01
    pub const UNIT_OR_WITH_PREV: u32 = 1;

    pub fn new() -> Self {
        Self { flags: Self::NONE }
    }

    pub fn with_or_prev() -> Self {
        Self {
            flags: Self::UNIT_OR_WITH_PREV,
        }
    }

    pub fn has_or_with_prev(&self) -> bool {
        (self.flags & Self::UNIT_OR_WITH_PREV) != 0
    }

    pub fn set_or_with_prev(&mut self, value: bool) {
        if value {
            self.flags |= Self::UNIT_OR_WITH_PREV;
        } else {
            self.flags &= !Self::UNIT_OR_WITH_PREV;
        }
    }
}

impl Default for PrereqUnitFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Record for a unit prerequisite
///
/// Matches C++ ProductionPrerequisite.h lines 77-82: struct PrereqUnitRec
#[derive(Debug, Clone)]
pub struct PrereqUnitRec {
    /// Name of the required unit/building template (cleared after resolution)
    /// Matches C++ line 81: AsciiString name
    pub name: String,

    /// Flags (e.g., OR with previous)
    /// Matches C++ line 80: Int flags
    pub flags: PrereqUnitFlags,

    /// Handle to the resolved template (set during name resolution)
    /// Matches C++ line 79: const ThingTemplate* unit
    pub unit: Option<ThingTemplateHandle>,
}

impl PrereqUnitRec {
    pub fn new(name: String, flags: PrereqUnitFlags) -> Self {
        Self {
            name,
            flags,
            unit: None,
        }
    }
}

/// Production prerequisite manager
///
/// Manages the requirements that must be satisfied before something
/// can be produced, including required buildings and sciences.
///
/// Matches C++ ProductionPrerequisite.h lines 29-89: class ProductionPrerequisite
///
/// ## C++ Implementation Notes
///
/// The C++ version uses two main data structures:
/// - `std::vector<PrereqUnitRec> m_prereqUnits` (line 87)
/// - `ScienceVec m_prereqSciences` (line 88)
///
/// Prerequisites can be combined with AND/OR logic:
/// - Multiple prerequisites with different flags = AND (all required)
/// - Prerequisites with UNIT_OR_WITH_PREV flag = OR (any one satisfies)
///
/// Example: To build a Tank, you need:
/// - (Barracks OR WarFactory) AND TechCenter
///   This would be encoded as:
///   - Barracks (no flag)
///   - WarFactory (OR flag)
///   - TechCenter (no flag)
#[derive(Debug)]
pub struct ProductionPrerequisite {
    /// List of required unit/building templates
    /// Matches C++ line 87: std::vector<PrereqUnitRec> m_prereqUnits
    prereq_units: Vec<PrereqUnitRec>,

    /// List of required sciences/technologies
    /// Matches C++ line 88: ScienceVec m_prereqSciences
    prereq_sciences: Vec<ScienceType>,
}

/// Callback interface for counting owned objects by template.
///
/// `Common` does not own the authoritative object list; `GameLogic` (or another runtime layer)
/// should install an implementation at startup.
pub trait PrereqObjectCounter: Send + Sync {
    /// Count objects owned by `player` that match `templates` (index-aligned), writing counts into
    /// `counts[0..templates.len()]`.
    fn count_objects_by_thing_template(
        &self,
        player: &Player,
        templates: &[ThingTemplateHandle],
        ignore_dead: bool,
        counts: &mut [i32],
    );
}

static PREREQ_OBJECT_COUNTER: OnceLock<Box<dyn PrereqObjectCounter>> = OnceLock::new();

pub fn set_prereq_object_counter(counter: Box<dyn PrereqObjectCounter>) -> Result<(), String> {
    PREREQ_OBJECT_COUNTER
        .set(counter)
        .map_err(|_| "PrereqObjectCounter already set".to_string())
}

impl ProductionPrerequisite {
    /// Create a new ProductionPrerequisite
    ///
    /// Matches C++ ProductionPrerequisite.cpp lines 30-33: ProductionPrerequisite()
    pub fn new() -> Self {
        let mut prereq = Self {
            prereq_units: Vec::new(),
            prereq_sciences: Vec::new(),
        };
        // C++ line 32: calls init()
        prereq.init();
        prereq
    }

    /// Initialize/reset all prerequisites
    ///
    /// Matches C++ ProductionPrerequisite.cpp lines 41-46: init()
    pub fn init(&mut self) {
        // C++ lines 43-44: Clear both vectors
        self.prereq_units.clear();
        self.prereq_sciences.clear();
    }

    /// Resolve template names to template pointers
    ///
    /// This should be called after all templates have been loaded
    /// to convert string names to actual template references.
    ///
    /// Matches C++ ProductionPrerequisite.cpp lines 49-73: resolveNames
    pub fn resolve_names(&mut self) {
        for unit_rec in &mut self.prereq_units {
            if !unit_rec.name.is_empty() {
                // C++ line 61: Find template at "top most" level (not override sub-templates)
                // This means we conceptually only have one template for any given thing,
                // only the data is overridden.
                //
                let resolved = resolve_thing_template_handle_by_name(&unit_rec.name);
                debug_assert!(
                    resolved.is_some(),
                    "could not find prereq template '{}'",
                    unit_rec.name
                );
                unit_rec.unit = resolved;

                // C++ line 68: Clear the name as we're done with it
                unit_rec.name.clear();
            }
        }
    }

    /// Calculate how many of each prerequisite unit the player owns
    ///
    /// Returns the number of prereq units processed
    ///
    /// Matches C++ ProductionPrerequisite.cpp lines 76-86: calcNumPrereqUnitsOwned
    pub fn calc_num_prereq_units_owned(
        &self,
        player: &Player,
        counts: &mut [i32; MAX_PREREQ],
    ) -> usize {
        // C++ lines 78-81: Limit count to MAX_PREREQ
        let cnt = std::cmp::min(self.prereq_units.len(), MAX_PREREQ);

        // C++ lines 82-83: Build array of ThingTemplate pointers
        // const ThingTemplate *tmpls[MAX_PREREQ];
        // for (int i = 0; i < cnt; i++)
        //     tmpls[i] = m_prereqUnits[i].unit;

        // C++ line 84: player->countObjectsByThingTemplate(cnt, tmpls, false, counts);
        // This counts how many objects the player owns that match each template.
        // The 'false' parameter means don't ignore dead objects.
        //
        // `Common` can't do this directly; it is provided by a runtime callback when available.
        let templates: Vec<ThingTemplateHandle> = self.prereq_units[..cnt]
            .iter()
            .map(|rec| rec.unit.unwrap_or(ThingTemplateHandle::INVALID))
            .collect();
        if let Some(counter) = PREREQ_OBJECT_COUNTER.get() {
            counter.count_objects_by_thing_template(player, &templates, false, &mut counts[..cnt]);
            return cnt;
        }

        for i in 0..cnt {
            counts[i] = 0;
        }

        // C++ line 85: Return the count
        cnt
    }

    /// Get all possible build facility templates (for OR requirements)
    ///
    /// Returns templates that could satisfy the first group of OR'd prerequisites
    ///
    /// Matches C++ ProductionPrerequisite.cpp lines 89-101: getAllPossibleBuildFacilityTemplates
    pub fn get_all_possible_build_facility_templates(
        &self,
        max_templates: usize,
    ) -> Vec<ThingTemplateHandle> {
        let mut templates = Vec::new();

        // C++ lines 92-99: Iterate through prereq units
        for (i, unit_rec) in self.prereq_units.iter().enumerate() {
            // C++ lines 94-95: Stop if this isn't OR'd with previous (new requirement group)
            if i > 0 && !unit_rec.flags.has_or_with_prev() {
                break;
            }

            // C++ lines 96-97: Stop if we've reached the maximum
            if templates.len() >= max_templates {
                break;
            }

            // C++ line 98: Add this template to the list
            if let Some(handle) = unit_rec.unit {
                templates.push(handle);
            }
        }

        // C++ line 100: Return the count
        templates
    }

    /// Get an existing build facility template that the player owns
    ///
    /// Returns the first template from the OR group that the player has
    ///
    /// Matches C++ ProductionPrerequisite.cpp lines 104-120: getExistingBuildFacilityTemplate
    pub fn get_existing_build_facility_template(
        &self,
        player: &Player,
    ) -> Option<ThingTemplateHandle> {
        // C++ line 106: Assert that player is not null
        // DEBUG_ASSERTCRASH(player, ("player may not be null"));

        // C++ line 107: Check if we have any prerequisites
        if self.prereq_units.is_empty() {
            return None;
        }

        // C++ lines 109-110: Calculate how many of each prereq unit the player owns
        let mut own_count = [0i32; MAX_PREREQ];
        let cnt = self.calc_num_prereq_units_owned(player, &mut own_count);

        // C++ lines 111-117: Iterate through the OR group
        for i in 0..cnt {
            // C++ lines 113-114: Stop if this isn't OR'd with previous (new requirement group)
            if i > 0 && !self.prereq_units[i].flags.has_or_with_prev() {
                break;
            }

            // C++ lines 115-116: If the player owns at least one, return this template
            if own_count[i] > 0 {
                if let Some(handle) = self.prereq_units[i].unit {
                    return Some(handle);
                }
            }
        }

        // C++ line 119: Return NULL if no matching facility found
        None
    }

    /// Check if all prerequisites are satisfied by the player
    ///
    /// Matches C++ ProductionPrerequisite.cpp lines 123-160: isSatisfied
    pub fn is_satisfied(&self, player: &Player) -> bool {
        // C++ lines 127-128: Null check (we use references in Rust, so this is implicit)
        // if (!player) return false;

        // C++ lines 130-135: Check all required sciences
        for &science in &self.prereq_sciences {
            // Use the Player's has_science method (via ScienceAccess trait)
            if !player.has_science(science) {
                return false;
            }
        }

        // C++ lines 137-139: Calculate how many of each prereq unit the player owns
        let mut own_count = [0i32; MAX_PREREQ];
        let cnt = self.calc_num_prereq_units_owned(player, &mut own_count);

        // C++ lines 141-149: Handle OR cases (start at index 1)
        // This lumps together OR'd prerequisites so that if the player has ANY of them,
        // the requirement is satisfied
        for i in 1..cnt {
            if self.prereq_units[i].flags.has_or_with_prev() {
                // C++ line 146: Lump together for prerequisite purposes
                own_count[i] += own_count[i - 1];
                // C++ line 147: Flag for "ignore me"
                own_count[i - 1] = -1;
            }
        }

        // C++ lines 151-157: Check all non-ignored requirements
        for i in 0..cnt {
            // C++ lines 153-154: Skip entries marked with the magic "ignore me" flag
            if own_count[i] == -1 {
                continue;
            }
            // C++ lines 155-156: Everything not ignored is required
            if own_count[i] == 0 {
                return false;
            }
        }

        // C++ line 159: All prerequisites satisfied
        true
    }

    /// Add a unit prerequisite
    ///
    /// If `or_with_previous` is true, this unit is an alternate to the
    /// previously added unit (OR relationship). Otherwise, it's required
    /// in addition to other entries (AND relationship).
    ///
    /// Matches C++ ProductionPrerequisite.cpp lines 168-176: addUnitPrereq
    pub fn add_unit_prereq(&mut self, unit_name: String, or_with_previous: bool) {
        // C++ lines 170-172: Create PrereqUnitRec structure
        let flags = if or_with_previous {
            PrereqUnitFlags::with_or_prev()
        } else {
            PrereqUnitFlags::new()
        };

        // C++ line 173: unit = NULL (will be resolved later in resolveNames)
        let info = PrereqUnitRec::new(unit_name, flags);

        // C++ line 174: Push to vector
        self.prereq_units.push(info);
    }

    /// Add multiple unit prerequisites with OR relationship
    ///
    /// The first unit is required, and subsequent units are alternatives (OR'd)
    ///
    /// Matches C++ ProductionPrerequisite.cpp lines 184-193: addUnitPrereq (vector version)
    pub fn add_unit_prereq_group(&mut self, units: &[String]) {
        // C++ line 186: First unit is not OR'd
        let mut or_with_previous = false;

        // C++ lines 187-191: Iterate through units
        for unit in units {
            self.add_unit_prereq(unit.clone(), or_with_previous);
            // C++ line 190: All subsequent units are OR'd with previous
            or_with_previous = true;
        }
    }

    /// Add a science prerequisite
    ///
    /// Matches C++ ProductionPrerequisite.h line 40: addSciencePrereq
    pub fn add_science_prereq(&mut self, science: ScienceType) {
        self.prereq_sciences.push(science);
    }

    /// Get list of unfulfilled requirements as a formatted string
    ///
    /// Matches C++ ProductionPrerequisite.cpp lines 198-289: getRequiresList
    pub fn get_requires_list(&self, player: &Player) -> String {
        // C++ lines 201-203: If player is invalid, return empty string
        // (In Rust we use references, so this is implicit)

        let mut requires_list = String::new();

        // C++ lines 207-209: Calculate how many of each prereq unit the player owns
        let mut own_count = [0i32; MAX_PREREQ];
        let cnt = self.calc_num_prereq_units_owned(player, &mut own_count);

        // C++ lines 212-220: Initialize OR requirements tracking
        let mut or_requirements = [false; MAX_PREREQ];

        // C++ lines 221-230: Account for "or" unit cases, start loop at 1
        for i in 1..cnt {
            if self.prereq_units[i].flags.has_or_with_prev() {
                // C++ line 226: Set the flag for this unit to be "ored" with previous
                or_requirements[i] = true;
                // C++ line 227: Lump 'em together for prereq purposes
                own_count[i] += own_count[i - 1];
                // C++ line 228: Flag for "ignore me"
                own_count[i - 1] = -1;
            }
        }

        // C++ lines 232-269: Check to see if anything is required
        let mut first_requirement = true;
        for i in 0..cnt {
            // C++ line 239: We have an unfulfilled requirement
            if own_count[i] == 0 {
                // C++ lines 241-249: Handle OR requirement display
                if or_requirements[i] {
                    requires_list.push_str(" OR ");
                }

                // C++ lines 251-253: Get the requirement and then its name
                let unit_name = self.prereq_units[i]
                    .unit
                    .and_then(display_name_for_template_handle)
                    .unwrap_or_else(|| "<Building Required>".to_string());

                // C++ lines 255-258: Get command button and translate name
                // (Commented out in C++ too)

                // C++ lines 260-264: Format name appropriately with 'returns' if necessary
                if first_requirement {
                    first_requirement = false;
                } else {
                    requires_list.push('\n');
                }

                // C++ line 267: Add it to the list
                requires_list.push_str(&unit_name);
            }
        }

        // C++ lines 271-286: Check for science requirements
        let mut has_sciences = true;
        for &science in &self.prereq_sciences {
            if !player.has_science(science) {
                has_sciences = false;
                break;
            }
        }

        if !has_sciences {
            if !first_requirement {
                requires_list.push('\n');
            }
            // In C++ this comes from TheGameText->fetch("CONTROLBAR:GeneralsPromotion").
            requires_list.push_str("General's Promotion Required");
        }

        // C++ line 289: Return final list
        requires_list
    }

    /// Get all unit prerequisites
    pub fn get_unit_prereqs(&self) -> &[PrereqUnitRec] {
        &self.prereq_units
    }

    /// Get all science prerequisites
    pub fn get_science_prereqs(&self) -> &[ScienceType] {
        &self.prereq_sciences
    }

    /// Check if there are any prerequisites
    pub fn has_prerequisites(&self) -> bool {
        !self.prereq_units.is_empty() || !self.prereq_sciences.is_empty()
    }

    /// Check if there are any unit prerequisites
    pub fn has_unit_prerequisites(&self) -> bool {
        !self.prereq_units.is_empty()
    }

    /// Check if there are any science prerequisites
    pub fn has_science_prerequisites(&self) -> bool {
        !self.prereq_sciences.is_empty()
    }

    /// Clear all prerequisites
    pub fn clear(&mut self) {
        self.init();
    }

    /// Remove a specific science prerequisite
    pub fn remove_science_prereq(&mut self, science: ScienceType) {
        self.prereq_sciences.retain(|&s| s != science);
    }

    /// Check if a specific science is required
    pub fn requires_science(&self, science: ScienceType) -> bool {
        self.prereq_sciences.contains(&science)
    }
}

impl Default for ProductionPrerequisite {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ProductionPrerequisite {
    fn clone(&self) -> Self {
        Self {
            prereq_units: self.prereq_units.clone(),
            prereq_sciences: self.prereq_sciences.clone(),
        }
    }
}

// ============================================================================
// Serialization Support
// ============================================================================
//
// The C++ version inherits from Snapshot and implements:
// - void crc(Xfer *xfer)           - Checksum for network sync validation
// - void xfer(Xfer *xfer)          - Serialize/deserialize state
// - void loadPostProcess()         - Post-load fixup (resolve pointers, etc.)
//
// These methods are essential for:
// 1. Save/Load game functionality
// 2. Network synchronization in multiplayer
// 3. Replay system
//
//
// The original C++ implementation serializes prerequisite state as part of save/load snapshots.
// In Rust we keep the same behavior: transfer science IDs, prereq unit names, and flags, and
// then resolve template handles on post-load.

impl Snapshotable for ProductionPrerequisite {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut tmp = self.clone();
        tmp.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ uses UnsignedByte (u8) for version - matches C++ parity
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        // Sciences
        xfer.xfer_vec_int(&mut self.prereq_sciences)
            .map_err(|e| e.to_string())?;

        // Units: count + per-record (name, flags)
        let mut unit_count: u16 = self.prereq_units.len().min(u16::MAX as usize) as u16;
        xfer.xfer_unsigned_short(&mut unit_count)
            .map_err(|e| e.to_string())?;

        match xfer.get_xfer_mode() {
            crate::common::system::XferMode::Save | crate::common::system::XferMode::Crc => {
                for rec in self.prereq_units.iter().take(unit_count as usize) {
                    let mut name = rec.name.clone();
                    if name.is_empty() {
                        if let Some(handle) = rec.unit {
                            if let Some(template_name) = name_for_template_handle(handle) {
                                name = template_name;
                            }
                        }
                    }
                    xfer.xfer_ascii_string(&mut name)
                        .map_err(|e| e.to_string())?;

                    let mut flags = rec.flags.flags;
                    xfer.xfer_u32(&mut flags).map_err(|e| e.to_string())?;
                }
            }
            crate::common::system::XferMode::Load => {
                self.prereq_units.clear();
                self.prereq_units.reserve(unit_count as usize);

                for _ in 0..unit_count {
                    let mut name = String::new();
                    xfer.xfer_ascii_string(&mut name)
                        .map_err(|e| e.to_string())?;

                    let mut flags = 0u32;
                    xfer.xfer_u32(&mut flags).map_err(|e| e.to_string())?;

                    self.prereq_units.push(PrereqUnitRec {
                        name,
                        flags: PrereqUnitFlags { flags },
                        unit: None,
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.resolve_names();
        Ok(())
    }
}

fn resolve_thing_template_handle_by_name(name: &str) -> Option<ThingTemplateHandle> {
    let factory_guard = get_thing_factory().ok()?;
    let factory = factory_guard.as_ref()?;
    let tmpl = factory.find_template(name, false)?;
    Some(ThingTemplateHandle::new(tmpl.get_template_id() as u32))
}

fn template_id_from_handle(handle: ThingTemplateHandle) -> Option<u16> {
    let value = handle.value();
    u16::try_from(value).ok()
}

fn display_name_for_template_handle(handle: ThingTemplateHandle) -> Option<String> {
    let template_id = template_id_from_handle(handle)?;
    let factory_guard = get_thing_factory().ok()?;
    let factory = factory_guard.as_ref()?;
    let tmpl = factory.find_by_template_id(template_id)?;
    if tmpl.get_display_name().is_empty() {
        Some(tmpl.get_name().clone())
    } else {
        Some(tmpl.get_display_name().clone())
    }
}

fn name_for_template_handle(handle: ThingTemplateHandle) -> Option<String> {
    let template_id = template_id_from_handle(handle)?;
    let factory_guard = get_thing_factory().ok()?;
    let factory = factory_guard.as_ref()?;
    let tmpl = factory.find_by_template_id(template_id)?;
    Some(tmpl.get_name().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_production_prerequisite_init() {
        let mut prereq = ProductionPrerequisite::new();

        assert!(!prereq.has_prerequisites());
        assert!(!prereq.has_unit_prerequisites());
        assert!(!prereq.has_science_prerequisites());

        prereq.add_unit_prereq("Barracks".to_string(), false);
        prereq.add_science_prereq(123);

        assert!(prereq.has_prerequisites());
        assert!(prereq.has_unit_prerequisites());
        assert!(prereq.has_science_prerequisites());

        prereq.clear();

        assert!(!prereq.has_prerequisites());
    }

    #[test]
    fn test_prereq_unit_flags() {
        let mut flags = PrereqUnitFlags::new();
        assert!(!flags.has_or_with_prev());

        flags.set_or_with_prev(true);
        assert!(flags.has_or_with_prev());

        flags.set_or_with_prev(false);
        assert!(!flags.has_or_with_prev());

        let or_flags = PrereqUnitFlags::with_or_prev();
        assert!(or_flags.has_or_with_prev());
    }

    #[test]
    fn test_unit_prereq_group() {
        let mut prereq = ProductionPrerequisite::new();
        let units = vec![
            "Barracks".to_string(),
            "WarFactory".to_string(),
            "Airfield".to_string(),
        ];

        prereq.add_unit_prereq_group(&units);

        let unit_prereqs = prereq.get_unit_prereqs();
        assert_eq!(unit_prereqs.len(), 3);

        // First unit should not be OR'd
        assert!(!unit_prereqs[0].flags.has_or_with_prev());

        // Subsequent units should be OR'd
        assert!(unit_prereqs[1].flags.has_or_with_prev());
        assert!(unit_prereqs[2].flags.has_or_with_prev());
    }

    #[test]
    fn test_science_prereqs() {
        let mut prereq = ProductionPrerequisite::new();

        prereq.add_science_prereq(100);
        prereq.add_science_prereq(200);

        assert!(prereq.requires_science(100));
        assert!(prereq.requires_science(200));
        assert!(!prereq.requires_science(300));

        prereq.remove_science_prereq(100);

        assert!(!prereq.requires_science(100));
        assert!(prereq.requires_science(200));

        let science_prereqs = prereq.get_science_prereqs();
        assert_eq!(science_prereqs.len(), 1);
        assert_eq!(science_prereqs[0], 200);
    }
}
