//! TEAM_GARRISON_NEAREST_BUILDING — C++ ScriptActions::doTeamGarrisonNearestBuilding.

use super::*;
use crate::common::KindOf;

impl ScriptActionDispatcher {
    /// C++ ScriptActions.cpp:3358-3422 `doTeamGarrisonNearestBuilding`.
    ///
    /// Partition-filters nearest garrisonable buildings (internet center for
    /// money hackers), near-to-far, and fills remaining contain slots with
    /// infantry that are not `KINDOF_NO_GARRISON`.
    pub(crate) fn do_team_garrison_nearest_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        log::info!("Team '{}' garrisoning nearest building", team_name);
        if super::dual_world_registry_unavailable() {
            crate::scripting::request_host_script_garrison_enter(
                crate::scripting::HostScriptGarrisonEnterExitRequest::TeamGarrisonNearest {
                    team: team_name,
                },
            );
            return Ok(ScriptActionResult::Success);
        }

        let Some(team_arc) = self.get_team_by_name(&team_name).ok() else {
            return Ok(ScriptActionResult::Success);
        };
        let members = team_arc
            .read()
            .ok()
            .map(|team| team.get_members().to_vec())
            .unwrap_or_default();
        if members.is_empty() {
            return Ok(ScriptActionResult::Success);
        }

        let leader_id = members[0];
        let Some(leader_obj) = TheGameLogic::find_object_by_id(leader_id) else {
            return Ok(ScriptActionResult::Success);
        };
        let (leader_pos, leader_off_map, leader_is_hacker, leader_player_mask) =
            if let Ok(leader) = leader_obj.read() {
                (
                    *leader.get_position(),
                    leader.is_off_map(),
                    leader.is_kind_of(KindOf::MoneyHacker),
                    leader
                        .get_controlling_player()
                        .and_then(|p| p.read().ok().map(|player| player.get_player_mask()))
                        .unwrap_or_else(crate::common::PlayerMaskType::none),
                )
            } else {
                return Ok(ScriptActionResult::Success);
            };

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(ScriptActionResult::Success);
        };

        let mut buildings: Vec<(f32, crate::common::ObjectID)> = Vec::new();
        for id in partition.get_objects_in_range(&leader_pos, 1_000_000.0) {
            if id == leader_id {
                continue;
            }
            {
                enum _ObjFlow<T> { Cont, Ret(T), Fall }
                let _flow = OBJECT_REGISTRY.with_object(id, |obj| {
                    if obj.is_effectively_dead() || obj.is_off_map() != leader_off_map {
                        return _ObjFlow::Cont;
                    }
                    let is_internet_center = obj.is_kind_of(KindOf::FSInternetCenter);
                    if leader_is_hacker {
                        if !is_internet_center {
                            return _ObjFlow::Cont;
                        }
                    } else if is_internet_center || !obj.is_kind_of(KindOf::Structure) {
                        return _ObjFlow::Cont;
                    }
                    let Some(contain) = obj.get_contain() else {
                        return _ObjFlow::Cont;
                    };
                    let Ok(contain_guard) = contain.lock() else {
                        return _ObjFlow::Cont;
                    };
                    if !leader_is_hacker {
                        let entered_mask = contain_guard.get_player_who_entered();
                        if entered_mask != crate::common::PlayerMaskType::none()
                            && entered_mask != leader_player_mask
                        {
                            return _ObjFlow::Cont;
                        }
                    }
                    let slots = contain_guard.get_contain_max() - contain_guard.get_contain_count() as i32;
                    if slots <= 0 {
                        return _ObjFlow::Cont;
                    }
                    let pos = obj.get_position();
                    let dx = pos.x - leader_pos.x;
                    let dy = pos.y - leader_pos.y;
                    let dz = pos.z - leader_pos.z;
                    buildings.push((dx * dx + dy * dy + dz * dz, id));
                    _ObjFlow::Fall
                });
                match _flow {
                    None | Some(_ObjFlow::Cont) => continue,
                    Some(_ObjFlow::Ret(v)) => return v,
                    Some(_ObjFlow::Fall) => {}
                }
            }
        }
        buildings.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut member_idx = 0usize;
        for (_, building_id) in buildings {
            let Some(building) = TheGameLogic::find_object_by_id(building_id) else {
                continue;
            };
            let slots_available = {
                let Ok(obj) = building.read() else {
                    continue;
                };
                let Some(contain) = obj.get_contain() else {
                    continue;
                };
                let Ok(contain_guard) = contain.lock() else {
                    continue;
                };
                contain_guard.get_contain_max() - contain_guard.get_contain_count() as i32
            };
            if slots_available <= 0 {
                continue;
            }

            let mut filled = 0i32;
            while filled < slots_available && member_idx < members.len() {
                let member_id = members[member_idx];
                member_idx += 1;
                {
                    enum _ObjFlow<T> { Cont, Ret(T), Fall }
                    let _flow = OBJECT_REGISTRY.with_object_mut(member_id, |mut member| {
                        if !member.is_kind_of(KindOf::Infantry) || member.is_kind_of(KindOf::NoGarrison) {
                            return _ObjFlow::Cont;
                        }
                        let Some(ai_arc) = member.get_ai_update_interface() else {
                            return _ObjFlow::Cont;
                        };
                        member.leave_group();
                        if let Ok(mut ai_guard) = ai_arc.lock() {
                            let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                            let mut params =
                                AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
                            params.obj = Some(building_id);
                            let _ = ai_guard.execute_command(&params);
                            filled += 1;
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
            if member_idx >= members.len() {
                break;
            }
        }

        Ok(ScriptActionResult::Success)
    }
}
