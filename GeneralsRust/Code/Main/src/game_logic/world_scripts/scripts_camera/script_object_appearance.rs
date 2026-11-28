use super::*;
use generals_sim::GameLogicResult;
use generals_sim::scripting::object_appearance_actions::ObjectAppearanceAction;
use generals_sim::scripting::object_status_actions::ObjectStatusTarget;

impl GameLogic {
    pub(super) fn apply_owned_script_object_appearance(
        &mut self,
        action: &ObjectAppearanceAction,
        this_object: Option<ObjectId>,
        calling_team: Option<u32>,
        condition_team: Option<u32>,
    ) -> GameLogicResult<()> {
        match action {
            ObjectAppearanceAction::CustomColor { unit, color } => {
                if let Some(id) = self.owned_named_script_object(unit, this_object)
                    && let Some(object) = self.objects.get_mut(&id)
                {
                    object.set_custom_indicator_color_raw(*color);
                }
            }
            ObjectAppearanceAction::Flash {
                target,
                seconds,
                white,
            } => {
                let team_flash = matches!(target, ObjectStatusTarget::Team(_));
                for id in
                    self.script_status_targets(target, this_object, calling_team, condition_team)?
                {
                    let color = self.script_flash_color(id, *white);
                    let Some(object) = self
                        .objects
                        .get_mut(&id)
                        .filter(|object| object.has_drawable)
                    else {
                        // C++ doTeamFlash breaks at the first member without a Drawable.
                        break;
                    };
                    if team_flash {
                        object.set_team_script_flash(*seconds, color);
                    } else {
                        object.set_script_flash(*seconds, color);
                    }
                }
            }
            ObjectAppearanceAction::Emoticon {
                target,
                name,
                frames,
            } => {
                let members = match target {
                    ObjectStatusTarget::Named(_) => self.script_status_targets(
                        target,
                        this_object,
                        calling_team,
                        condition_team,
                    )?,
                    ObjectStatusTarget::Team(team) => {
                        let Some((_, members)) =
                            self.resolve_owned_script_team(team, calling_team, condition_team)?
                        else {
                            return Ok(());
                        };
                        // ScriptActions::doTeamEmoticon admits the Team through AIGroup::add,
                        // changing the accepted members' group before touching Drawables.
                        let group = self.create_team_ai_group(&members);
                        self.ai_group_members(group)
                    }
                };
                for id in members {
                    if let Some(object) = self
                        .objects
                        .get_mut(&id)
                        .filter(|object| object.has_drawable)
                    {
                        object.set_emoticon(name, *frames);
                    }
                }
            }
        }
        Ok(())
    }

    /// ScriptActions.cpp uses Object::getIndicatorColor or RGB white.
    fn script_flash_color(&self, id: ObjectId, white: bool) -> u32 {
        if white {
            return 0x00FF_FFFF;
        }
        let Some(object) = self.objects.get(&id) else {
            return 0xFF00_0000;
        };
        if let Some(color) = object.custom_indicator_color {
            return color;
        }
        object
            .owner_player_id
            .and_then(|player| self.players.get(&player))
            .map(|player| crate::game_logic::host_radar::pack_player_color_argb(player.color_rgb))
            .unwrap_or(0xFF00_0000)
    }
}
