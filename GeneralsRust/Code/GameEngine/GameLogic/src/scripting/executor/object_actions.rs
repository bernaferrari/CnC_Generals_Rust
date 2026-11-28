//! Standalone Core adapter for authored object effects.

use super::ScriptActionDispatcher;
use crate::object::{Object, ObjectScriptStatusBit};
use crate::scripting::core::{TEAM_THE_PLAYER, THIS_OBJECT};
use generals_sim::scripting::ScriptActionResult;
use generals_sim::scripting::object_actions::{ObjectAction, ObjectPanelFlag, PanelTarget};

impl ScriptActionDispatcher<'_> {
    pub(crate) fn execute_standalone_object_action(
        &self,
        action: ObjectAction<'_>,
    ) -> ScriptActionResult {
        let engine = self.context.borrowed_engine();
        let named_id = |name: &str| {
            if name == THIS_OBJECT {
                engine.script_object_id()
            } else {
                engine.cached_named_object_id(name)
            }
        };
        match action {
            ObjectAction::PanelFlag {
                target,
                flag,
                enabled,
            } => {
                let apply = |id| {
                    if let Some(object) = engine.standalone_object(id)
                        && let Ok(mut object) = object.write()
                    {
                        apply_panel_flag(&mut object, flag, enabled);
                    }
                };
                match target {
                    PanelTarget::Named(name) => {
                        if let Some(id) = named_id(name) {
                            apply(id);
                        }
                    }
                    PanelTarget::Team(name) => {
                        let resolved;
                        let name = if name == TEAM_THE_PLAYER {
                            resolved = self.resolve_team_name_token(name);
                            resolved.as_str()
                        } else {
                            name
                        };
                        if let Some(team) = engine.standalone_context_team(name) {
                            let members = team.read().map(|team| team.get_members().to_vec());
                            if let Ok(members) = members {
                                for id in members {
                                    apply(id);
                                }
                            }
                        }
                    }
                }
            }
            ObjectAction::TrainHeld { name, held } => {
                if let Some(object) = named_id(name).and_then(|id| engine.standalone_object(id))
                    && let Ok(object) = object.read()
                    && let Some(module) = object.find_update_module("RailroadBehavior")
                {
                    module.with_module(|module| {
                        if let Some(train) = module.get_train_control_interface() {
                            train.set_held(held);
                        }
                    });
                }
            }
            ObjectAction::IgnoreUnknownPanelFlag => {}
        }
        ScriptActionResult::Success
    }
}

fn apply_panel_flag(object: &mut Object, flag: ObjectPanelFlag, enabled: bool) {
    match flag {
        ObjectPanelFlag::Enabled => {
            object.set_script_status(ObjectScriptStatusBit::ScriptDisabled, !enabled)
        }
        ObjectPanelFlag::Powered => {
            object.set_script_status(ObjectScriptStatusBit::ScriptUnderpowered, !enabled)
        }
        ObjectPanelFlag::Unsellable => {
            object.set_script_status(ObjectScriptStatusBit::Unsellable, enabled)
        }
        ObjectPanelFlag::PlayerTargetable => {
            object.set_script_status(ObjectScriptStatusBit::ScriptTargetable, enabled)
        }
        ObjectPanelFlag::Selectable => {
            if object.is_selectable() != enabled {
                object.set_selectable(enabled);
            }
        }
        ObjectPanelFlag::Indestructible => {
            if let Some(body) = object.get_body_module()
                && let Ok(mut body) = body.lock()
            {
                let _ = body.set_indestructible(enabled);
            }
        }
        ObjectPanelFlag::AiRecruitable => {
            if let Some(ai) = object.get_ai_update_interface()
                && let Ok(mut ai) = ai.lock()
            {
                ai.set_is_recruitable(enabled);
            }
        }
    }
}
