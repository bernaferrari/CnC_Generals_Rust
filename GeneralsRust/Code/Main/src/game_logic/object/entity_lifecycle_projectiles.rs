//! Window projectile / special-object flight residuals (KNOWN_GAPS shrink).
//!
//! C++ persists these via LifetimeUpdate / HeightDie / MissileAIUpdate module
//! snapshots inside Object::xfer v9 (Object.cpp:4264-4356). Pending FX drain
//! queues stay transient and are not in this payload.

use super::Object;
use super::entity_lifecycle_flight::LaterProjectileFlight;
use crate::game_logic::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub(crate) struct ProjectileFlightResiduals {
    pub carpet_bomb_payload: bool,
    pub artillery_barrage_shell: bool,
    pub a10_strike_missile: bool,
    pub leaflet_transport_target: Option<Vec3>,
    pub leaflet_container: bool,
    pub paradrop_transport_target: Option<Vec3>,
    pub paradrop_parachute: bool,
    pub daisy_cutter_bomb: bool,
    pub anthrax_bomb_payload: bool,
    pub sneak_tunnel_start: bool,
    pub cluster_mines_bomb: bool,
    pub emp_pulse_bomb: bool,
    pub emp_pulse_spheroid: bool,
    pub emp_pulse_spheroid_expires_frame: Option<u32>,
    pub particle_trail_remnant: bool,
    pub particle_trail_remnant_expires_frame: Option<u32>,
    pub nuke_radiation_field: bool,
    pub nuke_radiation_field_expires_frame: Option<u32>,
    pub anthrax_toxin_field: bool,
    pub anthrax_toxin_field_expires_frame: Option<u32>,
    pub spectre_howitzer_shell: bool,
    pub spectre_howitzer_shell_expires_frame: Option<u32>,
    pub particle_orbital_laser: bool,
    pub particle_orbital_laser_expires_frame: Option<u32>,
    pub particle_connector_laser: bool,
    pub particle_connector_laser_expires_frame: Option<u32>,
    pub point_defense_laser_beam: bool,
    pub point_defense_laser_beam_expires_frame: Option<u32>,
    pub missile_defender_laser_beam: bool,
    pub missile_defender_laser_beam_expires_frame: Option<u32>,
    pub booby_trap_special: bool,
    pub booby_trap_attached_to: Option<ObjectId>,
    pub countermeasure_flare: bool,
    pub countermeasure_flare_expires_frame: Option<u32>,
    pub angry_mob_member: bool,
    pub angry_mob_nexus_id: Option<ObjectId>,
    pub weapon_laser_beam: bool,
    pub weapon_laser_beam_expires_frame: Option<u32>,
    pub later: LaterProjectileFlight,
}

impl ProjectileFlightResiduals {
    pub(crate) fn present(object: &Object) -> bool {
        object.carpet_bomb_payload
            || object.artillery_barrage_shell
            || object.a10_strike_missile
            || object.leaflet_transport_target.is_some()
            || object.leaflet_container
            || object.paradrop_transport_target.is_some()
            || object.paradrop_parachute
            || object.daisy_cutter_bomb
            || object.anthrax_bomb_payload
            || object.sneak_tunnel_start
            || object.cluster_mines_bomb
            || object.emp_pulse_bomb
            || object.emp_pulse_spheroid
            || object.emp_pulse_spheroid_expires_frame.is_some()
            || object.particle_trail_remnant
            || object.particle_trail_remnant_expires_frame.is_some()
            || object.nuke_radiation_field
            || object.nuke_radiation_field_expires_frame.is_some()
            || object.anthrax_toxin_field
            || object.anthrax_toxin_field_expires_frame.is_some()
            || object.spectre_howitzer_shell
            || object.spectre_howitzer_shell_expires_frame.is_some()
            || object.particle_orbital_laser
            || object.particle_orbital_laser_expires_frame.is_some()
            || object.particle_connector_laser
            || object.particle_connector_laser_expires_frame.is_some()
            || object.point_defense_laser_beam
            || object.point_defense_laser_beam_expires_frame.is_some()
            || object.missile_defender_laser_beam
            || object.missile_defender_laser_beam_expires_frame.is_some()
            || object.booby_trap_special
            || object.booby_trap_attached_to.is_some()
            || object.countermeasure_flare
            || object.countermeasure_flare_expires_frame.is_some()
            || object.angry_mob_member
            || object.angry_mob_nexus_id.is_some()
            || object.weapon_laser_beam
            || object.weapon_laser_beam_expires_frame.is_some()
            || LaterProjectileFlight::present(object)
    }

    pub(crate) fn from_object(object: &Object) -> Self {
        Self {
            carpet_bomb_payload: object.carpet_bomb_payload,
            artillery_barrage_shell: object.artillery_barrage_shell,
            a10_strike_missile: object.a10_strike_missile,
            leaflet_transport_target: object.leaflet_transport_target,
            leaflet_container: object.leaflet_container,
            paradrop_transport_target: object.paradrop_transport_target,
            paradrop_parachute: object.paradrop_parachute,
            daisy_cutter_bomb: object.daisy_cutter_bomb,
            anthrax_bomb_payload: object.anthrax_bomb_payload,
            sneak_tunnel_start: object.sneak_tunnel_start,
            cluster_mines_bomb: object.cluster_mines_bomb,
            emp_pulse_bomb: object.emp_pulse_bomb,
            emp_pulse_spheroid: object.emp_pulse_spheroid,
            emp_pulse_spheroid_expires_frame: object.emp_pulse_spheroid_expires_frame,
            particle_trail_remnant: object.particle_trail_remnant,
            particle_trail_remnant_expires_frame: object.particle_trail_remnant_expires_frame,
            nuke_radiation_field: object.nuke_radiation_field,
            nuke_radiation_field_expires_frame: object.nuke_radiation_field_expires_frame,
            anthrax_toxin_field: object.anthrax_toxin_field,
            anthrax_toxin_field_expires_frame: object.anthrax_toxin_field_expires_frame,
            spectre_howitzer_shell: object.spectre_howitzer_shell,
            spectre_howitzer_shell_expires_frame: object.spectre_howitzer_shell_expires_frame,
            particle_orbital_laser: object.particle_orbital_laser,
            particle_orbital_laser_expires_frame: object.particle_orbital_laser_expires_frame,
            particle_connector_laser: object.particle_connector_laser,
            particle_connector_laser_expires_frame: object.particle_connector_laser_expires_frame,
            point_defense_laser_beam: object.point_defense_laser_beam,
            point_defense_laser_beam_expires_frame: object.point_defense_laser_beam_expires_frame,
            missile_defender_laser_beam: object.missile_defender_laser_beam,
            missile_defender_laser_beam_expires_frame: object
                .missile_defender_laser_beam_expires_frame,
            booby_trap_special: object.booby_trap_special,
            booby_trap_attached_to: object.booby_trap_attached_to,
            countermeasure_flare: object.countermeasure_flare,
            countermeasure_flare_expires_frame: object.countermeasure_flare_expires_frame,
            angry_mob_member: object.angry_mob_member,
            angry_mob_nexus_id: object.angry_mob_nexus_id,
            weapon_laser_beam: object.weapon_laser_beam,
            weapon_laser_beam_expires_frame: object.weapon_laser_beam_expires_frame,
            later: LaterProjectileFlight::from_object(object),
        }
    }

    pub(crate) fn apply(self, object: &mut Object) {
        object.carpet_bomb_payload = self.carpet_bomb_payload;
        object.artillery_barrage_shell = self.artillery_barrage_shell;
        object.a10_strike_missile = self.a10_strike_missile;
        object.leaflet_transport_target = self.leaflet_transport_target;
        object.leaflet_container = self.leaflet_container;
        object.paradrop_transport_target = self.paradrop_transport_target;
        object.paradrop_parachute = self.paradrop_parachute;
        object.daisy_cutter_bomb = self.daisy_cutter_bomb;
        object.anthrax_bomb_payload = self.anthrax_bomb_payload;
        object.sneak_tunnel_start = self.sneak_tunnel_start;
        object.cluster_mines_bomb = self.cluster_mines_bomb;
        object.emp_pulse_bomb = self.emp_pulse_bomb;
        object.emp_pulse_spheroid = self.emp_pulse_spheroid;
        object.emp_pulse_spheroid_expires_frame = self.emp_pulse_spheroid_expires_frame;
        object.particle_trail_remnant = self.particle_trail_remnant;
        object.particle_trail_remnant_expires_frame = self.particle_trail_remnant_expires_frame;
        object.nuke_radiation_field = self.nuke_radiation_field;
        object.nuke_radiation_field_expires_frame = self.nuke_radiation_field_expires_frame;
        object.anthrax_toxin_field = self.anthrax_toxin_field;
        object.anthrax_toxin_field_expires_frame = self.anthrax_toxin_field_expires_frame;
        object.spectre_howitzer_shell = self.spectre_howitzer_shell;
        object.spectre_howitzer_shell_expires_frame = self.spectre_howitzer_shell_expires_frame;
        object.particle_orbital_laser = self.particle_orbital_laser;
        object.particle_orbital_laser_expires_frame = self.particle_orbital_laser_expires_frame;
        object.particle_connector_laser = self.particle_connector_laser;
        object.particle_connector_laser_expires_frame = self.particle_connector_laser_expires_frame;
        object.point_defense_laser_beam = self.point_defense_laser_beam;
        object.point_defense_laser_beam_expires_frame = self.point_defense_laser_beam_expires_frame;
        object.missile_defender_laser_beam = self.missile_defender_laser_beam;
        object.missile_defender_laser_beam_expires_frame =
            self.missile_defender_laser_beam_expires_frame;
        object.booby_trap_special = self.booby_trap_special;
        object.booby_trap_attached_to = self.booby_trap_attached_to;
        object.countermeasure_flare = self.countermeasure_flare;
        object.countermeasure_flare_expires_frame = self.countermeasure_flare_expires_frame;
        object.angry_mob_member = self.angry_mob_member;
        object.angry_mob_nexus_id = self.angry_mob_nexus_id;
        object.weapon_laser_beam = self.weapon_laser_beam;
        object.weapon_laser_beam_expires_frame = self.weapon_laser_beam_expires_frame;
        self.later.apply(object);
    }
}
