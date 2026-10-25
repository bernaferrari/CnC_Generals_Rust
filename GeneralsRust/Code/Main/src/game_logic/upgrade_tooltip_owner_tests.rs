use super::*;
use crate::game_logic::{Player, Team};
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::receiving_module_content::{
    ReceivingActionDefinitions, ReceivingModuleContent,
};
const UPGRADE: &str = "Upgrade_ReceivingTooltipOwner";
const COMMAND: &str = "Command_ReceivingTooltipOwner";
fn world(cost: u32) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "command player", false));
    world.add_player(Player::new(7, Team::USA, "actual local", true));
    register_test_upgrade(&mut world, UPGRADE, "PLAYER", cost, 1);
    let mut definitions = ReceivingActionDefinitions::default();
    let command = definitions.buttons.new_command_button(COMMAND.into());
    command.upgrade = UPGRADE.into();
    command.command = "PLAYER_UPGRADE".into();
    command.purchased_label = "OwnerPurchased".into();
    let mut content =
        ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()));
    content.admit_action_definitions(Arc::new(definitions));
    world.receiving_module_content = Some(content);
    world
}
#[test]
fn actual_native_upgrade_tooltip_observes_receiving_local_player_and_catalog_each_input_operation()
{
    let mut owner = world(321);
    let peer = world(654);
    let before = owner.receiving_upgrade_tooltip_observer();
    owner
        .get_player_mut(7)
        .unwrap()
        .add_completed_upgrade(UPGRADE);
    let after = owner.receiving_upgrade_tooltip_observer();
    let other = peer.receiving_upgrade_tooltip_observer();
    assert!(
        !before(COMMAND, UPGRADE).completed,
        "operation observation remains immutable"
    );
    let observed = after(COMMAND, UPGRADE);
    assert!(observed.completed);
    assert_eq!(observed.template.unwrap().get_cost(), 321);
    assert_eq!(observed.command.unwrap().purchased_label, "OwnerPurchased");
    let observed = other(COMMAND, UPGRADE);
    assert!(!observed.completed);
    assert_eq!(observed.template.unwrap().get_cost(), 654);
    assert!(!owner.get_player(0).unwrap().has_unlocked_upgrade(UPGRADE));
    assert!(after("MissingReceivingCommand", UPGRADE).command.is_none());
    assert!(
        after(COMMAND, "Upgrade_MissingReceivingDefinition")
            .template
            .is_none()
    );
    // Drop/reset of the peer cannot select its catalog for the owner's operation.
    drop(other);
    drop(peer);
    assert_eq!(after(COMMAND, UPGRADE).template.unwrap().get_cost(), 321);
}

#[test]
fn receiving_tooltip_observes_first_selected_object_completion_and_drone_conflict() {
    use crate::game_logic::{KindOf, Object, ObjectId, ThingTemplate};
    let mut owner = world(321);
    let mut peer = world(654);
    let battle = "Upgrade_AmericaBattleDrone";
    let scout = "Upgrade_AmericaScoutDrone";
    for world in [&mut owner, &mut peer] {
        register_test_upgrade(world, battle, "OBJECT", 100, 1);
        register_test_upgrade(world, scout, "OBJECT", 100, 1);
        let mut template = ThingTemplate::new("ReceivingTooltipSelected");
        template.add_kind_of(KindOf::Selectable).set_health(100.);
        let mut object = Object::new(template, ObjectId(42), Team::USA);
        object.owner_player_id = Some(7);
        world.add_object(object);
        world.select_objects_no_sound(7, vec![ObjectId(42)]);
    }
    owner
        .host_object_mut(ObjectId(42))
        .unwrap()
        .apply_upgrade_tag(battle);
    let receiving = owner.receiving_upgrade_tooltip_observer();
    let foreign = peer.receiving_upgrade_tooltip_observer();
    let owned = receiving(COMMAND, battle);
    assert!(!owned.completed, "object upgrade stays off the Player mask");
    assert!(owned.object_completed);
    assert_eq!(receiving(COMMAND, scout).object_affected, Some(false));
    assert!(!foreign(COMMAND, battle).object_completed);
    assert_eq!(foreign(COMMAND, scout).object_affected, Some(true));
}
