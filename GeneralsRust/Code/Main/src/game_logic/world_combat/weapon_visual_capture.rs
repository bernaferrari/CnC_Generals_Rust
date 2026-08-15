//! Pre-mutation owner seam for C++ `WeaponTemplate::fireWeaponTemplate` visuals.
//!
//! Capture must run before `consume_ammo_on_fire_named`, projectile queue, or
//! stealth break. Barrel read is non-advancing; advance stays in
//! `record_accepted_weapon_discharge`.

use super::super::*;
use crate::presentation_frame::FrozenWeaponVisualSourceGate;

/// Object-local facts sampled before ammo / stealth mutation.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingWeaponVisualDispatchCapture {
    pub weapon_slot: u8,
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

/// C++ `Weapon.cpp:904`: `isProjectileDetonation ? getProjectileDetonateFX(v) : getFireFX(v)`.
pub fn select_weapon_template_fx<'a>(
    is_projectile_detonation: bool,
    fire_fx: &'a str,
    projectile_detonate_fx: &'a str,
) -> &'a str {
    if is_projectile_detonation {
        projectile_detonate_fx
    } else {
        fire_fx
    }
}

fn selected_fx_name_for_weapon(
    weapon_name: Option<&str>,
    veterancy: crate::game_logic::VeterancyLevel,
    is_projectile_detonation: bool,
) -> String {
    let Some(name) = weapon_name else {
        return String::new();
    };
    let fire = crate::game_logic::weapon_bootstrap::host_fire_fx_for_weapon_name_at_veterancy(
        name, veterancy,
    );
    let detonate =
        crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name_at_veterancy(
            name, veterancy,
        );
    select_weapon_template_fx(is_projectile_detonation, &fire, &detonate).to_string()
}

fn host_weapon_recoil_amount(weapon_name: Option<&str>) -> f32 {
    use gamelogic::weapon::with_weapon_store;
    weapon_name
        .and_then(|name| {
            let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
            with_weapon_store(|store| {
                store
                    .find_weapon_template(name)
                    .map(|template| template.weapon_recoil)
            })
            .ok()
            .flatten()
        })
        .unwrap_or(0.0)
}

fn host_weapon_is_contact(weapon_name: Option<&str>, fallback_range: f32) -> bool {
    use gamelogic::weapon::with_weapon_store;
    let range = weapon_name
        .and_then(|name| {
            let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
            with_weapon_store(|store| {
                store
                    .find_weapon_template(name)
                    .map(|template| template.attack_range)
            })
            .ok()
            .flatten()
        })
        .unwrap_or(fallback_range);
    (range - 2.5) < 10.0
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
        let Some(fired_barrel) = self.fired_barrel_for_slot(slot) else {
            self.pending_weapon_visual_capture = None;
            return false;
        };
        let weapon_name = self.weapon_name_for_slot(slot).map(str::to_owned);
        let veterancy = self.experience.level;
        let selected_fx = selected_fx_name_for_weapon(
            weapon_name.as_deref(),
            veterancy,
            is_projectile_detonation,
        );
        let (suspend_fx_frame, ammo, fallback_range) = self
            .weapon_slot(slot)
            .map(|weapon| (weapon.suspend_fx_frame, weapon.ammo, weapon.range))
            .unwrap_or((0, None, 0.0));
        let source_pos = self.get_position();
        let capture = PendingWeaponVisualDispatchCapture {
            weapon_slot: slot,
            fired_barrel,
            source_has_drawable: !self.template_name.trim().is_empty(),
            source_is_stealthed: self.status.stealthed,
            source_is_detected: self.status.detected,
            source_is_disguised: self.status.disguised,
            source_is_mine: self.is_kind_of(KindOf::Mine),
            weapon_plays_fx_when_stealthed: weapon_name
                .as_deref()
                .map(crate::game_logic::weapon_bootstrap::host_play_fx_when_stealthed_for_weapon_name)
                .unwrap_or(false),
            selected_fx_is_present: !selected_fx.is_empty(),
            logic_frame,
            suspend_fx_frame,
            recoil_amount: host_weapon_recoil_amount(weapon_name.as_deref()),
            source_orientation: self.get_orientation(),
            source_pos: [source_pos.x, source_pos.y, source_pos.z],
            target_id,
            target_pos: target_pos.map(|pos| [pos.x, pos.y, pos.z]),
            is_contact_weapon: host_weapon_is_contact(weapon_name.as_deref(), fallback_range),
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
        let selected_fx =
            selected_fx_name_for_weapon(Some(weapon_template_name), self.experience.level, false);
        let source_pos = self.get_position();
        self.pending_weapon_visual_capture = Some(PendingWeaponVisualDispatchCapture {
            weapon_slot: 0,
            fired_barrel,
            source_has_drawable: !self.template_name.trim().is_empty(),
            source_is_stealthed: self.status.stealthed,
            source_is_detected: self.status.detected,
            source_is_disguised: self.status.disguised,
            source_is_mine: self.is_kind_of(KindOf::Mine),
            weapon_plays_fx_when_stealthed:
                crate::game_logic::weapon_bootstrap::host_play_fx_when_stealthed_for_weapon_name(
                    weapon_template_name,
                ),
            selected_fx_is_present: !selected_fx.is_empty(),
            logic_frame,
            suspend_fx_frame,
            recoil_amount: host_weapon_recoil_amount(Some(weapon_template_name)),
            source_orientation: self.get_orientation(),
            source_pos: [source_pos.x, source_pos.y, source_pos.z],
            target_id: None,
            target_pos: Some([target_pos.x, target_pos.y, target_pos.z]),
            is_contact_weapon: host_weapon_is_contact(Some(weapon_template_name), 0.0),
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
