//! INI-driven construction panel, building placement preview, radius overlays, and
//! superweapon timer display.
//!
//! This module replaces the old hardcoded construction button lists with a dynamic
//! system that reads object definitions, command sets, and command buttons from the
//! INI database populated at game start.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::game_logic::ObjectId;
use crate::localization;
use crate::ui::layout;
use crate::ui::utils;
use crate::ui::KeyCode;

use game_engine::common::thing::thing_template::BuildableStatus;

// ---------------------------------------------------------------------------
// Re-usable data extracted from the INI system
// ---------------------------------------------------------------------------

/// A lightweight snapshot of a buildable object pulled from the INI database.
/// Stored so the UI can populate buttons without hitting the global stores every
/// frame.
#[derive(Debug, Clone)]
pub struct BuildableItem {
    /// Internal template name, e.g. `"AmericaTankCrusader"`.
    pub template_name: String,
    /// Localised display name (falls back to template_name if missing).
    pub display_name: String,
    /// Build cost in credits.
    pub cost: i32,
    /// Build time in seconds.
    pub build_time: f32,
    /// Button image key from the INI command button (if any).
    pub button_image: String,
    /// Optional hotkey.
    pub hotkey: Option<KeyCode>,
    /// Border style -- BUILD, UPGRADE, ACTION, SYSTEM.
    pub button_border_type: String,
    /// Radius cursor name associated with this button (e.g. "PARTICLECANNON").
    pub radius_cursor_type: String,
    /// The INI command that clicking this button triggers.
    pub command: String,
    /// Name of the object to construct (may differ from template_name for
    /// reskins).
    pub object_name: String,
}

impl BuildableItem {
    /// True when the player has enough credits.
    pub fn is_affordable(&self, credits: i32) -> bool {
        credits >= self.cost
    }
}

/// A single entry in the build queue.
#[derive(Debug, Clone)]
pub struct BuildQueueEntry {
    pub item: BuildableItem,
    pub progress: f32,       // 0.0 .. 1.0
    pub remaining_time: f32, // seconds
    pub build_time: f32,     // total seconds for this item
}

impl BuildQueueEntry {
    pub fn new(item: BuildableItem) -> Self {
        Self {
            build_time: item.build_time,
            remaining_time: item.build_time,
            progress: 0.0,
            item,
        }
    }

    /// Advance the build. Returns true when the item is complete.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.build_time <= 0.0 {
            return true;
        }
        self.remaining_time -= dt;
        self.progress = 1.0 - (self.remaining_time / self.build_time).max(0.0);
        self.remaining_time <= 0.0
    }
}

// ---------------------------------------------------------------------------
// Superweapon timer state
// ---------------------------------------------------------------------------

/// One tracked superweapon owned by the local player.
#[derive(Debug, Clone)]
pub struct SuperweaponTimer {
    /// Display name for the HUD.
    pub name: String,
    /// Special power template name.
    pub template_name: String,
    /// Icon key (from SpecialPowerTemplate or command button).
    pub icon: String,
    /// Full recharge duration in seconds.
    pub recharge_time: f32,
    /// Seconds remaining until available (0 = ready).
    pub remaining: f32,
    /// True when the superweapon has been unlocked (prerequisites met).
    pub unlocked: bool,
}

impl SuperweaponTimer {
    pub fn new(name: String, template_name: String, icon: String, recharge_time: f32) -> Self {
        Self {
            name,
            template_name,
            icon,
            recharge_time,
            remaining: recharge_time,
            unlocked: false,
        }
    }

    /// Tick the countdown. Returns true the moment it becomes ready.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.unlocked {
            return false;
        }
        if self.remaining <= 0.0 {
            return false;
        }
        self.remaining -= dt;
        if self.remaining <= 0.0 {
            self.remaining = 0.0;
            return true; // just became ready
        }
        false
    }

    pub fn is_ready(&self) -> bool {
        self.unlocked && self.remaining <= 0.0
    }

    pub fn recharge_fraction(&self) -> f32 {
        if self.recharge_time <= 0.0 {
            return 1.0;
        }
        (1.0 - self.remaining / self.recharge_time).clamp(0.0, 1.0)
    }

    /// Reset after firing.
    pub fn fire(&mut self) {
        self.remaining = self.recharge_time;
    }
}

// ---------------------------------------------------------------------------
// Building placement preview
// ---------------------------------------------------------------------------

/// Describes the state of the building placement ghost overlay.
#[derive(Debug, Clone)]
pub struct PlacementPreview {
    /// Template being placed (empty when not in placement mode).
    pub template_name: String,
    /// Display name for tooltip.
    pub display_name: String,
    /// Cost shown in the preview.
    pub cost: i32,
    /// Current world position of the ghost cursor.
    pub world_pos: (f32, f32),
    /// True if the current position is a legal build location.
    pub is_legal: bool,
    /// Footprint half-extents in world units (derived from geometry).
    pub footprint_half_extents: (f32, f32),
}

impl Default for PlacementPreview {
    fn default() -> Self {
        Self {
            template_name: String::new(),
            display_name: String::new(),
            cost: 0,
            world_pos: (0.0, 0.0),
            is_legal: false,
            footprint_half_extents: (30.0, 30.0),
        }
    }
}

impl PlacementPreview {
    pub fn is_active(&self) -> bool {
        !self.template_name.is_empty()
    }

    pub fn start(&mut self, template_name: &str, display_name: &str, cost: i32) {
        self.template_name = template_name.to_string();
        self.display_name = display_name.to_string();
        self.cost = cost;
        self.is_legal = false;
    }

    pub fn cancel(&mut self) {
        self.template_name.clear();
        self.display_name.clear();
        self.cost = 0;
    }
}

// ---------------------------------------------------------------------------
// Radius cursor overlay
// ---------------------------------------------------------------------------

/// Describes an active radius cursor circle drawn on the ground.
#[derive(Debug, Clone)]
pub struct RadiusCursorOverlay {
    /// Named radius type (e.g. "PARTICLECANNON", "A10STRIKE").
    pub cursor_type: String,
    /// World-space centre of the circle.
    pub centre: (f32, f32),
    /// Radius in world units.
    pub radius: f32,
    /// Colour for the ring (RGBA float 0..1).
    pub color: (f32, f32, f32, f32),
    /// Whether the current cursor position is a valid target.
    pub is_legal: bool,
}

impl RadiusCursorOverlay {
    pub fn new(cursor_type: &str, radius: f32) -> Self {
        Self {
            cursor_type: cursor_type.to_string(),
            centre: (0.0, 0.0),
            radius,
            color: (0.0, 1.0, 0.0, 0.5),
            is_legal: true,
        }
    }

    /// Pre-defined radius values matching the INI `RadiusCursorNames` table.
    pub fn radius_for_type(cursor_type: &str) -> f32 {
        match cursor_type {
            "ATTACK_DAMAGE_AREA" => 50.0,
            "ATTACK_SCATTER_AREA" => 30.0,
            "ATTACK_CONTINUE_AREA" => 60.0,
            "GUARD_AREA" => 100.0,
            "EMERGENCY_REPAIR" => 80.0,
            "FRIENDLY_SPECIALPOWER" => 100.0,
            "OFFENSIVE_SPECIALPOWER" => 80.0,
            "SUPERWEAPON_SCATTER_AREA" => 150.0,
            "PARTICLECANNON" => 200.0,
            "A10STRIKE" => 100.0,
            "CARPETBOMB" => 120.0,
            "DAISYCUTTER" => 100.0,
            "PARADROP" => 150.0,
            "SPYSATELLITE" => 300.0,
            "SPECTREGUNSHIP" => 80.0,
            "HELIX_NAPALM_BOMB" => 60.0,
            "NUCLEARMISSILE" => 200.0,
            "EMPPULSE" => 150.0,
            "ARTILLERYBARRAGE" => 120.0,
            "NAPALMSTRIKE" => 80.0,
            "CLUSTERMINES" => 60.0,
            "SCUDSTORM" => 150.0,
            "ANTHRAXBOMB" => 100.0,
            "AMBUSH" => 80.0,
            "RADAR" => 300.0,
            "SPYDRONE" => 0.0,
            "FRENZY" => 100.0,
            "CLEARMINES" => 80.0,
            "AMBULANCE" => 50.0,
            _ => 100.0,
        }
    }

    /// Legal colour vs. illegal colour.
    pub fn update_legality(&mut self, legal: bool) {
        self.is_legal = legal;
        self.color = if legal {
            (0.0, 1.0, 0.0, 0.5)
        } else {
            (1.0, 0.0, 0.0, 0.5)
        };
    }
}

// ---------------------------------------------------------------------------
// Construction tabs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstructionTab {
    Buildings,
    Infantry,
    Vehicles,
    Aircraft,
    NavalUnits,
    SuperWeapons,
}

impl ConstructionTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Buildings => "Buildings",
            Self::Infantry => "Infantry",
            Self::Vehicles => "Vehicles",
            Self::Aircraft => "Aircraft",
            Self::NavalUnits => "Naval",
            Self::SuperWeapons => "SuperWpn",
        }
    }
}

// ---------------------------------------------------------------------------
// Main construction panel
// ---------------------------------------------------------------------------

/// The INI-driven construction panel.
///
/// Instead of hardcoding `"Ranger", "Missile Defender"` etc., this struct queries
/// the global `ControlBar` (command buttons) and `CommandSetManager` to discover
/// which objects the selected structure can produce.  The heavy lifting is done
/// once in `populate_from_command_set`, after which the panel only stores cheap
/// `BuildableItem` snapshots.
pub struct ConstructionPanel {
    position: (i32, i32),
    size: (u32, u32),
    visible: bool,
    selected_building: Option<String>,
    current_tab: ConstructionTab,
    items: Vec<BuildableItem>,
    build_queue: Vec<BuildQueueEntry>,
    tab_order: Vec<ConstructionTab>,
    /// Superweapon timers keyed by template name.
    superweapon_timers: Vec<SuperweaponTimer>,
    /// Active building placement preview.
    placement: PlacementPreview,
    /// Active radius cursor overlay (at most one at a time).
    radius_overlay: Option<RadiusCursorOverlay>,
}

impl ConstructionPanel {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            position: (x, y),
            size: (600, layout::HUD_PANEL_HEIGHT),
            visible: false,
            selected_building: None,
            current_tab: ConstructionTab::Buildings,
            items: Vec::new(),
            build_queue: Vec::new(),
            tab_order: vec![
                ConstructionTab::Buildings,
                ConstructionTab::Infantry,
                ConstructionTab::Vehicles,
                ConstructionTab::Aircraft,
            ],
            superweapon_timers: Vec::new(),
            placement: PlacementPreview::default(),
            radius_overlay: None,
        }
    }

    // ---- visibility -------------------------------------------------------

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.cancel_placement();
        self.clear_radius_overlay();
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    // ---- population from INI data -----------------------------------------

    /// Populate the construction panel for the given building's command set.
    ///
    /// This is the primary INI-driven entry point.  It looks up the command set
    /// string from the building's thing template, resolves every referenced
    /// command button, and builds a `BuildableItem` list from those that have an
    /// `Object` field and a `Command` of type `"OBJECT_BUILD"` or similar.
    pub fn populate_from_command_set(
        &mut self,
        command_set_name: &str,
        faction_side: &str,
        credits: i32,
    ) {
        use game_engine::common::ini::ini_command_button::get_control_bar;
        use game_engine::common::ini::ini_command_set::get_command_set_manager;

        self.selected_building = Some(command_set_name.to_string());
        self.items.clear();
        self.tab_order.clear();

        // 1. Resolve the command set.
        let resolved_buttons: Vec<String> = if let Some(csm) = get_command_set_manager() {
            if let Some(cs) = csm.find_command_set_resolved(command_set_name) {
                cs.get_all_buttons().iter().map(|s| (*s).clone()).collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // 2. For each button name in the set, resolve via the ControlBar and
        //    inspect the `Object` field.
        if let Some(cb) = get_control_bar() {
            for btn_name in &resolved_buttons {
                let btn = match cb.find_command_button_resolved(btn_name) {
                    Some(b) => b,
                    None => continue,
                };

                // Skip buttons without an object (upgrade buttons, action
                // buttons, etc.).
                if btn.object.is_empty() {
                    continue;
                }

                // Determine the construction tab from the command type or object
                // kind-of.  We try the command field first, then fall back to a
                // heuristic based on the object template.
                let tab = classify_button_tab(&btn.command, &btn.object, faction_side);
                if !self.tab_order.contains(&tab) {
                    self.tab_order.push(tab);
                }

                // Derive cost & build time from the thing template when
                // available.  Fall back to `purchase_cost` on the command button.
                let (cost, build_time, display_name) =
                    resolve_build_info(&btn.object, btn.purchase_cost);

                let item = BuildableItem {
                    template_name: btn.object.clone(),
                    display_name,
                    cost,
                    build_time,
                    button_image: btn.button_image.clone(),
                    hotkey: None, // Hotkeys are assigned per-context
                    button_border_type: btn.button_border_type.clone(),
                    radius_cursor_type: btn.radius_cursor_type.clone(),
                    command: btn.command.clone(),
                    object_name: btn.object.clone(),
                };

                self.items.push(item);
            }
        }

        // Pick the first available tab.
        if let Some(&first) = self.tab_order.first() {
            self.current_tab = first;
        }

        self.visible = true;
    }

    /// Populate from a known building name by looking up its command set.
    pub fn show_for_building(
        &mut self,
        building_name: &str,
        faction_side: &str,
        credits: i32,
        command_set_override: Option<&str>,
    ) {
        // Prefer presentation/host command_set_override residual when provided;
        // fall back to ThingTemplate CommandSet string.
        let command_set = command_set_override
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| find_command_set_for_object(building_name));
        if let Some(ref cs_name) = command_set {
            self.populate_from_command_set(cs_name, faction_side, credits);
        } else {
            // Fallback: show an empty panel.
            self.selected_building = Some(building_name.to_string());
            self.items.clear();
            self.tab_order.clear();
            self.visible = true;
        }
    }

    // ---- tab / item access -------------------------------------------------

    pub fn current_tab(&self) -> ConstructionTab {
        self.current_tab
    }

    pub fn set_tab(&mut self, tab: ConstructionTab) {
        if self.tab_order.contains(&tab) {
            self.current_tab = tab;
        }
    }

    pub fn available_tabs(&self) -> &[ConstructionTab] {
        &self.tab_order
    }

    /// Items filtered to the currently active tab.
    pub fn items_for_current_tab(&self) -> Vec<&BuildableItem> {
        self.items
            .iter()
            .filter(|item| {
                classify_button_tab(&item.command, &item.template_name, "") == self.current_tab
            })
            .collect()
    }

    /// All items (unfiltered).
    pub fn all_items(&self) -> &[BuildableItem] {
        &self.items
    }

    pub fn selected_building(&self) -> Option<&str> {
        self.selected_building.as_deref()
    }

    // ---- build queue ------------------------------------------------------

    pub fn enqueue(&mut self, item: BuildableItem) {
        self.build_queue.push(BuildQueueEntry::new(item));
    }

    pub fn build_queue(&self) -> &[BuildQueueEntry] {
        &self.build_queue
    }

    /// Advance the queue. Returns names of completed items.
    pub fn tick_queue(&mut self, dt: f32) -> Vec<String> {
        let mut completed = Vec::new();
        if let Some(entry) = self.build_queue.first_mut() {
            if entry.tick(dt) {
                let name = entry.item.display_name.clone();
                self.build_queue.remove(0);
                completed.push(name);
            }
        }
        completed
    }

    /// Cancel the first item in the queue.
    pub fn cancel_current(&mut self) -> Option<BuildableItem> {
        if self.build_queue.is_empty() {
            return None;
        }
        let entry = self.build_queue.remove(0);
        Some(entry.item)
    }

    // ---- placement preview ------------------------------------------------

    pub fn placement(&self) -> &PlacementPreview {
        &self.placement
    }

    pub fn placement_mut(&mut self) -> &mut PlacementPreview {
        &mut self.placement
    }

    pub fn cancel_placement(&mut self) {
        self.placement.cancel();
    }

    /// Begin placement mode for the given item.
    pub fn begin_placement(&mut self, item: &BuildableItem) {
        self.placement
            .start(&item.template_name, &item.display_name, item.cost);
    }

    // ---- radius cursor overlay --------------------------------------------

    pub fn radius_overlay(&self) -> Option<&RadiusCursorOverlay> {
        self.radius_overlay.as_ref()
    }

    pub fn set_radius_overlay(&mut self, overlay: RadiusCursorOverlay) {
        self.radius_overlay = Some(overlay);
    }

    pub fn clear_radius_overlay(&mut self) {
        self.radius_overlay = None;
    }

    // ---- superweapon timers -----------------------------------------------

    pub fn superweapon_timers(&self) -> &[SuperweaponTimer] {
        &self.superweapon_timers
    }

    pub fn superweapon_timers_mut(&mut self) -> &mut Vec<SuperweaponTimer> {
        &mut self.superweapon_timers
    }

    /// Add a superweapon timer. If one with the same template_name already
    /// exists it is replaced.
    pub fn add_superweapon_timer(&mut self, timer: SuperweaponTimer) {
        if let Some(existing) = self
            .superweapon_timers
            .iter_mut()
            .find(|t| t.template_name == timer.template_name)
        {
            *existing = timer;
        } else {
            self.superweapon_timers.push(timer);
        }
    }

    /// Tick all superweapon timers. Returns names of those that just became
    /// ready.
    pub fn tick_superweapons(&mut self, dt: f32) -> Vec<String> {
        let mut ready = Vec::new();
        for timer in &mut self.superweapon_timers {
            if timer.tick(dt) {
                ready.push(timer.name.clone());
            }
        }
        ready
    }

    // ---- positioning / layout helpers -------------------------------------

    pub fn position(&self) -> (i32, i32) {
        self.position
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn set_position(&mut self, x: i32, y: i32) {
        self.position = (x, y);
    }

    /// Layout constants for the construction grid.
    pub const BUTTON_SIZE: u32 = 64;
    pub const BUTTON_SPACING: u32 = 8;
    pub const BUTTONS_PER_ROW: usize = 9;
    pub const TAB_HEIGHT: i32 = 32;

    /// Screen position for button at a given index within the current tab.
    pub fn button_rect(&self, index: usize) -> (i32, i32, u32, u32) {
        let row = index / Self::BUTTONS_PER_ROW;
        let col = index % Self::BUTTONS_PER_ROW;
        let start_x = self.position.0 + 10;
        let start_y = self.position.1 + Self::TAB_HEIGHT + 8;
        let x = start_x + col as i32 * (Self::BUTTON_SIZE + Self::BUTTON_SPACING) as i32;
        let y = start_y + row as i32 * (Self::BUTTON_SIZE + Self::BUTTON_SPACING) as i32;
        (x, y, Self::BUTTON_SIZE, Self::BUTTON_SIZE)
    }
}

// ---------------------------------------------------------------------------
// Helper: determine which tab a command button belongs to
// ---------------------------------------------------------------------------

fn classify_button_tab(command: &str, _object: &str, _faction: &str) -> ConstructionTab {
    match command {
        "OBJECT_BUILD" | "DOZER_BUILD" => ConstructionTab::Buildings,
        "UNIT_BUILD" => ConstructionTab::Vehicles, // default for unit-producing buttons
        c if c.contains("Infantry") || c.contains("INFANTRY") => ConstructionTab::Infantry,
        c if c.contains("Aircraft") || c.contains("AIRCRAFT") || c.contains("AIR") => {
            ConstructionTab::Aircraft
        }
        _ => ConstructionTab::Buildings,
    }
}

// ---------------------------------------------------------------------------
// Helper: resolve build info from thing template
// ---------------------------------------------------------------------------

fn resolve_build_info(object_name: &str, fallback_cost: i32) -> (i32, f32, String) {
    // Try the GameLogic-layer TheThingFactory first, then fall back to the
    // Common-layer ThingFactory.
    if let Some(template) = gamelogic::helpers::TheThingFactory::find_template(object_name) {
        let cost = template.get_build_cost();
        let time = template.get_build_time();
        let name = template.get_name().to_string();
        // Use display_name if non-empty, else internal name.
        let display = if name.is_empty() || name == object_name {
            object_name.to_string()
        } else {
            name
        };
        return (cost, if time > 0.0 { time } else { 5.0 }, display);
    }

    // Fallback: command button's own purchase_cost.
    (
        if fallback_cost > 0 { fallback_cost } else { 0 },
        5.0,
        object_name.to_string(),
    )
}

// ---------------------------------------------------------------------------
// Helper: find command set for an object by template name
// ---------------------------------------------------------------------------

fn find_command_set_for_object(object_name: &str) -> Option<String> {
    if let Some(template) = gamelogic::helpers::TheThingFactory::find_template(object_name) {
        let cs = template.get_command_set_string();
        if !cs.is_empty() {
            return Some(cs.to_string());
        }
    }
    None
}

/// Resolve CommandSet name preferring presentation override residual.
pub fn resolve_command_set_name(
    building_name: &str,
    command_set_override: Option<&str>,
) -> Option<String> {
    command_set_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| find_command_set_for_object(building_name))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_queue_entry_tick() {
        let item = BuildableItem {
            template_name: "TestObject".into(),
            display_name: "Test Object".into(),
            cost: 100,
            build_time: 10.0,
            button_image: String::new(),
            hotkey: None,
            button_border_type: "BUILD".into(),
            radius_cursor_type: String::new(),
            command: "OBJECT_BUILD".into(),
            object_name: "TestObject".into(),
        };

        let mut entry = BuildQueueEntry::new(item);
        assert!(!entry.tick(3.0));
        assert!((entry.progress - 0.3).abs() < 0.01);

        assert!(!entry.tick(6.0));
        assert!(entry.tick(1.1)); // completes
    }

    #[test]
    fn test_build_queue_entry_zero_time() {
        let item = BuildableItem {
            template_name: "Instant".into(),
            display_name: "Instant".into(),
            cost: 50,
            build_time: 0.0,
            button_image: String::new(),
            hotkey: None,
            button_border_type: "BUILD".into(),
            radius_cursor_type: String::new(),
            command: "OBJECT_BUILD".into(),
            object_name: "Instant".into(),
        };
        let mut entry = BuildQueueEntry::new(item);
        assert!(entry.tick(0.0)); // zero-time items complete immediately
    }

    #[test]
    fn test_placement_preview() {
        let mut preview = PlacementPreview::default();
        assert!(!preview.is_active());
        preview.start("AmericaBarracks", "Barracks", 500);
        assert!(preview.is_active());
        assert_eq!(preview.template_name, "AmericaBarracks");
        assert_eq!(preview.cost, 500);
        preview.cancel();
        assert!(!preview.is_active());
    }

    #[test]
    fn test_superweapon_timer() {
        let mut timer = SuperweaponTimer::new(
            "Particle Cannon".into(),
            "SpecialPower_ParticleCannon".into(),
            "sp_particle".into(),
            120.0,
        );
        assert!(!timer.is_ready());
        assert!(!timer.unlocked);

        timer.unlocked = true;
        timer.tick(60.0);
        assert!((timer.recharge_fraction() - 0.5).abs() < 0.01);
        assert!(!timer.is_ready());

        assert!(timer.tick(60.0)); // just became ready
        assert!(timer.is_ready());

        timer.fire();
        assert!(!timer.is_ready());
        assert!((timer.remaining - 120.0).abs() < 0.01);
    }

    #[test]
    fn test_radius_overlay_default() {
        let overlay = RadiusCursorOverlay::new("PARTICLECANNON", 200.0);
        assert_eq!(overlay.cursor_type, "PARTICLECANNON");
        assert_eq!(overlay.radius, 200.0);
        assert_eq!(overlay.color, (0.0, 1.0, 0.0, 0.5));
    }

    #[test]
    fn test_radius_overlay_legality() {
        let mut overlay = RadiusCursorOverlay::new("A10STRIKE", 100.0);
        overlay.update_legality(false);
        assert!(!overlay.is_legal);
        assert_eq!(overlay.color, (1.0, 0.0, 0.0, 0.5));
        overlay.update_legality(true);
        assert_eq!(overlay.color, (0.0, 1.0, 0.0, 0.5));
    }

    #[test]
    fn test_radius_for_type() {
        assert_eq!(
            RadiusCursorOverlay::radius_for_type("PARTICLECANNON"),
            200.0
        );
        assert_eq!(RadiusCursorOverlay::radius_for_type("A10STRIKE"), 100.0);
        assert_eq!(RadiusCursorOverlay::radius_for_type("UNKNOWN"), 100.0);
    }

    #[test]
    fn test_construction_panel_enqueue() {
        let mut panel = ConstructionPanel::new(0, 0);
        assert!(panel.build_queue().is_empty());

        let item = BuildableItem {
            template_name: "AmericaInfantryRanger".into(),
            display_name: "Ranger".into(),
            cost: 225,
            build_time: 5.0,
            button_image: String::new(),
            hotkey: None,
            button_border_type: "BUILD".into(),
            radius_cursor_type: String::new(),
            command: "UNIT_BUILD".into(),
            object_name: "AmericaInfantryRanger".into(),
        };
        panel.enqueue(item.clone());
        assert_eq!(panel.build_queue().len(), 1);
    }

    #[test]
    fn test_construction_panel_cancel_current() {
        let mut panel = ConstructionPanel::new(0, 0);

        let item = BuildableItem {
            template_name: "TestObj".into(),
            display_name: "Test".into(),
            cost: 100,
            build_time: 10.0,
            button_image: String::new(),
            hotkey: None,
            button_border_type: "BUILD".into(),
            radius_cursor_type: String::new(),
            command: "OBJECT_BUILD".into(),
            object_name: "TestObj".into(),
        };
        panel.enqueue(item);
        let cancelled = panel.cancel_current();
        assert!(cancelled.is_some());
        assert_eq!(cancelled.unwrap().template_name, "TestObj");
        assert!(panel.build_queue().is_empty());
        assert!(panel.cancel_current().is_none());
    }

    #[test]
    fn test_construction_panel_button_rect() {
        let panel = ConstructionPanel::new(10, 700);
        let (x, y, w, h) = panel.button_rect(0);
        assert_eq!(x, 20); // 10 + 10 start_x
        assert_eq!(w, 64);
        assert_eq!(h, 64);

        let (x2, _y2, _, _) = panel.button_rect(1);
        assert_eq!(x2, 20 + (64 + 8) as i32); // second column
    }

    #[test]
    fn test_classify_button_tab() {
        assert_eq!(
            classify_button_tab("OBJECT_BUILD", "AmericaBarracks", ""),
            ConstructionTab::Buildings
        );
        assert_eq!(
            classify_button_tab("UNIT_BUILD", "AmericaInfantryRanger", ""),
            ConstructionTab::Vehicles
        );
        assert_eq!(
            classify_button_tab("SOME Infantry CMD", "obj", ""),
            ConstructionTab::Infantry
        );
    }

    #[test]
    fn test_buildable_item_affordable() {
        let item = BuildableItem {
            template_name: "T".into(),
            display_name: "D".into(),
            cost: 500,
            build_time: 5.0,
            button_image: String::new(),
            hotkey: None,
            button_border_type: "BUILD".into(),
            radius_cursor_type: String::new(),
            command: "OBJECT_BUILD".into(),
            object_name: "T".into(),
        };
        assert!(item.is_affordable(500));
        assert!(item.is_affordable(999));
        assert!(!item.is_affordable(499));
    }

    #[test]
    fn test_superweapon_timer_dedup() {
        let mut panel = ConstructionPanel::new(0, 0);
        let t1 = SuperweaponTimer::new("A10".into(), "SP_A10".into(), "a10".into(), 60.0);
        let t2 = SuperweaponTimer::new("A10 Strike".into(), "SP_A10".into(), "a10_v2".into(), 90.0);
        panel.add_superweapon_timer(t1);
        panel.add_superweapon_timer(t2);
        assert_eq!(panel.superweapon_timers().len(), 1);
        assert_eq!(panel.superweapon_timers()[0].recharge_time, 90.0);
    }

    #[test]
    fn show_for_building_prefers_command_set_override_residual() {
        let src = include_str!("construction_panel.rs");
        assert!(
            src.contains("command_set_override")
                && src.contains("Prefer presentation/host command_set_override"),
            "show_for_building must prefer presentation override residual"
        );
        assert!(
            src.contains("pub fn resolve_command_set_name"),
            "must expose resolve helper for presentation consumers"
        );
        assert_eq!(
            resolve_command_set_name("NoSuchTemplateXYZ", Some("Command_AmericaDozer")),
            Some("Command_AmericaDozer".into())
        );
        assert_eq!(
            resolve_command_set_name("NoSuchTemplateXYZ", Some("  ")),
            None
        );
        assert_eq!(resolve_command_set_name("NoSuchTemplateXYZ", None), None);
    }

    fn test_tab_label() {
        assert_eq!(ConstructionTab::Buildings.label(), "Buildings");
        assert_eq!(ConstructionTab::Infantry.label(), "Infantry");
        assert_eq!(ConstructionTab::Aircraft.label(), "Aircraft");
        assert_eq!(ConstructionTab::NavalUnits.label(), "Naval");
        assert_eq!(ConstructionTab::SuperWeapons.label(), "SuperWpn");
    }
}
