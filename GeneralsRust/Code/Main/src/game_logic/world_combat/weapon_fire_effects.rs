//! CPP Weapon.cpp889–949: fire FX, then fire OCL, before damage/flight.
use super::super::*;
use super::AcceptedWeaponDischarge;

struct AcceptedSelfDamagePose {
    epoch: u64,
    generation: u64,
}

impl GameLogic {
    /// Finish one prepared shot synchronously, after releasing its Object loan.
    /// All callers share CPP FireFX -> FireOCL -> delivery -> barrel ordering.
    pub(in crate::game_logic) fn deliver_owned_weapon_shot(
        &mut self,
        slot: u8,
        mut shot: crate::game_logic::combat::PendingProjectile,
        capture: Option<super::weapon_visual_capture::PendingWeaponVisualDispatchCapture>,
    ) -> bool {
        self.deliver_owned_weapon_shot_with_effects(
            slot,
            shot,
            capture,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn deliver_owned_weapon_shot_with_effects(
        &mut self,
        slot: u8,
        mut shot: crate::game_logic::combat::PendingProjectile,
        capture: Option<super::weapon_visual_capture::PendingWeaponVisualDispatchCapture>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        // Retain the selected immutable template before FireFX/FireOCL callbacks
        // can change content, selection, ownership or the firing object itself.
        let stream_binding = self.objects.get(&shot.shooter_id).and_then(|object| {
            object.selected_native_weapon_rule(slot).map(|rule| {
                crate::game_logic::combat::ProjectileStreamBinding {
                    slot,
                    name: rule.projectile_stream_name.clone(),
                }
            })
        });
        let source_pose = self.prepare_accepted_self_damage_pose(shot.shooter_id);
        let epoch = source_pose.epoch;
        let accepted = self.begin_owned_weapon_effects_with_effects(
            shot.shooter_id,
            slot,
            shot.target_id,
            shot.target_pos,
            shot.shooter_pos,
            capture,
            presentation_effects,
        );
        // Accepted effects belong to this callback, never to later flight.
        shot.fire_fx_name.clear();
        shot.fire_ocl_name.clear();
        if self.visual_world_epoch != epoch {
            // The receiving match itself ended during the effect phase. An
            // old shot must not enter its replacement match's combat queue.
            return false;
        }
        self.refresh_accepted_self_damage_pose(&mut shot, source_pose);
        self.materialize_owned_passive_shot_with_effects(
            shot,
            stream_binding,
            presentation_effects,
        );
        if let Some(accepted) = accepted {
            self.finish_accepted_weapon_discharge(accepted);
            true
        } else {
            false
        }
    }

    /// Retain source identity across callbacks without retaining a borrow.
    fn prepare_accepted_self_damage_pose(&mut self, source: ObjectId) -> AcceptedSelfDamagePose {
        AcceptedSelfDamagePose {
            epoch: self.visual_world_epoch,
            generation: self.stamp_visual_object_generation(source),
        }
    }

    /// CPP Weapon.cpp998 rereads the original source after fire FX/OCL.
    /// An accepted shot survives source removal; a replacement ID is never
    /// allowed to supply the original source's pose.
    fn refresh_accepted_self_damage_pose(
        &self,
        shot: &mut crate::game_logic::combat::PendingProjectile,
        pose: AcceptedSelfDamagePose,
    ) {
        if shot.self_damage_position.is_none() || pose.epoch != self.visual_world_epoch {
            return;
        }
        if let Some(source) = self.objects.get(&shot.shooter_id).filter(|object| {
            pose.generation != 0 && object.visual_object_generation == pose.generation
        }) {
            shot.self_damage_position = Some(source.get_position());
        }
    }

    pub(in crate::game_logic) fn begin_owned_weapon_effects(
        &mut self,
        source: ObjectId,
        slot: u8,
        target: Option<ObjectId>,
        target_position: Option<Vec3>,
        origin: Vec3,
        prepared_capture: Option<super::weapon_visual_capture::PendingWeaponVisualDispatchCapture>,
    ) -> Option<AcceptedWeaponDischarge> {
        self.begin_owned_weapon_effects_with_effects(
            source,
            slot,
            target,
            target_position,
            origin,
            prepared_capture,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn begin_owned_weapon_effects_with_effects(
        &mut self,
        source: ObjectId,
        slot: u8,
        target: Option<ObjectId>,
        target_position: Option<Vec3>,
        origin: Vec3,
        prepared_capture: Option<super::weapon_visual_capture::PendingWeaponVisualDispatchCapture>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<AcceptedWeaponDischarge> {
        let object = self.objects.get(&source)?;
        let name = object.weapon_name_for_slot(slot).map(str::to_owned);
        let native = object.selected_native_weapon_rule(slot);
        let native_rank = |rank| match rank {
            VeterancyLevel::Rookie => gamelogic::common::VeterancyLevel::Regular,
            VeterancyLevel::Veteran => gamelogic::common::VeterancyLevel::Veteran,
            VeterancyLevel::Elite => gamelogic::common::VeterancyLevel::Elite,
            VeterancyLevel::Heroic => gamelogic::common::VeterancyLevel::Heroic,
        };
        let rank = object.experience.level;
        let fx = native.as_ref().map(|rule| rule.get_fire_fx(native_rank(rank)).map(|fx| fx.name().to_owned()).unwrap_or_default()).unwrap_or_else(|| name.as_deref().map(|name| {
            self.weapon_effect_definitions.fire(name, rank).map(|effects| effects.fire_fx)
                .unwrap_or_else(|| crate::game_logic::weapon_bootstrap::host_fire_fx_for_weapon_name_at_veterancy(name, rank))
        }).unwrap_or_default());
        let play_when_stealthed = native
            .as_ref()
            .map(|rule| rule.play_fx_when_stealthed)
            .or_else(|| {
                name.as_deref().and_then(|name| {
                    self.weapon_effect_definitions
                        .fire(name, rank)
                        .map(|effects| effects.play_fx_when_stealthed)
                })
            });
        let object = self.objects.get_mut(&source)?;
        if let Some(capture) = prepared_capture {
            object.pending_weapon_visual_capture = Some(capture);
        } else {
            object.capture_pending_weapon_visual_dispatch(
                slot,
                self.frame,
                target,
                target_position,
            );
        }
        if let Some(capture) = object.pending_weapon_visual_capture.as_mut() {
            capture.source_pos = [origin.x, origin.y, origin.z];
            capture.selected_fx_is_present =
                !fx.trim().is_empty() && !fx.eq_ignore_ascii_case("None");
            capture.selected_fx_name = fx;
            if let Some(play) = play_when_stealthed {
                capture.weapon_plays_fx_when_stealthed = play;
            }
        }
        let accepted =
            self.begin_accepted_weapon_discharge_with_effects(source, slot, presentation_effects)?;
        // CPP943 samples the source rank again AFTER FireFX. No object borrow
        // spans this callback, and no queued shot can repeat these effects.
        if let Some(object) = self.objects.get(&source) {
            let rank = object.experience.level;
            let ocl = native.as_ref().map(|rule| rule.get_fire_ocl_name(native_rank(rank)).unwrap_or_default().to_owned()).unwrap_or_else(|| name.as_deref().map(|name| {
                self.weapon_effect_definitions.fire(name, rank).map(|effects| effects.fire_ocl)
                    .unwrap_or_else(|| crate::game_logic::weapon_bootstrap::host_fire_ocl_for_weapon_name_at_veterancy(name, rank))
            }).unwrap_or_default());
            let (team, orientation, velocity, source_position) = (
                object.team,
                object.get_orientation(),
                object.movement.velocity,
                object.get_position(),
            );
            if !ocl.trim().is_empty() && !ocl.eq_ignore_ascii_case("None") {
                self.execute_parsed_weapon_ocl_at_with_effects(
                    &ocl,
                    Some(source),
                    team,
                    rank,
                    orientation,
                    velocity,
                    source_position,
                    presentation_effects,
                );
            }
        }
        Some(accepted)
    }
}

#[cfg(test)]
mod accepted_self_pose_tests {
    use super::*;
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };

    fn prepared() -> (GameLogic, crate::game_logic::combat::PendingProjectile) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "owner", false));
        world.add_player(Player::new(1, Team::GLA, "target", false));
        let mut rules = std::collections::HashMap::new();
        parse_weapon_definitions_ini_into(
            &mut rules,
            r#"
Weapon OwnedSelfPose
 PrimaryDamage = 20
 AttackRange = 100
 DelayBetweenShots = 0
 DamageDealtAtSelfPosition = Yes
 ProjectileObject = NONE
End
"#,
            WeaponDefinitionLoadType::Normal,
        );
        world
            .admit_weapon_content_snapshot(
                rules
                    .into_values()
                    .map(|rule| (rule.name.clone(), rule))
                    .collect(),
                &[],
            )
            .unwrap();
        let mut source = ThingTemplate::new("OwnedSelfPoseSource");
        source
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable);
        source.set_primary_weapon_name("OwnedSelfPose");
        world.templates.insert(source.name.clone(), source);
        let target = ThingTemplate::new("OwnedSelfPoseTarget");
        world.templates.insert(target.name.clone(), target);
        let source = world
            .create_object_for_player("OwnedSelfPoseSource", 0, Vec3::ZERO)
            .unwrap();
        let target = world
            .create_object_for_player("OwnedSelfPoseTarget", 1, Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        world.frame = 30;
        let (_, shot) = world
            .objects
            .get_mut(&source)
            .unwrap()
            .prepare_passive_weapon_shot(
                0,
                target,
                1.0,
                false,
                false,
                30,
                Vec3::ZERO,
                &mut world.combat_system.fire_sound_loop_events,
                &mut world.attack_events,
                Some(&world.projectile_definitions),
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            )
            .unwrap();
        (world, shot)
    }
    #[test]
    fn accepted_self_damage_pose_rereads_original_owner_after_effect_phase() {
        // Explicit continuation control; not a claim of native FX execution.
        let (mut world, mut shot) = prepared();
        let pose = world.prepare_accepted_self_damage_pose(shot.shooter_id);
        let target = shot.target_id;
        assert_eq!(shot.self_damage_position, Some(Vec3::ZERO));
        let changed = Vec3::new(7.0, 3.0, 5.0);
        world.set_fixture_object_position(shot.shooter_id, changed);
        world.refresh_accepted_self_damage_pose(&mut shot, pose);
        assert_eq!(shot.target_id, target);
        assert_eq!(shot.self_damage_position, Some(changed));
    }
    #[test]
    fn accepted_self_damage_pose_rejects_reused_birth_after_reset() {
        let (mut world, mut shot) = prepared();
        let pose = world.prepare_accepted_self_damage_pose(shot.shooter_id);
        let before = shot.self_damage_position;
        world.reset();
        world.add_player(Player::new(0, Team::USA, "new owner", false));
        let replacement = world
            .create_object_for_player("OwnedSelfPoseSource", 0, Vec3::new(99.0, 0.0, 0.0))
            .unwrap();
        assert_eq!(replacement, shot.shooter_id);
        world.refresh_accepted_self_damage_pose(&mut shot, pose);
        assert_eq!(
            shot.self_damage_position, before,
            "new match cannot supply an old accepted source pose"
        );
    }
}
