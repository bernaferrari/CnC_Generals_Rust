//! Pre-mutation owner seam for C++ `WeaponTemplate::fireWeaponTemplate` visuals.
//!
//! Capture must run before `consume_ammo_on_fire_named`, projectile queue, or
//! stealth break. Barrel read is non-advancing; advance stays in
//! `record_accepted_weapon_discharge`.

use super::super::*;
use crate::presentation_frame::FrozenWeaponVisualSourceGate;
use generals_sim::weapon::WeaponSlotType;
use generals_sim::weapon::WeaponDefinition as WeaponTemplate;
use std::sync::Arc;

/// Object-local facts sampled before ammo / stealth mutation.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingWeaponVisualDispatchCapture {
    pub weapon_slot: WeaponSlotType,
    pub fired_barrel: u8,
    pub source_has_drawable: bool,
    pub source_is_stealthed: bool,
    pub source_is_detected: bool,
    pub source_is_disguised: bool,
    pub source_is_mine: bool,
    pub weapon_plays_fx_when_stealthed: bool,
    pub selected_fx_is_present: bool,
    pub logic_frame: u32,
    pub suspend_fx_frame: u32,
    pub recoil_amount: f32,
    pub source_orientation: f32,
    pub source_pos: [f32; 3],
    pub target_id: Option<ObjectId>,
    pub target_pos: Option<[f32; 3]>,
    pub is_contact_weapon: bool,
    pub ammo_at_capture: Option<u32>,
    pub stealthed_at_capture: bool,
    pub template_name: String,
    pub draw_state_revision: u64,
    pub model_condition_bits: u128,
    /// C++ `Weapon.cpp:904` selected FXList name after veterancy lookup.
    pub selected_fx_name: String,
    /// C++ `isProjectileDetonation` argument to `fireWeaponTemplate`.
    pub is_projectile_detonation: bool,
    /// Temporary behavior weapons must not advance the Object WeaponSet cursor.
    pub skip_object_barrel_advance: bool,
}

impl PendingWeaponVisualDispatchCapture {
    pub fn source_gate(&self, source_is_locally_controlled: bool) -> FrozenWeaponVisualSourceGate {
        FrozenWeaponVisualSourceGate {
            source_has_drawable: self.source_has_drawable,
            source_is_locally_controlled,
            source_is_stealthed: self.source_is_stealthed,
            source_is_detected: self.source_is_detected,
            source_is_disguised: self.source_is_disguised,
            source_is_mine: self.source_is_mine,
            weapon_plays_fx_when_stealthed: self.weapon_plays_fx_when_stealthed,
            selected_fx_is_present: self.selected_fx_is_present,
            logic_frame: self.logic_frame,
            suspend_fx_frame: self.suspend_fx_frame,
        }
    }
}

pub fn source_is_locally_controlled(
    owner_player_id: Option<u32>,
    local_player_id: Option<u32>,
) -> bool {
    match (owner_player_id, local_player_id) {
        (Some(owner), Some(local)) => owner == local,
        _ => false,
    }
}

pub fn recoil_dir_from_positions(source: Vec3, victim: Vec3, recoil_amount: f32) -> f32 {
    if recoil_amount == 0.0 {
        0.0
    } else {
        (victim.z - source.z).atan2(victim.x - source.x)
    }
}

pub fn geometry_center(position: Vec3, geometry: &GeometryInfo) -> Vec3 {
    let height = (geometry.bounds_max.y - geometry.bounds_min.y).max(0.0);
    Vec3::new(position.x, position.y + height * 0.5, position.z)
}

/// C++ `Weapon.cpp:904` selects one FX list using the firing object's veterancy.
pub(super) fn select_weapon_template_fx(
    rule: &WeaponTemplate,
    veterancy: VeterancyLevel,
    is_projectile_detonation: bool,
) -> &str {
    let fx = if is_projectile_detonation {
        rule.get_projectile_detonate_fx(veterancy)
    } else {
        rule.get_fire_fx(veterancy)
    };
    fx.map_or("", |list| list.name())
}

fn compatibility_weapon_rule(name: &str) -> Option<Arc<WeaponTemplate>> {
    crate::game_logic::weapon_bootstrap::legacy_weapon_definition(name)
}

impl Object {
    /// Snapshot visual-dispatch facts before ammo, projectile, or stealth mutation.
    pub fn capture_pending_weapon_visual_dispatch(
        &mut self,
        slot: u8,
        logic_frame: u32,
        target_id: Option<ObjectId>,
        target_pos: Option<Vec3>,
    ) -> bool {
        self.capture_pending_weapon_visual_dispatch_ex(
            slot,
            logic_frame,
            target_id,
            target_pos,
            false,
        )
    }

    pub fn capture_pending_weapon_visual_dispatch_ex(
        &mut self,
        slot: u8,
        logic_frame: u32,
        target_id: Option<ObjectId>,
        target_pos: Option<Vec3>,
        is_projectile_detonation: bool,
    ) -> bool {
        let Some(weapon_slot) = WeaponSlotType::from_u32(u32::from(slot)) else {
            self.pending_weapon_visual_capture = None;
            return false;
        };
        let Some(fired_barrel) = self.fired_barrel_for_slot(slot) else {
            self.pending_weapon_visual_capture = None;
            return false;
        };
        let (rule, compatibility) = match self.selected_native_weapon_rule(slot) {
            Some(rule) => (Some(rule), false),
            None => (
                self.weapon_name_for_slot(slot).and_then(compatibility_weapon_rule),
                true,
            ),
        };
        let fx_name = rule.as_deref().map_or("", |rule| {
            select_weapon_template_fx(rule, self.experience.level, is_projectile_detonation)
        });
        let selected_fx = if compatibility { fx_name.trim() } else { fx_name }.to_owned();
        let (suspend_fx_frame, ammo, fallback_range) = self
            .weapon_slot(slot)
            .map(|weapon| (weapon.suspend_fx_frame, weapon.ammo, weapon.range))
            .unwrap_or((0, None, 0.0));
        let source_pos = self.get_position();
        let capture = PendingWeaponVisualDispatchCapture {
            weapon_slot,
            fired_barrel,
            source_has_drawable: self.has_drawable,
            source_is_stealthed: self.status.stealthed,
            source_is_detected: self.status.detected,
            source_is_disguised: self.status.disguised,
            source_is_mine: self.is_kind_of(KindOf::Mine),
            weapon_plays_fx_when_stealthed: rule
                .as_ref()
                .is_some_and(|rule| rule.play_fx_when_stealthed),
            selected_fx_is_present: !selected_fx.is_empty(),
            logic_frame,
            suspend_fx_frame,
            recoil_amount: rule.as_ref().map_or(0.0, |rule| rule.weapon_recoil),
            source_orientation: self.get_orientation(),
            source_pos: [source_pos.x, source_pos.y, source_pos.z],
            target_id,
            target_pos: target_pos.map(|pos| [pos.x, pos.y, pos.z]),
            is_contact_weapon: rule.as_deref().map_or_else(
                || (fallback_range - 2.5) < 10.0,
                WeaponTemplate::is_contact_weapon,
            ),
            ammo_at_capture: ammo,
            stealthed_at_capture: self.status.stealthed,
            template_name: self.template_name.clone(),
            draw_state_revision: self.visual_draw_state_revision.max(1),
            model_condition_bits: self.model_condition_bits,
            selected_fx_name: selected_fx,
            is_projectile_detonation,
            skip_object_barrel_advance: false,
        };
        self.pending_weapon_visual_capture = Some(capture);
        true
    }

    pub fn capture_pending_temporary_weapon_visual_dispatch(
        &mut self,
        weapon_template_name: &str,
        current_barrel: i32,
        suspend_fx_frame: u32,
        ammo_in_clip: Option<u32>,
        logic_frame: u32,
        target_pos: Vec3,
    ) -> bool {
        let fired_barrel = u8::try_from(current_barrel.max(0)).unwrap_or(0);
        let rule = compatibility_weapon_rule(weapon_template_name);
        let selected_fx = rule
            .as_deref()
            .map_or("", |rule| {
                select_weapon_template_fx(rule, self.experience.level, false).trim()
            })
            .to_owned();
        let source_pos = self.get_position();
        self.pending_weapon_visual_capture = Some(PendingWeaponVisualDispatchCapture {
            weapon_slot: WeaponSlotType::Primary,
            fired_barrel,
            source_has_drawable: self.has_drawable,
            source_is_stealthed: self.status.stealthed,
            source_is_detected: self.status.detected,
            source_is_disguised: self.status.disguised,
            source_is_mine: self.is_kind_of(KindOf::Mine),
            weapon_plays_fx_when_stealthed: rule
                .as_ref()
                .is_some_and(|rule| rule.play_fx_when_stealthed),
            selected_fx_is_present: !selected_fx.is_empty(),
            logic_frame,
            suspend_fx_frame,
            recoil_amount: rule.as_ref().map_or(0.0, |rule| rule.weapon_recoil),
            source_orientation: self.get_orientation(),
            source_pos: [source_pos.x, source_pos.y, source_pos.z],
            target_id: None,
            target_pos: Some([target_pos.x, target_pos.y, target_pos.z]),
            is_contact_weapon: rule.as_deref().is_none_or(WeaponTemplate::is_contact_weapon),
            ammo_at_capture: ammo_in_clip,
            stealthed_at_capture: self.status.stealthed,
            template_name: self.template_name.clone(),
            draw_state_revision: self.visual_draw_state_revision.max(1),
            model_condition_bits: self.model_condition_bits,
            selected_fx_name: selected_fx,
            is_projectile_detonation: false,
            skip_object_barrel_advance: true,
        });
        true
    }

    pub fn take_pending_weapon_visual_capture(
        &mut self,
    ) -> Option<PendingWeaponVisualDispatchCapture> {
        self.pending_weapon_visual_capture.take()
    }
}
