//! Demo SuicideBomb through authored modules (FireWeaponWhenDeadBehavior.cpp,
//! CommandSetUpgrade.cpp, Weapon.cpp WEAPON_KILLS_SELF), not unit names.
use super::*;

const SUICIDE_UPGRADE: &str = "Demo_Upgrade_SuicideBomb";

fn authored_world(objects: &str) -> GameLogic {
    let mut logic = GameLogic::new();
    logic
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "receiving-demo-suicide.ini".into(),
                objects.to_string(),
            )])
            .unwrap(),
        )
        .unwrap();
    logic.add_player(Player::new(0, Team::GLA, "Demo", true));
    logic
}

fn object(name: &str, extra: &str) -> String {
    format!(
        "Object {name}\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{extra}End\n"
    )
}

fn admit_weapons(logic: &mut GameLogic) {
    let mut weapons = std::collections::HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut weapons,
            "Weapon TerroristSuicideNotARealWeapon\nRadiusDamageAffects = SUICIDE\nDamageType = UNRESISTABLE\nDeathType = SUICIDED\nEnd\n\
             Weapon DemoOwnerBlast\nPrimaryDamage = 40\nPrimaryDamageRadius = 100\nDamageType = UNRESISTABLE\nRadiusDamageAffects = ALLIES ENEMIES NEUTRALS\nProjectileObject = NONE\nEnd",
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        ),
        2
    );
    logic.admit_weapon_content_snapshot(weapons.into_iter().collect(), &[]).unwrap();
}

fn fire_tertiary(logic: &mut GameLogic, unit: ObjectId) -> crate::command_system::CommandResult {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    crate::command_executor::CommandExecutor::new(logic, 0)
        .execute_command(GameCommand {
            command_type: CommandType::DemoTertiarySuicide,
            player_id: 0,
            command_id: 0,
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            selected_units: vec![unit],
            modifier_keys: ModifierKeys::default(),
        })
        .unwrap()
}

/// Command_DemoTertiarySuicide fires the authored TERTIARY
/// `TerroristSuicideNotARealWeapon`; WEAPON_KILLS_SELF (Weapon.cpp:1296-1306)
/// kills the shooter with the weapon's SUICIDED death type. A unit without a
/// kills-self TERTIARY cannot suicide, whatever its name.
#[test]
fn tertiary_suicide_uses_the_authored_kills_self_weapon() {
    let tertiary = "WeaponSet\nConditions = None\nWeapon = PRIMARY None\nWeapon = TERTIARY TerroristSuicideNotARealWeapon\nEnd\n";
    let mut logic = authored_world(&format!(
        "{}{}",
        object("PlainBomber", tertiary),
        object("Demo_GLAInfantryRebel", "")
    ));
    admit_weapons(&mut logic);
    let bomber = logic
        .create_object_for_player("PlainBomber", 0, Vec3::ZERO)
        .unwrap();
    let named = logic
        .create_object_for_player("Demo_GLAInfantryRebel", 0, Vec3::new(50.0, 0.0, 0.0))
        .unwrap();

    assert!(!logic.issue_demo_tertiary_suicide(named));
    assert!(logic.host_object(named).unwrap().is_alive());

    assert!(logic.issue_demo_tertiary_suicide(bomber));
    let dead = logic.host_object(bomber).unwrap();
    assert!(!dead.is_alive());
    assert_eq!(
        dead.status.death_type,
        crate::game_logic::host_usa_pilot::HostDeathType::Suicided
    );
}

#[test]
fn tertiary_command_uses_each_worlds_active_receiving_override_and_death_weapon() {
    use crate::command_system::CommandResult;
    let sets = "WeaponSet\nConditions = None\nWeapon = PRIMARY None\nEnd\n\
                WeaponSet\nConditions = PLAYER_UPGRADE\nWeapon = TERTIARY TerroristSuicideNotARealWeapon\nEnd\n";
    let death = "Behavior = FireWeaponWhenDeadBehavior BlastTag\nStartsActive = Yes\nDeathWeapon = DemoOwnerBlast\nDeathTypes = NONE +SUICIDED\nEnd\nBehavior = KeepObjectDie KeepTag\nEnd\n";
    let source = format!("{}{}", object("OwnedDemo", &format!("{sets}{death}")), object("DemoWitness", ""));
    let mut a = authored_world(&source);
    let mut b = authored_world(&source);
    for world in [&mut a, &mut b] {
        admit_weapons(world);
    }
    a.apply_receiving_weapon_overrides(&[
        "Weapon TerroristSuicideNotARealWeapon\nDeathType = SUICIDED\nEnd".into(),
    ]).unwrap();
    b.apply_receiving_weapon_overrides(&[
        "Weapon TerroristSuicideNotARealWeapon\nDeathType = NORMAL\nEnd".into(),
    ]).unwrap();
    let mut ids = Vec::new();
    for world in [&mut a, &mut b] {
        let bomber = world.create_object_for_player("OwnedDemo", 0, Vec3::ZERO).unwrap();
        let absent = world.create_object_for_player("OwnedDemo", 0, Vec3::new(500.0, 0.0, 0.0)).unwrap();
        let witness = world.create_object_for_player("DemoWitness", 0, Vec3::new(10.0, 0.0, 0.0)).unwrap();
        world.host_object_mut(bomber).unwrap().set_weapon_condition_at_frame(3, true, 0);
        ids.push((bomber, absent, witness));
    }
    let (a_bomber, a_absent, a_witness) = ids[0];
    let (b_bomber, b_absent, b_witness) = ids[1];
    assert_eq!(fire_tertiary(&mut a, a_absent), CommandResult::InvalidCommand);
    assert_eq!(fire_tertiary(&mut b, b_absent), CommandResult::InvalidCommand);
    assert!(a.host_object(a_absent).unwrap().is_alive());
    assert!(b.host_object(b_absent).unwrap().is_alive());
    assert_eq!(fire_tertiary(&mut a, a_bomber), CommandResult::Success);
    assert_eq!(a.host_object(a_bomber).unwrap().status.death_type, crate::game_logic::host_usa_pilot::HostDeathType::Suicided);
    assert!(a.host_object(a_witness).unwrap().health.current < 100.0);
    assert!(b.host_object(b_bomber).unwrap().is_alive());
    assert_eq!(b.host_object(b_witness).unwrap().health.current, 100.0);
    assert_eq!(fire_tertiary(&mut b, b_bomber), CommandResult::Success);
    assert_eq!(b.host_object(b_bomber).unwrap().status.death_type, crate::game_logic::host_usa_pilot::HostDeathType::Normal);
    assert_eq!(b.host_object(b_witness).unwrap().health.current, 100.0);
    assert!(a.host_object(a_witness).unwrap().health.current < 100.0);
}

#[test]
fn tertiary_command_requires_the_selected_receiving_suicide_flag() {
    let sets = "WeaponSet\nConditions = None\nWeapon = TERTIARY TerroristSuicideNotARealWeapon\nEnd\n";
    let mut world = authored_world(&object("OwnedDemo", sets));
    admit_weapons(&mut world);
    world.apply_receiving_weapon_overrides(&[
        "Weapon TerroristSuicideNotARealWeapon\nRadiusDamageAffects = ENEMIES\nEnd".into(),
    ]).unwrap();
    let bomber = world.create_object_for_player("OwnedDemo", 0, Vec3::ZERO).unwrap();
    assert_eq!(fire_tertiary(&mut world, bomber), crate::command_system::CommandResult::InvalidCommand);
    assert!(world.host_object(bomber).unwrap().is_alive());
}

/// Object::initObject -> updateUpgradeModules (Object.cpp:514): a unit built
/// after the Player completed the upgrade runs its authored
/// CommandSetUpgrade; a retail-named template without the module does not.
#[test]
fn late_built_unit_runs_its_authored_suicide_command_set_upgrade() {
    let upgrade = format!(
        "Behavior = CommandSetUpgrade CsTag\nCommandSet = PlainBomberCommandSetUpgrade\nTriggeredBy = {SUICIDE_UPGRADE}\nEnd\n"
    );
    let mut logic = authored_world(&format!(
        "{}{}",
        object("PlainBomber", &upgrade),
        object("Demo_GLAInfantryRebel", "")
    ));
    logic.apply_host_upgrade_complete(Team::GLA, 0, SUICIDE_UPGRADE);
    assert!(
        logic
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(SUICIDE_UPGRADE)
    );
    let late = logic
        .create_object_for_player("PlainBomber", 0, Vec3::ZERO)
        .unwrap();
    let named = logic
        .create_object_for_player("Demo_GLAInfantryRebel", 0, Vec3::X)
        .unwrap();
    assert_eq!(
        logic.host_object(late).unwrap().command_set_override.as_deref(),
        Some("PlainBomberCommandSetUpgrade")
    );
    assert_eq!(logic.host_object(named).unwrap().command_set_override, None);
}
