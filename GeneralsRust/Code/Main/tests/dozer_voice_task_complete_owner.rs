//! A completed Dozer task resolves VoiceTaskComplete from its receiving Object.
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};
use glam::Vec3;

fn world_with_task_voice(sound: &str) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    world.add_player(Player::new(0, Team::USA, "Local", true));
    world
        .admit_object_ini_sources([(
            "TaskVoice.ini".into(),
            format!(
                "Object SharedTaskDozer\n KindOf = SELECTABLE VEHICLE DOZER\n Locomotor = SET_NORMAL AmericaVehicleDozerLocomotor\n VoiceTaskComplete = {sound}\n Body = ActiveBody Body\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = DozerAIUpdate AI\n RepairHealthPercentPerSecond = 2\n End\nEnd\n\
                 Object SharedTaskSite\n KindOf = STRUCTURE SELECTABLE IMMOBILE\n BuildCost = 0\n BuildTime = 10\n Body = ActiveBody Body\n MaxHealth = 1000\n InitialHealth = 1000\n End\nEnd\n"
            ),
        )])
        .unwrap();
    let dozer = world
        .create_object_for_player("SharedTaskDozer", 0, Vec3::ZERO)
        .expect("create receiving Dozer");
    let site = world
        .create_object_for_player("SharedTaskSite", 0, Vec3::ZERO)
        .expect("create receiving construction site");
    {
        let site = world.host_object_mut(site).unwrap();
        site.set_status_under_construction(true);
        site.construction_percent = 0.999;
        site.builder_id = Some(dozer);
    }
    // C++ task selection requires a positive order frame.
    world.update_with_dt(1.0 / 30.0);
    assert!(world.dozer_new_task_build(dozer, site).unwrap());
    (world, site)
}

fn complete_task_until_voice(world: &mut GameLogic) -> Vec<String> {
    for _ in 0..1200 {
        world.update_with_dt(1.0 / 30.0);
        let requests = world.queued_audio_events.drain_requests();
        let voices = requests
            .into_iter()
            .filter(|request| request.event_type.contains("TaskVoice"))
            .map(|request| request.event_type)
            .collect::<Vec<_>>();
        if !voices.is_empty() {
            return voices;
        }
    }
    Vec::new()
}

#[test]
fn same_id_worlds_emit_their_own_live_dozer_task_complete_voice() {
    let (mut first, first_site) = world_with_task_voice("FirstTaskVoice");
    let (mut peer, peer_site) = world_with_task_voice("PeerTaskVoice");
    assert_eq!(first_site, peer_site);

    let first_voices = complete_task_until_voice(&mut first);
    let peer_voices = complete_task_until_voice(&mut peer);
    assert_eq!(first_voices, ["FirstTaskVoice"]);
    assert_eq!(peer_voices, ["PeerTaskVoice"]);
}
