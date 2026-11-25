use super::*;
use crate::game_logic::host_upgrade_rules::register_test_upgrade;
use crate::save_load::snapshot::{SnapshotBuilder, decode_bincode_world_snapshot};

#[derive(Default)]
struct ControlBarRefreshes(usize);

impl crate::game_logic::ScriptUiEffects for ControlBarRefreshes {
    fn weather_visible(&mut self, _: bool) {}

    fn letterbox(
        &mut self,
        _: &mut crate::presentation_frame::camera::SessionCamera,
        _: bool,
        _: bool,
        _: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<'_>,
        _: bool,
    ) {
    }

    fn mark_control_bar_dirty(&mut self) {
        self.0 += 1;
    }
}

const TARGET: &str = "CommandSetFixture";
const TRIGGER: &str = "Upgrade_CommandSetTrigger";
const ALT: &str = "Upgrade_CommandSetAlternative";
const COMMAND_SET_MODULE: &str = "
 Behavior = CommandSetUpgrade Commands
  TriggeredBy = Upgrade_CommandSetTrigger
  CommandSet = RegularCommands
  CommandSetAlt = AlternativeCommands
  TriggerAlt = Upgrade_CommandSetAlternative
 End
";
const RADAR_MODULE: &str = "
 Behavior = RadarUpgrade Radar
  TriggeredBy = Upgrade_CommandSetTrigger
  RemovesUpgrades = Upgrade_CommandSetAlternative
 End
";

fn world(command_first: bool, upgrade_kind: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    for name in [TRIGGER, ALT] {
        register_test_upgrade(&mut world, name, upgrade_kind, 0, 1);
    }
    let (first, second) = if command_first {
        (COMMAND_SET_MODULE, RADAR_MODULE)
    } else {
        (RADAR_MODULE, COMMAND_SET_MODULE)
    };
    world
        .admit_object_ini_sources([(
            "ordered-commands.ini".into(),
            format!(
                "Object {TARGET}\nKindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n{first}{second}End\n"
            ),
        )])
        .unwrap();
    world
}

fn command_set(world: &GameLogic, id: ObjectId) -> Option<&str> {
    world
        .host_object(id)
        .unwrap()
        .command_set_override
        .as_deref()
}

#[test]
fn command_set_reads_live_upgrade_masks_at_its_authored_position() {
    let mut command_first = world(true, "OBJECT");
    let mut radar_first = world(false, "OBJECT");
    let id = command_first
        .create_object_for_player(TARGET, 1, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(
        radar_first.create_object_for_player(TARGET, 1, glam::Vec3::ZERO),
        Some(id)
    );

    for world in [&mut command_first, &mut radar_first] {
        world.apply_upgrade_to_object(id, ALT);
        assert!(command_set(world, id).is_none());
        let mut ui = ControlBarRefreshes::default();
        world.apply_upgrade_to_object_with_effects(
            id,
            TRIGGER,
            &mut crate::game_logic::ScriptPresentationEffects {
                ui: Some(&mut ui),
                ..Default::default()
            },
        );
        assert_eq!(ui.0, 1);
        assert!(
            !world
                .host_object(id)
                .unwrap()
                .has_object_upgrade_complete(ALT)
        );
        assert_eq!(world.get_player(1).unwrap().radar_count, 1);
    }
    assert_eq!(command_set(&command_first, id), Some("AlternativeCommands"));
    assert_eq!(command_set(&radar_first, id), Some("RegularCommands"));

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&command_first).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = world(true, "OBJECT");
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    for world in [&mut command_first, &mut restored] {
        assert_eq!(command_set(world, id), Some("AlternativeCommands"));
        world.apply_upgrade_to_object(id, TRIGGER);
        assert_eq!(
            command_set(world, id),
            Some("AlternativeCommands"),
            "the already executed module does not read TriggerAlt again"
        );
        assert!(world.remove_object_upgrade_with_receiving_rules(id, TRIGGER));
        assert_eq!(command_set(world, id), Some("AlternativeCommands"));
        world.apply_upgrade_to_object(id, TRIGGER);
        assert_eq!(command_set(world, id), Some("RegularCommands"));
    }
    assert_eq!(radar_first.get_player(1).unwrap().radar_count, 1);
}

#[test]
fn command_set_at_birth_reads_the_receiving_players_alternative() {
    let mut world = world(false, "PLAYER");
    world.add_completed_player_upgrade(1, ALT);
    world.add_completed_player_upgrade(1, TRIGGER);
    let id = world
        .create_object_for_player(TARGET, 1, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(command_set(&world, id), Some("AlternativeCommands"));
    assert!(
        !world
            .host_object(id)
            .unwrap()
            .has_object_upgrade_complete(ALT)
    );
    assert_eq!(world.get_player(1).unwrap().radar_count, 1);
}
