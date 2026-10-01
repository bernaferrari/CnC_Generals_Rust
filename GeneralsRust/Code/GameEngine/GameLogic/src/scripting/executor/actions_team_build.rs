//! Team build, recruit, reinforcement, wander, panic, stop, and merge actions
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use crate::object::registry::OBJECT_REGISTRY;
use super::*;

impl ScriptActionDispatcher {
    // ============================================================================
    // ADDITIONAL TEAM ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_build_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::debug!("Building team '{}'", team_name);

        // C++ ScriptActions::doBuildTeam uses only
        // `findTeamPrototype(teamName)->getControllingPlayer()`.  A live team
        // instance or the currently executing script side must not substitute
        // for a missing prototype owner.
        let owner_name = get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| factory.find_team_prototype(&team_name))
            .map(|prototype| prototype.get_owner_name().to_string())
            .filter(|owner| !owner.is_empty());

        if let Some(owner_name) = owner_name {
            if super::dual_world_registry_unavailable() {
                // Host never registers crate AiIntegrationManager players.
                // C++ player->getAI()->buildSpecificAITeam(proto, true).
                super::request_host_build_team(&owner_name, &team_name);
            } else {
                self.with_named_player_ai(&owner_name, |ai_player| {
                    if let Err(err) = ai_player.build_specific_ai_team(&team_name, true) {
                        log::debug!(
                            "BuildTeam '{}' failed for player '{}': {}",
                            team_name,
                            owner_name,
                            err
                        );
                    }
                });
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_recruit_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let recruit_radius = self.get_real_param(action, 1)?;
        log::debug!("Recruiting team '{}' radius {}", team_name, recruit_radius);

        // C++ ScriptActions::doRecruitTeam has the same prototype-controller
        // requirement as doBuildTeam.  Missing ownership is a no-op.
        let owner_name = get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| factory.find_team_prototype(&team_name))
            .map(|prototype| prototype.get_owner_name().to_string())
            .filter(|owner| !owner.is_empty());

        if let Some(owner_name) = owner_name {
            if super::dual_world_registry_unavailable() {
                // C++ player->recruitSpecificTeam(proto, recruitRadius).
                super::request_host_recruit_team(&owner_name, &team_name, recruit_radius);
            } else {
                self.with_named_player_ai(&owner_name, |ai_player| {
                    if let Err(err) = ai_player.recruit_specific_ai_team(&team_name, recruit_radius)
                    {
                        log::debug!(
                            "RecruitTeam '{}' failed for player '{}': {}",
                            team_name,
                            owner_name,
                            err
                        );
                    }
                });
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_create_reinforcement_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let waypoint_name = self.get_string_param(action, 1)?;

        let destination = {
            let waypoint_ascii = AsciiString::from(waypoint_name.as_str());
            get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|waypoint| *waypoint.get_location())
            })
        };

        let Some(destination) = destination else {
            log::warn!(
                "CREATE_REINFORCEMENT_TEAM: waypoint '{}' not found",
                waypoint_name
            );
            return Ok(ScriptActionResult::Success);
        };

        if super::dual_world_registry_unavailable() {
            let has_legacy_signature = action.get_parameter(2).is_some();
            if has_legacy_signature {
                let unit_type = waypoint_name;
                let spawn_pos = action
                    .get_parameter(2)
                    .map(|p| {
                        if p.get_parameter_type() == ParameterType::Coord3D {
                            let pos = p.get_coord();
                            crate::common::Coord3D::new(pos.x, pos.y, pos.z)
                        } else {
                            let waypoint = AsciiString::from(p.get_string());
                            get_terrain_logic()
                                .read()
                                .ok()
                                .and_then(|terrain| {
                                    terrain
                                        .get_waypoint_by_name(&waypoint)
                                        .map(|w| *w.get_location())
                                })
                                .unwrap_or(destination)
                        }
                    })
                    .unwrap_or(destination);
                let count = action.get_parameter(3).map(|p| p.get_int()).unwrap_or(1);
                for i in 0..count.max(0) {
                    let offset = (i as f32) * 5.0;
                    super::request_host_script_create(super::HostScriptCreateRequest::Object {
                        name: None,
                        thing: unit_type.clone(),
                        team: team_name.clone(),
                        x: spawn_pos.x + offset,
                        y: spawn_pos.y,
                        z: spawn_pos.z,
                        angle: 0.0,
                    });
                }
                return Ok(ScriptActionResult::Success);
            }
            super::request_host_script_create(super::HostScriptCreateRequest::ReinforcementTeam {
                team: team_name,
                waypoint: waypoint_name,
            });
            return Ok(ScriptActionResult::Success);
        }

        // Keep compatibility for custom scripts that used a non-C++ extension:
        // `CREATE_REINFORCEMENT_TEAM TeamName UnitType Coord Count`.
        let has_legacy_signature = action.get_parameter(2).is_some();
        if has_legacy_signature {
            let unit_type = waypoint_name;
            let spawn_pos = action
                .get_parameter(2)
                .map(|p| {
                    if p.get_parameter_type() == ParameterType::Coord3D {
                        let pos = p.get_coord();
                        crate::common::Coord3D::new(pos.x, pos.y, pos.z)
                    } else {
                        let waypoint = AsciiString::from(p.get_string());
                        get_terrain_logic()
                            .read()
                            .ok()
                            .and_then(|terrain| {
                                terrain
                                    .get_waypoint_by_name(&waypoint)
                                    .map(|w| *w.get_location())
                            })
                            .unwrap_or(destination)
                    }
                })
                .unwrap_or(destination);
            let count = action.get_parameter(3).map(|p| p.get_int()).unwrap_or(1);

            let team_arc = match self.get_or_create_team_by_name(&team_name) {
                Ok(team) => team,
                Err(err) => {
                    log::warn!(
                        "CREATE_REINFORCEMENT_TEAM: failed to get/create team: {}",
                        err
                    );
                    return Ok(ScriptActionResult::Success);
                }
            };

            let mut created_any = false;
            for i in 0..count.max(0) {
                let offset = (i as f32) * 5.0;
                let pos =
                    crate::common::Coord3D::new(spawn_pos.x + offset, spawn_pos.y, spawn_pos.z);
                let object_id = {
                    let manager_arc = get_object_manager();
                    let Ok(mut manager) = manager_arc.write() else {
                        log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock ObjectManager");
                        break;
                    };
                    match manager.create_object(
                        &unit_type,
                        pos,
                        Some(team_arc.clone()),
                        crate::object_manager::ObjectCreationFlags::from_template(),
                    ) {
                        Ok(id) => id,
                        Err(err) => {
                            log::warn!(
                                "CREATE_REINFORCEMENT_TEAM: failed to create '{}': {}",
                                unit_type,
                                err
                            );
                            continue;
                        }
                    }
                };

                let _ = crate::team::with_team_mut(team_arc, |team| {
                    team.add_member(object_id);
                    team.set_active();
                });
                created_any = true;
            }

            if !created_any {
                log::warn!(
                    "CREATE_REINFORCEMENT_TEAM: no units created for team '{}'",
                    team_name
                );
            }
            return Ok(ScriptActionResult::Success);
        }

        let (team_proto, team_arc) = {
            let Ok(mut factory) = get_team_factory().lock() else {
                log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock TeamFactory");
                return Ok(ScriptActionResult::Success);
            };

            let Some(proto) = factory.find_team_prototype(&team_name) else {
                log::warn!(
                    "CREATE_REINFORCEMENT_TEAM: team prototype '{}' not found",
                    team_name
                );
                return Ok(ScriptActionResult::Success);
            };

            let team = if let Some(existing) = factory.find_team(&team_name) {
                existing
            } else if let Some(created) = factory.create_inactive_team(&team_name) {
                created
            } else {
                log::warn!(
                    "CREATE_REINFORCEMENT_TEAM: failed to create inactive team '{}'",
                    team_name
                );
                return Ok(ScriptActionResult::Success);
            };

            (proto, team)
        };

        let _ = crate::team::with_team_mut(team_arc, |team| {
            if team.get_controlling_player_id().is_none() {
                let owner_name = team_proto.get_owner_name().to_string();
                if !owner_name.is_empty() {
                    if let Some(owner_index) = player_list().read().ok().and_then(|list| {
                        list.find_player_by_name(&owner_name)
                            .map(|player| player.get_player_index() as u32)
                    }) {
                        team.set_controlling_player_id(Some(owner_index));
                    }
                }
            }
        });

        let mut origin = destination;
        let mut need_move_to_destination = false;
        if !team_proto.get_start_reinforce_waypoint().is_empty() {
            let start_waypoint_name = team_proto.get_start_reinforce_waypoint();
            let start_waypoint_ascii = AsciiString::from(start_waypoint_name.as_str());
            if let Some(start) = get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&start_waypoint_ascii)
                    .map(|waypoint| *waypoint.get_location())
            }) {
                need_move_to_destination = start.x != destination.x || start.y != destination.y;
                origin = start;
            }
        }

        let mut created_any = false;
        let mut primary_transport_id: Option<ObjectID> = None;
        let mut transport_template_for_equivalence: Option<Arc<dyn crate::common::ThingTemplate>> =
            None;
        let mut put_in_container_template: Option<Arc<dyn crate::common::ThingTemplate>> = None;
        let transport_template_name = team_proto.get_transport_unit_type().to_string();

        // C++ parity: create reinforcement transport first so we can inspect DeliverPayload behavior.
        if !transport_template_name.is_empty() {
            let transport_id = {
                let manager_arc = get_object_manager();
                let Ok(mut manager) = manager_arc.write() else {
                    log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock ObjectManager");
                    return Ok(ScriptActionResult::Success);
                };
                match manager.create_object(
                    &transport_template_name,
                    origin,
                    Some(team_arc.clone()),
                    crate::object_manager::ObjectCreationFlags::from_template(),
                ) {
                    Ok(id) => id,
                    Err(err) => {
                        log::warn!(
                            "CREATE_REINFORCEMENT_TEAM: failed to create transport '{}': {}",
                            transport_template_name,
                            err
                        );
                        INVALID_ID
                    }
                }
            };

            if transport_id != INVALID_ID {
                let _ = crate::team::with_team_mut(team_arc, |team| {
                    team.add_member(transport_id);
                });
                primary_transport_id = Some(transport_id);
                created_any = true;

                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(transport_id, |transport| {
                        let _ = transport.set_position(&origin);
                        let _ = transport.set_orientation(0.0);
                        transport_template_for_equivalence = Some(transport.get_template().clone());

                        if let Some(dp_module) =
                            transport.find_update_module("DeliverPayloadAIUpdate")
                        {
                            let put_in_container_name = dp_module.with_module_data(|data| {
                                data.as_any()
                                    .downcast_ref::<crate::object::update::DeliverPayloadAIUpdateModuleData>()
                                    .and_then(|module_data| {
                                        let name = module_data.put_in_container_name.as_str();
                                        if name.is_empty() {
                                            None
                                        } else {
                                            Some(name.to_string())
                                        }
                                    })
                            });

                            if let Some(name) = put_in_container_name {
                                put_in_container_template =
                                    crate::helpers::TheThingFactory::find_template(&name);
                            }
                        }
                    });
            }
        }

        // Spawn configured unit composition for the team.
        let mut row_origin = origin;
        for info in team_proto.units_info() {
            if info.unit_thing_name.is_empty() {
                continue;
            }
            let unit_count = info.max_units.max(0) as usize;
            if unit_count == 0 {
                continue;
            }

            let mut row_last_pos = row_origin;
            let mut row_last_radius = 0.0f32;
            let mut row_spawned_any = false;

            for index in 0..unit_count {
                let object_id = {
                    let manager_arc = get_object_manager();
                    let Ok(mut manager) = manager_arc.write() else {
                        log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock ObjectManager");
                        break;
                    };
                    match manager.create_object(
                        info.unit_thing_name,
                        row_origin,
                        Some(team_arc.clone()),
                        crate::object_manager::ObjectCreationFlags::from_template(),
                    ) {
                        Ok(id) => id,
                        Err(err) => {
                            log::warn!(
                                "CREATE_REINFORCEMENT_TEAM: failed to create '{}': {}",
                                info.unit_thing_name,
                                err
                            );
                            continue;
                        }
                    }
                };

                let _ = crate::team::with_team_mut(team_arc, |team| {
                    team.add_member(object_id);
                });
                created_any = true;
                row_spawned_any = true;

                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                        let radius = obj.get_geometry_info().get_major_radius();
                        let mut pos = row_origin;
                        pos.x = row_origin.x + 2.25 * (index as f32) * radius;
                        if let Ok(terrain) = get_terrain_logic().read() {
                            pos.z = terrain.get_ground_height(pos.x, pos.y, None);
                        }
                        let _ = obj.set_position(&pos);
                        let _ = obj.set_orientation(0.0);
                        row_last_pos = pos;
                        row_last_radius = radius;
                    });
            }

            if row_spawned_any {
                row_origin.y = row_last_pos.y + 2.0 * row_last_radius;
            }
        }

        // C++ parity: if TeamStartsFull, pre-load units into transports already in the team
        // (excluding the reinforcement transport created above).
        if team_proto.get_team_starts_full() {
            let member_ids = crate::team::with_team(team_arc, |team| team.get_members().to_vec())
                .unwrap_or_default();

            let mut team_transports: Vec<ObjectID> = Vec::new();
            let mut loadable_units: Vec<ObjectID> = Vec::new();
            for member_id in member_ids {
                {
                    enum _ObjFlow<T> { Cont, Ret(T), Fall }
                    let _flow = OBJECT_REGISTRY.with_object(member_id, |member| {
                        
                        if Some(member_id) == primary_transport_id {
                            return _ObjFlow::Cont;
                        }
                        
                        if member.is_kind_of(crate::common::KindOf::Transport) {
                            if member.get_contain().is_some() {
                                team_transports.push(member_id);
                            }
                        } else {
                            loadable_units.push(member_id);
                        }
                        _ObjFlow::Fall
                    });
                    match _flow {
                        None | Some(_ObjFlow::Cont) => continue,
                        Some(_ObjFlow::Ret(v)) => return v,
                        Some(_ObjFlow::Fall) => {}
                    }
                }
            }

            for unit_id in loadable_units {
                {
                    enum _ObjFlow<T> { Cont, Ret(T), Fall }
                    let _flow = OBJECT_REGISTRY.with_object(unit_id, |unit_guard| {
                        
                        for transport_id in &team_transports {
                            let contain_arc = OBJECT_REGISTRY
                                .with_object(*transport_id, |t| t.get_contain())
                                .flatten();
                            let Some(contain_arc) = contain_arc else {
                                return _ObjFlow::Cont;
                            };
                            let Ok(mut contain_guard) = contain_arc.lock() else {
                                return _ObjFlow::Cont;
                            };
                            if contain_guard.is_valid_container_for(&unit_guard, true) {
                                let _ = contain_guard.add_to_contain(&unit_guard);
                                break;
                            }
                        }
                        _ObjFlow::Fall
                    });
                    match _flow {
                        None | Some(_ObjFlow::Cont) => continue,
                        Some(_ObjFlow::Ret(v)) => return v,
                        Some(_ObjFlow::Fall) => {}
                    }
                }
            }
        }

        let load_origin = destination;

        // Load remaining units into reinforcement transport(s), creating additional transports if full.
        if let Some(mut current_transport_id) = primary_transport_id {
            let mut transport_count = 1;
            let member_ids = crate::team::with_team(team_arc, |team| team.get_members().to_vec())
                .unwrap_or_default();

            for member_id in member_ids {
                {
                    enum _ObjFlow<T> { Cont, Ret(T), Fall }
                    let _flow = OBJECT_REGISTRY.with_object(member_id, |member_guard| {
                        
                        let is_transport_template = transport_template_for_equivalence
                            .as_ref()
                            .map(|template| {
                                member_guard
                                    .get_template()
                                    .is_equivalent_to(template.as_ref())
                            })
                            .unwrap_or(false);
                        if is_transport_template || member_guard.get_contained_by().is_some() {
                            return _ObjFlow::Cont;
                        }
                        
                        let Some((contains, full, transport_radius)) = OBJECT_REGISTRY.with_object(current_transport_id, |transport_guard| {
                            let transport_radius = transport_guard.get_geometry_info().get_major_radius();
                            let contain_arc = transport_guard.get_contain()?;
                            let contain_guard = contain_arc.lock().ok()?;
                            Some((
                                contain_guard.is_valid_container_for(&member_guard, false),
                                contain_guard.is_valid_container_for(&member_guard, true),
                                transport_radius,
                            ))
                        }).flatten() else {
                            return _ObjFlow::Cont;
                        };
                        
                        if !contains {
                            return _ObjFlow::Cont;
                        }
                        
                        drop(member_guard);
                        
                        if !full {
                            let mut pos = load_origin;
                            pos.x += (transport_count as f32) * transport_radius;
                            if let Ok(terrain) = get_terrain_logic().read() {
                                pos.z = terrain.get_ground_height(pos.x, pos.y, None);
                            }
                            let new_transport_id = {
                                let manager_arc = get_object_manager();
                                let Ok(mut manager) = manager_arc.write() else {
                                    log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock ObjectManager");
                                    return _ObjFlow::Cont;
                                };
                                match manager.create_object(
                                    &transport_template_name,
                                    pos,
                                    Some(team_arc.clone()),
                                    crate::object_manager::ObjectCreationFlags::from_template(),
                                ) {
                                    Ok(id) => id,
                                    Err(err) => {
                                        log::warn!(
                                            "CREATE_REINFORCEMENT_TEAM: failed to create overflow transport '{}': {}",
                                            transport_template_name,
                                            err
                                        );
                                        INVALID_ID
                                    }
                                }
                            };
                        
                            if new_transport_id != INVALID_ID {
                                let _ = OBJECT_REGISTRY.with_object_mut(new_transport_id, |new_transport| {
                                    let _ = new_transport.set_position(&pos);
                                    let _ = new_transport.set_orientation(0.0);
                                });
                                let _ = crate::team::with_team_mut(team_arc, |team| {
                                    team.add_member(new_transport_id);
                                });
                                current_transport_id = new_transport_id;
                                transport_count += 1;
                                created_any = true;
                            }
                        }
                        
                        let mut payload_object_id = member_id;
                        if let Some(put_in_container_template) = put_in_container_template.as_ref() {
                            let container_pos = load_origin;
                            let container_id = {
                                let manager_arc = get_object_manager();
                                let Ok(mut manager) = manager_arc.write() else {
                                    log::warn!("CREATE_REINFORCEMENT_TEAM: failed to lock ObjectManager");
                                    return _ObjFlow::Cont;
                                };
                                match manager.create_object(
                                    put_in_container_template.get_name().as_str(),
                                    container_pos,
                                    Some(team_arc.clone()),
                                    crate::object_manager::ObjectCreationFlags::from_template(),
                                ) {
                                    Ok(id) => id,
                                    Err(err) => {
                                        log::warn!(
                                            "CREATE_REINFORCEMENT_TEAM: failed to create payload container '{}': {}",
                                            put_in_container_template.get_name().as_str(),
                                            err
                                        );
                                        INVALID_ID
                                    }
                                }
                            };
                        
                            if container_id != INVALID_ID {
                                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(container_id, |container| {
                                        let _ = container.set_position(&container_pos);
                                        let _ = container.set_orientation(0.0);
                                    });
                                let _ = crate::team::with_team_mut(team_arc, |team| {
                                    team.add_member(container_id);
                                });
                                created_any = true;
                        
                                let inserted = OBJECT_REGISTRY.with_object(container_id, |container_guard| {
                                    let container_contain = container_guard.get_contain()?;
                                    let mut container_contain_guard = container_contain.lock().ok()?;
                                    if container_contain_guard.is_valid_container_for(&member_guard, true) {
                                        let _ = container_contain_guard.add_to_contain(&member_guard);
                                        Some(true)
                                    } else {
                                        Some(false)
                                    }
                                }).flatten().unwrap_or(false);
                        
                                if inserted {
                                    payload_object_id = container_id;
                                }
                            }
                        }
                        
                        {
                            enum _ObjFlow<T> { Cont, Ret(T), Fall }
                            let _flow = OBJECT_REGISTRY.with_object(payload_object_id, |payload_guard| {
                                
                                let contain_arc = OBJECT_REGISTRY
                                    .with_object(current_transport_id, |transport| transport.get_contain())
                                    .flatten();
                                if contain_arc.is_none() && OBJECT_REGISTRY.with_object(current_transport_id, |_| ()).is_none() {
                                    return _ObjFlow::Ret(_ObjFlow::Cont);
                                }
                                let Some(contain_arc) = contain_arc else {
                                    return _ObjFlow::Ret(_ObjFlow::Cont);
                                };
                                let Ok(mut contain_guard) = contain_arc.lock() else {
                                    return _ObjFlow::Ret(_ObjFlow::Cont);
                                };
                                let _ = contain_guard.add_to_contain(&payload_guard);
                                _ObjFlow::Fall
                            });
                            match _flow {
                                None => {
                                    return _ObjFlow::Cont;
                                }
                                Some(_ObjFlow::Cont) => continue,
                                Some(_ObjFlow::Ret(v)) => return v,
                                Some(_ObjFlow::Fall) => {}
                            }
                        }
                    });
                    match _flow {
                        None | Some(_ObjFlow::Cont) => continue,
                        Some(_ObjFlow::Ret(v)) => return v,
                        Some(_ObjFlow::Fall) => {}
                    }
                }
            }
        }

        let _ = crate::team::with_team_mut(team_arc, |team| {
            team.set_active();
        });

        if primary_transport_id.is_some() {
            let member_ids = crate::team::with_team(team_arc, |team| team.get_members().to_vec())
                .unwrap_or_default();

            for member_id in member_ids {
                let _ = OBJECT_REGISTRY.with_object_mut(member_id, |member| {
                    let is_transport_template = transport_template_for_equivalence
                        .as_ref()
                        .map(|template| member.get_template().is_equivalent_to(template.as_ref()))
                        .unwrap_or(false);
                    let is_held = member.is_disabled_by_type(crate::common::DisabledType::Held);
                    let Some(ai) = member.get_ai_update_interface_mut() else {
                        return;
                    };
                    if is_transport_template {
                        let mut used_deliver_payload = false;
                        if let Some(dp) = ai.get_deliver_payload_ai_update_interface() {
                            dp.deliver_payload_via_module_data(&destination);
                            used_deliver_payload = true;
                        }
                        if !used_deliver_payload {
                            let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                            if team_proto.get_transports_exit() {
                                let mut params = AiCommandParams::new(
                                    AiCommandType::MoveToPositionAndEvacuateAndExit,
                                    CommandSourceType::FromScript,
                                );
                                params.pos = destination;
                                let _ = ai.execute_command(&params);
                            } else {
                                let _ = ai.ai_move_to_and_evacuate(&destination);
                            }
                        }
                    } else if !is_held {
                        let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                        let mut params = AiCommandParams::new(
                            AiCommandType::MoveToPosition,
                            CommandSourceType::FromScript,
                        );
                        params.pos = destination;
                        let _ = ai.execute_command(&params);
                    }
                });
            }
        } else if created_any && need_move_to_destination {
            if let Ok(group_arc) = self.create_ai_group_from_team(&team_name) {
                if let Ok(group) = group_arc.write() {
                    group.group_move_to_position(
                        &destination,
                        false,
                        CommandSourceType::FromScript,
                    );
                }
            }
        }

        if !created_any {
            log::warn!(
                "CREATE_REINFORCEMENT_TEAM: team '{}' has no units configured",
                team_name
            );
        }
        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamWander()
    /// Iterates team members, selects wander locomotor, and issues waypoint wander.
    pub(crate) fn do_team_wander(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_path_label = self.get_string_param(action, 1)?;
        if super::dual_world_registry_unavailable() {
            super::request_host_team_loco_set(&team_name, "wander", Some(&waypoint_path_label));
            return Ok(ScriptActionResult::Success);
        }
        log::info!(
            "Team '{}' wandering on path '{}'",
            team_name,
            waypoint_path_label
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = crate::team::with_team(team_arc, |team| team.get_members().to_vec()).ok_or_else(|| {
            ScriptError::ExecutionFailed("Failed to read team".to_string())
        })?;

        for member_id in members {
            let Some(member_pos) = OBJECT_REGISTRY.with_object(member_id, |member| {
                if member.get_ai_update_interface().is_none() {
                    return None;
                }
                Some(*member.get_position())
            }).flatten() else {
                continue;
            };

            let waypoint_id = get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_closest_waypoint_on_path(&member_pos, &waypoint_path_label)
                    .map(|waypoint| waypoint.get_id())
            });
            let Some(waypoint_id) = waypoint_id else {
                return Ok(ScriptActionResult::Success);
            };

            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |member| {
                if let Some(ai) = member.get_ai_update_interface_mut() {
                    let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Wander);
                    let mut params =
                        AiCommandParams::new(AiCommandType::Wander, CommandSourceType::FromScript);
                    params.waypoint = Some(waypoint_id);
                    let _ = ai.execute_command(&params);
                }
            });
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamWanderInPlace()
    /// Iterates team members, selects wander locomotor, and issues wander-in-place.
    pub(crate) fn do_team_wander_in_place(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        if super::dual_world_registry_unavailable() {
            super::request_host_team_loco_set(&team_name, "wander", None);
            return Ok(ScriptActionResult::Success);
        }
        log::info!("Team '{}' wandering in place", team_name);

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = crate::team::with_team(team_arc, |team| team.get_members().to_vec()).ok_or_else(|| {
            ScriptError::ExecutionFailed("Failed to read team".to_string())
        })?;

        for member_id in members {
            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |member| {
                if let Some(ai) = member.get_ai_update_interface_mut() {
                    let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Wander);
                    let params = AiCommandParams::new(
                        AiCommandType::WanderInPlace,
                        CommandSourceType::FromScript,
                    );
                    let _ = ai.execute_command(&params);
                }
            });
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamPanic()
    /// Iterates team members, selects panic locomotor, and issues waypoint panic.
    pub(crate) fn do_team_panic(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_path_label = self.get_string_param(action, 1)?;
        if super::dual_world_registry_unavailable() {
            super::request_host_team_loco_set(&team_name, "panic", Some(&waypoint_path_label));
            return Ok(ScriptActionResult::Success);
        }
        log::debug!(
            "Team '{}' panicking on path '{}'",
            team_name,
            waypoint_path_label
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let members = crate::team::with_team(team_arc, |team| team.get_members().to_vec()).ok_or_else(|| {
            ScriptError::ExecutionFailed("Failed to read team".to_string())
        })?;

        for member_id in members {
            let Some(member_pos) = OBJECT_REGISTRY.with_object(member_id, |member| {
                if member.get_ai_update_interface().is_none() {
                    return None;
                }
                Some(*member.get_position())
            }).flatten() else {
                continue;
            };

            let waypoint_id = get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_closest_waypoint_on_path(&member_pos, &waypoint_path_label)
                    .map(|waypoint| waypoint.get_id())
            });
            let Some(waypoint_id) = waypoint_id else {
                return Ok(ScriptActionResult::Success);
            };

            let _ = OBJECT_REGISTRY.with_object_mut(member_id, |member| {
                if let Some(ai) = member.get_ai_update_interface_mut() {
                    let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Panic);
                    let mut params =
                        AiCommandParams::new(AiCommandType::Panic, CommandSourceType::FromScript);
                    params.waypoint = Some(waypoint_id);
                    let _ = ai.execute_command(&params);
                }
            });
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamStop()
    /// Issues stop command to team AI group
    pub(crate) fn do_team_stop(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::info!("Team '{}' stopping", team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_idle(super::HostScriptIdleRequest::TeamStop {
                team: team_name,
                disband: false,
            });
            return Ok(ScriptActionResult::Success);
        }

        match self.create_ai_group_from_team(&team_name) {
            Ok(group_arc) => {
                if let Ok(mut group) = group_arc.write() {
                    let params =
                        AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
                    let _ = group.ai_do_command(&params);
                }
            }
            Err(ScriptError::TeamNotFound(_)) => return Ok(ScriptActionResult::Success),
            Err(err) => return Err(err),
        }

        Ok(ScriptActionResult::Success)
    }

    /// C++ Reference: ScriptActions::doTeamStopAndDisband()
    /// Issues stop command and then disbands the team
    pub(crate) fn do_team_stop_and_disband(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        log::info!("Team '{}' stopping and disbanding", team_name);
        if super::dual_world_registry_unavailable() {
            super::request_host_script_idle(super::HostScriptIdleRequest::TeamStop {
                team: team_name,
                disband: true,
            });
            return Ok(ScriptActionResult::Success);
        }

        let Some(team_arc) = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name))
        else {
            return Ok(ScriptActionResult::Success);
        };

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        if let Ok(mut group) = group_arc.write() {
            let params = AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript);
            let _ = group.ai_do_command(&params);
        }

        let Some((members, player_id)) = crate::team::with_team(team_arc, |team| {
            (
                team.get_members().to_vec(),
                team.get_controlling_player_id(),
            )
        }) else {
            return Ok(ScriptActionResult::Success);
        };
        let default_team_name = player_id
            .and_then(|player_id| {
                player_list().read().ok().and_then(|players| {
                    players
                        .get_player(player_id as i32)
                        .and_then(|player| player.get_default_team_id())
                })
            })
            .and_then(|id| crate::team::with_team(id, |team| team.get_name().to_string()));
        for object_id in members {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                if let Some(ai) = obj.get_ai_update_interface_mut() {
                    ai.set_is_recruitable(true);
                }
            });
        }
        if let Some(default_team_name) = default_team_name {
            self.merge_team_into_team(&team_name, &default_team_name)?;
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_available_for_recruitment(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let available = self.get_int_param(action, 1)? != 0;
        log::debug!(
            "Team '{}' available for recruitment: {}",
            team_name,
            available
        );

        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                let _ = crate::team::with_team_mut(team_arc, |team_guard| {
                    team_guard.set_recruitable(available);
                });
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_collect_nearby(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::debug!("Team '{}' collecting nearby units", team_name);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_team_merge(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let source_team = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let target_team = self.resolve_team_name_token(&self.get_string_param(action, 1)?);
        log::debug!("Merging team '{}' into '{}'", source_team, target_team);
        // Live host objects are not in leftover OBJECT_REGISTRY. Queue so
        // GameLogic can rewrite `Object.team_instance_name` (census key).
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_merge_team(&source_team, &target_team);
        }

        self.merge_team_into_team(&source_team, &target_team)?;

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn merge_team_into_team(
        &self,
        source_team: &str,
        target_team: &str,
    ) -> Result<(), ScriptError> {
        let (source_team_arc, target_team_arc) = if let Ok(mut factory) = get_team_factory().lock()
        {
            (
                factory.find_team(source_team),
                factory
                    .find_team(target_team)
                    .or_else(|| factory.create_team(target_team)),
            )
        } else {
            (None, None)
        };
        let (Some(source_team_arc), Some(target_team_arc)) = (source_team_arc, target_team_arc)
        else {
            return Ok(());
        };
        if source_team_arc == target_team_arc {
            return Ok(());
        }

        let source_members = crate::team::with_team(source_team_arc, |team| team.get_members().to_vec())
            .unwrap_or_default();

        for object_id in &source_members {
            {
                enum _ObjFlow<T> { Cont, Ret(T), Fall }
                let _flow = OBJECT_REGISTRY.with_object_mut(*object_id, |mut object_guard| {
                    _ObjFlow::Fall
                });
                match _flow {
                    None | Some(_ObjFlow::Cont) => continue,
                    Some(_ObjFlow::Ret(v)) => return v,
                    Some(_ObjFlow::Fall) => {}
                }
            }
        }

        let _ = crate::team::with_team_mut(source_team_arc, |source_guard| {
            for object_id in &source_members {
                source_guard.remove_member(*object_id);
            }
            source_guard.delete_team(false);
        });
        let _ = crate::team::with_team_mut(target_team_arc, |target_guard| {
            for object_id in source_members {
                target_guard.add_member(object_id);
            }
            target_guard.set_active();
        });

        Ok(())
    }
}
