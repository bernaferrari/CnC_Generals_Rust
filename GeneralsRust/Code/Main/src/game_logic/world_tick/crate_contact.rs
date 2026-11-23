//! Authored MoneyCrateCollide and SalvageCrateCollide at the receiving partition contact.
use crate::game_logic::{
    AudioEventRequest, GameLogic, ObjectId, ScriptPresentationEffects, VeterancyLevel,
    host_money_crate::HostMoneyCrateRegistry, host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES,
    thing::AuthoredCrateCollide,
};
use game_engine::common::rts::science::SCIENCE_INVALID;
use game_engine::common::system::kind_of::KindOfMask;
use gamelogic::object::collide::crate_collide::{
    money_crate_collide::MoneyCrateCollideModuleData,
    salvage_crate_collide::SalvageCrateCollideModuleData,
};

impl GameLogic {
    pub(super) fn dispatch_owned_crate_contact(
        &mut self,
        crate_id: ObjectId,
        picker_id: ObjectId,
        collides: &[AuthoredCrateCollide],
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(crate_object) = self.objects.get(&crate_id) else {
            return Ok(false);
        };
        if crate_object.status.destroyed || crate_object.status.no_collisions {
            return Ok(false);
        }
        let Some(picker) = self.objects.get(&picker_id) else {
            return Ok(false);
        };
        let Some(player_id) = self.player_owner_for_host_object(picker) else {
            return Ok(false);
        };
        let player = &self.players[&player_id];
        // CrateCollide.cpp:127–168 and SalvageCrateCollide.cpp:38–48.
        // The ground and a neutral or effectively-dead picker cannot collect.
        // Crate body health is independent of pickup eligibility.
        let kinds = picker
            .get_template()
            .receiving_client_template
            .as_ref()
            .map_or_else(
                || picker.kind_of_cpp_mask(),
                |template| template.authored_kind_of_bits(),
            );
        if player.is_reserved_neutral()
            || picker.status.effectively_dead
            || kinds & KindOfMask::PARACHUTE.bits() != 0
        {
            return Ok(false);
        }
        let has_ai = picker.has_ai_update_interface();
        let is_structure = kinds & KindOfMask::STRUCTURE.bits() != 0;
        let crate_owner = self.player_owner_for_host_object(crate_object);
        let position = crate_object.get_position();
        let some_valid = collides.iter().find(|data| {
            let base = data.base();
            let building_attempt = base.is_building_pickup && is_structure;
            (!data.is_salvage() || kinds & KindOfMask::SALVAGER.bits() != 0)
                && (has_ai || building_attempt)
                && kinds & base.required_kind_of == base.required_kind_of
                && kinds & base.forbidden_kind_of == 0
                && (building_attempt || position.y <= self.raw_terrain_ground_height_at(position))
                && (!base.is_forbid_owner_player || crate_owner != Some(player_id))
                && (!base.is_human_only_pickup || player.is_human)
                && (base.pickup_science == SCIENCE_INVALID
                    || self
                        .receiving_module_content
                        .as_ref()
                        .is_some_and(|content| {
                            content
                                .science_facts()
                                .name(base.pickup_science)
                                .is_some_and(|name| player.has_unlocked_science(name))
                        }))
        });
        let Some(data) = some_valid else {
            return Ok(false);
        };

        let crate_birth = crate_object.visual_object_generation;
        let epoch = self.visual_world_epoch;
        let base = data.base();
        match data {
            AuthoredCrateCollide::Money(data) => self.pay_authored_money_crate(
                data,
                crate_id,
                picker_id,
                player_id,
                base.is_building_pickup && is_structure,
            ),
            AuthoredCrateCollide::Salvage(data) => {
                self.execute_authored_salvage_pickup(data, crate_id, picker_id, player_id, kinds);
            }
        }
        if let Some(fx) = &base.execute_fx {
            self.dispatch_fx_list_at_host_object_with_effects(fx, picker_id, None, effects);
        }
        if self.spawn_identity_matches(crate_id, crate_birth, epoch) {
            self.destroy_object_with_effects(crate_id, effects)?;
        }
        if self.get_draw_icon_ui() && self.spawn_identity_matches(crate_id, crate_birth, epoch) {
            let pos = self.objects[&crate_id].get_position();
            let view = &mut self.session_view;
            if let Some(animation) = view
                .definitions
                .animation_templates()
                .find(&base.execution_animation_template)
            {
                view.world_animations.admit(
                    crate::presentation_frame::PresentationWorldAnim {
                        identity: 0,
                        initial_frame: 0,
                        template: base.execution_animation_template.clone(),
                        position: pos,
                        display_time_seconds: base.execute_animation_display_time_seconds,
                        z_rise_per_second: base.execute_animation_z_rise_per_second,
                        // CrateCollide.cpp:116 passes WORLD_ANIM_FADE_ON_EXPIRE.
                        fades: true,
                        spawn_frame: self.frame,
                        crate_id,
                        picker_id,
                    },
                    animation,
                    &mut view.client_random,
                );
            }
        }
        Ok(true)
    }

    fn queue_authored_crate_sound(&mut self, picker_id: ObjectId, salvage_upgrade: bool) {
        if let Some(event) = self.session_view.definitions.salvage_sound(salvage_upgrade) {
            let event = AudioEventRequest::new(event).with_object(picker_id);
            self.queue_audio_event(event);
        }
    }

    fn pay_authored_money_crate(
        &mut self,
        data: &MoneyCrateCollideModuleData,
        crate_id: ObjectId,
        picker_id: ObjectId,
        player_id: u32,
        building_pickup: bool,
    ) {
        let completed = &self.players[&player_id].completed_upgrades;
        // Apply the first completed, receiving-authored upgrade. The original
        // function-static lookup reused the first entry for every subsequent row.
        let (boost, supply_lines_boost) = data
            .upgrade_boosts
            .iter()
            .find_map(|row| {
                let upgrade = self.upgrade_template(&row.upgrade_type)?;
                let name = upgrade.get_name().as_str();
                completed.contains(name).then(|| {
                    let supply_lines_boost = if name == UPGRADE_AMERICA_SUPPLY_LINES {
                        row.amount.max(0) as u32
                    } else {
                        0
                    };
                    (row.amount, supply_lines_boost)
                })
            })
            .unwrap_or_default();
        // C++ adds a signed boost to UnsignedInt; a zero amount still consumes
        // the crate, runs its pickup sound, and executes the base callbacks.
        let amount = data.money_provided.wrapping_add_signed(boost);
        if let Some((player, economy)) = self.player_and_economy_events_mut(player_id) {
            player.credit_supplies(amount, economy);
            player.add_money_earned(amount);
        }
        self.host_money_crates.record_authored_money_pickup(
            crate_id,
            amount,
            supply_lines_boost,
            building_pickup,
        );
        self.supply_lines_bonus_cash_total = self
            .supply_lines_bonus_cash_total
            .saturating_add(supply_lines_boost);
        self.queue_authored_crate_sound(picker_id, false);
    }

    fn execute_authored_salvage_pickup(
        &mut self,
        data: &SalvageCrateCollideModuleData,
        crate_id: ObjectId,
        picker_id: ObjectId,
        player_id: u32,
        kinds: u128,
    ) {
        let picker = &self.objects[&picker_id];
        let position = self.objects[&crate_id].get_position();
        let can_upgrade_armor =
            kinds & KindOfMask::ARMOR_SALVAGER.bits() != 0 && picker.armor_crate_upgrade < 2;
        let can_upgrade_weapon =
            kinds & KindOfMask::WEAPON_SALVAGER.bits() != 0 && picker.weapon_crate_upgrade() < 2;
        let can_gain_level =
            picker.is_trainable() && picker.experience.level != VeterancyLevel::Heroic;
        let play_sound = if can_upgrade_armor {
            self.objects
                .get_mut(&picker_id)
                .unwrap()
                .apply_salvage_armor_upgrade();
            Some(true)
        } else if can_upgrade_weapon && self.test_authored_salvage_chance(data.weapon_chance) {
            self.objects
                .get_mut(&picker_id)
                .unwrap()
                .apply_salvage_weapon_upgrade(self.frame);
            Some(true)
        } else if can_gain_level && self.test_authored_salvage_chance(data.level_chance) {
            let health_bonus = self.veterancy_health_bonus;
            self.with_object_veterancy_feedback_and_locomotors(
                picker_id,
                |picker, audio, feedback, locomotors| {
                    picker.apply_salvage_level_gain_in(
                        audio,
                        feedback,
                        Some(&health_bonus),
                        locomotors,
                    );
                },
            );
            // Promotion supplies its own audio.
            None
        } else {
            self.pay_authored_salvage_cash(data, crate_id, picker_id, player_id, position);
            Some(false)
        };
        if let Some(upgrade) = play_sound {
            self.queue_authored_crate_sound(picker_id, upgrade);
        }
        if let Some(player) = self.players.get_mut(&player_id) {
            player.academy_stats.record_salvage_collected();
        }
    }

    fn test_authored_salvage_chance(&self, chance: f32) -> bool {
        // SalvageCrateCollide.cpp:146–171. 100% consumes no stream draw.
        chance == 1. || self.logic_random.next_real(0., 1.) < chance
    }

    fn pay_authored_salvage_cash(
        &mut self,
        data: &SalvageCrateCollideModuleData,
        crate_id: ObjectId,
        picker_id: ObjectId,
        player_id: u32,
        position: glam::Vec3,
    ) {
        // SalvageCrateCollide.cpp:85–91,212–235. MoneyChance is parsed, but
        // this last branch is unconditional and a constant range draws nothing.
        let amount = if data.minimum_money == data.maximum_money {
            data.minimum_money
        } else {
            self.logic_random
                .next_int(data.minimum_money, data.maximum_money)
        };
        if amount <= 0 {
            return;
        }
        let amount = amount as u32;
        if let Some((player, economy)) = self.player_and_economy_events_mut(player_id) {
            player.credit_supplies(amount, economy);
            player.add_money_earned(amount);
            let color = player.color_rgb;
            self.host_money_crates.record_money_floating_text(
                HostMoneyCrateRegistry::salvage_money_floating_text(
                    crate_id, picker_id, position, amount, self.frame, color,
                ),
            );
        }
    }
}
