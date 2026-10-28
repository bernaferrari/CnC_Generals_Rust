//! Immediate factory inputs read from the registered host Object, never a frame.
use super::*;

#[cfg(feature = "game_client")]
impl GameLogic {
    pub(crate) fn current_native_factory_drawable_source(
        &self,
        id: ObjectId,
        host_epoch: u64,
        local_player_id: u32,
    ) -> Result<crate::game_logic::NativeFactoryDrawableSource, String> {
        use crate::game_logic::{KindOf, VeterancyLevel};
        use game_client::core::game_client::{PresentationDrawPhysics, PresentationDrawableSync};
        let object = self
            .host_object(id)
            .ok_or_else(|| format!("factory Object {} disappeared before Drawable birth", id.0))?;
        let template = object.get_template();
        // GameLogic.cpp4115 always supplies this Object's own template. Disguise
        // substitution belongs to its later update, not the fresh constructor.
        let receiving = template.receiving_client_template.as_ref().ok_or_else(|| {
            format!(
                "factory Object {} '{}' has no admitted receiving Drawable template",
                id.0, template.name
            )
        })?;
        let models = template.receiving_draw_models().ok_or_else(|| {
            format!(
                "factory Object {} '{}' has no admitted receiving Draw definitions",
                id.0, template.name
            )
        })?;
        let mesh_scale = super::build::direct_host_visual_mesh_scale(self, object, &template.name);
        if host_epoch == 0 || !mesh_scale.is_finite() {
            return Err(format!(
                "factory Object {} has invalid native epoch/scale",
                id.0
            ));
        }
        let facts = super::live_draw_source::current_condition_facts(self, object, local_player_id);
        let position = object.get_position();
        let orientation = object.get_orientation();
        if !position.is_finite() || !orientation.is_finite() {
            return Err(format!(
                "factory Object {} has nonfinite constructor pose",
                id.0
            ));
        }
        let (_, night) = super::world_env::current_model_environment(self);
        let indicator = if let Some(raw) = object.custom_indicator_color {
            [(raw >> 16) as u8, (raw >> 8) as u8, raw as u8]
        } else if let Some(player) = object
            .owner_player_id
            .and_then(|owner| self.get_player(owner))
        {
            let (r, g, b) = if night {
                player.color_night_rgb
            } else {
                player.color_rgb
            };
            [r, g, b]
        } else {
            [0, 0, 0]
        };
        let science =
            self.get_player(local_player_id).and_then(|player| {
                self.receiving_module_content.as_ref().map(|content| {
                    std::sync::Arc::new(
                gamelogic::object::draw::w3d_science_model_draw::ReceivingSciencePlayer::new(
                    player.is_alive && !player.is_observer,
                    player.unlocked_sciences.iter().filter_map(|name| content.science_type(name)),
                )
            )
                })
            });
        let health = self
            .host_authoritative_health(id)
            .unwrap_or(object.health.current);
        let velocity = object.movement.velocity;
        let acceleration = object.previous_acceleration();
        let speed_squared =
            velocity.x * velocity.x + velocity.z * velocity.z + velocity.y * velocity.y;
        let frame = self.get_current_frame() as u32;
        let (bomb_type, bomb_seconds) = super::build::freeze_sticky_bomb_overlay(object, frame);
        let (ammo_total, ammo_full) = object.get_ammo_pip_showing_info().unwrap_or((0, 0));
        let mut kinds = template.kind_of.iter().copied().collect::<Vec<_>>();
        kinds.sort_by_key(|kind| *kind as u32);
        let float_yaw = object.float_update.as_ref().map_or(0.0, |state| state.yaw);
        let float_pitch = object
            .float_update
            .as_ref()
            .map_or(0.0, |state| state.pitch);
        let render = crate::game_logic::NativeSlaveSource {
            identity: self.live_draw_playback_identity(object),
            template_name: template.name.clone(),
            receiving_draw_models: std::sync::Arc::clone(&models),
            condition_bits: facts.bits(),
            mesh_scale,
            float_yaw,
            float_pitch,
            under_construction: facts.under_construction,
            construction_percent: facts.construction_percent,
            max_height_above_position: if template.geometry_info.authored {
                template.geometry_info.max_height_above_position()
            } else {
                0.0
            },
        };
        let drawable = PresentationDrawableSync {
            module_admission: receiving.client_admission(),
            receiving_science_player: science,
            draw_indicator_color: indicator,
            draw_physics: PresentationDrawPhysics {
                speed: speed_squared.sqrt(),
                vel_x: velocity.x,
                vel_y: velocity.z,
                accel_x: acceleration.x,
                accel_y: acceleration.z,
                is_motive: object.is_motive(),
                airborne: object.is_above_terrain(),
                turning: match object.physics_turning {
                    crate::game_logic::object::PhysicsTurningType::TurnNegative => -1.0,
                    crate::game_logic::object::PhysicsTurningType::TurnNone => 0.0,
                    crate::game_logic::object::PhysicsTurningType::TurnPositive => 1.0,
                },
            },
            draw_velocity_magnitude_squared: speed_squared,
            draw_logic_frame: frame,
            object_id: id.0,
            ambient_audio_locality: Some(self.receiving_audio_locality_for_player(
                object.owner_player_id.map(|owner| owner as i32).unwrap_or(-1),
            )),
            host_epoch,
            resident: true,
            visual_template_name: template.name.clone(),
            template_name: template.name.clone(),
            position: position.to_array(),
            orientation,
            float_yaw,
            float_pitch,
            destroyed: facts.destroyed,
            model_condition_bits: facts.bits(),
            body_damage_state: facts.body_damage_state,
            kind_names: std::sync::Arc::new(
                kinds.iter().map(|kind| kind.name().to_owned()).collect(),
            ),
            team_color: self
                .player_house_color_rgba(object.owner_player_id)
                .unwrap_or(object.team_color),
            effectively_stealthed: object.is_effectively_stealthed(),
            // Drawable's constructor starts visible. Viewer-relative stealth
            // admission is performed by the ordinary later client update.
            scene_hidden_by_stealth: false,
            health_current: health,
            health_max: object.health.maximum,
            selected: object.selected,
            veterancy_level: match object.experience.level {
                VeterancyLevel::Rookie => 0,
                VeterancyLevel::Veteran => 1,
                VeterancyLevel::Elite => 2,
                VeterancyLevel::Heroic => 3,
            },
            under_construction: facts.under_construction,
            construction_percent: facts.construction_percent,
            sold: object.status.sold,
            ammo_pip_total: ammo_total.min(255) as u8,
            ammo_pip_full: ammo_full.min(255) as u8,
            occupant_count: facts.occupant_count.min(255) as u8,
            max_garrison: object
                .building_data
                .as_ref()
                .map_or(0, |building| building.max_garrison)
                .min(255) as u8,
            disabled: object.is_disabled(),
            is_carbomb: object.weapon_set_carbomb
                && object.owner_player_id == Some(local_player_id),
            bomb_type,
            bomb_timer_seconds: bomb_seconds,
            weapon_bonus_enthusiastic: object.weapon_bonus_enthusiastic,
            show_healing: (object.sole_healing_benefactor_expiration_frame > frame
                && object.sole_healing_benefactor_expiration_frame != 0)
                || object
                    .overlay_icon_active(&["DefaultHeal", "StructureHeal", "VehicleHeal"], frame),
            healing_icon_type: if object.is_kind_of(KindOf::Structure) {
                1
            } else if object.is_kind_of(KindOf::Vehicle) {
                2
            } else {
                0
            },
            garrisoned_ids: object.contained_units().iter().map(|id| id.0).collect(),
            emoticon_name: object.emoticon_name.clone(),
            emoticon_frames_left: object.emoticon_frames_left,
            formation_id: object.formation_id,
            caption: if object.name.trim().is_empty() || object.name.trim() == template.name {
                String::new()
            } else {
                object.name.trim().to_owned()
            },
            draw_module_names: models.module_class_names(),
            health_box_width: object.get_health_box_dimensions().1,
            health_box_z_offset: object.health_box_world_z_offset(),
        };
        Ok(crate::game_logic::NativeFactoryDrawableSource { drawable, render })
    }
}

impl GameLogic {
    pub(crate) fn admit_factory_drawable_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        #[cfg(feature = "game_client")]
        {
            if !self.requires_native_drawable_owner() && effects.view.is_none() {
                return Ok(());
            }
            let view = effects.view.as_deref_mut().ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(format!(
                    "Object {} birth requires its native Drawable owner",
                    id.0
                ))
            })?;
            let source = self
                .current_native_factory_drawable_source(
                    id,
                    view.host_epoch(),
                    view.local_player_id(),
                )
                .map_err(gamelogic::GameLogicError::Configuration)?;
            let birth = self.native_drawable_birth_context();
            let content = self.receiving_module_content.as_mut().ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(format!(
                    "Object {} birth requires its receiving content owner",
                    id.0
                ))
            })?;
            view.admit_factory_drawable(source, birth, content.module_factory_mut())
                .map_err(gamelogic::GameLogicError::Configuration)
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (id, effects);
            Ok(())
        }
    }
}
