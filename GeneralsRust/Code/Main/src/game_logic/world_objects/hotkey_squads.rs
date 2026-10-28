//! C++ Player-owned hotkey squads on the receiving GameLogic roster.
use super::super::*;

impl GameLogic {
    pub(crate) fn player_hotkey_squads(&self, player_id: u32) -> [Vec<ObjectId>; 10] {
        self.get_player(player_id)
            .map(|player| player.hotkey_squads.clone())
            .unwrap_or_else(|| std::array::from_fn(|_| Vec::new()))
    }

    pub fn player_hotkey_squad_ids(&self, player_id: u32, slot: u8) -> Vec<ObjectId> {
        self.get_player(player_id)
            .and_then(|player| player.hotkey_squad(slot))
            .unwrap_or_default()
            .to_vec()
    }

    pub(crate) fn replace_player_hotkey_squads(
        &mut self,
        player_id: u32,
        squads: [Vec<ObjectId>; 10],
    ) {
        if let Some(player) = self.get_player_mut(player_id) {
            player.replace_hotkey_squads(squads);
        }
    }

    /// Apply C++ `Player::processCreateTeamGameMessage` to this world's
    /// receiving player and object roster.
    pub fn process_player_create_team(&mut self, player_id: u32, slot: u8, ids: &[ObjectId]) {
        let live_ids: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| self.objects.contains_key(id))
            .collect();
        if let Some(player) = self.get_player_mut(player_id) {
            player.create_hotkey_squad(slot, &live_ids, |_| true);
        }
    }

    /// C++ Squad::getLiveObjects prunes missing IDs even for camera-only
    /// double taps and VIEW_TEAM, then filters its returned live objects.
    pub(crate) fn live_player_hotkey_squad_ids(
        &mut self,
        player_id: u32,
        slot: u8,
    ) -> Option<Vec<ObjectId>> {
        let stored = self.get_player(player_id)?.hotkey_squad(slot)?.to_vec();
        let extant: Vec<_> = stored
            .iter()
            .copied()
            .filter(|id| self.objects.contains_key(id))
            .collect();
        let live: Vec<_> = extant
            .iter()
            .copied()
            .filter(|id| {
                self.objects
                    .get(id)
                    .is_some_and(|object| object.is_selectable())
            })
            .collect();
        *self
            .get_player_mut(player_id)?
            .hotkey_squads
            .get_mut(slot as usize)? = extant;
        Some(live)
    }

    /// Apply C++ `Player::processSelectTeamGameMessage` to this receiver.
    pub fn process_player_select_team(&mut self, player_id: u32, slot: u8) -> bool {
        let Some(live) = self.live_player_hotkey_squad_ids(player_id, slot) else {
            return false;
        };
        let live: std::collections::HashSet<_> = live.into_iter().collect();
        self.get_player_mut(player_id)
            .is_some_and(|player| player.select_hotkey_squad(slot, |id| live.contains(&id)))
    }

    /// Apply C++ `Player::processAddTeamGameMessage` to this receiver.
    pub fn process_player_add_team(&mut self, player_id: u32, slot: u8) -> bool {
        let Some(live) = self.live_player_hotkey_squad_ids(player_id, slot) else {
            return false;
        };
        let live: std::collections::HashSet<_> = live.into_iter().collect();
        self.get_player_mut(player_id).is_some_and(|player| {
            player.add_hotkey_squad_to_selection(slot, |id| live.contains(&id))
        })
    }
}

#[cfg(test)]
#[test]
fn same_id_sessions_keep_replay_squads_and_empty_selection_through_snapshot_restore() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "same_id_sessions_keep_replay_squads_and_empty_selection_through_snapshot_restore",
        || {
            use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

            fn roster(name: &str, count: usize) -> (GameLogic, Vec<ObjectId>) {
                let mut logic = GameLogic::new();
                let mut template = ThingTemplate::new("HotkeySquadProbe");
                template
                    .set_health(100.0)
                    .add_kind_of(KindOf::Infantry)
                    .add_kind_of(KindOf::Selectable);
                logic.templates.insert(template.name.clone(), template);
                logic.add_player(Player::new(0, Team::USA, name, true));
                let ids = (0..count)
                    .map(|n| {
                        logic
                            .create_object_for_player(
                                "HotkeySquadProbe",
                                0,
                                glam::Vec3::new(n as f32 * 10.0, 0.0, 0.0),
                            )
                            .expect("owned roster object")
                    })
                    .collect();
                (logic, ids)
            }

            let (mut owner, owner_ids) = roster("Owner", 1);
            let (mut peer, peer_ids) = roster("Peer", 2);
            assert_eq!(owner_ids[0], peer_ids[0], "world-local IDs may collide");

            owner.process_player_create_team(0, 3, &owner_ids);
            peer.process_player_create_team(0, 3, &peer_ids);
            assert!(owner.process_player_select_team(0, 3));
            assert!(peer.process_player_select_team(0, 3));
            assert_eq!(owner.player_selected_objects(0), owner_ids);
            assert_eq!(peer.player_selected_objects(0), peer_ids);
            assert_eq!(owner.player_hotkey_squad_ids(0, 3), owner_ids);
            assert_eq!(peer.player_hotkey_squad_ids(0, 3), peer_ids);

            // An always-present empty slot clears only its receiving Player.
            assert!(owner.process_player_select_team(0, 9));
            assert_eq!(owner.player_selected_objects(0), Vec::<ObjectId>::new());
            assert_eq!(peer.player_selected_objects(0), peer_ids);

            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&owner).unwrap();
            let mut restored = GameLogic::new();
            let mut template = ThingTemplate::new("HotkeySquadProbe");
            template
                .set_health(100.0)
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable);
            restored.templates.insert(template.name.clone(), template);
            restored.add_player(Player::new(0, Team::USA, "Owner", true));
            builder
                .restore_from_snapshot(&snapshot, &mut restored)
                .unwrap();

            assert_eq!(restored.player_selected_objects(0), Vec::<ObjectId>::new());
            assert_eq!(restored.player_hotkey_squad_ids(0, 3), owner_ids);
            assert_eq!(peer.player_hotkey_squad_ids(0, 3), peer_ids);
            assert_eq!(peer.player_selected_objects(0), peer_ids);
        },
    );
}
