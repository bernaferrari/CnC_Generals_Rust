//! Real admitted source order and synchronous Body continuation, no tick pump.
use super::*;
use crate::game_logic::host_temporary_weapon_behavior::{
    FireWeaponWhenDamagedWeaponRole as Role, TemporaryWeaponRuntimeKey,
};
use crate::game_logic::{Player, Team};
use glam::Vec3;

fn fixture(mine_first: bool, rubble_reaction: bool) -> (GameLogic, ObjectId, u32) {
    let mine =
        "Behavior = MinefieldBehavior MineTag\n NumVirtualMines = 8\n Regenerates = Yes\nEnd\n";
    let role = if rubble_reaction {
        "Rubble"
    } else {
        "ReallyDamaged"
    };
    let reaction = format!(
        "Behavior = FireWeaponWhenDamagedBehavior ReactionTag\n StartsActive = Yes\n DamageAmount = 1\n ReactionWeapon{role} = OwnedMineReaction541\nEnd\n"
    );
    let ordered = if mine_first {
        format!("{mine}{reaction}")
    } else {
        format!("{reaction}{mine}")
    };
    let source = format!(
        "Object ChinaStandardMine\n KindOf = IMMOBILE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n{ordered}End\n"
    );
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "mine-callback-owner.ini".into(),
                source,
            )])
            .unwrap(),
        )
        .unwrap();
    let mut definitions = std::collections::HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon OwnedMineReaction541\n PrimaryDamage = 1\n PrimaryDamageRadius = 1\n DamageType = EXPLOSION\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n ClipSize = 2\n ClipReloadTime = 0\n DelayBetweenShots = 0\nEnd\n",
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        ),
        1
    );
    world
        .admit_weapon_content_snapshot(
            definitions
                .into_values()
                .map(|rule| (rule.name.clone(), rule))
                .collect(),
            &[],
        )
        .unwrap();
    world.add_player(Player::new(0, Team::China, "MineOwner", true));
    let id = world
        .create_object_for_player("ChinaStandardMine", 0, Vec3::ZERO)
        .unwrap();
    // The existing curated StandardMine runtime agrees with the admitted
    // eight-charge regenerative source. No mine tick is needed to bind it.
    let data = world
        .host_object_mut(id)
        .unwrap()
        .mine_data
        .as_mut()
        .unwrap();
    assert_eq!(data.num_virtual_mines, 8);
    assert!(data.regenerates);
    // No blast victims in this component scenario; eliminate unrelated FX.
    data.detonation_damage = 0.;
    data.detonation_radius = 0.;
    let object = world.host_object(id).unwrap();
    let metadata = &object.thing().template.fire_weapon_when_damaged_behaviors[0];
    let index = object
        .thing()
        .template
        .receiving_object_definition
        .as_ref()
        .unwrap()
        .behavior_modules
        .iter()
        .position(|module| {
            module
                .class_name
                .eq_ignore_ascii_case("FireWeaponWhenDamagedBehavior")
        })
        .unwrap() as u32;
    assert_eq!(metadata.module_source_index, index);
    assert_eq!(
        metadata.weapon_template_name(if rubble_reaction {
            Role::ReactionRubble
        } else {
            Role::ReactionReallyDamaged
        }),
        Some("OwnedMineReaction541")
    );
    (world, id, index)
}
fn ammo(world: &GameLogic, id: ObjectId, slot: u32, role: Role) -> u32 {
    world
        .host_object(id)
        .unwrap()
        .temporary_weapon_runtime
        .damaged
        .iter()
        .find_map(|runtime| {
            runtime.weapon(TemporaryWeaponRuntimeKey {
                module_source_index: slot,
                role,
            })
        })
        .unwrap()
        .ammo_in_clip
}
#[test]
fn authored_damage_order_observes_regen_clamp_before_later_reaction() {
    for mine_first in [true, false] {
        let (mut world, id, slot) = fixture(mine_first, false);
        let (peer, peer_id, peer_slot) = fixture(mine_first, false);
        assert_eq!(id, peer_id);
        assert_eq!(ammo(&world, id, slot, Role::ReactionReallyDamaged), 2);
        let result = world
            .apply_owned_kill(
                id,
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            )
            .unwrap();
        assert!(!result.destroyed());
        let object = world.host_object(id).unwrap();
        assert_eq!(
            object.health.current,
            crate::game_logic::host_mines::MINE_MIN_HEALTH
        );
        assert_eq!(
            object.mine_data.as_ref().unwrap().virtual_mines_remaining,
            0
        );
        assert!(object.status.masked);
        assert!(!object.status.on_die_started);
        assert_eq!(
            ammo(&world, id, slot, Role::ReactionReallyDamaged),
            if mine_first { 1 } else { 2 }
        );
        assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.);
        assert_eq!(
            ammo(&peer, peer_id, peer_slot, Role::ReactionReallyDamaged),
            2
        );
        assert_eq!(peer.mine_residual_proximity_detonations, 0);
        assert_eq!(world.mine_residual_proximity_detonations, 8);
    }
}
#[test]
fn later_rubble_reaction_runs_before_body_on_die_even_when_direct_delete_pending() {
    let (mut world, id, slot) = fixture(true, true);
    {
        let object = world.host_object_mut(id).unwrap();
        object.mine_data.as_mut().unwrap().regenerates = false;
        object.internal_change_health(-100.);
    }
    assert_eq!(
        world.run_authored_mine_and_weapon_damage_callbacks(
            id,
            100.,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            false,
            &mut Default::default(),
        ),
        Some(true)
    );
    let object = world.host_object(id).unwrap();
    assert_eq!(object.health.current, 0.);
    assert!(
        object.status.destroyed,
        "original detonateOnce requests direct deletion"
    );
    assert!(
        !object.status.on_die_started,
        "callback kernel cannot run Body's later onDie"
    );
    assert_eq!(ammo(&world, id, slot, Role::ReactionRubble), 1);
    assert!(world.has_pending_object_removals());
}
#[test]
fn canonical_healing_restores_virtual_count_immediately_without_firing_reaction() {
    let (mut world, id, slot) = fixture(true, false);
    {
        let object = world.host_object_mut(id).unwrap();
        object.internal_change_health(-50.);
        object.mine_data.as_mut().unwrap().virtual_mines_remaining = 4;
    }
    world.apply_owned_healing(id, 12.5, Some(id)).unwrap();
    assert_eq!(world.host_object(id).unwrap().health.current, 62.5);
    assert_eq!(
        world
            .host_object(id)
            .unwrap()
            .mine_data
            .as_ref()
            .unwrap()
            .virtual_mines_remaining,
        5
    );
    assert_eq!(ammo(&world, id, slot, Role::ReactionReallyDamaged), 2);
    assert_eq!(world.mine_residual_proximity_detonations, 0);
    assert!(!world.host_object(id).unwrap().status.on_die_started);
}
