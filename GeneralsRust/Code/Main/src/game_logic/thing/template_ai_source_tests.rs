use super::*;
use crate::game_logic::GameLogic;

fn admitted(name: &str, behaviors: &str) -> ReceivingClientTemplate {
    let mut world = GameLogic::new();
    let source = format!("Object {name}\nKindOf = VEHICLE DOZER\n{behaviors}End\n");
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "first-ai-capability.ini".into(),
                source,
            )])
            .unwrap(),
        )
        .unwrap();
    assert!(world.ensure_host_spawn_template(name));
    world.templates[name]
        .receiving_client_template
        .clone()
        .expect("actual admitted template")
}

#[test]
fn admitted_dozer_first_keeps_full_ordinal_and_exact_tag() {
    let source = admitted(
        "DozerFirst",
        "Behavior = DestroyDie Prefix\nEnd\nBehavior = DozerAIUpdate ExactDozerTag\nEnd\nBehavior = WorkerAIUpdate LaterWorker\nEnd\n",
    );
    assert_eq!(source.receiving_ai_module_name(), Some("DozerAIUpdate"));
    assert_eq!(
        source.dozer_ai_identity(),
        Some((1, "ExactDozerTag".into()))
    );
    assert_eq!(source.dozer_ai_is_worker(), Some(false));
}

#[test]
fn admitted_worker_first_keeps_worker_class_without_name_or_kind_inference() {
    let source = admitted(
        "NotAWorkerName",
        "Behavior = DestroyDie Prefix\nEnd\nBehavior = WorkerAIUpdate ExactWorkerTag\nEnd\nBehavior = DozerAIUpdate LaterDozer\nEnd\n",
    );
    assert_eq!(source.receiving_ai_module_name(), Some("WorkerAIUpdate"));
    assert_eq!(
        source.dozer_ai_identity(),
        Some((1, "ExactWorkerTag".into()))
    );
    assert_eq!(source.dozer_ai_is_worker(), Some(true));
}

#[test]
fn admitted_base_ai_blocks_secondary_dozer_and_worker_despite_dozer_kind() {
    let source = admitted(
        "SecondaryDozer",
        "Behavior = DestroyDie Prefix\nEnd\nBehavior = AIUpdateInterface ActualFirstAI\nEnd\nBehavior = DozerAIUpdate LaterDozer\nEnd\nBehavior = WorkerAIUpdate LaterWorker\nEnd\n",
    );
    assert_eq!(source.receiving_ai_module_name(), Some("AIUpdateInterface"));
    assert_eq!(source.dozer_ai_identity(), None);
    assert_eq!(source.dozer_ai_is_worker(), None);
}

#[test]
fn admitted_without_ai_has_no_construct_capability_despite_dozer_kind() {
    let source = admitted("NoAI", "Behavior = DestroyDie Prefix\nEnd\n");
    assert_eq!(source.receiving_ai_module_name(), None);
    assert_eq!(source.dozer_ai_identity(), None);
    assert_eq!(source.dozer_ai_is_worker(), None);
}

#[test]
fn same_name_actual_factory_uses_first_owned_ai_policy_and_keeps_peer() {
    use crate::game_logic::{Player, Team};
    fn world(behaviors: &str) -> GameLogic {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "AI owner", true));
        let ini = format!(
            "Object SharedAIActor\nKindOf = VEHICLE DOZER SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{behaviors}End\n"
        );
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "owned-ai-policy.ini".into(),
                    ini,
                )])
                .unwrap(),
            )
            .unwrap();
        world
    }
    let mut owner = world(
        "Behavior = WorkerAIUpdate FirstWorker\nAutoAcquireEnemiesWhenIdle = YES\nForbidPlayerCommands = YES\nEnd\n",
    );
    let mut peer = world(
        "Behavior = AIUpdateInterface FirstBase\nAutoAcquireEnemiesWhenIdle = NO\nForbidPlayerCommands = NO\nEnd\nBehavior = WorkerAIUpdate SecondaryWorker\nAutoAcquireEnemiesWhenIdle = YES\nForbidPlayerCommands = YES\nEnd\n",
    );
    let a = owner
        .create_object_for_player("SharedAIActor", 0, glam::Vec3::ZERO)
        .unwrap();
    let b = peer
        .create_object_for_player("SharedAIActor", 0, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(a, b);
    assert!(owner.host_object(a).unwrap().worker_ai_update);
    assert!(!peer.host_object(b).unwrap().worker_ai_update);
    assert!(owner.host_object(a).unwrap().auto_acquire_when_idle());
    assert!(!peer.host_object(b).unwrap().auto_acquire_when_idle());
    assert!(owner.host_object(a).unwrap().forbid_player_commands);
    assert!(!peer.host_object(b).unwrap().forbid_player_commands);
    assert_eq!(
        owner.host_object(a).unwrap().retained_ai_root_state(),
        crate::game_logic::AIState::Idle
    );
    assert_eq!(
        peer.host_object(b).unwrap().retained_ai_root_state(),
        crate::game_logic::AIState::Idle
    );
}
