//! Interpreter-local controls in C++ ScriptEngine.cpp:6354–6398, 6529–6639, 6797–6827.

use super::ScriptError;
use super::frame::{ScriptFade, TFade};
use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use crate::kind_of::classic::{kind_of_indices, parse_kind_of};
use game_engine::common::system::kind_of::{KindOfMask, kind_of_type_to_mask};

impl ScriptInterpreter {
    pub fn try_execute_control_action(
        &self,
        action: &ScriptAction,
        activation_changed: impl FnOnce(&str, bool),
        template_name: impl FnMut(&str) -> Option<String>,
        matching_templates: impl FnOnce(KindOfMask) -> Option<Vec<String>>,
    ) -> Result<bool, ScriptError> {
        use ScriptActionType::*;

        let parameter = |index| {
            action
                .get_parameter(index)
                .ok_or_else(|| ScriptError::missing_parameter(index))
        };
        let name = |index| parameter(index).map(|value| value.get_string());
        let int = |index| parameter(index).map(|value| value.get_int());
        let real = |index| parameter(index).map(|value| value.get_real());

        let fade = match action.action_type {
            CameraFadeAdd => Some(TFade::Add),
            CameraFadeSubtract => Some(TFade::Subtract),
            CameraFadeSaturate => Some(TFade::Saturate),
            CameraFadeMultiply => Some(TFade::Multiply),
            _ => None,
        };
        if let Some(mode) = fade {
            let fade = ScriptFade::start(mode, real(0)?, real(1)?, int(2)?, int(3)?, int(4)?);
            self.frame_mut().fade = fade;
            return Ok(true);
        }

        match action.action_type {
            NoOp => {}
            EnableScript | DisableScript => {
                let name = name(0)?;
                let enabled = action.action_type == EnableScript;
                self.sides().set_active(name, enabled);
                activation_changed(name, enabled);
            }
            SetTreeSway => {
                let (direction, intensity, lean, period, randomness) =
                    (real(0)?, real(1)?, real(2)?, int(3)?, real(4)?);
                self.frame_mut()
                    .breeze_info
                    .set_sway(direction, intensity, lean, period, randomness);
            }
            ChooseVictimAlwaysUsesNormal => {
                let enabled = int(0)? != 0;
                self.frame_mut().choose_victim_always_uses_normal = enabled;
            }
            SetDefaultAttackPriority => {
                self.set_priority_default(name(0)?, int(1)?);
            }
            SetAttackPriorityThing => {
                self.set_priority_thing(name(0)?, name(1)?, int(2)?, template_name);
            }
            SetAttackPriorityKindOf => {
                let set = name(0)?;
                let kind = parameter(1)?;
                let priority = int(2)?;
                let token = kind.get_string().trim();
                let mask = if token.is_empty() {
                    kind_of_type_to_mask(kind.get_int())
                } else {
                    let token = token.strip_prefix("KINDOF_").unwrap_or(token);
                    KindOfMask::from_string(token).or_else(|| {
                        parse_kind_of(token).map(|kind| {
                            let bits = kind_of_indices(kind)
                                .iter()
                                .fold(0, |bits, index| bits | (1u128 << *index));
                            KindOfMask::from_bits_truncate(bits)
                        })
                    })
                };
                if let Some(mask) = mask {
                    self.set_priority_for_templates(set, priority, || matching_templates(mask));
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub fn set_priority_default(&self, set: &str, priority: i32) -> bool {
        self.attack_priorities_mut()
            .with_mut(set, true, |row| row.default_priority = priority)
            .is_some()
    }

    pub fn set_priority_thing(
        &self,
        set: &str,
        type_or_list: &str,
        priority: i32,
        mut template_name: impl FnMut(&str) -> Option<String>,
    ) -> bool {
        // Release the catalog loan before the content owner resolves a name.
        let list = self.frame().object_types.get(type_or_list).cloned();
        let names = list
            .as_ref()
            .map(|list| list.iter().map(|name| name.as_str()));
        let mut names = names
            .into_iter()
            .flatten()
            .chain(list.is_none().then_some(type_or_list));
        names.all(|name| {
            let Some(name) = template_name(name) else {
                return false;
            };
            self.attack_priorities_mut()
                .with_mut(set, true, |row| row.set_priority(&name, priority))
                .is_some()
        })
    }

    pub fn set_priority_for_templates(
        &self,
        set: &str,
        priority: i32,
        matching_templates: impl FnOnce() -> Option<Vec<String>>,
    ) -> bool {
        // C++ creates the row before walking the template catalog even when
        // no template has the requested kind.
        if self
            .attack_priorities_mut()
            .with_mut(set, true, |_| ())
            .is_none()
        {
            return false;
        }
        let Some(names) = matching_templates() else {
            return false;
        };
        self.attack_priorities_mut()
            .with_mut(set, false, |row| {
                for name in names {
                    row.set_priority(&name, priority);
                }
            })
            .is_some()
    }
}
