//! CreateCrateDie runs synchronously at its authored onDie slot.
use super::*;
use crate::game_logic::host_fx_list_die::leftover_veterancy_from_host;
use game_engine::common::game_common::Relationship;
use gamelogic::object::crate_system::CrateSystem;
use gamelogic::object::die::CreateCrateDieModuleData;
use std::sync::Arc;

impl GameLogic {
    pub(super) fn drop_authored_crates_with_effects(
        &mut self,
        victim_id: ObjectId,
        killer_id: Option<ObjectId>,
        module_tag: Option<&str>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let Some(victim) = self.objects.get(&victim_id) else {
            return Ok(());
        };
        let killer = killer_id.and_then(|id| self.objects.get(&id));
        if killer
            .is_some_and(|killer| self.object_relationship(killer, victim) == Relationship::Allies)
        {
            return Ok(());
        }
        let Some(crate_names) = victim
            .thing()
            .template
            .receiving_client_template
            .as_ref()
            .and_then(|template| {
                let module = template.behavior_modules().iter().find(|entry| {
                    entry.name.as_str().eq_ignore_ascii_case("CreateCrateDie")
                        && Some(entry.module_tag.as_str()) == module_tag
                })?;
                let data = module
                    .data
                    .read()
                    .into_downcast::<CreateCrateDieModuleData>()?;
                Some(data.crate_name_list.clone())
            })
        else {
            return Ok(());
        };
        let victim_birth = Arc::downgrade(&victim.visual_draw_identity_token);
        let killer_birth = killer.map(|object| Arc::downgrade(&object.visual_draw_identity_token));
        let neutral_owner = self
            .players
            .values()
            .filter(|player| player.is_reserved_neutral())
            .map(|player| player.id)
            .min();

        // CreateCrateDie.cpp:55–94. Every resolved row rolls chance before
        // its optional eligibility gates. The declaration can repeat CrateData.
        for name in crate_names {
            let Some(victim) = self.objects.get(&victim_id).filter(|object| {
                Arc::as_ptr(&object.visual_draw_identity_token) == victim_birth.as_ptr()
            }) else {
                break;
            };
            let data = {
                let common = self.world_services.common();
                common.ini.crate_system.read().get(&name).map(|parsed| {
                    CrateSystem::template_from_parsed_with_science(parsed, |science| {
                        common
                            .ini
                            .rts_science
                            .read()
                            .get_science_from_internal_name(science)
                    })
                })
            };
            let Some(data) = data else {
                continue;
            };
            let killer = killer_id
                .zip(killer_birth.as_ref())
                .and_then(|(id, birth)| {
                    self.objects.get(&id).filter(|object| {
                        Arc::as_ptr(&object.visual_draw_identity_token) == birth.as_ptr()
                    })
                });
            let chance = self.logic_random.next_real(0., 1.);
            let killer_player = killer
                .and_then(|object| self.player_owner_for_host_object(object))
                .and_then(|id| self.players.get(&id));
            if !data.passes_on_die_gates(
                chance,
                leftover_veterancy_from_host(victim.experience.level),
                killer.map(|object| {
                    object
                        .thing()
                        .template
                        .receiving_client_template
                        .as_ref()
                        .map_or_else(
                        || object.kind_of_cpp_mask(),
                        crate::game_logic::thing::ReceivingClientTemplate::authored_kind_of_bits,
                    )
                }),
                killer_player
                    .is_some_and(|player| player.has_unlocked_science(&data.killer_science_name)),
            ) {
                continue;
            }
            let Some(crate_name) = data.select_crate(self.logic_random.next_real(0., 1.)) else {
                continue;
            };
            let known = self
                .receiving_module_content
                .as_ref()
                .is_some_and(|content| content.admitted_template(&crate_name).is_some());
            if !known {
                continue;
            }
            let Some(position) = self.find_owned_crate_drop_position(victim)? else {
                continue;
            };
            let layer = victim.pathfind_layer;

            // CreateCrateDie.cpp:208–225. Constructor draws (e.g. the stock
            // SalvageCrate's DeletionUpdate) precede the orientation draw.
            let mut birth = None;
            let Some(id) = self.create_object_with_initial_team_and_ui_birth_checked(
                &crate_name,
                Team::Neutral,
                neutral_owner,
                Vec3::ZERO,
                FactoryCreationPurpose::AtCreation,
                None,
                interpreter,
                effects,
                Some(&mut birth),
            )?
            else {
                continue;
            };
            let Some((_, (generation, epoch))) = birth else {
                continue;
            };
            self.set_owned_object_position_with_effects(id, position, self.frame, effects);
            if !self.spawn_identity_matches(id, generation, epoch) {
                continue;
            }
            let angle = self.logic_random.next_real(0., std::f32::consts::TAU);
            let Some(object) = self.objects.get_mut(&id) else {
                continue;
            };
            object.set_orientation_with_partition_owner(angle, &mut self.partition_manager);
            self.set_owned_pathfinder_layer(id, layer);
            if let Some(object) = self.objects.get_mut(&id) {
                use crate::game_logic::host_battlemaster::{
                    CRATE_DECAL_FADE_IN_RATE, CRATE_DECAL_SIZE_MULT, TERRAIN_DECAL_CRATE,
                };
                let decal_size = object.geometry_info().major_radius * CRATE_DECAL_SIZE_MULT;
                object.set_terrain_decal(TERRAIN_DECAL_CRATE);
                object.set_terrain_decal_size(decal_size, decal_size);
                object.set_terrain_decal_fade_target(1., CRATE_DECAL_FADE_IN_RATE);
            }
            if data.is_owned_by_maker {
                if let Some(team) = self
                    .objects
                    .get(&victim_id)
                    .filter(|object| {
                        Arc::as_ptr(&object.visual_draw_identity_token) == victim_birth.as_ptr()
                    })
                    .and_then(|object| self.player_owner_for_host_object(object))
                    .and_then(|player| self.host_default_team_id(player))
                {
                    self.set_host_object_team_with_effects(id, team, effects)?;
                }
            }
            if !self.spawn_identity_matches(id, generation, epoch) {
                continue;
            }
            if let Some((killer_id, birth)) = killer_id.zip(killer_birth.as_ref()) {
                let eligible = self.objects.get(&killer_id).is_some_and(|object| {
                    Arc::as_ptr(&object.visual_draw_identity_token) == birth.as_ptr()
                        && object.has_ai_update_interface()
                        && self
                            .player_owner_for_host_object(object)
                            .and_then(|id| self.players.get(&id))
                            .is_some_and(|player| player.is_computer())
                });
                if eligible {
                    self.notify_unit_crate(killer_id, id);
                }
            }
        }
        Ok(())
    }
}
