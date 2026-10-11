//! Borrowed upgrade-only ProductionUpdate phase; unit production retains its adapter.
use super::*;
use game_engine::common::thing::update_module::{
    UpdateScheduleContext, UpgradeProductionObservation,
};

impl ProductionUpdateComplete {
    pub(crate) fn queue_upgrade_owned(
        &mut self,
        name: String,
        cost: i32,
        frames: u32,
        player_id: ObjectID,
    ) -> Result<(), String> {
        if !self.has_queue_capacity() {
            return Err("Build queue is full".into());
        }
        self.queue.enqueue(BuildQueueEntry::new(
            name,
            ProductionType::Upgrade,
            cost,
            frames,
            player_id,
        ))?;
        if self.current_production.is_none() {
            self.current_production = self.queue.dequeue().map(ProductionEntryState::from_entry);
        }
        Ok(())
    }

    /// None selects the existing unit path; Some(None) is an incomplete upgrade.
    /// Take current production before effects, as ProductionUpdate.cpp:874 does.
    pub(crate) fn update_upgrade_owned(
        &mut self,
        context: &mut dyn UpdateScheduleContext,
        observation: UpgradeProductionObservation,
    ) -> Option<Option<String>> {
        if !self
            .current_production
            .as_ref()
            .is_some_and(|production| production.entry.production_type == ProductionType::Upgrade)
        {
            return None;
        }
        let frame = context.frame();
        self.last_update_frame = frame;
        if !self.production_enabled {
            return Some(None);
        }
        self.update_upgrade_animations_owned(context);
        if self.should_halt_production(observation.owner_sold, false) {
            return Some(None);
        }
        let modifiers = PlayerBuildModifiers {
            handicap_time_multiplier: observation.handicap_time_multiplier,
            energy_supply_ratio: observation.energy_supply_ratio,
            ..Default::default()
        };
        if !self.update_production_progress(1, &modifiers, None) {
            return Some(None);
        }
        Some(
            self.current_production
                .take()
                .map(|production| production.entry.template_name),
        )
    }

    pub(crate) fn resume_upgrade_owned(&mut self, context: &mut dyn UpdateScheduleContext) {
        // Effects can append work. Resume the actual queue only after they return.
        if self.current_production.is_none() {
            if let Some(entry) = self.queue.dequeue() {
                let mut state = ProductionEntryState::from_entry(entry);
                state.quantity_total = self.quantity_for_template(&state.entry.template_name);
                self.current_production = Some(state);
            }
        }
        let flag = MODELCONDITION_ACTIVELY_CONSTRUCTING;
        let active = self.current_production.is_some() || !self.queue.is_empty();
        context.change_owner_model_conditions(
            if active { 0 } else { flag.bits() },
            if active { flag.bits() } else { 0 },
        );
    }

    fn update_upgrade_animations_owned(&mut self, context: &mut dyn UpdateScheduleContext) {
        let frame = context.frame();
        for index in 0..self.data.num_door_animations.min(DOOR_COUNT_MAX as i32) as usize {
            let door = &mut self.doors[index];
            let transition = if door.door_opened_frame > 0 {
                if frame - door.door_opened_frame > self.data.door_opening_time {
                    door.door_opened_frame = 0;
                    door.door_wait_open_frame = frame;
                    Some((OPENING_FLAGS[index], WAITING_OPEN_FLAGS[index]))
                } else {
                    None
                }
            } else if door.door_wait_open_frame > 0 && !door.hold_open {
                if frame - door.door_wait_open_frame > self.data.door_wait_open_time {
                    door.door_wait_open_frame = 0;
                    door.door_closed_frame = frame;
                    Some((WAITING_OPEN_FLAGS[index], CLOSING_FLAGS[index]))
                } else {
                    None
                }
            } else if door.door_closed_frame > 0 && !door.hold_open {
                if frame - door.door_closed_frame > self.data.door_closing_time {
                    door.door_closed_frame = 0;
                    Some((CLOSING_FLAGS[index], ModelConditionFlags::empty()))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some((clear, set)) = transition {
                context.change_owner_model_conditions(clear.bits(), set.bits());
            }
        }
        if self.construction_complete_frame > 0
            && frame - self.construction_complete_frame > self.data.construction_complete_duration
        {
            self.construction_complete_frame = 0;
            context.change_owner_model_conditions(
                ModelConditionFlags::CONSTRUCTION_COMPLETE.bits(),
                0,
            );
        }
    }
}
