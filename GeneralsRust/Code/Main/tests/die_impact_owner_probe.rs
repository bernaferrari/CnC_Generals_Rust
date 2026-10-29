use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team, Weapon};
use glam::Vec3;

const DIE: &str = "DieImpactProjectile";
const OCL: &str = "DieImpactOcl";

fn world(child: &str) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    world
        .admit_object_creation_list_sources([format!(
            "ObjectCreationList {OCL}\nCreateObject\nObjectNames = {child}\nCount = 1\nEnd\nEnd\n"
        )])
        .unwrap();
    world
        .admit_object_ini_sources([(
            "die-impact.ini".into(),
            format!(
                "Object {DIE}\nKindOf = PROJECTILE\nBehavior = CreateObjectDie Die\nCreationList = {OCL}\nEnd\nEnd\nObject ImpactTarget\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\nObject {child}\nBody = ActiveBody Body\nMaxHealth = 10\nInitialHealth = 10\nEnd\nEnd\n"
            ),
        )])
        .unwrap();
    let target = world
        .create_object_for_player("ImpactTarget", 0, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let mut weapon = Weapon::default();
    weapon.damage = 1.0;
    weapon.range = 100.0;
    let projectile = world.combat_system_mut().fire_projectile(
        Vec3::ZERO,
        Vec3::new(10.0, 0.0, 0.0),
        &weapon,
        target,
        Some(target),
        30000.0,
    );
    let projectile = world
        .combat_system_mut()
        .projectile_mut(projectile)
        .unwrap();
    projectile.die_on_detonate = true;
    projectile.projectile_object_name = DIE.into();
    projectile.projectile_lifecycle = Some(
        generals_main::game_logic::weapon_bootstrap::HostProjectileLifecycle::DumbProjectile {
            max_lifespan_frames: 1,
        },
    );
    (world, target)
}

fn children(world: &GameLogic) -> Vec<String> {
    world
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name != "ImpactTarget")
        .map(|object| object.template_name.clone())
        .collect()
}

#[test]
fn impact_consumer_uses_receiving_projectile_definition() {
    let (mut a, a_target) = world("ChildA");
    let (mut b, b_target) = world("ChildB");
    assert_eq!(a_target, b_target);
    a.update();
    assert_eq!(children(&a), ["ChildA"]);
    assert!(children(&b).is_empty());
    b.update();
    assert_eq!(children(&b), ["ChildB"]);
    assert_eq!(children(&a), ["ChildA"]);
}
