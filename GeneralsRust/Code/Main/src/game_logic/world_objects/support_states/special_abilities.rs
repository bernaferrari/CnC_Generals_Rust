//! C++ SpecialAbilityUpdate, capture, sabotage, laser, and RiderChange behavior.
use super::super::super::*;
#[derive(Clone)]
struct RiderChangeEnterPlan {
    rider: crate::game_logic::RiderChangeRiderMetadata,
    /// Resolved before the first eject mutation.  A missing/changed host
    /// locomotor cannot therefore strand both riders after an old rider was
    /// removed from the bike.
    active_locomotor_name: String,
    /// The resolved active set member for the container's current surface
    /// (C++ `chooseGoodLocomotorFromCurrentSet`, AIUpdate.cpp:828-853).  Do
    /// not reduce this to the old three-field `Movement` bridge: C++
    /// chooseLocomotorSet also changes braking, damage movement, surface
    /// capability, and physics options.
    active_locomotor: crate::game_logic::locomotor_bootstrap::HostLocomotorBinding,
    previous_rider: Option<ObjectId>,
    container_position: glam::Vec3,
    /// C++ `wasSelected` is an in-game/local selection fact.  Keep the
    /// owning local player so the object bit and the player's selection list
    /// move together after a successful board.
    incoming_selection_player: Option<u32>,
    incoming_was_selected: bool,
    incoming_veterancy: VeterancyLevel,
}

/// Clear exactly the RiderChange state currently represented by the active
/// host.  C++ additionally clears `DOOR_1_CLOSING` while removing a rider;
/// keep that flag out of the next rider's model state even if an interrupted
/// generic transport animation left it behind.
fn clear_rider_change_runtime_state(
    container: &mut Object,
    authored_rider: Option<&crate::game_logic::RiderChangeRiderMetadata>,
) {
    let authored_model_mask = authored_rider
        .map(|rider| rider.model_condition_mask)
        .unwrap_or(0);
    let authored_status_mask = authored_rider
        .map(|rider| rider.object_status_mask)
        .unwrap_or(0);
    let door_closing_mask =
        crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
            "DOOR_1_CLOSING",
        )
        .filter(|bit| *bit < 128)
        .map(|bit| 1u128 << bit)
        .unwrap_or(0);

    container.model_condition_bits &=
        !(container.rider_change_model_condition_mask | authored_model_mask | door_closing_mask);
    container.object_status_bits &=
        !(container.rider_change_object_status_mask | authored_status_mask);
    container.rider_change_active_slot = None;
    container.rider_change_model_condition_mask = 0;
    container.rider_change_object_status_mask = 0;
    container.rider_change_weapon_set = None;
    container.rider_change_locomotor_set = None;
    container.rider_change_locomotor_name = None;
    // C++ clears the active WeaponSet before the replacement's set is chosen.
    // The bounded Combat Cycle bridge is the live representation of that set.
    container.combat_cycle_rider = 0;
    let _ = container.replace_weapon_set_slot(0, None);
    let _ = container.replace_weapon_set_slot(1, None);
    container.record_host_weapon_stats();
    container.record_host_model_condition();
}

/// Apply every Locomotor.ini field for which the live host Object has an
/// authoritative representation.  This is deliberately one helper so an
/// atomic RiderChange replacement cannot accidentally update its nominal
/// speed while leaving the old rider's braking, damage, or surface state.
fn apply_rider_change_locomotor_binding(
    container: &mut Object,
    binding: &crate::game_logic::locomotor_bootstrap::HostLocomotorBinding,
) {
    crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(container, binding);
}

/// C++ `SpecialAbilityUpdate::startPacking` / `onExit` always drop
/// `MODELCONDITION_RAISING_FLAG` so infantry capturers leave the flag pose.
pub(super) fn clear_raising_flag_model(object: &mut Object) {
    use crate::game_logic::host_enum_table_residual::raising_flag_model_bit;
    let bit = 1u128 << raising_flag_model_bit();
    if object.model_condition_bits & bit != 0 {
        object.model_condition_bits &= !bit;
        object.record_host_model_condition();
    }
}

/// C++ `getSingleLogicalBonePosition` residual: pristine attach bone, else origin.
fn special_ability_attach_bone_world(caster: &Object, bone_name: &str) -> glam::Vec3 {
    let origin = caster.get_position();
    if bone_name.is_empty() {
        return origin;
    }
    let model = caster.thing.template.get_model_name();
    if model.is_empty() {
        return origin;
    }
    let scale = caster.thing.template.asset_scale;
    let yaw = caster.get_orientation();
    let Some(local) =
        gamelogic::object::draw::lookup_pristine_bone_translation(model, scale, bone_name)
    else {
        return origin;
    };
    let (sin, cos) = yaw.sin_cos();
    let host_local = glam::Vec3::new(local.x, local.z, local.y);
    glam::Vec3::new(
        origin.x + host_local.x * cos - host_local.z * sin,
        origin.y + host_local.y,
        origin.z + host_local.x * sin + host_local.z * cos,
    )
}

pub(super) enum LeftoverSaTick {
    Waiting,
    Trigger,
    Finished,
}

impl GameLogic {
    pub(super) fn abort_capture_channel(&mut self, object_id: ObjectId) {
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.stop_moving();
            object.capture_channel = None;
            object.set_status_using_ability(false);
            object.set_target(None);
            clear_raising_flag_model(object);
        }
        self.stop_lotus_prep_sound_loop(object_id);
        self.leftover_sa_set_pack_model(object_id, false, false, false);
        // C++ shouldAbort: aiIdle(CMD_FROM_AI) then onExit. Pack-abort
        // without a replacement order must leave the source Idle.
        self.set_ai_state_decision_aware(object_id, AIState::Idle);
        self.hero_abilities.clear_capture_flash(object_id);
    }

    /// Complete C++ capture packing.  Ownership changes at the end of
    /// preparation, but the source remains busy until PackTime has elapsed.
    pub(super) fn finish_capture_channel(&mut self, object_id: ObjectId) {
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.stop_moving();
            object.capture_channel = None;
            object.set_status_using_ability(false);
            object.set_target(None);
            clear_raising_flag_model(object);
        }
        self.stop_lotus_prep_sound_loop(object_id);
        self.leftover_sa_set_pack_model(object_id, false, false, false);
        // C++ handlePackingProcessing pack-complete → finishAbility → aiIdle.
        self.set_ai_state_decision_aware(object_id, AIState::Idle);
        self.hero_abilities.clear_capture_flash(object_id);
    }

    /// C++ `onExit` when a player/script order replaces capture.
    /// Do not stop a newly issued move — only drop the stale channel.
    pub(super) fn abort_capture_channel_on_new_order(&mut self, object_id: ObjectId) {
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.capture_channel = None;
            object.set_status_using_ability(false);
            clear_raising_flag_model(object);
        }
        self.stop_lotus_prep_sound_loop(object_id);
        self.leftover_sa_set_pack_model(object_id, false, false, false);
        self.hero_abilities.clear_capture_flash(object_id);
    }

    /// C++ `SpecialAbilityUpdate::triggerAbilityEffect` AwardXPForTriggering
    /// (`SpecialAbilityUpdate.cpp:1248-1253`) plus skill-points fallback
    /// (`:1256-1264`). SkillPointsForTriggering defaults to -1, so retail
    /// uses the same AwardXP integer.
    pub(super) fn award_ability_trigger_experience(&mut self, object_id: ObjectId, award_xp: i32) {
        if award_xp <= 0 {
            return;
        }
        let (owner_player_id, team) = match self.objects.get(&object_id) {
            Some(object) => (object.owner_player_id, object.team),
            None => return,
        };
        self.award_experience(object_id, award_xp as f32);
        let player_id = owner_player_id.or_else(|| self.player_id_for_team(team));
        if let Some(id) = player_id {
            let _ = self.add_player_skill_points(id, award_xp);
        }
    }

    /// Retail Object INI `AwardXPForTriggering` for the four capture powers.
    pub(super) fn award_xp_for_capture_trigger(kind: crate::game_logic::CapturePowerKind) -> i32 {
        use crate::game_logic::CapturePowerKind;
        match kind {
            CapturePowerKind::Ranger | CapturePowerKind::RedGuard => {
                crate::game_logic::host_structure_economy_residual::CAPTURE_AWARD_XP as i32
            }
            CapturePowerKind::Rebel => {
                crate::game_logic::host_gla_rebel::REBEL_CAPTURE_AWARD_XP as i32
            }
            CapturePowerKind::BlackLotus => {
                crate::game_logic::host_hero_abilities::LOTUS_CAPTURE_AWARD_XP as i32
            }
            CapturePowerKind::None => 0,
        }
    }

    pub(super) fn leftover_sa_kind(
        ability: PendingSpecialAbility,
    ) -> Option<crate::game_logic::host_hero_abilities::LeftoverSaKind> {
        use crate::game_logic::host_hero_abilities::LeftoverSaKind;
        match ability {
            PendingSpecialAbility::StealCashHack { .. } => Some(LeftoverSaKind::StealCash),
            PendingSpecialAbility::DisableVehicleHack { .. } => {
                Some(LeftoverSaKind::DisableVehicle)
            }
            PendingSpecialAbility::PlantTimedDemoCharge { .. } => Some(LeftoverSaKind::PlantTimed),
            PendingSpecialAbility::PlantRemoteDemoCharge { .. } => {
                Some(LeftoverSaKind::PlantRemote)
            }
            _ => None,
        }
    }

    pub(super) fn leftover_timings_for(
        &self,
        object_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
    ) -> (
        crate::game_logic::host_hero_abilities::LeftoverSaTimings,
        f32,
    ) {
        use crate::game_logic::host_hero_abilities::{leftover_sa_timings, LeftoverSaKind};
        let mut timings = leftover_sa_timings(kind);
        let mut variation = 0.0;
        if matches!(
            kind,
            LeftoverSaKind::PlantTimed | LeftoverSaKind::PlantRemote
        ) {
            let meta = self.objects.get(&object_id).and_then(|object| {
                if matches!(kind, LeftoverSaKind::PlantTimed) {
                    object.thing.template.charge_plant_ability_for_timed()
                } else {
                    object.thing.template.charge_plant_ability_for_remote()
                }
            });
            if let Some(meta) = meta {
                timings.unpack_ms = meta.unpack_time_ms;
                timings.pack_ms = meta.pack_time_ms;
                timings.flee_range = meta.flee_range_after_completion;
                timings.flip_after_unpack = meta.flip_object_after_unpacking;
                variation = meta.pack_unpack_variation_factor;
            } else {
                // C++ skips unpack/flee when the module is absent / UnpackTime is 0.
                timings.unpack_ms = 0;
                timings.pack_ms = 0;
                timings.flee_range = 0.0;
                timings.flip_after_unpack = false;
            }
        }
        (timings, variation)
    }

    pub(crate) fn leftover_sa_exclusive_pending(ability: PendingSpecialAbility) -> bool {
        matches!(
            ability,
            PendingSpecialAbility::StealCashHack { .. }
                | PendingSpecialAbility::DisableVehicleHack { .. }
                | PendingSpecialAbility::PlantTimedDemoCharge { .. }
                | PendingSpecialAbility::PlantRemoteDemoCharge { .. }
                | PendingSpecialAbility::PlantBoobyTrap { .. }
        )
    }

    pub(crate) fn leftover_sa_target_is_ally(
        &self,
        object_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        use gamelogic::common::Relationship;
        let Some(source) = self.objects.get(&object_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        matches!(
            self.object_relationship(source, target),
            Relationship::Allies
        )
    }

    fn leftover_sa_target_stealthed_undetected(&self, target_id: ObjectId) -> bool {
        self.objects
            .get(&target_id)
            .is_some_and(|target| target.status.stealthed && !target.status.detected)
    }

    fn leftover_sa_within_abort_range(
        &self,
        object_id: ObjectId,
        target_id: ObjectId,
        abort_range: f32,
    ) -> bool {
        const HUGE: f32 = 10_000_000.0;
        if !abort_range.is_finite() || abort_range >= HUGE {
            return true;
        }
        let Some(source) = self.objects.get(&object_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let edge = crate::game_logic::host_hero_abilities::leftover_bounding_sphere_2d(
            source.get_position(),
            source.selection_radius,
            target.get_position(),
            target.selection_radius,
        );
        crate::game_logic::host_hero_abilities::leftover_within_abort_range(edge, abort_range)
    }

    fn leftover_sa_should_abort_prep(
        &self,
        object_id: ObjectId,
        target_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
        abort_range: f32,
    ) -> bool {
        use crate::game_logic::host_hero_abilities::LeftoverSaKind;
        let Some(target) = self.objects.get(&target_id) else {
            return true;
        };
        if !target.is_alive() || target.status.destroyed {
            return true;
        }
        if !self.leftover_sa_within_abort_range(object_id, target_id, abort_range) {
            return true;
        }
        let stealthed = self.leftover_sa_target_stealthed_undetected(target_id);
        match kind {
            LeftoverSaKind::StealCash | LeftoverSaKind::DisableVehicle => {
                stealthed || self.leftover_sa_target_is_ally(object_id, target_id)
            }
            LeftoverSaKind::PlantTimed | LeftoverSaKind::PlantRemote => stealthed,
            LeftoverSaKind::LaserGuided => {
                // C++ continuePreparation: ALLIES ("captured by a colleague") ends the laser.
                self.leftover_sa_target_is_ally(object_id, target_id)
            }
        }
    }

    pub(crate) fn leftover_sa_set_pack_model(
        &mut self,
        object_id: ObjectId,
        unpacking: bool,
        packing: bool,
        firing_a: bool,
    ) {
        use crate::game_logic::host_enum_table_residual::{
            firing_a_model_bit, packing_model_bit, raising_flag_model_bit, unpacking_model_bit,
        };
        let Some(object) = self.objects.get_mut(&object_id) else {
            return;
        };
        let before = object.model_condition_bits;
        object.model_condition_bits &= !(1u128 << unpacking_model_bit());
        object.model_condition_bits &= !(1u128 << packing_model_bit());
        object.model_condition_bits &= !(1u128 << firing_a_model_bit());
        // C++ startPacking/onExit always clear RAISING_FLAG.
        object.model_condition_bits &= !(1u128 << raising_flag_model_bit());
        if unpacking {
            object.model_condition_bits |= 1u128 << unpacking_model_bit();
        }
        if packing {
            object.model_condition_bits |= 1u128 << packing_model_bit();
        }
        if firing_a {
            object.model_condition_bits |= 1u128 << firing_a_model_bit();
        }
        if object.model_condition_bits != before {
            object.record_host_model_condition();
        }
    }

    fn leftover_sa_queue_pack_unpack_sound(
        &mut self,
        object_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
        packing: bool,
    ) {
        use crate::game_logic::host_hero_abilities::LeftoverSaKind;
        if !matches!(
            kind,
            LeftoverSaKind::StealCash | LeftoverSaKind::DisableVehicle
        ) {
            return;
        }
        let name = if packing {
            crate::game_logic::host_hero_abilities::LOTUS_PACK_SOUND
        } else {
            crate::game_logic::host_hero_abilities::LOTUS_UNPACK_SOUND
        };
        self.queue_audio_event(
            AudioEventRequest::new(name)
                .with_object(object_id)
                .with_priority(150),
        );
    }

    /// C++ `triggerAbilityEffect` TheAudio add of INI `TriggerSound`
    /// (`SpecialAbilityUpdate.cpp:1267-1269`; leftover
    /// `special_ability_update.rs:1196-1204`) with the source object ID.
    fn leftover_sa_play_trigger_sound(&mut self, object_id: ObjectId, name: &str) {
        if name.is_empty() {
            return;
        }
        // Leftover `special_ability_update.rs:1196-1204` TheAudio add with object ID.
        // Live drain (`play_sound_through_the_audio_at`) is leftover TheAudio add.
        self.queue_object_named_audio(object_id, name, 160, false, false);
    }

    pub(super) fn leftover_sa_queue_trigger_sound(
        &mut self,
        object_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
    ) {
        let authored = self
            .objects
            .get(&object_id)
            .and_then(|object| object.thing.template.leftover_sa_trigger_sound.clone())
            .filter(|name| !name.is_empty());
        let name = authored.or_else(|| {
            crate::game_logic::host_hero_abilities::leftover_sa_trigger_sound(kind)
                .map(str::to_string)
        });
        let Some(name) = name else {
            return;
        };
        self.leftover_sa_play_trigger_sound(object_id, &name);
    }

    /// C++ `SpecialAbilityUpdate` PackSound / UnpackSound / PrepSoundLoop.
    fn queue_object_named_audio(
        &mut self,
        object_id: ObjectId,
        name: &str,
        priority: u8,
        looping: bool,
        stop: bool,
    ) {
        if name.is_empty() {
            return;
        }
        let mut req = AudioEventRequest::new(name)
            .with_object(object_id)
            .with_priority(priority);
        if stop {
            req = req.stopping();
        } else if looping {
            req = req.looping();
        }
        self.queue_audio_event(req);
    }

    fn queue_hacker_disable_pack_unpack_sound(&mut self, object_id: ObjectId, packing: bool) {
        let name = if packing {
            crate::game_logic::host_hacker_disable::HACKER_DISABLE_PACK_SOUND
        } else {
            crate::game_logic::host_hacker_disable::HACKER_DISABLE_UNPACK_SOUND
        };
        self.queue_object_named_audio(object_id, name, 150, false, false);
    }

    fn queue_hacker_disable_prep_sound_loop(&mut self, object_id: ObjectId) {
        self.queue_object_named_audio(
            object_id,
            crate::game_logic::host_hacker_disable::HACKER_DISABLE_PREP_SOUND_LOOP,
            140,
            true,
            false,
        );
    }

    fn stop_hacker_disable_prep_sound_loop(&mut self, object_id: ObjectId) {
        self.queue_object_named_audio(
            object_id,
            crate::game_logic::host_hacker_disable::HACKER_DISABLE_PREP_SOUND_LOOP,
            140,
            false,
            true,
        );
    }

    fn stop_lotus_prep_sound_loop(&mut self, object_id: ObjectId) {
        self.queue_object_named_audio(
            object_id,
            crate::game_logic::host_hero_abilities::LOTUS_PREP_SOUND_LOOP,
            140,
            false,
            true,
        );
    }

    /// C++ `SpecialAbilityUpdate::startUnpacking` / `startPacking` audio.
    /// Authored PackSound/UnpackSound first; Lotus falls back to retail
    /// `BlackLotusPack` / `BlackLotusUnpack` when the module omitted them.
    fn queue_capture_pack_unpack_sound(
        &mut self,
        object_id: ObjectId,
        power: crate::game_logic::CapturePowerKind,
        packing: bool,
    ) {
        let authored = self.objects.get(&object_id).and_then(|object| {
            if packing {
                object.thing.template.capture_pack_sound.clone()
            } else {
                object.thing.template.capture_unpack_sound.clone()
            }
        });
        let name = authored.or_else(|| {
            if matches!(power, crate::game_logic::CapturePowerKind::BlackLotus) {
                Some(if packing {
                    crate::game_logic::host_hero_abilities::LOTUS_PACK_SOUND.to_string()
                } else {
                    crate::game_logic::host_hero_abilities::LOTUS_UNPACK_SOUND.to_string()
                })
            } else {
                None
            }
        });
        let Some(name) = name.filter(|name| !name.is_empty()) else {
            return;
        };
        self.queue_audio_event(
            AudioEventRequest::new(&name)
                .with_object(object_id)
                .with_priority(150),
        );
    }

    /// C++ `triggerAbilityEffect` INI `TriggerSound` on the capturer.
    /// Authored first; Lotus falls back to retail `BlackLotusTrigger`.
    pub(super) fn queue_capture_trigger_sound(
        &mut self,
        object_id: ObjectId,
        power: crate::game_logic::CapturePowerKind,
    ) {
        let authored = self
            .objects
            .get(&object_id)
            .and_then(|object| object.thing.template.capture_trigger_sound.clone())
            .filter(|name| !name.is_empty());
        let name = authored.or_else(|| {
            if matches!(power, crate::game_logic::CapturePowerKind::BlackLotus) {
                Some(crate::game_logic::host_hero_abilities::LOTUS_TRIGGER_SOUND.to_string())
            } else {
                None
            }
        });
        let Some(name) = name else {
            return;
        };
        self.leftover_sa_play_trigger_sound(object_id, &name);
    }

    /// C++ `startPacking(success)` VoiceCaptureBuildingComplete / VoiceTaskComplete.
    /// Never queues the slot token; skip when the authored event is empty.
    fn queue_capture_task_complete_voice(
        &mut self,
        object_id: ObjectId,
        power: crate::game_logic::CapturePowerKind,
    ) {
        let Some(template_name) = self
            .objects
            .get(&object_id)
            .map(|object| object.thing.template.name.clone())
        else {
            return;
        };
        let event = match power {
            crate::game_logic::CapturePowerKind::BlackLotus => {
                crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound(
                    &template_name,
                    "VoiceCaptureBuildingComplete",
                )
            }
            _ => crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound(
                &template_name,
                "VoiceTaskComplete",
            )
            .or_else(|| {
                let factory = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
                let factory = factory.as_ref()?;
                let tmpl = factory.find_template(&template_name, false)?;
                tmpl.get_voice_task_complete().and_then(|event| {
                    let name = event.get_event_name();
                    if name.is_empty() {
                        None
                    } else {
                        Some(name.to_string())
                    }
                })
            }),
        };
        let Some(event) = event else {
            return;
        };
        self.queue_audio_event(
            AudioEventRequest::new(&event)
                .with_object(object_id)
                .with_priority(150),
        );
    }

    /// C++ `startUnpacking`: UNPACKING + UnpackSound.
    pub(super) fn begin_capture_unpacking_pose(
        &mut self,
        object_id: ObjectId,
        power: crate::game_logic::CapturePowerKind,
    ) {
        if let Some(object) = self.objects.get_mut(&object_id) {
            // C++ startFacing aiIdle before unpack/prep so leftover approach is not "in use + moving".
            object.stop_moving();
        }
        self.leftover_sa_set_pack_model(object_id, true, false, false);
        self.queue_capture_pack_unpack_sound(object_id, power, false);
    }

    /// C++ `startPacking(success)`: PACKING + PackSound + VoiceTaskComplete.
    pub(super) fn begin_capture_packing_pose(
        &mut self,
        object_id: ObjectId,
        power: crate::game_logic::CapturePowerKind,
        success: bool,
    ) {
        self.stop_lotus_prep_sound_loop(object_id);
        self.leftover_sa_set_pack_model(object_id, false, true, false);
        self.queue_capture_pack_unpack_sound(object_id, power, true);
        if success {
            self.queue_capture_task_complete_voice(object_id, power);
        }
    }

    fn leftover_sa_flip_orientation(&mut self, object_id: ObjectId) {
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.set_orientation(object.get_orientation() + std::f32::consts::PI);
        }
    }

    fn leftover_sa_consume_prep_charge(
        &mut self,
        object_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
    ) -> bool {
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::host_hero_abilities::LeftoverSaKind;
        let required = match kind {
            LeftoverSaKind::StealCash => Some(SpecialPowerType::BlackLotusStealCash),
            LeftoverSaKind::DisableVehicle => Some(SpecialPowerType::BlackLotusDisableVehicle),
            _ => None,
        };
        if let Some(power) = required {
            return self.consume_special_power_charge_for(object_id, &power);
        }
        let candidates: &[SpecialPowerType] = match kind {
            LeftoverSaKind::PlantTimed => &[
                SpecialPowerType::TankHunterTnt,
                SpecialPowerType::BurtonTimedCharges,
                SpecialPowerType::DemoRebelTimedCharges,
                SpecialPowerType::DemoKellTimedCharges,
                SpecialPowerType::DemoKellStickyCharges,
                SpecialPowerType::BattleBusDemoTrapRollout,
            ],
            LeftoverSaKind::PlantRemote => &[
                SpecialPowerType::BurtonRemoteCharges,
                SpecialPowerType::DemoKellRemoteCharges,
            ],
            _ => return true,
        };
        let Some(power) = self.objects.get(&object_id).and_then(|object| {
            candidates
                .iter()
                .find(|power| {
                    object
                        .thing
                        .template
                        .special_power_module_for_command(power)
                        .is_some()
                })
                .cloned()
        }) else {
            return true;
        };
        self.consume_special_power_charge_for(object_id, &power)
    }

    /// C++ `SpecialAbilityUpdate::startPreparation` → `markSpecialPowerTriggered(NULL)`
    /// → `aboutToDoSpecialPower` ScriptEngine TRIGGERED (not COMPLETED).
    pub(crate) fn leftover_sa_notify_start_preparation(
        &self,
        object_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
    ) {
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::host_hero_abilities::LeftoverSaKind;
        let candidates: &[SpecialPowerType] = match kind {
            LeftoverSaKind::StealCash => &[SpecialPowerType::BlackLotusStealCash],
            LeftoverSaKind::DisableVehicle => &[SpecialPowerType::BlackLotusDisableVehicle],
            LeftoverSaKind::PlantTimed => &[
                SpecialPowerType::TankHunterTnt,
                SpecialPowerType::BurtonTimedCharges,
                SpecialPowerType::DemoRebelTimedCharges,
                SpecialPowerType::DemoKellTimedCharges,
                SpecialPowerType::DemoKellStickyCharges,
                SpecialPowerType::BattleBusDemoTrapRollout,
            ],
            LeftoverSaKind::PlantRemote => &[
                SpecialPowerType::BurtonRemoteCharges,
                SpecialPowerType::DemoKellRemoteCharges,
            ],
            LeftoverSaKind::LaserGuided => &[
                SpecialPowerType::MissileDefenderLaserGuided,
                SpecialPowerType::LaserGuidedHowitzer,
            ],
        };
        let power = self
            .objects
            .get(&object_id)
            .and_then(|object| {
                candidates
                    .iter()
                    .find(|power| {
                        object
                            .thing
                            .template
                            .special_power_module_for_command(power)
                            .is_some()
                            || object.special_power_cooldowns.contains_key(*power)
                    })
                    .cloned()
            })
            .or_else(|| candidates.first().cloned());
        if let Some(power) = power {
            self.notify_script_engine_special_power_event(object_id, &power, true, false);
        }
    }

    fn leftover_begin_unpacking(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
        unpack_ms: u32,
    ) {
        use crate::game_logic::host_hero_abilities::{LeftoverSaChannel, LeftoverSaPhase};
        self.leftover_sa_set_pack_model(object_id, true, false, false);
        self.leftover_sa_queue_pack_unpack_sound(object_id, kind, false);
        self.hero_abilities.set_leftover_channel(
            object_id,
            LeftoverSaChannel::new(kind, target_id, LeftoverSaPhase::Unpacking, unpack_ms),
        );
    }

    fn leftover_finish_unpack(
        &mut self,
        object_id: ObjectId,
        timings: &crate::game_logic::host_hero_abilities::LeftoverSaTimings,
    ) {
        self.leftover_sa_set_pack_model(object_id, false, false, false);
        if timings.flip_after_unpack {
            self.leftover_sa_flip_orientation(object_id);
        }
    }

    fn leftover_reset_laser_primary(&mut self, object_id: ObjectId) {
        if let Some(object) = self.objects.get_mut(&object_id) {
            let _ = object.set_weapon_lock(0, crate::game_logic::WeaponLockType::LockedTemporarily);
        }
    }

    pub(crate) fn abort_leftover_sa_channel_on_new_order(&mut self, object_id: ObjectId) {
        self.stop_lotus_prep_sound_loop(object_id);
        self.leftover_kill_special_objects(object_id);
        self.hero_abilities.take_leftover_channel(object_id);
        self.leftover_sa_set_pack_model(object_id, false, false, false);
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.set_status_using_ability(false);
        }
    }

    /// C++ `SpecialAbilityUpdate::isWithinStartAbilityRange`: 2D bounding
    /// sphere vs full StartAbilityRange, then if `ApproachRequiresLOS`
    /// (default TRUE) LOS-iterate the undersized envelope.
    pub(super) fn leftover_sa_within_start_range(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        start_range: f32,
    ) -> bool {
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let edge = crate::game_logic::host_hero_abilities::leftover_bounding_sphere_2d(
            source.get_position(),
            source.selection_radius,
            target.get_position(),
            target.selection_radius,
        );
        if !crate::game_logic::host_hero_abilities::leftover_within_start_ability_range(
            edge,
            start_range,
        ) {
            return false;
        }
        if !crate::game_logic::host_hero_abilities::leftover_sa_approach_requires_los() {
            return true;
        }
        if edge > crate::game_logic::host_hero_abilities::leftover_sa_los_iterate_range(start_range)
        {
            return false;
        }
        self.leftover_sa_has_los(source_id, target_id)
    }

    /// C++ SpecialAbilityUpdate approach → StartAbilityRange 3 → startPreparation
    /// (`markSpecialPowerTriggered`) → `createSpecialObject` NapalmBomb.
    pub(super) fn update_helix_napalm_bomb_channel(
        &mut self,
        object_id: ObjectId,
        ability: crate::game_logic::PendingSpecialAbility,
    ) {
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::host_helix_napalm::helix_napalm_in_start_range;

        let Some(target) = ability.helix_napalm_target() else {
            self.pending_special_abilities.remove(&object_id);
            return;
        };
        let Some((position, selection_radius, can_move, alive)) =
            self.objects.get(&object_id).map(|o| {
                (
                    o.get_position(),
                    o.selection_radius,
                    o.can_move(),
                    o.is_alive(),
                )
            })
        else {
            self.pending_special_abilities.remove(&object_id);
            return;
        };
        if !alive {
            self.pending_special_abilities.remove(&object_id);
            return;
        }
        if !helix_napalm_in_start_range(position, selection_radius, target) {
            if can_move {
                self.path_approach_with_state(object_id, target, AIState::SpecialAbility);
            }
            return;
        }
        let power = self.objects.get(&object_id).and_then(|o| {
            [
                SpecialPowerType::HelixNapalmBomb,
                SpecialPowerType::HelixNukeBomb,
            ]
            .into_iter()
            .find(|p| {
                o.thing
                    .template
                    .special_power_module_for_command(p)
                    .is_some()
            })
        });
        if let Some(power) = power {
            if !self.consume_special_power_charge_for(object_id, &power) {
                // C++ PersistenceRequiresRecharge: freeze until SPM ready.
                return;
            }
            self.notify_script_engine_special_power_event(object_id, &power, true, false);
        }
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.set_status_using_ability(true);
            obj.stop_moving();
        }
        let dropped = self.activate_helix_napalm_bomb(object_id, target).is_some();
        self.pending_special_abilities.remove(&object_id);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.set_status_using_ability(false);
            if dropped {
                obj.set_ai_state(AIState::Idle);
            }
        }
        let _ = dropped;
    }

    /// C++ `PartitionFilterLineOfSight` residual used after the 2D range gate.
    fn leftover_sa_has_los(&self, source_id: ObjectId, target_id: ObjectId) -> bool {
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let source_position = source.get_position();
        let target_position = target.get_position();
        let source_eye = glam::Vec3::new(
            source_position.x,
            source_position.y + source.selection_radius.max(5.0) * 0.5,
            source_position.z,
        );
        let target_eye = glam::Vec3::new(
            target_position.x,
            target_position.y + target.selection_radius.max(5.0) * 0.5,
            target_position.z,
        );
        self.is_clear_line_of_sight_terrain(source_eye, target_eye)
    }

    /// C++ `isWithinStartAbilityRange` for Lotus steal/disable.
    pub(super) fn leftover_lotus_within_start_range(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        start_range: f32,
    ) -> bool {
        self.leftover_sa_within_start_range(source_id, target_id, start_range)
    }

    fn leftover_sa_unit_can_face(&self, object_id: ObjectId) -> bool {
        self.objects.get(&object_id).is_some_and(|o| o.can_move())
    }

    fn leftover_sa_facing_complete(&self, object_id: ObjectId, target_id: ObjectId) -> bool {
        let Some(source) = self.objects.get(&object_id) else {
            return true;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return true;
        };
        crate::game_logic::host_hero_abilities::leftover_sa_is_facing_target(
            source.get_position(),
            source.get_orientation(),
            target.get_position(),
        )
    }

    /// C++ `startFacing`: idle, reset physics, face the target.
    fn leftover_start_facing(&mut self, object_id: ObjectId, target_id: ObjectId) {
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.stop_moving();
            obj.set_ai_state(AIState::SpecialAbility);
        }
        let _ = self.private_face_object(object_id, target_id);
    }

    /// Continue facing; returns true while the unit is still turning.
    ///
    /// Leftover-march ANGLE/`locoUpdate_moveTowardsAngle` (or POSITION_EXPLICIT
    /// when minSpeed>0). Do not one-frame yaw-snap via `rotate_towards_position`.
    fn leftover_continue_facing(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        dt: f32,
    ) -> bool {
        if !self.leftover_sa_unit_can_face(object_id) {
            return false;
        }
        if self.leftover_sa_facing_complete(object_id, target_id) {
            return false;
        }
        let Some(target_pos) = self.objects.get(&target_id).map(|t| t.get_position()) else {
            return false;
        };
        let frame = self.frame;
        if let Some(obj) = self.objects.get_mut(&object_id) {
            if !obj.face_active {
                obj.face_can_turn_in_place = obj.min_speed == 0.0;
                obj.face_active = true;
            }
            obj.tick_face_towards(target_pos, dt.max(1.0 / 30.0), frame)
        } else {
            false
        }
    }

    pub(crate) fn leftover_kill_special_objects(&mut self, producer_id: ObjectId) {
        let laser = self
            .hero_abilities
            .leftover_channel(producer_id)
            .is_some_and(|channel| {
                channel.kind == crate::game_logic::host_hero_abilities::LeftoverSaKind::LaserGuided
            });
        let stale: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if (o.weapon_laser_beam || o.missile_defender_laser_beam)
                    && o.producer_id == Some(producer_id)
                {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for sid in stale {
            if let Some(o) = self.objects.get_mut(&sid) {
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.weapon_laser_beam = false;
                o.missile_defender_laser_beam = false;
            }
            self.mark_object_for_destruction(sid, None);
        }
        self.weapon_lasers
            .retain(|laser| laser.from_id != producer_id);
        if let Some(mut channel) = self.hero_abilities.leftover_channel(producer_id).copied() {
            channel.special_object_id = None;
            self.hero_abilities
                .set_leftover_channel(producer_id, channel);
        }
        if laser {
            self.leftover_reset_laser_primary(producer_id);
        }
    }

    /// C++ `createSpecialObject` + `initLaser` for BinaryDataStream.
    fn leftover_spawn_binary_data_stream(
        &mut self,
        from_id: ObjectId,
        to_id: ObjectId,
        lifetime_frames: u32,
        laser_name: &str,
    ) -> Option<ObjectId> {
        self.leftover_kill_special_objects(from_id);
        let (from, to) = self.special_ability_laser_endpoints(from_id, to_id)?;

        let bid =
            self.spawn_weapon_laser_beam_object(laser_name, from_id, Some(to_id), from, to)?;
        let expires = self.frame.saturating_add(lifetime_frames.max(1));
        if let Some(o) = self.objects.get_mut(&bid) {
            o.weapon_laser_beam_expires_frame = Some(expires);
        }
        self.weapon_lasers
            .retain(|laser| laser.from_id != from_id || laser.laser_name != laser_name);
        self.weapon_lasers
            .push(crate::game_logic::host_weapon_laser::ResidualWeaponLaser {
                laser_name: laser_name.to_string(),
                laser_bone_name: String::new(),
                from_id,
                to_id: Some(to_id),
                from_x: from.x,
                from_y: from.y,
                from_z: from.z,
                to_x: to.x,
                to_y: to.y,
                to_z: to.z,
                expires_frame: expires,
                scroll_offset: 0.0,
            });
        self.hero_abilities.record_leftover_binary_stream();
        Some(bid)
    }

    /// C++ `SpecialAbilityUpdate::initLaser` start/end for a live caster/target pair.
    pub(crate) fn special_ability_laser_endpoints(
        &self,
        caster_id: ObjectId,
        target_id: ObjectId,
    ) -> Option<(glam::Vec3, glam::Vec3)> {
        self.special_ability_laser_endpoints_from_bone(caster_id, target_id, "")
    }

    /// C++ `getSingleLogicalBonePosition` then `getCenterPosition` (origin fallback).
    pub(crate) fn special_ability_laser_endpoints_from_bone(
        &self,
        caster_id: ObjectId,
        target_id: ObjectId,
        attach_bone: &str,
    ) -> Option<(glam::Vec3, glam::Vec3)> {
        let caster = self.objects.get(&caster_id)?;
        let target = self.objects.get(&target_id)?;
        let geom = &target.thing.template.geometry_info;
        let start = special_ability_attach_bone_world(caster, attach_bone);
        Some(
            crate::game_logic::host_weapon_laser::special_ability_laser_endpoints(
                start,
                target.get_position(),
                geom.max_height_above_position(),
                target.selection_radius,
                geom.authored,
            ),
        )
    }

    /// C++ `continuePreparation` re-initLaser for MD / Lotus disable beams.
    pub(crate) fn reinit_special_ability_laser(
        &mut self,
        caster_id: ObjectId,
        target_id: ObjectId,
        special_object_id: Option<ObjectId>,
    ) -> bool {
        let md_beam = special_object_id
            .and_then(|sid| self.objects.get(&sid))
            .is_some_and(|o| o.missile_defender_laser_beam);
        let bone_name = self
            .weapon_lasers
            .iter()
            .find(|l| l.from_id == caster_id && l.to_id == Some(target_id))
            .map(|l| l.laser_bone_name.clone())
            .unwrap_or_else(|| {
                if md_beam {
                    crate::game_logic::host_missile_defender::LASER_GUIDED_ATTACH_BONE.to_string()
                } else {
                    String::new()
                }
            });
        let Some((from, to)) =
            self.special_ability_laser_endpoints_from_bone(caster_id, target_id, &bone_name)
        else {
            return false;
        };
        let frame = self.frame;
        let mut found = false;
        for laser in &mut self.weapon_lasers {
            if laser.from_id == caster_id && laser.to_id == Some(target_id) {
                laser.retarget((from.x, from.y, from.z), (to.x, to.y, to.z));
                laser.keep_alive(
                    frame,
                    crate::game_logic::host_weapon_laser::WEAPON_LASER_LIFETIME_FRAMES,
                );
                found = true;
            }
        }
        if !found {
            if special_object_id.is_none() {
                return false;
            }
            let (laser_name, life) = if md_beam {
                (
                    crate::game_logic::host_missile_defender::LASER_GUIDED_SPECIAL_OBJECT,
                    crate::game_logic::host_missile_defender::LASER_GUIDED_BEAM_LIFETIME_FRAMES,
                )
            } else {
                (
                    crate::game_logic::host_hero_abilities::LOTUS_DISABLE_SPECIAL_OBJECT,
                    crate::game_logic::host_hero_abilities::LOTUS_DISABLE_PREP_FRAMES,
                )
            };
            self.weapon_lasers.push(
                crate::game_logic::host_weapon_laser::ResidualWeaponLaser::with_bone_lifetime(
                    laser_name,
                    bone_name,
                    caster_id,
                    Some(target_id),
                    (from.x, from.y, from.z),
                    (to.x, to.y, to.z),
                    frame,
                    life,
                ),
            );
            found = true;
        }
        if let Some(sid) = special_object_id {
            if let Some(o) = self.objects.get_mut(&sid) {
                o.set_position(from);
                let extra = if o.missile_defender_laser_beam {
                    crate::game_logic::host_missile_defender::LASER_GUIDED_BEAM_LIFETIME_FRAMES
                } else {
                    crate::game_logic::host_hero_abilities::LOTUS_DISABLE_PREP_FRAMES
                };
                let until = frame.saturating_add(extra.max(1));
                if o.missile_defender_laser_beam {
                    o.missile_defender_laser_beam_expires_frame = Some(until);
                }
                if o.weapon_laser_beam {
                    o.weapon_laser_beam_expires_frame = Some(until);
                }
            }
        }
        found
    }

    pub(super) fn leftover_spawn_disable_fx(
        &mut self,
        caster_id: ObjectId,
        target_id: ObjectId,
        template_name: &str,
        effect_duration_frames: u32,
        do_fx: bool,
    ) -> bool {
        let (is_structure, footprint, pos, yaw, geom) = match self.objects.get(&target_id) {
            Some(target) => {
                let geom = target.thing.template.geometry_info;
                let area =
                    crate::game_logic::host_hero_abilities::leftover_disable_fx_footprint_area(
                        geom.authored,
                        geom.geom_type as u32,
                        geom.major_radius,
                        geom.minor_radius,
                        target.selection_radius,
                    );
                (
                    target.is_kind_of(KindOf::Structure)
                        || target.object_type == ObjectType::Building,
                    area,
                    target.get_position(),
                    target.get_orientation(),
                    geom,
                )
            }
            None => return do_fx,
        };
        let (new_do_fx, emit, interleave) =
            crate::game_logic::host_hero_abilities::leftover_disable_fx_pulse(
                do_fx,
                footprint,
                is_structure,
            );
        if emit {
            let offset =
                crate::game_logic::host_hero_abilities::leftover_disable_fx_footprint_offset(&geom);
            // C++ SpecialAbilityUpdate.cpp:1386-1395 attachToObject(target) +
            // setPosition(footprint offset) + setSystemLifetime(duration * interleave).
            let lifetime =
                crate::game_logic::host_hero_abilities::leftover_disable_fx_system_lifetime(
                    effect_duration_frames,
                    interleave,
                );
            let pid = self.combat_particles.attach_named_to_object_local(
                target_id,
                pos,
                yaw,
                offset,
                self.frame,
                template_name,
                crate::game_logic::combat_particles::CombatParticleKind::DisableFx,
                Some(lifetime),
            );
            let _ = caster_id;
            if let Some(pid) = pid {
                self.hero_abilities
                    .record_leftover_disable_fx_until(pid, self.frame.saturating_add(lifetime));
            }
            self.hero_abilities.record_leftover_disable_fx();
        }
        new_do_fx
    }

    /// C++ `ParticleSystem::setSystemLifetime` for DisableFX BinaryShower.
    pub(super) fn expire_leftover_disable_fx(&mut self) {
        let now = self.frame;
        for id in self.hero_abilities.take_expired_disable_fx(now) {
            self.combat_particles.deactivate(id);
        }
    }

    pub(super) fn leftover_probe_booby_at_target(
        &mut self,
        planter_id: ObjectId,
        target_id: ObjectId,
        planter_team: crate::game_logic::Team,
    ) -> bool {
        let trap_position = self.objects.get(&target_id).map(|t| t.get_position());
        let planter_ally = self
            .booby_trap
            .plant(target_id)
            .map(|plant| plant.planter_team == planter_team)
            .unwrap_or(false);
        let target_is_trapped = self.booby_trap.is_booby_trapped(target_id)
            || self
                .objects
                .get(&target_id)
                .map(|t| t.status.booby_trapped)
                .unwrap_or(false);
        if planter_ally || !target_is_trapped {
            return false;
        }
        if let Some(trap_position) = trap_position {
            let _ = self.detonate_booby_trap_at(
                target_id,
                trap_position,
                Some(planter_id),
                true,
                false,
            );
        }
        let target_dead = self
            .objects
            .get(&target_id)
            .map(|t| !t.is_alive() || t.status.destroyed)
            .unwrap_or(true);
        let planter_dead = self
            .objects
            .get(&planter_id)
            .map(|p| !p.is_alive() || p.status.destroyed)
            .unwrap_or(true);
        target_dead || planter_dead
    }

    /// C++ `OpenContain::addToContain`: `checkAndDetonateBoobyTrap(rider)`
    /// then cancel containment if the container or rider is now dead.
    /// Leftover `should_cancel_containment_after_booby_trap` already matches;
    /// live host must detonate on GameWorld objects, not leftover-only.
    pub(in super::super::super) fn should_cancel_containment_after_booby_trap(
        &mut self,
        container_id: ObjectId,
        rider_id: ObjectId,
    ) -> bool {
        let Some(rider_team) = self.objects.get(&rider_id).map(|r| r.team) else {
            return false;
        };
        self.leftover_probe_booby_at_target(rider_id, container_id, rider_team)
    }

    fn leftover_nearest_own_mine(
        &self,
        dest: glam::Vec3,
        team: crate::game_logic::Team,
        flee_range: f32,
    ) -> Option<glam::Vec3> {
        let mut best: Option<(f32, glam::Vec3)> = None;
        for obj in self.objects.values() {
            if obj.team != team || !obj.is_kind_of(KindOf::Mine) {
                continue;
            }
            let p = obj.get_position();
            let d = crate::game_logic::host_hero_abilities::horizontal_distance(dest, p);
            if d > flee_range {
                continue;
            }
            if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, p));
            }
        }
        best.map(|(_, p)| p)
    }

    fn leftover_start_sa_preparation(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
        prep_ms: u32,
    ) -> bool {
        use crate::game_logic::host_hero_abilities::{
            LeftoverSaChannel, LeftoverSaKind, LeftoverSaPhase, LOTUS_DISABLE_SPECIAL_OBJECT,
            LOTUS_STEAL_SPECIAL_OBJECT,
        };
        let target_alive = self
            .objects
            .get(&target_id)
            .is_some_and(|target| target.is_alive() && !target.status.destroyed);
        if !target_alive
            || self.leftover_sa_target_is_ally(object_id, target_id)
            || self.leftover_sa_target_stealthed_undetected(target_id)
        {
            return false;
        }
        if !self.leftover_sa_consume_prep_charge(object_id, kind) {
            return false;
        }
        if let Some(object) = self.objects.get_mut(&object_id) {
            if !object.is_alive() {
                return false;
            }
            object.set_ai_state(AIState::SpecialAbility);
            object.set_status_using_ability(true);
        } else {
            return false;
        }
        let laser_name = match kind {
            LeftoverSaKind::StealCash => Some(LOTUS_STEAL_SPECIAL_OBJECT),
            LeftoverSaKind::DisableVehicle => Some(LOTUS_DISABLE_SPECIAL_OBJECT),
            _ => None,
        };
        let prep_frames = crate::game_logic::host_hero_abilities::hero_ms_to_frames(prep_ms).max(1);
        let special_object_id = laser_name.and_then(|name| {
            self.leftover_spawn_binary_data_stream(object_id, target_id, prep_frames, name)
        });
        if matches!(
            kind,
            LeftoverSaKind::StealCash | LeftoverSaKind::DisableVehicle
        ) {
            // C++ startPreparation: tryInfiltrationEvent after createSpecialObject/initLaser.
            self.try_infiltration_event(target_id);
            self.hero_abilities.record_leftover_infiltration();
            self.leftover_sa_set_pack_model(object_id, false, false, true);
        }
        self.leftover_sa_notify_start_preparation(object_id, kind);
        let mut channel =
            LeftoverSaChannel::new(kind, target_id, LeftoverSaPhase::Preparing, prep_ms);
        channel.special_object_id = special_object_id;
        self.hero_abilities.set_leftover_channel(object_id, channel);
        if matches!(
            kind,
            LeftoverSaKind::StealCash | LeftoverSaKind::DisableVehicle
        ) {
            self.queue_object_named_audio(
                object_id,
                crate::game_logic::host_hero_abilities::LOTUS_PREP_SOUND_LOOP,
                140,
                true,
                false,
            );
        }
        true
    }

    pub(super) fn leftover_flee_after_plant(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        team: crate::game_logic::Team,
        flee_range: f32,
        flip_after_unpack: bool,
    ) {
        let (pos, dir) = match self.objects.get(&object_id) {
            Some(obj) => (obj.get_position(), obj.unit_direction_xz()),
            None => return,
        };
        let dest_guess = crate::game_logic::host_hero_abilities::leftover_flee_position(
            pos,
            dir,
            flee_range,
            flip_after_unpack,
            None,
        );
        let mine = self.leftover_nearest_own_mine(dest_guess, team, flee_range);
        let dest = crate::game_logic::host_hero_abilities::leftover_flee_position(
            pos,
            dir,
            flee_range,
            flip_after_unpack,
            mine,
        );
        self.hero_abilities.take_leftover_channel(object_id);
        self.pending_special_abilities.remove(&object_id);
        self.leftover_sa_set_pack_model(object_id, false, false, false);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.set_status_using_ability(false);
        }
        self.path_approach_with_state(object_id, dest, AIState::Moving);
        let _ = target_id;
    }

    pub(super) fn tick_leftover_special_ability(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
        dt: f32,
    ) -> LeftoverSaTick {
        use crate::game_logic::host_hero_abilities::{
            leftover_should_unstealth_during_unpack, LeftoverSaChannel, LeftoverSaPhase,
        };
        const EPS: f32 = 0.000_1;
        let (timings, variation) = self.leftover_timings_for(object_id, kind);
        let channel = match self.hero_abilities.leftover_channel(object_id).copied() {
            Some(channel) if channel.kind != kind || channel.target_id != target_id => {
                self.abort_leftover_sa_channel_on_new_order(object_id);
                None
            }
            other => other,
        };
        match channel {
            None => {
                if crate::game_logic::host_hero_abilities::leftover_sa_need_to_face()
                    && self.leftover_sa_unit_can_face(object_id)
                {
                    self.leftover_start_facing(object_id, target_id);
                    self.hero_abilities.set_leftover_channel(
                        object_id,
                        LeftoverSaChannel::new(kind, target_id, LeftoverSaPhase::Facing, 0),
                    );
                    return LeftoverSaTick::Waiting;
                }
                let unpack_ms =
                    crate::game_logic::vary_pack_unpack_duration_ms(timings.unpack_ms, variation);
                if unpack_ms > 0 {
                    self.leftover_begin_unpacking(object_id, target_id, kind, unpack_ms);
                    return LeftoverSaTick::Waiting;
                }
                if !self.leftover_start_sa_preparation(object_id, target_id, kind, timings.prep_ms)
                {
                    self.abort_leftover_sa_channel_on_new_order(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    return LeftoverSaTick::Finished;
                }
                if timings.prep_ms == 0 {
                    LeftoverSaTick::Trigger
                } else {
                    LeftoverSaTick::Waiting
                }
            }
            Some(channel) if channel.phase == LeftoverSaPhase::Facing => {
                if self.leftover_continue_facing(object_id, target_id, dt) {
                    self.hero_abilities.set_leftover_channel(object_id, channel);
                    return LeftoverSaTick::Waiting;
                }
                let unpack_ms =
                    crate::game_logic::vary_pack_unpack_duration_ms(timings.unpack_ms, variation);
                if unpack_ms > 0 {
                    self.leftover_begin_unpacking(object_id, target_id, kind, unpack_ms);
                    return LeftoverSaTick::Waiting;
                }
                if !self.leftover_start_sa_preparation(object_id, target_id, kind, timings.prep_ms)
                {
                    self.abort_leftover_sa_channel_on_new_order(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    return LeftoverSaTick::Finished;
                }
                if timings.prep_ms == 0 {
                    LeftoverSaTick::Trigger
                } else {
                    LeftoverSaTick::Waiting
                }
            }
            Some(mut channel) if channel.phase == LeftoverSaPhase::Unpacking => {
                channel.remaining_seconds = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if timings.pre_trigger_unstealth_ms > 0
                    && leftover_should_unstealth_during_unpack(
                        channel.remaining_seconds,
                        timings.pre_trigger_unstealth_ms,
                    )
                    && !channel.unstealthed
                {
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.set_status_detected(true);
                    }
                    channel.unstealthed = true;
                }
                if channel.remaining_seconds > EPS {
                    self.hero_abilities.set_leftover_channel(object_id, channel);
                    return LeftoverSaTick::Waiting;
                }
                self.leftover_finish_unpack(object_id, &timings);
                if !self.leftover_start_sa_preparation(object_id, target_id, kind, timings.prep_ms)
                {
                    self.abort_leftover_sa_channel_on_new_order(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    return LeftoverSaTick::Finished;
                }
                if timings.prep_ms == 0 {
                    LeftoverSaTick::Trigger
                } else {
                    LeftoverSaTick::Waiting
                }
            }
            Some(channel) if channel.phase == LeftoverSaPhase::Preparing => {
                if self.leftover_sa_should_abort_prep(
                    object_id,
                    target_id,
                    kind,
                    timings.abort_range,
                ) {
                    let pack_ms =
                        crate::game_logic::vary_pack_unpack_duration_ms(timings.pack_ms, variation);
                    if pack_ms > 0 {
                        self.leftover_begin_packing(object_id, target_id, kind, pack_ms);
                        return LeftoverSaTick::Waiting;
                    }
                    self.abort_leftover_sa_channel_on_new_order(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    return LeftoverSaTick::Finished;
                }
                if kind == crate::game_logic::host_hero_abilities::LeftoverSaKind::DisableVehicle {
                    // C++ continuePreparation re-initLaser each prep frame.
                    let _ = self.reinit_special_ability_laser(
                        object_id,
                        target_id,
                        channel.special_object_id,
                    );
                }
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);

                if remaining > EPS {
                    self.hero_abilities.set_leftover_channel(
                        object_id,
                        LeftoverSaChannel {
                            remaining_seconds: remaining,
                            ..channel
                        },
                    );
                    LeftoverSaTick::Waiting
                } else {
                    LeftoverSaTick::Trigger
                }
            }
            Some(channel) if channel.phase == LeftoverSaPhase::Packing => {
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if remaining > EPS {
                    self.hero_abilities.set_leftover_channel(
                        object_id,
                        LeftoverSaChannel {
                            remaining_seconds: remaining,
                            ..channel
                        },
                    );
                    LeftoverSaTick::Waiting
                } else {
                    self.hero_abilities.take_leftover_channel(object_id);
                    self.pending_special_abilities.remove(&object_id);
                    self.leftover_sa_set_pack_model(object_id, false, false, false);
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        obj.stop_moving();
                        obj.set_status_using_ability(false);
                        obj.set_target(None);
                        obj.set_ai_state(AIState::Idle);
                    }
                    LeftoverSaTick::Finished
                }
            }
            Some(_) => LeftoverSaTick::Finished,
        }
    }
    pub(super) fn leftover_begin_packing(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        kind: crate::game_logic::host_hero_abilities::LeftoverSaKind,
        pack_ms: u32,
    ) {
        use crate::game_logic::host_hero_abilities::{LeftoverSaChannel, LeftoverSaPhase};
        self.stop_lotus_prep_sound_loop(object_id);
        self.leftover_kill_special_objects(object_id);
        if pack_ms == 0 {
            self.hero_abilities.take_leftover_channel(object_id);
            self.pending_special_abilities.remove(&object_id);
            self.leftover_sa_set_pack_model(object_id, false, false, false);
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.stop_moving();
                obj.set_status_using_ability(false);
                obj.set_target(None);
                obj.set_ai_state(AIState::Idle);
            }
            return;
        }
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.stop_moving();
            obj.set_status_using_ability(false);
            obj.set_ai_state(AIState::SpecialAbility);
        }
        self.leftover_sa_set_pack_model(object_id, false, true, false);
        self.leftover_sa_queue_pack_unpack_sound(object_id, kind, true);
        self.hero_abilities.set_leftover_channel(
            object_id,
            LeftoverSaChannel::new(kind, target_id, LeftoverSaPhase::Packing, pack_ms),
        );
    }

    pub(super) fn apply_leftover_capture_fx(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        remaining_seconds: f32,
        preparation_time_ms: u32,
    ) {
        if !crate::game_logic::host_hero_abilities::LOTUS_CAPTURE_DO_CAPTURE_FX {
            return;
        }
        let phase = self
            .hero_abilities
            .capture_flash_phase
            .entry(object_id)
            .or_insert(0.0);
        if crate::game_logic::host_hero_abilities::leftover_capture_fx_should_flash(
            phase,
            remaining_seconds,
            preparation_time_ms,
        ) {
            // C++ continuePreparation: capturer getIndicatorColor, saturateRGB,
            // flashAsSelected(&myHouseColor), DefectorTimerTickSound on target.
            let capturer_owner = self.objects.get(&object_id).and_then(|o| o.owner_player_id);
            let house = capturer_owner
                .and_then(|id| self.player_color_rgb(id))
                .map(|(r, g, b)| [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0])
                .unwrap_or([1.0, 1.0, 1.0]);
            let flash = crate::game_logic::host_hero_abilities::saturate_selection_flash_rgb(
                house,
                crate::game_logic::host_hero_abilities::SELECTION_FLASH_SATURATION_FACTOR,
            );

            let target_pos = self.objects.get(&target_id).map(|t| t.get_position());
            if let Some(target) = self.objects.get_mut(&target_id) {
                target.flash_as_selected_with_color(flash);
            }
            if let Some(pos) = target_pos {
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(
                        crate::game_logic::host_defector_special_power::DEFECTOR_TIMER_TICK_AUDIO,
                    )
                    .with_object(target_id)
                    .with_position(pos)
                    .with_priority(160),
                );
            }
        }
    }

    /// Validate every mutable participant in the RiderChange replacement
    /// before taking the first item out of the old containment list.  The
    /// simulation is single-threaded, so once this returns `Some` the
    /// following transaction has no fallible lookup or capacity operation.
    fn rider_change_enter_plan(
        &self,
        rider_id: ObjectId,
        container_id: ObjectId,
    ) -> Option<RiderChangeEnterPlan> {
        if !self.can_unit_enter_normal_target(rider_id, container_id) {
            return None;
        }
        let rider = self.objects.get(&rider_id)?;
        let container = self.objects.get(&container_id)?;
        if !container.supports_authored_rider_change_normal_enter() {
            // A parsed roster is the sole admission/effect capability.  Do
            // not use a Combat Bike basename or legacy transport flag here:
            // those are old bootstrap state, not RiderChange authority.
            return None;
        }
        // RiderChangeContain is a one-rider mobile module.  The transaction
        // below writes the mobile occupant list directly after preflight; a
        // malformed structure carrying the same Behavior would otherwise
        // have a different authoritative list (`BuildingData`) and could
        // lose the rider.  Reject it before the first mutation.
        if container.building_data.is_some() {
            return None;
        }
        let rider_metadata = container
            .authored_rider_change_rider_for_template(&rider.template_name)?
            .clone();
        // Parse-time metadata can outlive a LocomotorStore reload or an old
        // snapshot.  Re-resolve the complete exact authored SET_NORMAL /
        // SET_SLUGGISH row before any eject mutation; no generic
        // template/default locomotor (nor a single arbitrarily chosen
        // surface member) is a valid substitute for this RiderN set.
        let active_locomotor_name = rider_metadata.active_locomotor_name.clone()?;
        let set_ok = rider_metadata
            .locomotor_set
            .eq_ignore_ascii_case("SET_NORMAL")
            || rider_metadata
                .locomotor_set
                .eq_ignore_ascii_case("SET_SLUGGISH");
        if !set_ok
            || rider_metadata.active_locomotor_names.is_empty()
            || rider_metadata.active_locomotor_surfaces == 0
        {
            return None;
        }

        let complete =
            crate::game_logic::locomotor_bootstrap::resolve_complete_host_locomotor_set(
                &rider_metadata.active_locomotor_names,
            )?;
        let names_match_metadata = complete.locomotor_names.len()
            == rider_metadata.active_locomotor_names.len()
            && complete
                .locomotor_names
                .iter()
                .zip(rider_metadata.active_locomotor_names.iter())
                .all(|(resolved, parsed)| resolved.eq_ignore_ascii_case(parsed));
        if !names_match_metadata
            || !complete
                .representative_name
                .eq_ignore_ascii_case(&active_locomotor_name)
            || complete.locomotor_surfaces != rider_metadata.active_locomotor_surfaces
        {
            // Do not let stale parsed metadata turn a changed/ambiguous
            // Locomotor store into a partial replacement transaction.
            return None;
        }
        // C++ `RiderChangeContain::onContaining` calls
        // ai->chooseLocomotorSet(SET_*) (RiderChangeContain.cpp:215) and the
        // AI then binds the member for the current position surface
        // (`chooseGoodLocomotorFromCurrentSet`, AIUpdate.cpp:828-853).
        let container_cell = self
            .pathfinding_system
            .grid
            .world_to_grid(container.get_position());
        let acceptable = crate::game_logic::locomotor_bootstrap::valid_locomotor_surfaces_for_cell_type(
            self.pathfinding_system.grid.cell_type(container_cell),
        );
        let (active_locomotor_name, active_locomotor) =
            crate::game_logic::locomotor_bootstrap::choose_host_locomotor_set_member_for_surfaces(
                &complete.locomotor_names,
                acceptable,
            )?;
        let occupants = container.contained_units();
        if occupants.len() > 1 {
            // The source C++ module has one active rider.  A malformed/stale
            // host roster cannot be safely reduced by guessing which unit to
            // evict, so reject before touching either side.
            return None;
        }
        let incoming_selection_player = rider
            .owner_player_id
            .filter(|player_id| rider.selected && self.is_local_player(*player_id));
        if occupants.first().copied() == Some(rider_id) {
            // A stale duplicate Enter is harmless only when the relationship
            // is already internally consistent; the caller repairs the rider
            // state below without adding it a second time.
            return (rider.contained_by == Some(container_id)).then_some(RiderChangeEnterPlan {
                rider: rider_metadata,
                active_locomotor_name,
                active_locomotor,
                previous_rider: Some(rider_id),
                container_position: container.get_position(),
                incoming_selection_player,
                incoming_was_selected: incoming_selection_player.is_some(),
                incoming_veterancy: rider.experience.level,
            });
        }
        if rider.contained_by.is_some() {
            // `execute_enter` removes an old container before pathing.  If a
            // stale link survives that stage, do not let this transaction make
            // a unit belong to two containers.
            return None;
        }
        let previous_rider = occupants.first().copied();
        if let Some(previous_rider_id) = previous_rider {
            let previous = self.objects.get(&previous_rider_id)?;
            if previous_rider_id == rider_id
                || !previous.is_alive()
                || previous.contained_by != Some(container_id)
                || container
                    .authored_rider_change_rider_for_template(&previous.template_name)
                    .is_none()
            {
                return None;
            }
        }
        if incoming_selection_player.is_some() && !container.selected && !container.is_selectable()
        {
            // Selection transfer is a modeled C++ side effect.  Do not drop a
            // selected rider into an unselectable custom container and pretend
            // the transfer succeeded.
            return None;
        }
        Some(RiderChangeEnterPlan {
            rider: rider_metadata,
            active_locomotor_name,
            active_locomotor,
            previous_rider,
            container_position: container.get_position(),
            incoming_selection_player,
            incoming_was_selected: incoming_selection_player.is_some(),
            incoming_veterancy: rider.experience.level,
        })
    }

    /// Complete the C++ `RiderChangeContain::onContaining` replacement at the
    /// arrival boundary.  This intentionally bypasses generic
    /// `Object::add_occupant`: C++ ignores capacity, ejects the prior rider,
    /// and boards the new rider as one ordered transaction.
    pub(in super::super::super) fn rider_change_enter_at_arrival(
        &mut self,
        rider_id: ObjectId,
        container_id: ObjectId,
    ) -> bool {
        let Some(plan) = self.rider_change_enter_plan(rider_id, container_id) else {
            return false;
        };

        // Idempotent duplicate: retain the valid existing relation but repair
        // the arrival state without disturbing veterancy or rider effects.
        if plan.previous_rider == Some(rider_id) {
            if let Some(rider) = self.objects.get_mut(&rider_id) {
                rider.stop_moving();
                rider.set_target(Some(container_id));
                rider.set_contained_by(Some(container_id));
                rider.set_position(plan.container_position);
                rider.set_ai_state(AIState::Docked);
                rider.set_status_moving(false);
                rider.set_status_attacking(false);
            }
            return true;
        }

        // C++ OpenContain::addToContain checkAndDetonateBoobyTrap(rider).
        if self.should_cancel_containment_after_booby_trap(container_id, rider_id) {
            return false;
        }

        // Phase 1: eject the old rider.  All IDs/list membership were checked
        // above, so the remove cannot fail after any state has changed.
        if let Some(previous_rider_id) = plan.previous_rider {
            let (previous_metadata, bike_veterancy, previous_has_controller) = {
                let container = self
                    .objects
                    .get(&container_id)
                    .expect("RiderChange preflight container disappeared");
                let previous = self
                    .objects
                    .get(&previous_rider_id)
                    .expect("RiderChange preflight rider disappeared");
                (
                    container
                        .authored_rider_change_rider_for_template(&previous.template_name)
                        .expect("RiderChange preflight roster disappeared")
                        .clone(),
                    container.experience.level,
                    previous.owner_player_id.is_some(),
                )
            };
            let removed = self
                .objects
                .get_mut(&container_id)
                .expect("RiderChange preflight container disappeared")
                .remove_occupant(previous_rider_id);
            debug_assert!(removed, "RiderChange preflight must make eject infallible");
            if !removed {
                return false;
            }
            if let Some(previous) = self.objects.get_mut(&previous_rider_id) {
                previous.stop_moving();
                // C++ delegates the exact exit-door/path placement to
                // TransportContain::aiEvacuateInstantly.  That authored
                // exit-path matrix is not represented by this bounded host;
                // keep the ejected rider at the container origin instead of
                // fabricating a pseudo-random offset.
                previous.set_position(plan.container_position);
                previous.set_contained_by(None);
                previous.set_target(None);
                previous.set_ai_state(AIState::Idle);
                previous.set_status_moving(false);
                previous.set_status_attacking(false);
                previous.deselect();
                if previous_has_controller {
                    previous.set_rider_change_veterancy_level(bike_veterancy);
                }
            }
            if let Some(container) = self.objects.get_mut(&container_id) {
                clear_rider_change_runtime_state(container, Some(&previous_metadata));
                if previous_has_controller {
                    container.set_rider_change_veterancy_level(VeterancyLevel::Rookie);
                }
            }
        } else if let Some(container) = self.objects.get_mut(&container_id) {
            // An initial-payload Combat Cycle can have a visible legacy rider
            // without a tracked Object.  Clear only its existing host bridge
            // before applying the explicitly parsed replacement.
            clear_rider_change_runtime_state(container, None);
        }

        // Phase 2: apply the exact parsed RiderN state and publish the sole
        // contained body.  There are no capacity checks or fallible template
        // lookups after the preflight.
        {
            let container = self
                .objects
                .get_mut(&container_id)
                .expect("RiderChange preflight container disappeared");
            container.model_condition_bits |= plan.rider.model_condition_mask;
            container.object_status_bits |= plan.rider.object_status_mask;
            container.rider_change_active_slot = Some(plan.rider.slot);
            container.rider_change_model_condition_mask = plan.rider.model_condition_mask;
            container.rider_change_object_status_mask = plan.rider.object_status_mask;
            container.rider_change_weapon_set = Some(plan.rider.weapon_set.clone());
            container.rider_change_locomotor_set = Some(plan.rider.locomotor_set.clone());
            container.rider_change_locomotor_name = Some(plan.active_locomotor_name.clone());
            container.set_command_set_override(Some(plan.rider.command_set.clone()));
            if container.status.stealthed {
                container.set_status_detected(true);
            }
            container.occupants.push(rider_id);
            container.record_host_model_condition();
            apply_rider_change_locomotor_binding(container, &plan.active_locomotor);
        }
        {
            let rider = self
                .objects
                .get_mut(&rider_id)
                .expect("RiderChange preflight incoming rider disappeared");
            rider.stop_moving();
            rider.set_status_attacking(false);
            rider.target_location = None;
            rider.set_status_force_attack(false);
            rider.set_target(Some(container_id));
            rider.set_contained_by(Some(container_id));
            rider.set_position(plan.container_position);
            rider.set_ai_state(AIState::Docked);
            rider.set_status_moving(false);
            rider.set_rider_change_veterancy_level(VeterancyLevel::Rookie);
            rider.deselect();
        }

        // This is a visual/weapon mirror only.  It receives the parsed RiderN
        // ordinal, never the incoming rider's template spelling; the generic
        // `refresh_combat_cycle_rider_weapon` is intentionally not called.
        let switched = self.apply_combat_cycle_rider(
            container_id,
            crate::game_logic::host_combat_cycle::CombatCycleRider::from_u8(plan.rider.slot),
        );
        debug_assert!(
            switched,
            "RiderChange preflight must make weapon bridge infallible"
        );
        if let Some(container) = self.objects.get_mut(&container_id) {
            container.set_rider_change_veterancy_level(plan.incoming_veterancy);
            if plan.incoming_was_selected && !container.selected {
                container.select();
            }
        }
        if let Some(player_id) = plan.incoming_selection_player {
            // C++ sends MSG_CREATE_SELECTED_GROUP(false, bike) before
            // TransportContain automatically deselects the rider.  Mirror
            // both the per-object bits above and the authoritative local
            // selection roster, preserving every unrelated selected unit.
            if let Some(player) = self.players.get_mut(&player_id) {
                player.selected_objects.retain(|id| *id != rider_id);
                if !player.selected_objects.contains(&container_id) {
                    player.selected_objects.push(container_id);
                }
            }
        }
        self.record_combat_cycle_residual_load();
        true
    }

    /// C++ `RiderChangeContain::onRemoving` for an ordinary player Exit (or
    /// moving a rider to a different container).  This is deliberately
    /// separate from replacement above: only ordinary removal starts the
    /// delayed scuttle and transfers selected state back to the rider.
    pub(in super::super::super) fn rider_change_remove_occupant(
        &mut self,
        container_id: ObjectId,
        rider_id: ObjectId,
    ) -> bool {
        let Some((
            rider_metadata,
            position,
            bike_veterancy,
            rider_has_controller,
            was_moving,
            transfer_selection,
        )) = self.objects.get(&container_id).and_then(|container| {
            if !container.supports_authored_rider_change_normal_enter()
                || container.contained_units() != vec![rider_id]
            {
                return None;
            }
            let rider = self.objects.get(&rider_id)?;
            if rider.contained_by != Some(container_id) || !rider.is_alive() {
                return None;
            }
            let rider_metadata = container
                .authored_rider_change_rider_for_template(&rider.template_name)?
                .clone();
            let transfer_selection = container.selected
                && container
                    .owner_player_id
                    .is_some_and(|player_id| self.is_local_player(player_id));
            Some((
                rider_metadata,
                container.get_position(),
                container.experience.level,
                rider.owner_player_id.is_some(),
                container.status.moving,
                transfer_selection,
            ))
        })
        else {
            return false;
        };

        let removed = self
            .objects
            .get_mut(&container_id)
            .expect("RiderChange preflight container disappeared")
            .remove_occupant(rider_id);
        debug_assert!(removed, "RiderChange preflight must make exit infallible");
        if !removed {
            return false;
        }

        if let Some(rider) = self.objects.get_mut(&rider_id) {
            rider.stop_moving();
            rider.set_position(position);
            rider.set_contained_by(None);
            rider.set_target(None);
            rider.set_ai_state(AIState::Idle);
            rider.set_status_moving(false);
            rider.set_status_attacking(false);
            if rider_has_controller {
                rider.set_rider_change_veterancy_level(bike_veterancy);
            }
        }
        if let Some(container) = self.objects.get_mut(&container_id) {
            clear_rider_change_runtime_state(container, Some(&rider_metadata));
            if rider_has_controller {
                container.set_rider_change_veterancy_level(VeterancyLevel::Rookie);
            }
            container.rider_change_scuttled_on_frame = self.frame.max(1);
            container.set_status_unselectable(true);
            container.model_condition_bits |= container
                .thing
                .template
                .contain_module
                .rider_change_scuttle_status_mask;
            if !was_moving {
                let _ = container.apply_status_bits_upgrade_masks(&["IMMOBILE"], &[]);
            }
            container.record_host_model_condition();
            if transfer_selection {
                container.deselect();
            }
        }
        if transfer_selection {
            if let Some(rider) = self.objects.get_mut(&rider_id) {
                rider.select();
            }
            if let Some(player_id) = self
                .objects
                .get(&container_id)
                .and_then(|container| container.owner_player_id)
            {
                if let Some(player) = self.players.get_mut(&player_id) {
                    player.selected_objects.retain(|id| *id != container_id);
                    if !player.selected_objects.contains(&rider_id) {
                        player.selected_objects.push(rider_id);
                    }
                }
            }
        }
        true
    }

    /// C++ `OpenContain::onCollide` (`OpenContain.cpp:777-814`): when a unit
    /// whose `getEnterTarget` is this container arrives, eject every rider
    /// whose controlling player differs. `KINDOF_STEALTH_GARRISON` riders are
    /// `markAsDetected` so they do not stay cloaked after the kick-out.
    pub(super) fn eject_foreign_occupants_on_enter(
        &mut self,
        container_id: ObjectId,
        entering_id: ObjectId,
    ) {
        let Some(enterer) = self.objects.get(&entering_id) else {
            return;
        };
        let enterer_owner = enterer.owner_player_id;
        let enterer_team = enterer.team;
        let Some((position, occupants)) = self
            .objects
            .get(&container_id)
            .map(|container| (container.get_position(), container.contained_units()))
        else {
            return;
        };

        let mut kicked = Vec::new();
        for occupant_id in occupants {
            let Some(occupant) = self.objects.get(&occupant_id) else {
                continue;
            };
            let different_player = match (occupant.owner_player_id, enterer_owner) {
                (Some(a), Some(b)) => a != b,
                _ => occupant.team != enterer_team,
            };
            if !different_player {
                continue;
            }
            let stealth_garrison = occupant.is_kind_of(KindOf::StealthGarrison);
            kicked.push((occupant_id, stealth_garrison));
        }
        if kicked.is_empty() {
            return;
        }

        if let Some(container) = self.objects.get_mut(&container_id) {
            for (occupant_id, _) in &kicked {
                let _ = container.remove_occupant(*occupant_id);
            }
        }
        for (occupant_id, stealth_garrison) in kicked {
            let _ = self.unit_command_exit_drop(occupant_id, position);
            if stealth_garrison {
                if let Some(occupant) = self.objects.get_mut(&occupant_id) {
                    occupant.set_status_detected(true);
                }
            }
        }
        self.recalc_garrison_apparent_controller(container_id);
    }

    /// C++ `GarrisonContain::removeAllContained(TRUE)` on a capture trigger.
    /// It exposes and ejects occupants; it does not kill them and it does not
    /// defect the garrisonable structure in that same ability use.
    pub(super) fn evacuate_garrison_for_capture(&mut self, structure_id: ObjectId) {
        let Some((position, occupants)) = self
            .objects
            .get(&structure_id)
            .map(|structure| (structure.get_position(), structure.contained_units()))
        else {
            return;
        };

        if let Some(structure) = self.objects.get_mut(&structure_id) {
            for occupant_id in &occupants {
                let _ = structure.remove_occupant(*occupant_id);
            }
        }

        for occupant_id in occupants {
            let expose_stealth = self
                .objects
                .get(&occupant_id)
                .is_some_and(|occupant| occupant.status.stealthed);
            let _ = self.unit_command_exit_drop(occupant_id, position);
            if expose_stealth {
                if let Some(occupant) = self.objects.get_mut(&occupant_id) {
                    occupant.set_status_detected(true);
                }
            }
        }
    }

    /// C++ `SpecialAbilityUpdate::startPreparation` for a capture power.
    /// Crucially, the real SpecialPower timer starts here—not when the player
    /// clicks a distant target. `consume_special_power_charge_for` supplies
    /// the existing per-ability timer and paused/disabled readiness gates.
    pub(super) fn start_capture_preparation(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        power: crate::game_logic::CapturePowerKind,
        preparation_time_ms: u32,
    ) -> bool {
        let Some(power_type) = power.special_power_type() else {
            return false;
        };
        let Some(source_team) = self.objects.get(&object_id).map(|object| object.team) else {
            return false;
        };

        // C++ checks/detonates a hostile trap when preparation begins, before
        // it marks the SpecialPower as triggered or begins its progress bar.
        let trap_position = self
            .objects
            .get(&target_id)
            .map(|target| target.get_position());
        let planter_is_ally = self
            .booby_trap
            .plant(target_id)
            .map(|plant| plant.planter_team == source_team)
            .unwrap_or(false);
        if let Some(trap_position) = trap_position {
            let target_is_trapped = self.booby_trap.is_booby_trapped(target_id)
                || self
                    .objects
                    .get(&target_id)
                    .map(|target| target.status.booby_trapped)
                    .unwrap_or(false);
            if !planter_is_ally && target_is_trapped {
                let _ = self.detonate_booby_trap_at(
                    target_id,
                    trap_position,
                    Some(object_id),
                    true,
                    false,
                );
            }
        }

        if !self.can_unit_capture_building(object_id, target_id, false)
            || !self.consume_special_power_charge_for(object_id, &power_type)
        {
            return false;
        }

        // C++ starts victim warning/infiltration exactly as the preparation
        // timer begins, giving the defender time to react before ownership
        // changes at trigger time.
        self.try_eva_building_being_stolen(target_id);
        self.try_infiltration_event(target_id);

        let Some(object) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !object.is_alive() {
            return false;
        }
        object.stop_moving();
        object.set_ai_state(AIState::Capturing);
        object.capture_channel = Some(crate::game_logic::CaptureChannelState::new(
            crate::game_logic::CaptureChannelPhase::Preparing,
            preparation_time_ms,
        ));
        object.set_status_using_ability(true);
        let infantry_flag = matches!(
            power,
            crate::game_logic::CapturePowerKind::Ranger
                | crate::game_logic::CapturePowerKind::RedGuard
                | crate::game_logic::CapturePowerKind::Rebel
        );
        let lotus = matches!(power, crate::game_logic::CapturePowerKind::BlackLotus);
        if infantry_flag {
            use crate::game_logic::host_enum_table_residual::{
                raising_flag_model_bit, unpacking_model_bit,
            };
            object.model_condition_bits &= !(1u128 << unpacking_model_bit());
            object.model_condition_bits |= 1u128 << raising_flag_model_bit();
            object.record_host_model_condition();
        }
        drop(object);
        if lotus {
            self.leftover_sa_set_pack_model(object_id, false, false, true);
            let prep_frames =
                crate::game_logic::host_hero_abilities::hero_ms_to_frames(preparation_time_ms)
                    .max(1);
            let _ = self.leftover_spawn_binary_data_stream(
                object_id,
                target_id,
                prep_frames,
                crate::game_logic::host_hero_abilities::LOTUS_CAPTURE_SPECIAL_OBJECT,
            );
            self.queue_object_named_audio(
                object_id,
                crate::game_logic::host_hero_abilities::LOTUS_PREP_SOUND_LOOP,
                140,
                true,
                false,
            );
        }
        // C++ startPreparation markSpecialPowerTriggered → ScriptEngine TRIGGERED.
        self.notify_script_engine_special_power_event(object_id, &power_type, true, false);
        true
    }

    /// Clear a completed Hacker Disable Building channel.  Unlike a generic
    /// `PendingSpecialAbility`, HDB has an authored PackTime, so this is only
    /// called after the packing timer has completed (or when the source itself
    /// is no longer able to pack).  Keep the order, channel and visible
    /// `IS_USING_ABILITY` state in one place so a later command cannot inherit
    /// an old physical channel.
    fn finish_hacker_disable_building_channel(&mut self, object_id: ObjectId) {
        self.stop_hacker_disable_prep_sound_loop(object_id);
        self.leftover_kill_special_objects(object_id);
        self.pending_special_abilities.remove(&object_id);
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.stop_moving();
            object.hacker_disable_channel = None;
            object.set_status_using_ability(false);
            object.set_target(None);
            object.set_ai_state(AIState::Idle);
        }
    }

    /// Begin the parsed HDB `PackTime` after an interrupted or completed
    /// channel.  C++ packs on target death, alliance/relation loss, range
    /// abort, and after a non-persistent trigger; it does not instantly turn
    /// the hacker idle in any of those cases.
    fn begin_hacker_disable_building_packing(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        pack_time_ms: u32,
    ) {
        self.stop_hacker_disable_prep_sound_loop(object_id);
        self.leftover_kill_special_objects(object_id);
        let mut finish_now = false;
        let pack_time_ms = self
            .objects
            .get(&object_id)
            .and_then(|object| object.thing.template.hacker_disable_building.as_ref())
            .map(|meta| {
                crate::game_logic::vary_pack_unpack_duration_ms(
                    pack_time_ms,
                    meta.pack_unpack_variation_factor,
                )
            })
            .unwrap_or(pack_time_ms);
        if let Some(object) = self.objects.get_mut(&object_id) {
            if !object.is_alive() {
                finish_now = true;
            } else {
                object.stop_moving();
                object.hacker_disable_channel =
                    Some(crate::game_logic::HackerDisableChannelState::new(
                        target_id,
                        crate::game_logic::HackerDisableChannelPhase::Packing,
                        pack_time_ms,
                    ));
                object.set_status_using_ability(false);
                object.set_ai_state(AIState::SpecialAbility);
            }
        } else {
            finish_now = true;
        }
        if !finish_now {
            self.queue_hacker_disable_pack_unpack_sound(object_id, true);
        }
        if finish_now || pack_time_ms == 0 {
            self.finish_hacker_disable_building_channel(object_id);
        }
    }

    /// Resolve the live ownership relationship for a channel already in
    /// flight.  Exact player ownership is authoritative; the old team path is
    /// retained only for wholly ownerless synthetic worlds so same-faction
    /// different-player objects never become accidental HDB targets.
    fn hacker_disable_building_channel_has_enemy_relation(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        use gamelogic::common::Relationship;

        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let mut relation = match (source.owner_player_id, target.owner_player_id) {
            (Some(source_owner), Some(target_owner))
                if self.player_owner_for_host_object(source) == Some(source_owner)
                    && self.player_owner_for_host_object(target) == Some(target_owner) =>
            {
                self.player_relationship(source_owner, target_owner)
            }
            (None, None) if self.uses_legacy_team_ownership_fallback() => {
                if source.team == target.team {
                    Relationship::Allies
                } else if source.team == Team::Neutral || target.team == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            }
            _ => Relationship::Neutral,
        };
        // C++ Object::getRelationship (Object.cpp:1548-1568) default
        // hostility: two living players on different non-neutral teams are
        // ENEMIES even when the lobby carries no explicit playerEnemies row.
        // The click-authority gate (can_unit_hacker_disable_building) applies
        // this fallback; the running channel must not be stricter than the
        // click that opened it.
        if relation == Relationship::Neutral
            && source.team != Team::Neutral
            && target.team != Team::Neutral
            && source.team != target.team
        {
            relation = Relationship::Enemies;
        }
        relation == Relationship::Enemies
    }

    /// C++ `SpecialAbilityUpdate::isWithinStartAbilityRange` uses a 2D
    /// bounding-sphere envelope and shaves one quarter of a pathfinding cell
    /// from the approach threshold.  It is deliberately not the old fixed
    /// 150-unit HDB residual.
    fn hacker_disable_building_within_start_range(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        metadata: &crate::game_logic::HackerDisableBuildingMetadata,
    ) -> bool {
        const PATHFIND_CELL_SIZE_F: f32 = 10.0;

        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let source_position = source.get_position();
        let target_position = target.get_position();
        let dx = source_position.x - target_position.x;
        let dz = source_position.z - target_position.z;
        let edge_distance = ((dx * dx + dz * dz).sqrt()
            - source.selection_radius.max(0.0)
            - target.selection_radius.max(0.0))
        .max(0.0);
        let start_range = (metadata.start_ability_range - PATHFIND_CELL_SIZE_F * 0.25).max(0.0);
        if edge_distance > start_range {
            return false;
        }
        if !metadata.approach_requires_los {
            return true;
        }
        let source_eye = glam::Vec3::new(
            source_position.x,
            source_position.y + source.selection_radius.max(5.0) * 0.5,
            source_position.z,
        );
        let target_eye = glam::Vec3::new(
            target_position.x,
            target_position.y + target.selection_radius.max(5.0) * 0.5,
            target_position.z,
        );
        self.is_clear_line_of_sight_terrain(source_eye, target_eye)
    }

    /// C++ `SpecialAbilityUpdate::isWithinAbilityAbortRange` has no start
    /// range undersize.  The source module default is effectively infinite,
    /// so only a finite authored abort range can interrupt preparation.
    fn hacker_disable_building_within_abort_range(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        metadata: &crate::game_logic::HackerDisableBuildingMetadata,
    ) -> bool {
        const DEFAULT_ABILITY_RANGE: f32 = 10_000_000.0;

        if metadata.ability_abort_range >= DEFAULT_ABILITY_RANGE {
            return true;
        }
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let source_position = source.get_position();
        let target_position = target.get_position();
        let dx = source_position.x - target_position.x;
        let dz = source_position.z - target_position.z;
        let edge_distance = ((dx * dx + dz * dz).sqrt()
            - source.selection_radius.max(0.0)
            - target.selection_radius.max(0.0))
        .max(0.0);
        edge_distance <= metadata.ability_abort_range
    }

    /// Enter HDB preparation and begin the exact parsed SpecialPower reload.
    /// The executor freezes click-time readiness, but this repeats the C++
    fn start_hacker_disable_building_preparation(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        metadata: &crate::game_logic::HackerDisableBuildingMetadata,
    ) -> bool {
        if !self.can_unit_hacker_disable_building(object_id, target_id, false)
            || !self.consume_hacker_disable_building_charge(object_id)
        {
            return false;
        }
        if self
            .objects
            .get(&object_id)
            .is_none_or(|object| !object.is_alive())
        {
            return false;
        }
        let prep_frames = crate::game_logic::host_hero_abilities::hero_ms_to_frames(
            metadata
                .preparation_time_ms
                .max(metadata.persistent_prep_time_ms),
        )
        .max(1);
        let _ = self.leftover_spawn_binary_data_stream(
            object_id,
            target_id,
            prep_frames,
            crate::game_logic::host_hacker_disable::HACKER_DISABLE_SPECIAL_OBJECT,
        );
        self.try_infiltration_event(target_id);
        self.hero_abilities.record_leftover_infiltration();
        // C++ startPreparation: UNPACKING → FIRING_A (hacker typing / microwave).
        self.leftover_sa_set_pack_model(object_id, false, false, true);
        self.queue_hacker_disable_prep_sound_loop(object_id);
        let power = metadata.command_power();
        self.notify_script_engine_special_power_event(object_id, &power, true, false);
        let Some(object) = self.objects.get_mut(&object_id) else {
            return false;
        };
        object.stop_moving();
        object.hacker_disable_channel = Some(crate::game_logic::HackerDisableChannelState::new(
            target_id,
            crate::game_logic::HackerDisableChannelPhase::Preparing,
            metadata.preparation_time_ms,
        ));
        object.set_status_using_ability(true);
        object.set_ai_state(AIState::SpecialAbility);
        true
    }

    /// Trigger the HDB effect at the authored preparation boundary.  A target
    /// that is already DISABLED_HACKED remains legal; C++ refreshes its
    /// `EffectDuration` and continues the persistent channel.
    fn trigger_hacker_disable_building(
        &mut self,
        object_id: ObjectId,
        target_id: ObjectId,
        metadata: &crate::game_logic::HackerDisableBuildingMetadata,
    ) {
        let duration_frames =
            ((metadata.effect_duration_ms as u64 * 30 + 999) / 1_000).min(u32::MAX as u64) as u32;
        let until = self.frame.saturating_add(duration_frames);
        if let Some(target) = self.objects.get_mut(&target_id) {
            target.apply_disabled_hacked(until);
        } else {
            self.begin_hacker_disable_building_packing(object_id, target_id, metadata.pack_time_ms);
            return;
        }
        let do_fx = *self
            .hero_abilities
            .disable_fx_toggle
            .get(&object_id)
            .unwrap_or(&true);
        let new_do_fx = self.leftover_spawn_disable_fx(
            object_id,
            target_id,
            crate::game_logic::host_hacker_disable::HACKER_DISABLE_FX_PARTICLE,
            duration_frames,
            do_fx,
        );
        self.hero_abilities
            .disable_fx_toggle
            .insert(object_id, new_do_fx);
        self.hacker_disable_building_count = self.hacker_disable_building_count.saturating_add(1);

        // HDB is an intrinsic persistent SpecialAbilityUpdate. An omitted
        // PersistentPrepTime is the C++ zero default, not a signal to turn it
        // into a one-shot ability; retain it as a zero-duration preparation
        // which may trigger on the following logic update without an in-tick
        // infinite loop.
        // `PersistenceRequiresRecharge` is the only persistent path that
        // starts/gates another reload. Ordinary HDB uses its authored
        // PersistentPrepTime continuously while the target remains legal.
        if metadata.persistence_requires_recharge
            && !self.consume_hacker_disable_building_charge(object_id)
        {
            self.begin_hacker_disable_building_packing(object_id, target_id, metadata.pack_time_ms);
            return;
        }
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.hacker_disable_channel =
                Some(crate::game_logic::HackerDisableChannelState::new(
                    target_id,
                    crate::game_logic::HackerDisableChannelPhase::Preparing,
                    metadata.persistent_prep_time_ms,
                ));
            object.set_status_using_ability(true);
            object.set_ai_state(AIState::SpecialAbility);
        }
    }

    /// Dedicated C++ `SpecialAbilityUpdate` HDB state machine.  This lives
    /// ahead of generic PendingSpecialAbility handling so the old fixed range,
    /// instant effect, and "already hacked" rejection cannot accidentally
    /// re-enter the player-facing command path.
    pub(super) fn update_hacker_disable_building_channel(
        &mut self,
        object_id: ObjectId,
        pending_target_id: ObjectId,
        dt: f32,
    ) {
        const COMPLETE_EPSILON: f32 = 0.000_1;

        let Some((metadata, channel)) = self.objects.get(&object_id).and_then(|object| {
            object
                .thing
                .template
                .hacker_disable_building
                .clone()
                .map(|metadata| (metadata, object.hacker_disable_channel))
        }) else {
            self.finish_hacker_disable_building_channel(object_id);
            return;
        };
        let Some(channel) = channel else {
            self.finish_hacker_disable_building_channel(object_id);
            return;
        };
        if channel.target_id != pending_target_id {
            self.begin_hacker_disable_building_packing(
                object_id,
                channel.target_id,
                metadata.pack_time_ms,
            );
            return;
        }

        // Packing remains meaningful after its target dies, defects, or
        // leaves visibility.  It is the only phase that intentionally does
        // not ask the live target authority again.
        if channel.phase == crate::game_logic::HackerDisableChannelPhase::Packing {
            let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
            if remaining > COMPLETE_EPSILON {
                if let Some(object) = self.objects.get_mut(&object_id) {
                    object.hacker_disable_channel =
                        Some(crate::game_logic::HackerDisableChannelState {
                            target_id: channel.target_id,
                            phase: crate::game_logic::HackerDisableChannelPhase::Packing,
                            remaining_seconds: remaining,
                        });
                }
            } else {
                self.finish_hacker_disable_building_channel(object_id);
            }
            return;
        }

        let source_valid = self.objects.get(&object_id).is_some_and(|source| {
            source.is_alive()
                && !source.is_disabled()
                && metadata.update_module_starts_attack
                && !metadata.scripted_special_power_only
        });
        if !source_valid {
            self.finish_hacker_disable_building_channel(object_id);
            return;
        }
        let target_alive = self
            .objects
            .get(&channel.target_id)
            .is_some_and(|target| target.is_alive());
        if !target_alive
            || !self
                .hacker_disable_building_channel_has_enemy_relation(object_id, channel.target_id)
        {
            self.begin_hacker_disable_building_packing(
                object_id,
                channel.target_id,
                metadata.pack_time_ms,
            );
            return;
        }

        match channel.phase {
            crate::game_logic::HackerDisableChannelPhase::Approaching => {
                // Revalidate typed click authority after physical movement but
                // never re-demand the cooldown that is intentionally spent
                // only once preparation begins.
                if !self.can_unit_hacker_disable_building(object_id, channel.target_id, false) {
                    self.begin_hacker_disable_building_packing(
                        object_id,
                        channel.target_id,
                        metadata.pack_time_ms,
                    );
                    return;
                }
                if self.hacker_disable_building_within_start_range(
                    object_id,
                    channel.target_id,
                    &metadata,
                ) {
                    if crate::game_logic::host_hacker_disable::hacker_disable_need_to_face()
                        && self.leftover_sa_unit_can_face(object_id)
                        && (self.leftover_continue_facing(object_id, channel.target_id, dt) || {
                            self.leftover_start_facing(object_id, channel.target_id);
                            self.leftover_continue_facing(object_id, channel.target_id, dt)
                        })
                    {
                        return;
                    }
                    if metadata.unpack_time_ms == 0 {
                        if self.start_hacker_disable_building_preparation(
                            object_id,
                            channel.target_id,
                            &metadata,
                        ) && metadata.preparation_time_ms == 0
                        {
                            self.trigger_hacker_disable_building(
                                object_id,
                                channel.target_id,
                                &metadata,
                            );
                        } else if self
                            .objects
                            .get(&object_id)
                            .is_some_and(|object| object.hacker_disable_channel.is_none())
                        {
                            self.begin_hacker_disable_building_packing(
                                object_id,
                                channel.target_id,
                                metadata.pack_time_ms,
                            );
                        }
                    } else {
                        let unpack_ms = crate::game_logic::vary_pack_unpack_duration_ms(
                            metadata.unpack_time_ms,
                            metadata.pack_unpack_variation_factor,
                        );
                        if let Some(object) = self.objects.get_mut(&object_id) {
                            object.stop_moving();
                            object.hacker_disable_channel =
                                Some(crate::game_logic::HackerDisableChannelState::new(
                                    channel.target_id,
                                    crate::game_logic::HackerDisableChannelPhase::Unpacking,
                                    unpack_ms,
                                ));
                            object.set_status_using_ability(false);
                            object.set_ai_state(AIState::SpecialAbility);
                            drop(object);
                            self.queue_hacker_disable_pack_unpack_sound(object_id, false);
                        }
                    }
                } else if self.objects.get(&object_id).is_some_and(Object::can_move) {
                    let target_position = self
                        .objects
                        .get(&channel.target_id)
                        .map(Object::get_position)
                        .unwrap_or_default();
                    self.path_approach_with_state(
                        object_id,
                        target_position,
                        AIState::SpecialAbility,
                    );
                } else {
                    self.begin_hacker_disable_building_packing(
                        object_id,
                        channel.target_id,
                        metadata.pack_time_ms,
                    );
                }
            }
            crate::game_logic::HackerDisableChannelPhase::Unpacking => {
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if remaining > COMPLETE_EPSILON {
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.hacker_disable_channel =
                            Some(crate::game_logic::HackerDisableChannelState {
                                target_id: channel.target_id,
                                phase: crate::game_logic::HackerDisableChannelPhase::Unpacking,
                                remaining_seconds: remaining,
                            });
                    }
                } else if self.start_hacker_disable_building_preparation(
                    object_id,
                    channel.target_id,
                    &metadata,
                ) && metadata.preparation_time_ms == 0
                {
                    self.trigger_hacker_disable_building(object_id, channel.target_id, &metadata);
                } else if self
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.hacker_disable_channel.is_none())
                {
                    self.begin_hacker_disable_building_packing(
                        object_id,
                        channel.target_id,
                        metadata.pack_time_ms,
                    );
                }
            }
            crate::game_logic::HackerDisableChannelPhase::Preparing => {
                let target_is_pure_stealth = self
                    .objects
                    .get(&channel.target_id)
                    .is_some_and(Object::is_effectively_stealthed);
                if target_is_pure_stealth
                    || !self.hacker_disable_building_within_abort_range(
                        object_id,
                        channel.target_id,
                        &metadata,
                    )
                {
                    self.begin_hacker_disable_building_packing(
                        object_id,
                        channel.target_id,
                        metadata.pack_time_ms,
                    );
                    return;
                }
                // Source C++ waits at the persistent preparation boundary
                // only when this exact module opted into recharge gating.
                if metadata.persistence_requires_recharge
                    && !self.is_hacker_disable_building_ready(object_id)
                {
                    return;
                }
                let remaining = (channel.remaining_seconds - dt.max(0.0)).max(0.0);
                if remaining > COMPLETE_EPSILON {
                    if let Some(object) = self.objects.get_mut(&object_id) {
                        object.hacker_disable_channel =
                            Some(crate::game_logic::HackerDisableChannelState {
                                target_id: channel.target_id,
                                phase: crate::game_logic::HackerDisableChannelPhase::Preparing,
                                remaining_seconds: remaining,
                            });
                    }
                } else {
                    self.trigger_hacker_disable_building(object_id, channel.target_id, &metadata);
                }
            }
            crate::game_logic::HackerDisableChannelPhase::Packing => {
                unreachable!("packing is handled before HDB participant validation")
            }
        }
    }
}
