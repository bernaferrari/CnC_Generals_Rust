//! Behavior suite extracted from `pilots_and_movement`.
use super::*;

#[test]
fn internet_center_subdual_idles_occupant_hackers_and_resumes() {
    use crate::game_logic::host_hacker_income::{
        HACKER_CASH_INTERVAL_FAST_FRAMES, HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_REGULAR,
    };
    use crate::game_logic::{
        ContainAdmission, ContainModuleKind, ContainModuleMetadata, HackInternetAIUpdateMetadata,
        KindOf, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);

    let mut hacker_t = ThingTemplate::new("TestHackerSubdual");
    hacker_t
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::MoneyHacker)
        .set_health(100.0);
    hacker_t.transport_slot_count = Some(1);
    hacker_t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
        unpack_time_frames: 0,
        pack_time_frames: 0,
        cash_update_delay_frames: HACKER_CASH_INTERVAL_FRAMES,
        cash_update_delay_fast_frames: HACKER_CASH_INTERVAL_FAST_FRAMES,
        regular_cash_amount: HACKER_CASH_REGULAR,
        veteran_cash_amount: 6,
        elite_cash_amount: 8,
        heroic_cash_amount: 10,
        xp_per_cash_update: 1.0,
        pack_unpack_variation_factor: 0.0,
    });
    logic.templates.insert("TestHackerSubdual".into(), hacker_t);

    let mut ic_t = ThingTemplate::new("TestInternetCenterSubdual");
    ic_t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSInternetCenter)
        .set_health(2000.0);
    ic_t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::InternetHack,
        slots: Some(8),
        admission: ContainAdmission::MoneyHackerOnly,
        ..Default::default()
    };
    logic
        .templates
        .insert("TestInternetCenterSubdual".into(), ic_t);

    let ic = logic
        .create_object("TestInternetCenterSubdual", Team::China, Vec3::ZERO)
        .expect("ic");
    if let Some(obj) = logic.host_object_mut(ic) {
        obj.set_status_under_construction(false);
    }
    let hacker = logic
        .create_object("TestHackerSubdual", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("hacker");
    assert!(logic.host_object_mut(ic).expect("ic").add_occupant(hacker));
    if let Some(obj) = logic.host_object_mut(hacker) {
        obj.set_contained_by(Some(ic));
        obj.set_ai_state(AIState::Docked);
    }

    logic.frame = 0;
    logic.update_hacker_income();
    assert!(
        logic.hacker_income().is_hacking(hacker),
        "contained hacker must auto-start"
    );

    if let Some(obj) = logic.host_object_mut(ic) {
        obj.set_disabled_subdued(true);
    }
    logic.update_hacker_income();
    assert!(
        !logic.hacker_income().is_hacking(hacker),
        "subdued IC must stop occupant hacking"
    );
    assert_eq!(
        logic.host_object(hacker).expect("rider").ai_state,
        AIState::Idle,
        "occupants must go Idle"
    );

    let cash_at_stun = logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    logic.frame = HACKER_CASH_INTERVAL_FAST_FRAMES + 2;
    logic.update_hacker_income();
    assert_eq!(
        logic
            .get_player_mut_by_team(Team::China)
            .map(|p| p.resources.supplies)
            .unwrap_or(u32::MAX),
        cash_at_stun,
        "subdued IC must not deposit"
    );

    if let Some(obj) = logic.host_object_mut(ic) {
        obj.set_disabled_subdued(false);
    }
    logic.update_hacker_income();
    assert!(
        logic.hacker_income().is_hacking(hacker),
        "IC un-subdual must resume HackInternet"
    );
}

#[test]
fn hacker_field_residual_deposits_cash_on_interval() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::host_hacker_income::{
        HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_REGULAR, HACKER_UNIT_CASH_PING_AUDIO,
    };
    use crate::game_logic::{HackInternetAIUpdateMetadata, KindOf, ThingTemplate};

    clear_test_template_voices();
    set_test_per_unit_sound(
        "TestHacker",
        HACKER_UNIT_CASH_PING_AUDIO,
        "ChinaHackerCashPing",
    );

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);

    if !game_logic.templates.contains_key("TestHacker") {
        let mut t = ThingTemplate::new("TestHacker");
        t.add_kind_of(KindOf::Infantry).set_health(100.0);
        t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
            unpack_time_frames: 219,
            pack_time_frames: 154,
            cash_update_delay_frames: HACKER_CASH_INTERVAL_FRAMES,
            cash_update_delay_fast_frames: 54,
            regular_cash_amount: HACKER_CASH_REGULAR,
            veteran_cash_amount: 6,
            elite_cash_amount: 8,
            heroic_cash_amount: 10,
            xp_per_cash_update: 1.0,
            pack_unpack_variation_factor: 0.5,
        });
        game_logic.templates.insert("TestHacker".to_string(), t);
    }

    let hacker_id = game_logic
        .create_object("TestHacker", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("hacker");

    let cash_before = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    // Field residual: not auto-hacking until command.
    game_logic.frame = 0;
    game_logic.update_hacker_income();
    assert!(!game_logic.hacker_income().is_hacking(hacker_id));

    assert!(
        game_logic.start_hacker_internet_hack(hacker_id),
        "field start_hacking must succeed for living hacker"
    );
    game_logic.update_hacker_income();
    let mid = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(mid, cash_before, "no deposit before field interval");

    let first = 219 + HACKER_CASH_INTERVAL_FRAMES + 1;
    game_logic.frame = first - 1;
    game_logic.update_hacker_income();
    assert_eq!(
        game_logic
            .get_player_mut_by_team(Team::China)
            .map(|p| p.resources.supplies)
            .unwrap_or(0),
        cash_before,
        "C++ UNPACKING then field delay before first cash"
    );
    game_logic.queued_audio_events.clear();
    game_logic.frame = first;
    game_logic.update_hacker_income();
    let cash_after = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        cash_after,
        cash_before.saturating_add(HACKER_CASH_REGULAR),
        "field hacker must deposit residual ${HACKER_CASH_REGULAR}"
    );
    assert_eq!(
        game_logic
            .get_player_mut_by_team(Team::China)
            .map(|p| p.statistics.money_earned)
            .unwrap_or(0),
        HACKER_CASH_REGULAR
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "ChinaHackerCashPing" && e.object_id == Some(hacker_id) }),
        "UnitCashPing must play the authored per-unit event: {:?}",
        game_logic.queued_audio_events
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != HACKER_UNIT_CASH_PING_AUDIO),
        "must not queue the UnitCashPing slot token: {:?}",
        game_logic.queued_audio_events
    );
    assert!(game_logic.honesty_hacker_income_ok());
    // Field path is not internet-center classified.
    assert!(!game_logic.honesty_hacker_internet_center_ok());
    clear_test_template_voices();
}

#[test]
fn hacker_move_command_stops_hacking() {
    use crate::game_logic::host_hacker_income::{HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_REGULAR};
    use crate::game_logic::{HackInternetAIUpdateMetadata, KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    let mut t = ThingTemplate::new("TestHackerMoveStop");
    t.add_kind_of(KindOf::Infantry).set_health(100.0);
    t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
        unpack_time_frames: 0,
        pack_time_frames: 0,
        cash_update_delay_frames: HACKER_CASH_INTERVAL_FRAMES,
        cash_update_delay_fast_frames: 54,
        regular_cash_amount: HACKER_CASH_REGULAR,
        veteran_cash_amount: 6,
        elite_cash_amount: 8,
        heroic_cash_amount: 10,
        xp_per_cash_update: 1.0,
        pack_unpack_variation_factor: 0.0,
    });
    game_logic
        .templates
        .insert("TestHackerMoveStop".to_string(), t);

    let hacker_id = game_logic
        .create_object("TestHackerMoveStop", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("hacker");
    assert!(game_logic.start_hacker_internet_hack(hacker_id));
    assert!(game_logic.hacker_income().is_hacking(hacker_id));
    assert!(game_logic.unit_command_move_to(hacker_id, Vec3::new(40.0, 0.0, 0.0)));
    assert!(
        !game_logic.hacker_income().is_hacking(hacker_id),
        "move must PACKING-stop hack cash"
    );
    game_logic.frame = HACKER_CASH_INTERVAL_FRAMES + 1;
    game_logic.update_hacker_income();
    assert_eq!(
        game_logic
            .get_player_mut_by_team(Team::China)
            .map(|p| p.resources.supplies)
            .unwrap_or(0),
        0,
        "no cash after packing"
    );
}

#[test]
fn hacker_field_unpack_stamps_unpacking_then_firing_a() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::host_enum_table_residual::{firing_a_model_bit, unpacking_model_bit};
    use crate::game_logic::host_hacker_income::{
        HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_REGULAR, HACKER_UNIT_UNPACK_AUDIO,
        HackerInternetPhase,
    };
    use crate::game_logic::{HackInternetAIUpdateMetadata, KindOf, ThingTemplate};

    clear_test_template_voices();
    set_test_per_unit_sound(
        "TestHackerUnpackPose",
        HACKER_UNIT_UNPACK_AUDIO,
        "ChinaHackerVoiceUnpack",
    );
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    let mut t = ThingTemplate::new("TestHackerUnpackPose");
    t.add_kind_of(KindOf::Infantry).set_health(100.0);
    t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
        unpack_time_frames: 2,
        pack_time_frames: 2,
        cash_update_delay_frames: HACKER_CASH_INTERVAL_FRAMES,
        cash_update_delay_fast_frames: 54,
        regular_cash_amount: HACKER_CASH_REGULAR,
        veteran_cash_amount: 6,
        elite_cash_amount: 8,
        heroic_cash_amount: 10,
        xp_per_cash_update: 1.0,
        pack_unpack_variation_factor: 0.0,
    });
    game_logic
        .templates
        .insert("TestHackerUnpackPose".to_string(), t);
    let hacker_id = game_logic
        .create_object("TestHackerUnpackPose", Team::China, Vec3::ZERO)
        .expect("hacker");
    assert!(game_logic.start_hacker_internet_hack(hacker_id));
    let bits = game_logic.objects[&hacker_id].model_condition_bits;
    assert_ne!(bits & (1u128 << unpacking_model_bit()), 0);
    assert!(
        game_logic.queued_audio_events.iter().any(|e| {
            e.event_type == "ChinaHackerVoiceUnpack" && e.object_id == Some(hacker_id)
        }),
        "UnitUnpack must play the authored per-unit event: {:?}",
        game_logic.queued_audio_events
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != HACKER_UNIT_UNPACK_AUDIO),
        "must not queue the UnitUnpack slot token: {:?}",
        game_logic.queued_audio_events
    );
    assert_eq!(
        game_logic.hacker_income().pack_phase(hacker_id),
        Some(HackerInternetPhase::Unpacking)
    );

    game_logic.frame = 2;
    game_logic.update_hacker_income();
    let bits = game_logic.objects[&hacker_id].model_condition_bits;
    assert_eq!(bits & (1u128 << unpacking_model_bit()), 0);
    assert_ne!(bits & (1u128 << firing_a_model_bit()), 0);
    assert_eq!(
        game_logic.hacker_income().pack_phase(hacker_id),
        Some(HackerInternetPhase::Hacking)
    );
    clear_test_template_voices();
}

#[test]
fn hacker_move_while_hacking_packs_before_walking() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::host_enum_table_residual::packing_model_bit;
    use crate::game_logic::host_hacker_income::{
        HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_REGULAR, HACKER_UNIT_PACK_AUDIO,
        HackerInternetPhase,
    };
    use crate::game_logic::{HackInternetAIUpdateMetadata, KindOf, ThingTemplate};

    clear_test_template_voices();
    set_test_per_unit_sound(
        "TestHackerPackHold",
        HACKER_UNIT_PACK_AUDIO,
        "ChinaHackerVoicePack",
    );
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    let mut t = ThingTemplate::new("TestHackerPackHold");
    t.add_kind_of(KindOf::Infantry).set_health(100.0);
    t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
        unpack_time_frames: 0,
        pack_time_frames: 3,
        cash_update_delay_frames: HACKER_CASH_INTERVAL_FRAMES,
        cash_update_delay_fast_frames: 54,
        regular_cash_amount: HACKER_CASH_REGULAR,
        veteran_cash_amount: 6,
        elite_cash_amount: 8,
        heroic_cash_amount: 10,
        xp_per_cash_update: 1.0,
        pack_unpack_variation_factor: 0.0,
    });
    game_logic
        .templates
        .insert("TestHackerPackHold".to_string(), t);
    let hacker_id = game_logic
        .create_object("TestHackerPackHold", Team::China, Vec3::ZERO)
        .expect("hacker");
    assert!(game_logic.start_hacker_internet_hack(hacker_id));
    game_logic.update_hacker_income();
    assert_eq!(
        game_logic.hacker_income().pack_phase(hacker_id),
        Some(HackerInternetPhase::Hacking)
    );

    game_logic.queued_audio_events.clear();
    assert!(game_logic.unit_command_move_to(hacker_id, Vec3::new(40.0, 0.0, 0.0)));
    assert!(!game_logic.hacker_income().is_hacking(hacker_id));
    assert_eq!(
        game_logic.hacker_income().pack_phase(hacker_id),
        Some(HackerInternetPhase::Packing)
    );
    assert_ne!(
        game_logic.objects[&hacker_id].model_condition_bits & (1u128 << packing_model_bit()),
        0
    );
    assert_ne!(game_logic.objects[&hacker_id].ai_state, AIState::Moving);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "ChinaHackerVoicePack" && e.object_id == Some(hacker_id) }),
        "UnitPack must play the authored per-unit event: {:?}",
        game_logic.queued_audio_events
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != HACKER_UNIT_PACK_AUDIO),
        "must not queue the UnitPack slot token: {:?}",
        game_logic.queued_audio_events
    );

    game_logic.frame = 3;
    game_logic.update_hacker_income();
    assert_eq!(game_logic.objects[&hacker_id].ai_state, AIState::Moving);
    clear_test_template_voices();
}

#[test]
fn sticky_bomb_plays_authored_created_and_ping_sounds() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::host_mines::{
        HostMineKind, STICKY_BOMB_CREATED_AUDIO, TANK_HUNTER_TNT_OBJECT, UNIT_BOMB_PING_AUDIO,
        UNIT_BOMB_PING_FRAMES,
    };
    use crate::game_logic::{KindOf, ThingTemplate};

    clear_test_template_voices();
    set_test_per_unit_sound(
        TANK_HUNTER_TNT_OBJECT,
        STICKY_BOMB_CREATED_AUDIO,
        "TNTStickyBombCreated",
    );
    set_test_per_unit_sound(
        TANK_HUNTER_TNT_OBJECT,
        UNIT_BOMB_PING_AUDIO,
        "TNTStickyBombPing",
    );
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut charge_t = ThingTemplate::new(TANK_HUNTER_TNT_OBJECT);
    charge_t.add_kind_of(KindOf::Mine).set_health(1.0);
    logic
        .templates
        .insert(TANK_HUNTER_TNT_OBJECT.to_string(), charge_t);
    let mut tgt_t = ThingTemplate::new("StickyVic");
    tgt_t.add_kind_of(KindOf::Vehicle).set_health(200.0);
    logic.templates.insert("StickyVic".into(), tgt_t);
    let target = logic
        .create_object("StickyVic", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("target");
    let charge = logic
        .place_mine_kind(
            HostMineKind::TimedDemoCharge,
            TANK_HUNTER_TNT_OBJECT,
            Team::GLA,
            Vec3::new(10.0, 0.0, 10.0),
            None,
            Some(target),
            Some(300),
        )
        .expect("charge");
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "TNTStickyBombCreated" && e.position.is_some() }),
        "StickyBombCreated must play the authored event at bomb pos: {:?}",
        logic.queued_audio_events
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != STICKY_BOMB_CREATED_AUDIO),
        "must not queue the StickyBombCreated slot token: {:?}",
        logic.queued_audio_events
    );

    logic.queued_audio_events.clear();
    let ping_at = logic
        .objects
        .get(&charge)
        .and_then(|o| o.mine_data.as_ref())
        .and_then(|md| md.next_ping_frame)
        .unwrap_or(UNIT_BOMB_PING_FRAMES);
    logic.frame = ping_at;
    logic.update_sticky_bomb_attachments();
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "TNTStickyBombPing" && e.object_id == Some(charge) }),
        "UnitBombPing must play the authored event on the charge: {:?}",
        logic.queued_audio_events
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != UNIT_BOMB_PING_AUDIO),
        "must not queue the UnitBombPing slot token: {:?}",
        logic.queued_audio_events
    );
    clear_test_template_voices();
}

#[test]
fn sticky_bomb_missing_unit_sound_stays_silent() {
    use crate::game_logic::audio_dispatch_impl::clear_test_template_voices;
    use crate::game_logic::host_mines::{
        HostMineKind, STICKY_BOMB_CREATED_AUDIO, UNIT_BOMB_PING_AUDIO,
    };
    use crate::game_logic::{KindOf, ThingTemplate};

    clear_test_template_voices();
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut charge_t = ThingTemplate::new("SilentSticky");
    charge_t.add_kind_of(KindOf::Mine).set_health(1.0);
    logic.templates.insert("SilentSticky".into(), charge_t);
    let charge = logic
        .place_mine_kind(
            HostMineKind::RemoteDemoCharge,
            "SilentSticky",
            Team::GLA,
            Vec3::ZERO,
            None,
            None,
            None,
        )
        .expect("charge");
    assert!(
        logic.queued_audio_events.iter().all(|e| {
            e.event_type != STICKY_BOMB_CREATED_AUDIO
                && e.event_type != "SilentStickyStickyBombCreated"
        }),
        "missing StickyBombCreated must stay silent: {:?}",
        logic.queued_audio_events
    );
    logic.queued_audio_events.clear();
    logic.frame = 30;
    logic.update_sticky_bomb_attachments();
    assert!(
        logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != UNIT_BOMB_PING_AUDIO),
        "missing UnitBombPing must stay silent: {:?}",
        logic.queued_audio_events
    );
    let _ = charge;
}

#[test]
fn hacker_residual_rejects_non_hacker() {
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    if !game_logic.templates.contains_key("TestRedGuard") {
        let mut t = ThingTemplate::new("TestRedGuard");
        t.add_kind_of(KindOf::Infantry).set_health(100.0);
        game_logic.templates.insert("TestRedGuard".to_string(), t);
    }
    let id = game_logic
        .create_object("TestRedGuard", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("red guard");
    assert!(!game_logic.start_hacker_internet_hack(id));
    assert!(!game_logic.hacker_income().is_hacking(id));
}

#[test]
fn steal_cash_hack_command_transfers_cash_after_reach() {
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    // Seed victim cash so steal is observable on both sides.
    {
        let victim = game_logic
            .get_player_mut_by_team(Team::GLA)
            .expect("GLA player");
        victim.resources.supplies = 5_000;
    }
    let attacker_cash_before = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);

    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("lotus should be created");
    // TestBuilding is residual cash-generator template name.
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert_eq!(
        game_logic.hero_abilities().cash_steals,
        0,
        "cash hack must not resolve at issue range"
    );
    {
        let lotus = game_logic
            .host_object(lotus_id)
            .expect("lotus should exist");
        assert_eq!(
            lotus.ai_state,
            AIState::SpecialAbility,
            "steal cash must enter SpecialAbility on issue"
        );
    }

    // StartAbilityRange 150 residual: mid-range should still complete.
    {
        let lotus = game_logic
            .host_object_mut(lotus_id)
            .expect("lotus should exist");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    // C++ NeedToFace then Unpack 6730ms + Prep 6000ms before trigger.
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 7.0);
    game_logic.update_ai(&[lotus_id, target_id], 6.0);

    assert!(
        game_logic.honesty_steal_cash_ok(),
        "steal cash residual honesty"
    );
    assert_eq!(
        game_logic.hero_abilities().cash_stolen_total,
        crate::game_logic::host_hero_abilities::STEAL_CASH_DEFAULT_AMOUNT
    );
    let attacker_cash_after = game_logic
        .players
        .values()
        .find(|p| p.team == Team::China)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);
    assert!(
        attacker_cash_after > attacker_cash_before,
        "attacker must gain cash (before={attacker_cash_before}, after={attacker_cash_after})"
    );
    let victim_cash = game_logic
        .players
        .values()
        .find(|p| p.team == Team::GLA)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);
    assert!(
        victim_cash < 5_000,
        "victim must lose cash (remaining={victim_cash})"
    );
}

#[test]
fn steal_cash_hack_awards_lotus_award_xp_for_triggering() {
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    {
        let victim = game_logic
            .get_player_mut_by_team(Team::GLA)
            .expect("GLA player");
        victim.resources.supplies = 5_000;
    }

    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.thing.template.is_trainable = true;
    }
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    // C++ leftover SpecialAbilityUpdate: NeedToFace, Unpack 6730ms, Prep 6000ms.
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 7.0);
    game_logic.update_ai(&[lotus_id, target_id], 6.0);

    let lotus = game_logic.host_object(lotus_id).expect("lotus after steal");
    assert_eq!(
        lotus.experience.current,
        crate::game_logic::host_hero_abilities::LOTUS_STEAL_AWARD_XP as f32,
        "StealCashHack must grant AwardXPForTriggering=20"
    );
    assert!(
        game_logic.hero_abilities().cash_stolen_total > 0 || game_logic.honesty_steal_cash_ok(),
        "steal trigger that awards XP must also steal cash"
    );
}

#[test]
fn steal_cash_hack_rejects_non_lotus_and_non_cash_targets() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    let tank_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .expect("tank");
    let supply_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
        .expect("barracks");

    // Non-lotus unit cannot steal.
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack {
            target_id: supply_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let tank = game_logic.host_object(tank_id).expect("tank");
        assert_ne!(
            tank.ai_state,
            AIState::SpecialAbility,
            "non-lotus must not enter StealCashHack"
        );
    }

    // Lotus cannot steal from non-cash-generator barracks.
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("lotus");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack {
            target_id: barracks_id,
        },
        player_id: 1,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object(lotus_id).expect("lotus");
        assert_ne!(
            lotus.ai_state,
            AIState::SpecialAbility,
            "steal cash rejects non-cash-generator structure"
        );
    }
    assert!(!game_logic.honesty_steal_cash_ok());
}

#[test]
fn steal_cash_hack_uses_controlling_players_not_faction_slot() {
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let mut china_bystander = Player::new(11, Team::China, "ChinaBystander", true);
    china_bystander.resources.supplies = 9_000;
    game_logic.add_player(china_bystander);
    let mut gla_victim = Player::new(12, Team::GLA, "GlaVictim", true);
    gla_victim.resources.supplies = 5_000;
    game_logic.add_player(gla_victim);
    if let Some(slot) = game_logic.get_player_mut(2) {
        slot.resources.supplies = 1;
    }

    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    if let Some(lotus) = game_logic.host_object_mut(lotus_id) {
        lotus.owner_player_id = Some(1);
    }
    if let Some(target) = game_logic.host_object_mut(target_id) {
        target.owner_player_id = Some(12);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 7.0);
    game_logic.update_ai(&[lotus_id, target_id], 6.0);

    let china_caster = game_logic
        .get_player(1)
        .expect("china caster")
        .resources
        .supplies;
    let china_other = game_logic
        .get_player(11)
        .expect("china other")
        .resources
        .supplies;
    let gla_slot = game_logic
        .get_player(2)
        .expect("gla slot")
        .resources
        .supplies;
    let gla_owner = game_logic
        .get_player(12)
        .expect("gla owner")
        .resources
        .supplies;
    assert_eq!(
        china_other, 9_000,
        "same-faction bystander must not bank Lotus steal"
    );
    assert_eq!(gla_slot, 1, "first GLA slot must not be the victim");
    assert_eq!(gla_owner, 4_000, "building owner must lose 1000");
    assert!(
        china_caster >= 1_000,
        "Lotus controller must receive the steal (have {china_caster})"
    );
}

#[test]
fn leftover_lotus_prep_aborts_when_target_stealthed() {
    use crate::game_logic::host_hero_abilities::LeftoverSaPhase;
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    {
        let victim = game_logic.get_player_mut_by_team(Team::GLA).expect("gla");
        victim.resources.supplies = 5_000;
    }
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 7.0);
    let phase = game_logic
        .hero_abilities()
        .leftover_channel(lotus_id)
        .map(|ch| ch.phase);
    assert_eq!(phase, Some(LeftoverSaPhase::Preparing), "must be mid-prep");
    if let Some(target) = game_logic.host_object_mut(target_id) {
        target.set_status_stealthed(true);
        target.set_status_detected(false);
    }
    game_logic.update_ai(&[lotus_id, target_id], 0.2);
    assert_eq!(
        game_logic.hero_abilities().cash_stolen_total,
        0,
        "stealthed target must abort Lotus steal"
    );
    let phase = game_logic
        .hero_abilities()
        .leftover_channel(lotus_id)
        .map(|ch| ch.phase);
    assert!(
        phase != Some(LeftoverSaPhase::Preparing),
        "prep must pack or end after stealth abort, got {phase:?}"
    );
}

#[test]
fn leftover_lotus_unpack_sets_model_and_unpack_sound() {
    use crate::game_logic::host_enum_table_residual::unpacking_model_bit;
    use crate::game_logic::host_hero_abilities::{LOTUS_UNPACK_SOUND, LeftoverSaPhase};
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.queued_audio_events.clear();
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    let lotus = game_logic.host_object(lotus_id).expect("lotus");
    assert_ne!(
        lotus.model_condition_bits & (1u128 << unpacking_model_bit()),
        0,
        "Lotus unpack must set MODELCONDITION_UNPACKING"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|event| event.event_type == LOTUS_UNPACK_SOUND),
        "Lotus unpack must queue BlackLotusUnpack"
    );
    assert_eq!(
        game_logic
            .hero_abilities()
            .leftover_channel(lotus_id)
            .map(|ch| ch.phase),
        Some(LeftoverSaPhase::Unpacking)
    );
}

#[test]
fn leftover_sa_trigger_queues_ini_trigger_sound() {
    use crate::game_logic::host_hero_abilities::LOTUS_TRIGGER_SOUND;
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    if let Some(template) = game_logic.templates.get_mut("ChinaInfantryBlackLotus") {
        template.leftover_sa_trigger_sound = Some(LOTUS_TRIGGER_SOUND.to_string());
    }
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    {
        let victim = game_logic
            .get_player_mut_by_team(Team::GLA)
            .expect("GLA player");
        victim.resources.supplies = 5_000;
    }
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 7.0);
    game_logic.queued_audio_events.clear();
    game_logic.update_ai(&[lotus_id, target_id], 6.0);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|event| event.event_type == LOTUS_TRIGGER_SOUND),
        "leftover_sa trigger must queue INI TriggerSound BlackLotusTrigger, got {:?}",
        game_logic
            .queued_audio_events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        game_logic.queued_audio_events.iter().all(|event| {
            event.event_type != "BlackLotusStealCash"
                && event.event_type != "BlackLotusDisableVehicle"
                && event.event_type != "BlackLotusCaptureBuilding"
        }),
        "leftover_sa trigger must not invent cash/disable/capture SFX names"
    );
}

#[test]
fn leftover_sa_hot_swap_aborts_in_flight_channel() {
    use crate::game_logic::host_hero_abilities::{LeftoverSaKind, LeftoverSaPhase};
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    let supply_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    let tank_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("tank");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack {
            target_id: supply_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(supply_id);
    }
    game_logic.update_ai(&[lotus_id, supply_id], 1.0);
    game_logic.update_ai(&[lotus_id, supply_id], 1.0);
    assert_eq!(
        game_logic
            .hero_abilities()
            .leftover_channel(lotus_id)
            .map(|ch| ch.kind),
        Some(LeftoverSaKind::StealCash)
    );
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DisableVehicleHack { target_id: tank_id },
        player_id: 1,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic
            .hero_abilities()
            .leftover_channel(lotus_id)
            .is_none(),
        "new leftover SA must onExit the in-flight steal channel"
    );
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(10.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(tank_id);
    }
    game_logic.update_ai(&[lotus_id, tank_id], 1.0);
    let kind = game_logic
        .hero_abilities()
        .leftover_channel(lotus_id)
        .map(|ch| ch.kind);
    assert!(
        kind != Some(LeftoverSaKind::StealCash),
        "hot-swap must not keep the steal leftover timer, got {kind:?}"
    );
    let _ = LeftoverSaPhase::Facing;
}

#[test]
fn leftover_burton_flips_180_after_unpack() {
    use crate::game_logic::ChargePlantAbilityMetadata;
    let mut game_logic = GameLogic::new();
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let mut burton = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    burton
        .charge_plant_abilities
        .push(ChargePlantAbilityMetadata {
            special_power_template: "SpecialAbilityColonelBurtonTimedCharges".into(),
            unpack_time_ms: 200,
            pack_time_ms: 0,
            pack_unpack_variation_factor: 0.0,
            flee_range_after_completion: 0.0,
            flip_object_after_unpacking: true,
            flip_object_after_packing: false,
        });
    game_logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton);
    let burton_id = game_logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(2.0, 0.0, 0.0),
        )
        .expect("burton");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bldg");
    let facing_before = game_logic
        .host_object(burton_id)
        .expect("burton")
        .get_orientation();
    game_logic.queue_pending_special_ability(
        burton_id,
        crate::game_logic::PendingSpecialAbility::PlantTimedDemoCharge { target_id },
    );
    if let Some(burton) = game_logic.host_object_mut(burton_id) {
        burton.set_ai_state(AIState::SpecialAbility);
        burton.target = Some(target_id);
    }
    game_logic.update_ai(&[burton_id, target_id], 1.0 / 30.0);
    game_logic.update_ai(&[burton_id, target_id], 0.3);
    let facing_after = game_logic
        .host_object(burton_id)
        .expect("burton")
        .get_orientation();
    let delta = (facing_after - facing_before).abs();
    let flipped = (delta - std::f32::consts::PI).abs() < 0.05
        || (delta - std::f32::consts::TAU).abs() < 0.05
        || (delta - 0.0).abs() > 2.5;
    assert!(
        flipped,
        "Burton must rotate ~PI after unpack (before={facing_before} after={facing_after})"
    );
}

#[test]
fn plant_start_range_requires_approach_los() {
    let mut blocked = GameLogic::new();
    ensure_test_structure_template(&mut blocked);
    ensure_test_player_for_team(&mut blocked, Team::USA);
    ensure_test_player_for_team(&mut blocked, Team::GLA);
    install_test_mid_ridge(&mut blocked);
    let mut burton = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    blocked
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton);
    let burton_id = blocked
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(-80.0, 0.0, 0.0),
        )
        .expect("burton");
    let target_id = blocked
        .create_object("TestBuilding", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("bldg");
    if let Some(unit) = blocked.host_object_mut(burton_id) {
        unit.set_selection_radius(80.0);
        unit.target = Some(target_id);
        unit.set_ai_state(AIState::SpecialAbility);
    }
    if let Some(target) = blocked.host_object_mut(target_id) {
        target.set_selection_radius(80.0);
    }
    blocked.queue_pending_special_ability(
        burton_id,
        crate::game_logic::PendingSpecialAbility::PlantTimedDemoCharge { target_id },
    );
    blocked.update_ai(&[burton_id, target_id], 1.0 / 30.0);
    assert!(
        blocked
            .hero_abilities()
            .leftover_channel(burton_id)
            .is_none(),
        "blocked terrain LOS must not start Burton plant unpack"
    );

    let mut clear = GameLogic::new();
    ensure_test_structure_template(&mut clear);
    ensure_test_player_for_team(&mut clear, Team::USA);
    ensure_test_player_for_team(&mut clear, Team::GLA);
    let mut burton = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    clear
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton);
    let burton_id = clear
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(-80.0, 0.0, 0.0),
        )
        .expect("burton");
    let target_id = clear
        .create_object("TestBuilding", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("bldg");
    if let Some(unit) = clear.host_object_mut(burton_id) {
        unit.set_selection_radius(80.0);
        unit.target = Some(target_id);
        unit.set_ai_state(AIState::SpecialAbility);
    }
    if let Some(target) = clear.host_object_mut(target_id) {
        target.set_selection_radius(80.0);
    }
    clear.queue_pending_special_ability(
        burton_id,
        crate::game_logic::PendingSpecialAbility::PlantTimedDemoCharge { target_id },
    );
    clear.update_ai(&[burton_id, target_id], 1.0 / 30.0);
    assert!(
        clear.hero_abilities().leftover_channel(burton_id).is_some(),
        "clear LOS in start range must begin Burton plant leftover channel"
    );
}

#[test]
fn burton_and_tnt_plant_reject_bridges() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let mut bridge = ThingTemplate::new("TestBridge");
    bridge
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Bridge)
        .add_kind_of(KindOf::Selectable)
        .set_health(2000.0);
    game_logic.templates.insert("TestBridge".into(), bridge);
    let mut burton = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton);
    let mut hunter = ThingTemplate::new("ChinaInfantryTankHunter");
    hunter
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    hunter
        .special_power_modules
        .push(SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: None,
            module_kind: SpecialPowerModuleKind::SpecialAbility,
            special_power_template: "SpecialAbilityTankHunterTNTAttack".into(),
            special_power_template_id: 1,
            command_power: Some(SpecialPowerType::TankHunterTnt),
            reload_time_frames: 225,
            required_science: None,
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: true,
            starts_paused: false,
            scripted_special_power_only: false,
        });
    game_logic
        .templates
        .insert("ChinaInfantryTankHunter".into(), hunter);

    let burton_id = game_logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(2.0, 0.0, 0.0),
        )
        .expect("burton");
    let hunter_id = game_logic
        .create_object(
            "ChinaInfantryTankHunter",
            Team::China,
            Vec3::new(4.0, 0.0, 0.0),
        )
        .expect("hunter");
    let bridge_id = game_logic
        .create_object("TestBridge", Team::GLA, Vec3::ZERO)
        .expect("bridge");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::PlantTimedDemoCharge {
            target_id: bridge_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![burton_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic.pending_special_ability(burton_id).is_none(),
        "Burton command must not queue a plant on a bridge"
    );

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::TankHunterTnt,
            target: PowerTarget::Object(bridge_id),
        },
        player_id: 1,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hunter_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic.pending_special_ability(hunter_id).is_some(),
        "Tank Hunter TNT must queue a leftover-legal plant on a bridge"
    );

    game_logic.queue_pending_special_ability(
        burton_id,
        crate::game_logic::PendingSpecialAbility::PlantTimedDemoCharge {
            target_id: bridge_id,
        },
    );
    if let Some(burton) = game_logic.host_object_mut(burton_id) {
        burton.set_ai_state(AIState::SpecialAbility);
        burton.target = Some(bridge_id);
    }
    game_logic.update_ai(&[burton_id, bridge_id], 1.0 / 30.0);
    assert!(
        game_logic.pending_special_ability(burton_id).is_none(),
        "plant tick must abort a Burton charge already queued on a bridge"
    );

    game_logic.queue_pending_special_ability(
        hunter_id,
        crate::game_logic::PendingSpecialAbility::PlantTimedDemoCharge {
            target_id: bridge_id,
        },
    );
    if let Some(hunter) = game_logic.host_object_mut(hunter_id) {
        hunter.set_ai_state(AIState::SpecialAbility);
        hunter.target = Some(bridge_id);
    }
    game_logic.update_ai(&[hunter_id, bridge_id], 1.0 / 30.0);
    assert!(
        game_logic.pending_special_ability(hunter_id).is_some()
            || game_logic
                .tank_hunter_tnt_last_plant_frame(hunter_id)
                .is_some(),
        "Tank Hunter plant tick must not abort a leftover-legal bridge"
    );
}

#[test]
fn tank_hunter_tnt_does_not_consume_cooldown_at_click() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};
    let mut game_logic = GameLogic::new();
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let mut hunter = ThingTemplate::new("ChinaInfantryTankHunter");
    hunter
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    hunter
        .special_power_modules
        .push(SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: None,
            module_kind: SpecialPowerModuleKind::SpecialAbility,
            special_power_template: "SpecialAbilityTankHunterTNTAttack".into(),
            special_power_template_id: 1,
            command_power: Some(SpecialPowerType::TankHunterTnt),
            reload_time_frames: 225,
            required_science: None,
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: true,
            starts_paused: false,
            scripted_special_power_only: false,
        });
    game_logic
        .templates
        .insert("ChinaInfantryTankHunter".into(), hunter);
    let hunter_id = game_logic
        .create_object(
            "ChinaInfantryTankHunter",
            Team::China,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("hunter");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bldg");
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::TankHunterTnt,
            target: PowerTarget::Object(target_id),
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hunter_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let hunter = game_logic.host_object(hunter_id).expect("hunter");
    assert!(
        !hunter
            .special_power_cooldowns
            .contains_key(&SpecialPowerType::TankHunterTnt),
        "TNT must not start reload at click"
    );
}

#[test]
fn leftover_laser_abort_resets_primary_weapon_lock() {
    use crate::game_logic::WeaponLockType;
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let md_id = game_logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("md");
    let tgt_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("tgt");
    {
        let md = game_logic.host_object_mut(md_id).expect("md");
        md.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 200.0,
            ..crate::game_logic::Weapon::default()
        });
        md.secondary_weapon = md.weapon.clone();
        let _ = md.set_weapon_lock(1, WeaponLockType::LockedTemporarily);
    }
    assert!(game_logic.activate_missile_defender_laser_guided(md_id, tgt_id));
    if let Some(tgt) = game_logic.host_object_mut(tgt_id) {
        tgt.set_status_stealthed(true);
        tgt.set_status_detected(false);
    }
    game_logic.update_ai(&[md_id, tgt_id], 1.0 / 30.0);
    let md = game_logic.host_object(md_id).expect("md");
    assert_eq!(md.weapon_lock_slot, 0, "laser abort must lock PRIMARY");
    assert_eq!(md.weapon_lock_type, WeaponLockType::LockedTemporarily);
}

#[test]
fn disable_vehicle_hack_command_disables_after_reach() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_black_lotus_template(&mut game_logic);

    // China Lotus vs GLA vehicle residual.
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(190.0, 0.0, 0.0),
        )
        .expect("lotus should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    let initial_health = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DisableVehicleHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // Confirm command was accepted (pending special ability + SpecialAbility state).
    {
        let lotus = game_logic
            .host_object(lotus_id)
            .expect("lotus should exist");
        assert_eq!(
            lotus.ai_state,
            AIState::SpecialAbility,
            "disable hack must enter SpecialAbility on issue"
        );
        assert_eq!(lotus.target, Some(target_id));
    }
    assert!(
        !game_logic
            .host_object(target_id)
            .expect("target")
            .is_hacked_disabled(),
        "disable hack must not apply immediately on command issue"
    );
    assert!(!game_logic.honesty_disable_vehicle_hack_ok());

    // Beyond StartAbilityRange 150 — stays pending.
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    assert!(
        !game_logic
            .host_object(target_id)
            .expect("target")
            .is_hacked_disabled(),
        "disable hack stays pending out of range"
    );

    // Within StartAbilityRange 150 residual (not melee pad).
    {
        let lotus = game_logic
            .host_object_mut(lotus_id)
            .expect("lotus should exist");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    // C++ NeedToFace then Unpack 2000ms + Prep 2000ms.
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 2.1);
    game_logic.update_ai(&[lotus_id, target_id], 2.1);

    let target_after = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after.health.current, initial_health,
        "disable hack residual must not damage HP"
    );
    assert_eq!(
        target_after.team,
        Team::GLA,
        "disable hack must not change ownership"
    );
    assert!(
        target_after.is_hacked_disabled(),
        "vehicle must be DISABLED_HACKED"
    );
    assert!(!target_after.can_move(), "hacked vehicle cannot move");
    assert!(
        game_logic.honesty_disable_vehicle_hack_ok(),
        "disable vehicle residual honesty"
    );
    assert_eq!(game_logic.hero_abilities().vehicle_disables, 1);

    // Expire residual timer → vehicle recovers.
    let until = target_after.status.disabled_hacked_until_frame;
    assert!(until > game_logic.frame);
    game_logic.frame = until;
    game_logic.update_ai(&[target_id], 1.0 / 60.0);
    let recovered = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert!(
        !recovered.is_hacked_disabled(),
        "DISABLED_HACKED must clear after EffectDuration"
    );
    assert!(recovered.can_move(), "recovered vehicle can move again");
}

#[test]
fn black_lotus_capture_building_without_upgrade() {
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    // No Capture upgrade on China team — Lotus still captures via hero residual.
    assert!(!game_logic.team_has_completed_capture_upgrade(Team::China));

    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("lotus");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let lotus = game_logic.host_object(lotus_id).expect("lotus");
        assert_eq!(
            lotus.ai_state,
            AIState::Capturing,
            "Black Lotus CaptureBuilding must enter Capturing without upgrade"
        );
        assert_eq!(lotus.target, Some(building_id));
    }
    assert!(!game_logic.honesty_black_lotus_capture_ok());

    // Beyond StartAbilityRange 150 — still walking.
    game_logic.update_ai(&[lotus_id, building_id], 1.0 / 60.0);
    assert_eq!(
        game_logic.host_object(building_id).expect("building").team,
        Team::GLA,
        "capture must not complete out of StartAbilityRange"
    );

    // Within residual range 150.
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::Capturing);
        lotus.target = Some(building_id);
    }
    game_logic.update_ai(&[lotus_id, building_id], 1.0 / 60.0);

    assert_eq!(
        game_logic.host_object(building_id).expect("building").team,
        Team::China,
        "Black Lotus residual capture must transfer ownership"
    );
    assert!(
        game_logic.honesty_black_lotus_capture_ok(),
        "capture residual honesty"
    );
    assert_eq!(game_logic.hero_abilities().building_captures, 1);
    {
        let lotus = game_logic.host_object(lotus_id).expect("lotus");
        assert_ne!(
            lotus.ai_state,
            AIState::Capturing,
            "captor leaves Capturing after complete"
        );
    }
}

#[test]
fn disable_vehicle_hack_rejects_non_lotus() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let actor_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .expect("actor");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DisableVehicleHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![actor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let actor = game_logic.host_object(actor_id).expect("actor");
    assert_ne!(
        actor.ai_state,
        AIState::SpecialAbility,
        "non-lotus must not DisableVehicleHack"
    );
    assert!(!game_logic.honesty_disable_vehicle_hack_ok());
}

#[test]
fn ecm_missile_jam_scatters_in_flight_projectile() {
    use crate::game_logic::host_ecm_jam::HOST_ECM_JAM_RADIUS;

    let mut logic = GameLogic::new();
    let mut ecm_tpl = ThingTemplate::new("ChinaTankECM");
    ecm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    logic.templates.insert("ChinaTankECM".to_string(), ecm_tpl);

    let mut tom_tpl = ThingTemplate::new("AmericaVehicleTomahawk");
    tom_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0);
    logic
        .templates
        .insert("AmericaVehicleTomahawk".to_string(), tom_tpl);

    let ecm = logic
        .create_object("ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("ecm");
    let tom = logic
        .create_object(
            "AmericaVehicleTomahawk",
            Team::USA,
            Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("tom");
    let aim = Vec3::new(200.0, 0.0, 0.0);
    let from = Vec3::new(50.0, 5.0, 0.0);
    let missile = logic
        .spawn_tomahawk_missile_projectile(tom, from, aim, None)
        .expect("missile");
    if let Some(o) = logic.objects.get_mut(&missile) {
        o.set_position(Vec3::new(10.0, 5.0, 0.0));
    }
    let aim_before = logic
        .objects
        .get(&missile)
        .and_then(|o| o.tomahawk_missile_aim)
        .expect("aim");
    let hp_before = logic.objects.get(&missile).unwrap().health.current;

    logic.update_ecm_missile_jam();
    assert!(logic.honesty_ecm_missile_jam_ok());
    let o = logic.objects.get(&missile).expect("missile still exists");
    assert!(o.is_alive(), "jam must not explode the missile");
    assert!(
        (o.health.current - hp_before).abs() < 1e-3,
        "SUBDUAL_MISSILE must not deal HP, before={hp_before} after={}",
        o.health.current
    );
    assert!(o.ecm_missile_jammed, "missile should be marked jammed");
    let aim_after = o.tomahawk_missile_aim.expect("aim after");
    assert!(
        (aim_after[0] - aim_before[0]).abs() > 0.1 || (aim_after[2] - aim_before[2]).abs() > 0.1,
        "jam should scatter aim"
    );
    let _ = (ecm, HOST_ECM_JAM_RADIUS);
}

#[test]
fn ecm_jam_spawns_disable_stream_laser() {
    use crate::game_logic::host_ecm_jam::{
        ECM_DISABLE_STREAM_LASER, ECM_VEHICLE_DISABLER_DELAY_FRAMES,
    };

    let mut logic = GameLogic::new();
    let mut ecm_tpl = ThingTemplate::new("ChinaTankECM");
    ecm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    logic.templates.insert("ChinaTankECM".to_string(), ecm_tpl);

    let mut tank_tpl = ThingTemplate::new("AmericaTankCrusader");
    tank_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), tank_tpl);

    let ecm = logic
        .create_object("ChinaTankECM", Team::China, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("ecm");
    let enemy = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("enemy");
    // Ensure enemy has a weapon residual so jam candidates include it.
    {
        let o = logic.host_object_mut(enemy).unwrap();
        o.weapon = Some(Weapon {
            damage: 25.0,
            range: 150.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        // One 24 SUBDUAL_VEHICLE pulse fills this bar (ActiveBody.cpp:1292).
        o.health.current = 20.0;
        o.health.maximum = 20.0;
        o.max_health = 20.0;
    }
    // Align to laser pulse cadence.
    logic.frame = ECM_VEHICLE_DISABLER_DELAY_FRAMES;
    logic.update_ecm_jam_field();
    assert!(logic.host_object(enemy).unwrap().status.weapons_jammed);
    assert!(logic.host_object(enemy).unwrap().status.disabled_subdued);
    assert!(logic.honesty_ecm_jam_ok());
    assert!(
        logic.honesty_ecm_laser_ok(),
        "ECMDisableStream laser should spawn on jam pulse"
    );
    let beams = logic
        .objects
        .values()
        .filter(|o| o.weapon_laser_beam && o.template_name == ECM_DISABLE_STREAM_LASER)
        .count();
    assert!(beams >= 1, "ECMDisableStream beam object expected");
    assert!(
        logic
            .weapon_lasers
            .iter()
            .any(|l| l.laser_name == ECM_DISABLE_STREAM_LASER),
        "presentation residual laser expected"
    );
    let _ = ecm;
}

#[test]
fn ecm_jam_residual_jams_enemy_weapons_in_radius() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut ecm_tpl = crate::game_logic::ThingTemplate::new("ChinaTankECM");
    ecm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_cost(800, 0);
    game_logic
        .templates
        .insert("ChinaTankECM".to_string(), ecm_tpl);

    let ecm_id = game_logic
        .create_object("ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("ecm tank");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy tank");
    let ally_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(30.0, 0.0, 0.0))
        .expect("ally tank");

    // Bind weapons so residual jam target filter accepts units.
    for id in [enemy_id, ally_id, ecm_id] {
        let unit = game_logic.host_object_mut(id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 25.0,
            range: 150.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        if id == enemy_id {
            // One 24 SUBDUAL_VEHICLE pulse fills the bar (ActiveBody.cpp:1292).
            unit.health.current = 20.0;
            unit.health.maximum = 20.0;
            unit.max_health = 20.0;
        }
    }

    assert_eq!(game_logic.ecm_residual_jams(), 0);
    assert!(!game_logic.honesty_ecm_jam_ok());
    assert!(
        !game_logic
            .host_object(enemy_id)
            .expect("enemy")
            .is_weapons_jammed(),
        "enemy must not start jammed"
    );

    // One residual pulse — continuous aura, no command required.
    game_logic.update_ecm_jam_field();

    let enemy = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        enemy.is_weapons_jammed(),
        "enemy weapons must be jammed near ECM residual"
    );
    assert!(
        enemy.is_subdued_disabled(),
        "C++ DISABLED_SUBDUED after ECM vehicle subdual"
    );
    assert!(
        !enemy.can_attack(),
        "jammed enemy must fail can_attack residual"
    );
    assert!(
        !enemy.can_fire(0.0),
        "jammed enemy must fail can_fire residual"
    );
    assert!(
        !enemy.can_move(),
        "DISABLED_SUBDUED skips AI/locomotor (full halt)"
    );

    let ally = game_logic.host_object(ally_id).expect("ally");
    assert!(
        !ally.is_weapons_jammed(),
        "same-team ally must not be jammed by own ECM"
    );
    assert!(ally.can_attack(), "ally with weapons must still can_attack");

    assert!(
        game_logic.ecm_residual_jams() > 0,
        "must record ECM jam honesty ticks"
    );
    assert!(
        game_logic.honesty_ecm_jam_ok(),
        "ECM jam residual honesty path"
    );

    // Combat path must not fire while jammed.
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.target = Some(ecm_id);
        enemy.set_ai_state(AIState::Attacking);
        enemy.set_status_attacking(true);
    }
    let ecm_hp_before = game_logic.host_object(ecm_id).expect("ecm").health.current;
    game_logic.update_combat(&[enemy_id, ecm_id, ally_id], 1.0 / 30.0);
    let ecm_hp_after = game_logic.host_object(ecm_id).expect("ecm").health.current;
    assert_eq!(
        ecm_hp_before, ecm_hp_after,
        "jammed enemy must not damage ECM via combat residual"
    );

    // Leave radius — C++ SubdualDamageHelper lingers; do not instantly unjam.
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_position(Vec3::new(400.0, 0.0, 0.0));
    }
    game_logic.update_ecm_jam_field();
    assert!(
        game_logic
            .host_object(enemy_id)
            .expect("enemy")
            .is_subdued_disabled(),
        "leaving radius must linger while subdual bar is full"
    );
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.subdual_damage = 0.0;
    }
    game_logic.update_ecm_jam_field();
    let enemy = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        !enemy.is_weapons_jammed(),
        "weapons recover after subdual heals out"
    );
    assert!(
        !enemy.is_subdued_disabled(),
        "DISABLED_SUBDUED clears after subdual heals out"
    );
    assert!(enemy.can_attack(), "recovered enemy must can_attack again");

    assert!(
        game_logic
            .host_object(ecm_id)
            .map(|e| e.is_alive())
            .unwrap_or(false),
        "ECM tank must remain alive"
    );
}

#[test]
fn ecm_jam_residual_out_of_range_then_in_range() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut ecm_tpl = crate::game_logic::ThingTemplate::new("Tank_ChinaTankECM");
    ecm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(800, 0);
    game_logic
        .templates
        .insert("Tank_ChinaTankECM".to_string(), ecm_tpl);

    let _ecm_id = game_logic
        .create_object("Tank_ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("ecm");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(300.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.weapon = Some(Weapon {
            damage: 20.0,
            range: 120.0,
            last_fire_time: -5.0,
            ..Weapon::default()
        });
        enemy.health.current = 20.0;
        enemy.health.maximum = 20.0;
        enemy.max_health = 20.0;
    }

    game_logic.update_ecm_jam_field();
    assert!(
        !game_logic
            .host_object(enemy_id)
            .expect("enemy")
            .is_weapons_jammed(),
        "out-of-range enemy must not be jammed"
    );
    // First pulse with no cover does not increment honesty.
    assert_eq!(game_logic.ecm_residual_jams(), 0);
    assert!(!game_logic.honesty_ecm_jam_ok());

    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_position(Vec3::new(20.0, 0.0, 0.0));
    }
    game_logic.update_ecm_jam_field();
    assert!(
        game_logic
            .host_object(enemy_id)
            .expect("enemy")
            .is_weapons_jammed(),
        "walk-in to ECM radius must jam weapons"
    );
    assert!(game_logic.honesty_ecm_jam_ok());
}

#[test]
fn special_power_completion_die_notifies_script() {
    use crate::game_logic::script_events::{self, ScriptEvent};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let _ = script_events::drain_events(); // clear
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    let mut t = ThingTemplate::new("ScudStormMissile");
    t.set_health(100.0);
    logic.templates.insert("ScudStormMissile".into(), t);
    let id = logic
        .create_object("ScudStormMissile", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    if let Some(obj) = logic.objects.get_mut(&id) {
        obj.set_special_power_completion("SuperweaponScudStorm", 42);
    }
    logic.destroy_object(id);
    assert!(
        logic.special_power_completion_log.notifications >= 1,
        "SpecialPowerCompletionDie must notify on destroy"
    );
    let evs = script_events::drain_events();
    assert!(
        evs.iter().any(|e| matches!(
            e,
            ScriptEvent::CompletedSpecialPower {
                special_power_name,
                creator_id: 42,
                ..
            } if special_power_name == "SuperweaponScudStorm"
        )),
        "expected CompletedSpecialPower event, got {evs:?}"
    );
}

#[test]
fn ocl_weapon_spawn_sets_special_power_completion_creator() {
    // C++ ObjectCreationList.cpp:386-393 + Weapon.cpp:1103-1113 setCreator.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    let mut t = ThingTemplate::new("ScudStormMissile");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Projectile);
    logic.templates.insert("ScudStormMissile".into(), t);
    let id = logic
        .create_object("ScudStormMissile", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    if let Some(obj) = logic.objects.get_mut(&id) {
        obj.bind_special_power_completion_creator(77);
    }
    let data = logic
        .objects
        .get(&id)
        .and_then(|o| o.special_power_completion.clone())
        .expect("completion die residual");
    assert!(data.creator_set);
    assert_eq!(data.creator_id, 77);
    assert_eq!(data.special_power_name, "SuperweaponScudStorm");
}

#[test]
fn power_plant_rods_extend_upgrading_then_upgraded() {
    use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaPowerPlant");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    t.add_kind_of(KindOf::PowerPlant);
    logic.templates.insert("AmericaPowerPlant".into(), t);
    let id = logic
        .create_object("AmericaPowerPlant", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert!(logic.begin_power_plant_rods_extend(id));
    let upgrading = model_condition_bit_name_index("POWER_PLANT_UPGRADING").unwrap();
    let upgraded = model_condition_bit_name_index("POWER_PLANT_UPGRADED").unwrap();
    let o = logic.objects.get(&id).unwrap();
    assert_ne!(o.model_condition_bits & (1u128 << upgrading), 0);
    assert_eq!(o.model_condition_bits & (1u128 << upgraded), 0);
    // Advance frames past extend time.
    logic.frame = o.power_plant_rods_done_frame;
    logic.update_power_plant_rods();
    let o = logic.objects.get(&id).unwrap();
    assert_eq!(o.model_condition_bits & (1u128 << upgrading), 0);
    assert_ne!(o.model_condition_bits & (1u128 << upgraded), 0);
    assert!(logic.special_power_completion_log.rods_extend_completes >= 1);
}

#[test]
fn supply_warehouse_create_seeds_starting_boxes() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("SupplyWarehouse");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Harvestable);
    logic.templates.insert("SupplyWarehouse".into(), t);
    let id = logic
        .create_object("SupplyWarehouse", Team::Neutral, glam::Vec3::ZERO)
        .unwrap();
    let supplies = logic.objects.get(&id).unwrap().stored_resources.supplies;
    assert_eq!(supplies, 400 * 75, "StartingBoxes 400 × $75");
    assert!(logic.supply_create_warehouse_registers >= 1);
}

#[test]
fn china_mines_upgrade_places_structure_minefield() {
    use crate::game_logic::{HostGeometryInfo, HostGeometryType, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("ChinaBarracks");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    // Retail ChinaBarracks footprint (FactionUnit.ini): BOX 36 x 44.
    t.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        is_small: false,
        height: 30.0,
        major_radius: 36.0,
        minor_radius: 44.0,
        authored: true,
    };
    logic.templates.insert("ChinaBarracks".into(), t);
    let id = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(100.0, 0.0, 100.0),
        )
        .unwrap();
    let before = logic.objects.len();
    logic.apply_upgrade_to_object(id, "Upgrade_ChinaMines");
    let after = logic.objects.len();
    // C++ SmartBorder do/while (GenerateMinefieldBehavior.cpp:366-419):
    // bounding circle of the 36x44 box = sqrt(36^2+44^2) ~= 56.85, ring at
    // +30 -> ceil(2*pi*86.85/60) = 10 mines, then the diameter expand stops.
    assert!(
        after >= before + 10,
        "SmartBorder ring should place 10 mines, before={before} after={after}"
    );
    assert!(logic.structure_minefield_placements >= 10);
    let mines = logic
        .objects
        .values()
        .filter(|o| o.template_name == "ChinaStandardMine")
        .count();
    assert!(mines >= 10, "mines={mines}");
}

#[test]
fn sub_objects_upgrade_bomb_truck_bio_load() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLAVehicleBombTruck");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("GLAVehicleBombTruck".into(), t);
    let id = logic
        .create_object("GLAVehicleBombTruck", Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_GLABombTruckBioBomb");
    let o = logic.objects.get(&id).unwrap();
    assert!(
        o.sub_object_visibility.is_shown("Bombload02"),
        "bio bomb must show Bombload02"
    );
    assert!(logic.sub_objects_upgrades.honesty_ok());
}

#[test]
fn sub_objects_upgrade_helix_bomb_wing() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("ChinaVehicleHelix");
    t.set_health(500.0);
    t.add_kind_of(KindOf::Aircraft);
    logic.templates.insert("ChinaVehicleHelix".into(), t);
    let id = logic
        .create_object("ChinaVehicleHelix", Team::China, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_HelixNapalmBomb");
    assert!(
        logic
            .objects
            .get(&id)
            .unwrap()
            .sub_object_visibility
            .is_shown("BombWing")
    );
}

#[test]
fn sub_objects_upgrade_bomb_truck_both_loads() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLAVehicleBombTruck");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("GLAVehicleBombTruck".into(), t);
    let id = logic
        .create_object("GLAVehicleBombTruck", Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_GLABombTruckBioBomb");
    logic.apply_upgrade_to_object(id, "Upgrade_GLABombTruckHighExplosiveBomb");
    assert!(
        logic
            .objects
            .get(&id)
            .unwrap()
            .sub_object_visibility
            .is_shown("Bombload04")
    );
}

#[test]
fn replace_object_upgrade_fake_gla_becomes_real() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::GLA, "GLA", true));
    let mut fake = ThingTemplate::new("FakeGLABarracks");
    fake.set_health(500.0);
    fake.add_kind_of(KindOf::Structure);
    logic.templates.insert("FakeGLABarracks".into(), fake);
    let mut real = ThingTemplate::new("GLABarracks");
    real.set_health(1000.0);
    real.add_kind_of(KindOf::Structure);
    logic.templates.insert("GLABarracks".into(), real);

    let id = logic
        .create_object(
            "FakeGLABarracks",
            Team::GLA,
            glam::Vec3::new(10.0, 0.0, 20.0),
        )
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_BecomeRealGLABarracks");
    assert!(
        logic.objects.get(&id).is_none(),
        "fake must be removed from world"
    );
    assert!(
        logic
            .objects
            .values()
            .any(|o| o.template_name == "GLABarracks" && o.is_alive()),
        "real barracks must spawn"
    );
    assert!(logic.replace_grant_command_upgrades.replace_count >= 1);
}

#[test]
fn grant_science_upgrade_moab() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    let mut t = ThingTemplate::new("AmericaCommandCenter");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("AmericaCommandCenter".into(), t);
    let id = logic
        .create_object("AmericaCommandCenter", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_AmericaMOAB");
    let p = logic
        .players
        .values()
        .find(|p| p.team == Team::USA)
        .unwrap();
    assert!(
        p.has_unlocked_science("SCIENCE_MOAB"),
        "MOAB science must be granted"
    );
}

#[test]
fn command_set_upgrade_emp_mines_on_cc() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("ChinaCommandCenter");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("ChinaCommandCenter".into(), t);
    let id = logic
        .create_object("ChinaCommandCenter", Team::China, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_ChinaEMPMines");
    let cs = logic
        .objects
        .get(&id)
        .and_then(|o| o.command_set_override.clone());
    assert_eq!(cs.as_deref(), Some("ChinaCommandCenterCommandSetUpgrade"));
}

#[test]
fn weapon_set_upgrade_sets_player_upgrade_flag() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert!(!logic.objects.get(&id).unwrap().weapon_set_player_upgrade);
    logic.apply_upgrade_to_object(id, "Upgrade_AmericaRangerFlashBangGrenade");
    assert!(logic.objects.get(&id).unwrap().weapon_set_player_upgrade);
    assert!(logic.upgrade_module_residuals.weapon_set_applications > 0);
}

#[test]
fn armor_upgrade_sets_armor_set_and_chemsuit_decal() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_AmericaChemicalSuits");
    let o = logic.objects.get(&id).unwrap();
    assert!(o.armor_set_player_upgrade);
    assert!(o.terrain_decal_chemsuit);
}

#[test]
fn locomotor_set_upgrade_worker_shoes_speed() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLAInfantryWorker");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("GLAInfantryWorker".into(), t);
    let id = logic
        .create_object("GLAInfantryWorker", Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_GLAWorkerShoes");
    let o = logic.objects.get(&id).unwrap();
    assert!(o.locomotor_upgrade);
    assert!(
        o.movement.max_speed >= 30.0,
        "WorkerShoes speed residual, got {}",
        o.movement.max_speed
    );
}

#[test]
fn cost_modifier_upgrade_reduces_vehicle_cost() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    let pid = 1u32;
    let mut b = ThingTemplate::new("CostPad");
    b.set_health(500.0);
    b.add_kind_of(KindOf::Structure);
    logic.templates.insert("CostPad".into(), b);
    let mut tank_t = ThingTemplate::new("TestTank");
    tank_t.set_health(200.0);
    tank_t.add_kind_of(KindOf::Vehicle);
    tank_t.build_cost.supplies = 1000;
    logic.templates.insert("TestTank".into(), tank_t);

    let pad = logic
        .create_object("CostPad", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(
        logic.modified_build_cost_supplies(pid, "TestTank", 1000),
        1000
    );
    logic.apply_upgrade_to_object(pad, "Upgrade_CostReduction");
    assert_eq!(
        logic.modified_build_cost_supplies(pid, "TestTank", 1000),
        900
    );
    assert!(logic.upgrade_module_residuals.honesty_ok());
}

#[test]
fn weapon_bonus_upgrade_sets_player_upgrade_condition() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLATankScorpion");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("GLATankScorpion".into(), t);
    let id = logic
        .create_object("GLATankScorpion", Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    assert!(!logic.objects.get(&id).unwrap().weapon_bonus_player_upgrade);
    logic.apply_upgrade_to_object(id, "Upgrade_GLAAPRockets");
    assert!(logic.objects.get(&id).unwrap().weapon_bonus_player_upgrade);
}
