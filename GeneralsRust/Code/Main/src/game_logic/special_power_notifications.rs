//! Synchronous ScriptEngine special-power event visits on the receiving owner.
use super::{GameLogic, ObjectId, ScriptPresentationEffects};
use gamelogic::scripting::engine::ScriptEngine;

/// A short loan with only event delivery and its immediate UI ending observation exposed; it owns no interpreter.
/// The historical effects type already carries the simulation PartitionManager.
/// This is an operation capability, not presentation state or a World locator.
#[derive(Clone, Copy)]
pub(crate) struct ScriptEventRecipient<'a>(&'a ScriptEngine);
impl<'a> ScriptEventRecipient<'a> {
    pub(crate) fn new(engine: &'a ScriptEngine) -> Self {
        Self(engine)
    }
    pub(crate) fn is_game_ending(self) -> bool {
        self.0.is_game_ending()
    }
    pub(crate) fn notify_object_creation_or_destruction(self, frame: u32) {
        self.0.set_frame_object_count_changed(frame);
    }
    pub(crate) fn run_team_script(
        self,
        world: &mut GameLogic,
        name: &str,
        team_id: u32,
        player_id: Option<u32>,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        world.run_owned_team_script_with_effects_checked(self.0, name, team_id, player_id, effects)
    }
    fn notify(self, player: usize, power: &str, source: u32, triggered: bool, completed: bool) {
        if triggered {
            self.0
                .notify_of_triggered_special_power(player, power, source);
        }
        if completed {
            self.0
                .notify_of_completed_special_power(player, power, source);
        }
    }
}

impl GameLogic {
    pub fn notify_script_engine_special_power_event(
        &self,
        source_object: ObjectId,
        power: &crate::command_system::SpecialPowerType,
        triggered: bool,
        completed: bool,
    ) {
        self.notify_script_engine_special_power_event_with_recipient(
            source_object,
            power,
            triggered,
            completed,
            None,
        )
    }
    pub(crate) fn notify_script_engine_special_power_event_with_recipient(
        &self,
        source_object: ObjectId,
        power: &crate::command_system::SpecialPowerType,
        triggered: bool,
        completed: bool,
        recipient: Option<ScriptEventRecipient<'_>>,
    ) {
        if !triggered && !completed {
            return;
        }
        let player = self
            .host_object(source_object)
            .and_then(|object| self.player_owner_for_host_object(object))
            .unwrap_or(0);
        let name =
            crate::game_logic::host_special_power_enum_residual::special_power_ini_template_name(
                power,
            );
        self.notify_receiving_special_power_event(
            player as usize,
            name,
            source_object.0,
            triggered,
            completed,
            recipient,
        );
    }
    fn notify_receiving_special_power_event(
        &self,
        player: usize,
        power: &str,
        source: u32,
        triggered: bool,
        completed: bool,
        recipient: Option<ScriptEventRecipient<'_>>,
    ) {
        // The already-supplied operation loan wins while the World field is out.
        // Ordinary/scriptless calls may use only this World's existing owner.
        if let Some(recipient) = recipient.or_else(|| {
            self.owned_script_engine
                .as_ref()
                .map(ScriptEventRecipient::new)
        }) {
            recipient.notify(player, power, source, triggered, completed);
        }
    }
    /// Transfer completion responsibility before launch or placement.
    /// A shooter with a completion module notifies now; its projectile must not.
    pub(super) fn bind_weapon_projectile_completion(
        &mut self,
        shooter: ObjectId,
        projectile: ObjectId,
        effects: &ScriptPresentationEffects<'_>,
    ) {
        use super::native_die_definition::NativeDieKind;
        let shooter_has_completion = self
            .objects
            .get(&shooter)
            .and_then(|object| object.get_template().native_die_declarations.as_deref())
            .is_some_and(|modules| {
                modules
                    .iter()
                    .any(|module| matches!(&module.kind, NativeDieKind::Completion { .. }))
            });
        let creator = if shooter_has_completion {
            self.maybe_notify_special_power_completion(shooter, effects);
            gamelogic::common::INVALID_OBJECT_ID
        } else {
            shooter.0
        };
        let Some(actor) = self.objects.get_mut(&projectile) else {
            return;
        };
        let power = actor
            .get_template()
            .native_die_declarations
            .as_deref()
            .and_then(|modules| {
                modules.iter().find_map(|module| match &module.kind {
                    NativeDieKind::Completion { power } => Some(power),
                    _ => None,
                })
            })
            .filter(|power| !power.is_empty())
            .cloned();
        if let Some(power) = power {
            actor.set_special_power_completion(power, creator);
        }
    }

    pub(crate) fn maybe_notify_special_power_completion(
        &mut self,
        id: ObjectId,
        effects: &ScriptPresentationEffects<'_>,
    ) {
        let Some(object) = self.objects.get(&id) else {
            return;
        };
        let Some(data) = object.special_power_completion.as_ref() else {
            return;
        };
        if !data.creator_set || data.creator_id == gamelogic::common::INVALID_OBJECT_ID {
            return;
        }
        let power = data.special_power_name.clone();
        let creator = data.creator_id;
        let player = self.player_owner_for_host_object(object).unwrap_or(0);
        // CPP SpecialPowerCompletionDie45: this exact onDie visit, before the
        // next authored callback. The existing log stays diagnostic only.
        self.notify_receiving_special_power_event(
            player as usize,
            &power,
            creator,
            false,
            true,
            effects.script_events,
        );
        self.special_power_completion_log
            .record_notify(&power, creator);
    }
}
