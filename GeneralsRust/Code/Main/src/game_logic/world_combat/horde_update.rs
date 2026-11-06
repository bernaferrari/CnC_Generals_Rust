//! C++ `HordeUpdate::update` (HordeUpdate.cpp:236-330) for every Object whose
//! template authors a `HordeUpdate` module. Membership parameters
//! (`Count`, `Radius`, `RubOffRadius`, `KindOf`, `AlliesOnly`, `ExactMatch`,
//! `UpdateRate`) and recipients come from that module; no unit name selects
//! a horde member.
use super::super::*;
use crate::game_logic::HordeUpdateRules;
use crate::game_logic::host_battlemaster::{
    LeftoverHordeScanUnit, evaluate_leftover_horde_blob_scan,
    leftover_horde_bounding_sphere_radius, leftover_horde_take_wake,
};

struct HordeMember {
    id: ObjectId,
    team: Team,
    owner: Option<u32>,
    template_name: String,
    kind_of: u128,
    off_map: bool,
    is_infantry: bool,
    rules: HordeUpdateRules,
    scan: LeftoverHordeScanUnit,
}

impl GameLogic {
    /// One logic frame of every live authored `HordeUpdate` module.
    pub(crate) fn update_horde_modules(&mut self) {
        let members: Vec<HordeMember> = self
            .objects
            .registered()
            .filter_map(|(id, object)| {
                let rules = object.thing().template.horde_update.clone()?;
                if !object.is_alive() {
                    return None;
                }
                let position = object.get_position();
                let geometry = &(*object.geometry_info());
                Some(HordeMember {
                    id: *id,
                    team: object.team,
                    owner: object.owner_player_id,
                    template_name: object.template_name.clone(),
                    kind_of: object.kind_of_cpp_mask(),
                    off_map: object.status.off_map,
                    is_infantry: object.is_kind_of(KindOf::Infantry),
                    rules,
                    scan: LeftoverHordeScanUnit {
                        x: position.x,
                        y: position.y,
                        z: position.z,
                        sphere_radius: leftover_horde_bounding_sphere_radius(
                            geometry.authored,
                            geometry.bounding_sphere_radius(),
                            object.selection_radius,
                        ),
                        alive: true,
                    },
                })
            })
            .collect();
        if members.is_empty() {
            return;
        }

        let units: Vec<LeftoverHordeScanUnit> = members.iter().map(|m| m.scan).collect();
        let membership = evaluate_leftover_horde_blob_scan(
            &units,
            |i| (members[i].rules.min_count, members[i].rules.rub_off_radius),
            |i, j, dist| {
                let (me, other) = (&members[i], &members[j]);
                // PartitionFilterHordeMember::allow (HordeUpdate.cpp:62-84).
                dist <= me.rules.min_dist
                    && (!me.rules.exact_match || me.template_name == other.template_name)
                    && crate::game_logic::host_radar_stealth_vision_residual::detector_accepts_kindof_residual(
                        other.kind_of,
                        me.rules.kind_of_mask,
                        0,
                    )
                    && (!me.rules.allies_only
                        || self.horde_allies_only(me.owner, me.team, other.owner, other.team))
                    && me.off_map == other.off_map
            },
        );

        let draw_icon = self.get_draw_icon_ui();
        let frame = self.frame;
        let mut vehicle_grants = 0u32;
        let mut to_refresh = Vec::new();
        for (member, scanned) in members.iter().zip(membership) {
            let morale = self.owner_morale_upgrades(member.owner);
            let Some(object) = self.objects.get_mut(&member.id) else {
                continue;
            };
            let (due, initialized, last, next) = leftover_horde_take_wake(
                object.horde_wake_initialized,
                member.is_infantry,
                frame,
                object.last_horde_refresh_frame,
                object.horde_next_wake_frame,
                member.rules.update_rate_frames,
            );
            object.horde_wake_initialized = initialized;
            object.last_horde_refresh_frame = last;
            object.horde_next_wake_frame = next;
            // Infantry sleep UpdateRate (UPDATE_SLEEP); vehicles run every
            // frame for the decal but rescan only when the rate elapsed.
            if !due && member.is_infantry {
                continue;
            }
            let was = object.weapon_bonus_horde;
            let now = if due { scanned.in_horde } else { was };
            if due {
                object.weapon_bonus_horde = now;
                let morale_before = (
                    object.weapon_bonus_nationalism,
                    object.weapon_bonus_fanaticism,
                );
                super::tanks_and_upgrades::apply_evaluate_morale_bonus(object, morale);
                let morale_changed = morale_before
                    != (
                        object.weapon_bonus_nationalism,
                        object.weapon_bonus_fanaticism,
                    );
                if now && !was && !member.is_infantry {
                    vehicle_grants = vehicle_grants.saturating_add(1);
                }
                if now != was || now || morale_changed {
                    to_refresh.push((member.id, now && !was));
                }
            }
            object.apply_horde_terrain_decal(was, now, draw_icon);
        }
        self.battlemaster_residual_horde_grants = self
            .battlemaster_residual_horde_grants
            .saturating_add(vehicle_grants);
        for (id, granted) in to_refresh {
            self.refresh_horde_residual_weapon(id, granted);
        }
    }

    /// Re-derive the residual weapon stats that still bake HORDE /
    /// NATIONALISM into the bound Weapon. Each refresh is keyed on the
    /// authored weapon it owns and is a no-op for other objects.
    fn refresh_horde_residual_weapon(&mut self, id: ObjectId, granted: bool) {
        let Some(object) = self.objects.get(&id) else {
            return;
        };
        let red_guard = object.authors_red_guard_machine_gun();
        let tank_hunter = object.authors_tank_hunter_missile();
        let minigunner = object.authors_minigunner_gun();
        if granted {
            if red_guard {
                self.red_guard_residual_horde_grants =
                    self.red_guard_residual_horde_grants.saturating_add(1);
            } else if tank_hunter {
                self.tank_hunter_residual_horde_grants =
                    self.tank_hunter_residual_horde_grants.saturating_add(1);
            } else if minigunner {
                self.minigunner_residual_horde_grants =
                    self.minigunner_residual_horde_grants.saturating_add(1);
            }
        }
        if red_guard {
            self.refresh_red_guard_weapon(id);
        } else if tank_hunter {
            self.refresh_tank_hunter_weapon(id);
        } else if minigunner {
            self.refresh_minigunner_weapon(id);
        } else {
            self.refresh_battlemaster_weapon(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game_logic::{GameLogic, ObjectId, Player, Team};
    use glam::Vec3;

    const INFANTRY_HORDE: &str = "Behavior = HordeUpdate HordeTag\nUpdateRate = 1000\nRadius = 30\nKindOf = INFANTRY\nAlliesOnly = Yes\nExactMatch = No\nCount = 5\nAction = HORDE\n";

    fn object_ini(name: &str, kind_of: &str, modules: &str) -> String {
        format!(
            "Object {name}\nKindOf = {kind_of} SELECTABLE\nGeometry = CYLINDER\nGeometryMajorRadius = 1.0\nGeometryHeight = 2.0\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{modules}End\n"
        )
    }

    fn world(objects: &[String]) -> GameLogic {
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "receiving-horde.ini".into(),
                    objects.concat(),
                )])
                .unwrap(),
            )
            .unwrap();
        world.add_player(Player::new(0, Team::China, "Owner", true));
        world
    }

    fn spawn(world: &mut GameLogic, name: &str, player: u32, xs: &[f32]) -> Vec<ObjectId> {
        xs.iter()
            .map(|&x| {
                world
                    .create_object_for_player(name, player, Vec3::new(x, 0.0, 0.0))
                    .unwrap()
            })
            .collect()
    }

    fn in_horde(world: &GameLogic, id: ObjectId) -> bool {
        world.host_object(id).unwrap().weapon_bonus_horde
    }

    /// HordeUpdate.cpp:268-283: an object short of `Count` joins as an
    /// honorary member within the authored `RubOffRadius` (retail infantry
    /// author 60, not the module default 20) of a true member it scanned.
    #[test]
    fn rub_off_radius_comes_from_the_authored_module() {
        for (rub_off, expected) in [("60", true), ("20", false)] {
            let modules = format!("{INFANTRY_HORDE}RubOffRadius = {rub_off}\nEnd\n");
            let mut world = world(&[object_ini("PlainRifleman", "INFANTRY", &modules)]);
            let core = spawn(&mut world, "PlainRifleman", 0, &[0.0, 5.0, 10.0, 15.0, 20.0]);
            let straggler = spawn(&mut world, "PlainRifleman", 0, &[45.0])[0];
            world.update_horde_modules();
            assert!(core.iter().all(|&id| in_horde(&world, id)));
            assert_eq!(in_horde(&world, straggler), expected, "RubOffRadius {rub_off}");
        }
    }

    /// `Radius` / `Count` are the module's: five units 40 apart horde only
    /// when the authored scan radius reaches them.
    #[test]
    fn radius_and_count_come_from_the_authored_module() {
        for (radius, count, expected) in [("30", "5", false), ("200", "5", true), ("200", "6", false)] {
            let modules = format!(
                "Behavior = HordeUpdate HordeTag\nRadius = {radius}\nCount = {count}\nKindOf = INFANTRY\nEnd\n"
            );
            let mut world = world(&[object_ini("PlainRifleman", "INFANTRY", &modules)]);
            let ids = spawn(&mut world, "PlainRifleman", 0, &[0.0, 40.0, 80.0, 120.0, 160.0]);
            world.update_horde_modules();
            assert!(
                ids.iter().all(|&id| in_horde(&world, id) == expected),
                "Radius {radius} Count {count}"
            );
        }
    }

    /// Recipients and neighbours are objects authoring HordeUpdate
    /// (`getHUI`), never a template name: a retail-named template without
    /// the module neither hordes nor counts.
    #[test]
    fn only_objects_authoring_horde_update_participate() {
        let modules = format!("{INFANTRY_HORDE}End\n");
        let mut world = world(&[
            object_ini("PlainRifleman", "INFANTRY", &modules),
            object_ini("ChinaInfantryRedguard", "INFANTRY", ""),
        ]);
        let authored = spawn(&mut world, "PlainRifleman", 0, &[0.0, 5.0, 10.0]);
        let named = spawn(&mut world, "ChinaInfantryRedguard", 0, &[2.0, 7.0, 12.0]);
        world.update_horde_modules();
        assert!(authored.iter().all(|&id| !in_horde(&world, id)));
        assert!(named.iter().all(|&id| !in_horde(&world, id)));

        let mut world2 = self::world(&[object_ini("PlainRifleman", "INFANTRY", &modules)]);
        let five = spawn(&mut world2, "PlainRifleman", 0, &[0.0, 5.0, 10.0, 2.0, 7.0]);
        world2.update_horde_modules();
        assert!(five.iter().all(|&id| in_horde(&world2, id)));
    }

    /// PartitionFilterHordeMember: `ExactMatch` requires the same template
    /// and `KindOf` must match the other object.
    #[test]
    fn exact_match_and_kind_of_filter_neighbours() {
        let vehicle = "Behavior = HordeUpdate HordeTag\nRadius = 75\nKindOf = VEHICLE\nExactMatch = Yes\nCount = 5\nRubOffRadius = 150\nEnd\n";
        let infantry = format!("{INFANTRY_HORDE}End\n");
        let mut world = world(&[
            object_ini("PlainTankA", "VEHICLE", vehicle),
            object_ini("PlainTankB", "VEHICLE", vehicle),
            object_ini("PlainRifleman", "INFANTRY", &infantry),
        ]);
        let a = spawn(&mut world, "PlainTankA", 0, &[0.0, 10.0, 20.0]);
        let b = spawn(&mut world, "PlainTankB", 0, &[5.0, 15.0]);
        let riflemen = spawn(&mut world, "PlainRifleman", 0, &[1.0, 6.0, 11.0]);
        world.update_horde_modules();
        assert!(a.iter().chain(&b).all(|&id| !in_horde(&world, id)));
        // Riflemen do not count vehicles toward their INFANTRY horde.
        assert!(riflemen.iter().all(|&id| !in_horde(&world, id)));

        let more = spawn(&mut world, "PlainTankA", 0, &[30.0, 40.0]);
        world.frame = 31;
        world.update_horde_modules();
        assert!(a.iter().chain(&more).all(|&id| in_horde(&world, id)));
        assert!(b.iter().all(|&id| !in_horde(&world, id)));
    }

    /// C++ HordeUpdate.cpp:77-79 AlliesOnly uses getRelationship == ALLIES:
    /// a same-faction enemy blob is not one horde.
    #[test]
    fn allies_only_excludes_same_faction_enemies() {
        let modules = format!("{INFANTRY_HORDE}End\n");
        let mut world = world(&[object_ini("PlainRifleman", "INFANTRY", &modules)]);
        let mut a = Player::new(1, Team::China, "ChinaA", true);
        a.alliance_team = 0;
        let mut b = Player::new(2, Team::China, "ChinaB", false);
        b.alliance_team = 1;
        world.add_player(a);
        world.add_player(b);
        let a_ids = spawn(&mut world, "PlainRifleman", 1, &[0.0, 8.0, 16.0]);
        let b_ids = spawn(&mut world, "PlainRifleman", 2, &[4.0, 12.0, 20.0]);
        world.update_horde_modules();
        assert!(a_ids.iter().chain(&b_ids).all(|&id| !in_horde(&world, id)));

        let a_more = spawn(&mut world, "PlainRifleman", 1, &[24.0, 2.0]);
        world.frame = 30;
        world.update_horde_modules();
        assert!(in_horde(&world, a_ids[0]));
        assert!(a_more.iter().all(|&id| in_horde(&world, id)));
        assert!(!in_horde(&world, b_ids[0]));
    }
}
