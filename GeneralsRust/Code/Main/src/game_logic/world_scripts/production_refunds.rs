//! Paid production cancellation and its exact controlling-player refund.
//! C++ ProductionUpdate.cpp:308-357,443-472. Queue order and ledger handling
//! remain at their existing host boundaries; Player owns the refund accounting.
use super::super::super::*;

impl GameLogic {
    /// Reconcile player-owned state and refund a production entry removed from
    /// a producer queue.
    ///
    /// Player upgrades have two coupled states in C++, not just the queue
    /// entry: `ProductionUpdate::cancelUpgrade` also removes the player's
    /// `IN_PRODUCTION` upgrade status.  Leaving that status behind makes a
    /// cancelled research item impossible to buy again.
    ///
    /// C++ `ProductionUpdate::cancelUnitCreate` / `cancelUpgrade` deposit to
    /// `getObject()->getControllingPlayer()` (ProductionUpdate.cpp:316, :456).
    /// Fail-closed: an explicit but stale owner is not rewritten to the first
    /// same-faction slot.
    pub(in crate::game_logic::game_logic) fn refund_cancelled_production_item(
        &mut self,
        owner_player_id: Option<u32>,
        team: Team,
        item: &ProductionItem,
    ) {
        let Some(player_id) = self.player_owner_for_event(owner_player_id, team) else {
            return;
        };

        let mut cancelled_upgrade = None;

        if let Some(player) = self.get_player_mut(player_id) {
            if item.is_upgrade() {
                let player_id = player.id;
                if !player.cancel_queued_upgrade(&item.template_name, &item.cost) {
                    // A normal queue always has the matching player state.
                    // If a restored/legacy save left only the producer entry,
                    // still refund its recorded cost rather than deleting paid
                    // research with no recovery path.
                    player.refund_production_cost(&item.cost);
                }
                cancelled_upgrade = Some((player_id, item.template_name.clone()));
            } else {
                player.refund_production_cost(&item.cost);
            }
        }

        if let Some((player_id, upgrade_name)) = cancelled_upgrade {
            self.record_host_upgrade_cancelled(player_id, &upgrade_name);
        }
    }

    /// Cancel a queued production item by template name (last match).
    ///
    /// C++ cancelUnitCreate is by ProductionID. Name-based callers (hotkey /
    /// legacy HUD) click the newest duplicate icon; first-match would refund
    /// the in-progress head and leave the fresh tail.
    pub fn cancel_production(&mut self, producer_id: ObjectId, template_name: String) -> bool {
        let Some((team, owner_player_id)) = self
            .objects
            .get(&producer_id)
            .map(|p| (p.team, p.owner_player_id))
        else {
            return false;
        };
        // C++ cancelUnitCreate requires getControllingPlayer(); do not refund
        // an arbitrary same-faction teammate when ownership is ambiguous.
        if self.player_owner_for_event(owner_player_id, team).is_none() {
            return false;
        }

        let cancel_pos = self.objects.get(&producer_id).and_then(|producer| {
            producer.building_data.as_ref().and_then(|building| {
                building
                    .production_queue
                    .iter()
                    .rposition(|item| item.template_name.eq_ignore_ascii_case(&template_name))
            })
        });
        if let Some(pos) = cancel_pos {
            self.unreserve_airfield_door_for_cancelled_queue_item(producer_id, pos, &template_name);
        }
        let mut cancelled: Option<ProductionItem> = None;
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                if let Some(pos) = cancel_pos {
                    cancelled = building.cancel_production(pos);
                }
            }
        }

        if let Some(item) = cancelled {
            self.refund_cancelled_production_item(owner_player_id, team, &item);
            crate::game_logic::host_production_log::record_cancel(producer_id, item.template_name);
            // Wave 485: last cancelled item clears factory exit-delay residual.
            if let Some(producer) = self.objects.get_mut(&producer_id) {
                if let Some(building) = producer.building_data.as_mut() {
                    if building.production_queue.is_empty() && building.exit_delay_remaining > 0.0 {
                        building.exit_delay_remaining = 0.0;
                        crate::game_logic::host_production_progress_log::record_exit_delay_only(
                            producer_id,
                            0.0,
                        );
                    }
                }
            }
            return true;
        }

        false
    }

    /// Cancel exactly one displayed production-queue slot and refund its owner.
    ///
    /// C++ ControlBar cancellation is positional: duplicate templates can be
    /// queued more than once, and clicking the second icon must leave the first
    /// one intact.  `cancel_production` remains the name-based API used by
    /// older callers; the authoritative HUD bridge uses this index-preserving
    /// variant.
    pub fn cancel_production_at_index(
        &mut self,
        producer_id: ObjectId,
        queue_index: usize,
    ) -> bool {
        let Some((team, owner_player_id)) = self
            .objects
            .get(&producer_id)
            .map(|producer| (producer.team, producer.owner_player_id))
        else {
            return false;
        };
        if self.player_owner_for_event(owner_player_id, team).is_none() {
            return false;
        }

        if let Some(name) = self.objects.get(&producer_id).and_then(|producer| {
            producer
                .building_data
                .as_ref()
                .and_then(|building| building.production_queue.get(queue_index))
                .map(|item| item.template_name.clone())
        }) {
            self.unreserve_airfield_door_for_cancelled_queue_item(producer_id, queue_index, &name);
        }
        let mut cancelled = None;
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                cancelled = building.cancel_production(queue_index);
            }
        }

        let Some(item) = cancelled else {
            return false;
        };

        self.refund_cancelled_production_item(owner_player_id, team, &item);
        crate::game_logic::host_production_log::record_cancel(producer_id, item.template_name);

        // The final cancellation releases the factory door immediately just as
        // the name-based path does; otherwise stale exit-delay state can keep a
        // completed producer visually occupied.
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                if building.production_queue.is_empty() && building.exit_delay_remaining > 0.0 {
                    building.exit_delay_remaining = 0.0;
                    crate::game_logic::host_production_progress_log::record_exit_delay_only(
                        producer_id,
                        0.0,
                    );
                }
            }
        }
        true
    }

    /// Wave 985: host production pause residual (ControlBar empty dual-world queue).
    pub fn set_production_paused(&mut self, producer_id: ObjectId, paused: bool) -> bool {
        let Some(producer) = self.objects.get_mut(&producer_id) else {
            return false;
        };
        let Some(building) = producer.building_data.as_mut() else {
            return false;
        };
        building.set_production_paused(paused);
        true
    }

    /// Cancel every queued production item on a producer and refund the owner.
    pub fn cancel_all_production(&mut self, producer_id: ObjectId) -> bool {
        let Some((team, owner_player_id)) = self
            .objects
            .get(&producer_id)
            .map(|p| (p.team, p.owner_player_id))
        else {
            return false;
        };
        if self.player_owner_for_event(owner_player_id, team).is_none() {
            return false;
        }

        let mut cancelled_items: Vec<ProductionItem> = Vec::new();
        let mut cancelled_any = false;
        let mut cancelled_names: Vec<String> = Vec::new();
        let mut cleared_exit_delay = false;
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                for item in building.production_queue.drain(..) {
                    cancelled_names.push(item.template_name.clone());
                    cancelled_items.push(item);
                    cancelled_any = true;
                }
                // Wave 485: empty queue clears QueueProductionExitUpdate residual.
                if cancelled_any && building.exit_delay_remaining > 0.0 {
                    building.exit_delay_remaining = 0.0;
                    cleared_exit_delay = true;
                }
            }
        }
        self.unreserve_all_airfield_exit_doors(producer_id);

        if cancelled_any {
            for item in &cancelled_items {
                self.refund_cancelled_production_item(owner_player_id, team, item);
            }
            // Wave 484: sole-tick skips per-frame progress log — Cancel refreshes
            // GW producer queue snapshot after host drain (sell/death/cancel-all).
            if cancelled_names.is_empty() {
                crate::game_logic::host_production_log::record_cancel(producer_id, String::new());
            } else {
                for name in cancelled_names {
                    crate::game_logic::host_production_log::record_cancel(producer_id, name);
                }
            }
            // Wave 485: publish exit-delay clear so GW sole-tick does not hold a ghost timer.
            if cleared_exit_delay {
                crate::game_logic::host_production_progress_log::record_exit_delay_only(
                    producer_id,
                    0.0,
                );
            }
        }

        cancelled_any
    }
}
