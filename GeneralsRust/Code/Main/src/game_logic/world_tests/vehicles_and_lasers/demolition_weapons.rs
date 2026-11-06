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
