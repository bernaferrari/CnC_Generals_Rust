
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
        let default_team_id = player_id.and_then(|player_id| {
            player_list().read().ok().and_then(|players| {
                players
                    .get_player(player_id as i32)
                    .and_then(|player| player.get_default_team_id())
            })
        });
        let default_team_name = default_team_id
            .and_then(|id| crate::team::with_team(id, |team| team.get_name().to_string()));
        for object_id in members {
            let ai_arc = OBJECT_REGISTRY
                .with_object(object_id, |obj| obj.get_ai_update_interface())
                .flatten();
            if let Some(ai_arc) = ai_arc {
                {
                        let mut ai = ai_arc;
                    ai.set_is_recruitable(true);
                }
            }
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

        let team_id = get_team_factory().lock().ok().and_then(|mut factory| factory.find_team(&team_name));
        if let Some(team_id) = team_id {
            let _ = crate::team::with_team_mut(team_id, |team| {
                team.set_recruitable(available);
            });
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
