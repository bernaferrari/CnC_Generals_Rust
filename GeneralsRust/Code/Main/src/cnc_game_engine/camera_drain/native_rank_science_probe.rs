//! Real Main command-map ingress against two independent native Engine worlds.
use super::super::{CnCGameEngine, GameState};
use crate::game_logic::{GameLogic, Player, Team};
use game_engine::common::rts::science::{ScienceNameFacts, ScienceStore};
use game_engine::common::ini::ini_rank::RankInfo;
use std::sync::Arc;
use winit::{event_loop::EventLoop, keyboard::{Key, KeyCode, PhysicalKey}, window::WindowAttributes};

const COMMANDS: &str = "\
CommandMap DEMO_GIVE_RANKLEVEL\n Key = KEY_U\n Transition = DOWN\n UseableIn = GAME\nEnd\n\
CommandMap DEMO_TAKE_RANKLEVEL\n Key = KEY_D\n Transition = DOWN\n UseableIn = GAME\nEnd\n\
CommandMap DEMO_GIVE_SCIENCEPURCHASEPOINTS\n Key = KEY_P\n Transition = DOWN\n UseableIn = GAME\nEnd\n\
CommandMap DEMO_GIVE_ALL_SCIENCES\n Key = KEY_G\n Transition = DOWN\n UseableIn = GAME\nEnd\n";

fn fixture(world: &mut GameLogic, suffix: &str, cap: i32, second_points: u32) -> anyhow::Result<[i32; 3]> {
    let names = [format!("SCIENCE_{suffix}_Base"), format!("SCIENCE_{suffix}_Rank"), format!("SCIENCE_{suffix}_Extra")];
    let ini = format!("Science {}\n IsGrantable = No\nEnd\nScience {}\n IsGrantable = No\nEnd\nScience {}\n IsGrantable = Yes\nEnd\n", names[0], names[1], names[2]);
    let mut store = ScienceStore::new_owned();
    store.load_from_sources(&[("fixture-science.ini", ini.as_bytes(), false)])?;
    let facts = Arc::new(ScienceNameFacts::from_store(&store));
    let ids = [facts.science(&names[0]).unwrap(), facts.science(&names[1]).unwrap(), facts.science(&names[2]).unwrap()];
    let content = crate::assets::ReceivingObjectContent::from_sources_with_sciences(Vec::<(String, String)>::new(), facts).map_err(anyhow::Error::msg)?;
    world.admit_receiving_module_content(content).map_err(anyhow::Error::msg)?;
    let mut ranks = world.world_services.common().ini.rank.write();
    ranks.init();
    for (points, skill, science) in [(2, 0, ids[0]), (second_points, 137, ids[1]), (11, 419, ids[2])] {
        let mut rank = RankInfo::new();
        rank.science_purchase_points_granted = points;
        rank.skill_points_needed = skill;
        rank.sciences_granted.push(science);
        ranks.add_rank(rank)?;
    }
    drop(ranks);
    world.clear_all_players();
    world.add_player(Player::new(0, Team::USA, suffix, true).with_script_identity(suffix));
    assert!(world.select_local_player(Some(0)));
    world.set_rank_level_limit(cap);
    Ok(ids)
}

fn press(engine: &mut CnCGameEngine, letter: char, code: KeyCode) -> anyhow::Result<()> {
    engine.handle_mapped_key_press(&Key::Character(letter.to_string().into()), Some(PhysicalKey::Code(code)), false).map_err(anyhow::Error::msg)
}

fn acquired(world: &GameLogic) -> Vec<i32> {
    world.owned_script_engine.as_ref().unwrap().snapshot_xfer_tail().acquired_sciences[0].clone()
}

fn state(world: &GameLogic) -> (u32, i32, i32) {
    let p = world.get_player(0).unwrap();
    (p.rank_level, p.skill_points, p.science_purchase_points)
}

/// Execute a real TeamFactory on-create subroutine through the native map loan.
fn on_create_rank_down(engine: &mut CnCGameEngine, base_science: i32) -> anyhow::Result<()> {
    use crate::game_logic::NativeMapAdmission;
    use gamelogic::scripting::core::{Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList};
    use game_engine::common::well_known_keys::{
        key_team_executes_actions_on_create, key_team_is_singleton, key_team_name,
        key_team_owner, key_team_production_condition,
    };
    let mut action = ScriptAction::new(ScriptActionType::PlayerSetRanklevel);
    action.add_parameter(Parameter::with_string(ParameterType::Side, "Alpha".into()))?;
    action.add_parameter(Parameter::with_int(ParameterType::Int, 1))?;
    let mut script = Script::new();
    script.script_name = "AdmissionRankDown".into();
    script.is_subroutine = true;
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let interpreter = gamelogic::scripting::engine::ScriptEngine::new_in_published_world()?;
    interpreter.set_script_list_for_player(0, Some(Box::new(list)))?;
    engine.game_logic.install_owned_script_engine(interpreter);
    let team = engine.game_logic.with_own_services_mut(|_| {
        let mut dict = game_engine::common::dict::Dict::new();
        dict.set_ascii_string(key_team_name(), "teamAlpha");
        dict.set_ascii_string(key_team_owner(), "Alpha");
        dict.set_bool(key_team_is_singleton(), true);
        dict.set_bool(key_team_executes_actions_on_create(), true);
        dict.set_ascii_string(key_team_production_condition(), "AdmissionRankDown");
        dict
    });
    let owner = engine.game_client.window_manager_owner();
    owner.with_manager(|manager| engine.control_bar.reset(manager))
        .map_err(|error| anyhow::anyhow!("control bar reset: {error}"))?;
    let mut candidate = super::super::map_native_admission::NativeMapCandidate::with_epoch(417);
    let policy = super::super::native_view_effects::receiving_native_drawable_policy_for(&engine.game_logic);
    let text = engine.game_client.ensure_game_text_store();
    let client = std::cell::RefCell::new(&mut engine.game_client);
    let bar = &mut engine.control_bar;
    let mut rank_ui = |event| {
        super::super::map_native_admission::deliver_map_rank_ui(&client, bar, event)
    };
    let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
    let mut attempt = candidate.attempt(
        engine.render_pipeline.borrow_native_draw_services(),
        &mut engine.graphics_system,
        engine.current_player_id,
        policy,
        &mut retirement,
        None,
    ).map_err(anyhow::Error::msg)?
        .with_game_text(text)
        .with_rank_ui(&mut rank_ui);
    let accepted = engine.game_logic.with_own_services_mut(|world| {
        world.sync_legacy_team_factory_from_team_dicts_with_effects(&[team], &mut attempt.effects())
    });
    assert!(accepted);
    assert_eq!(state(&engine.game_logic), (1, 0, 2));
    assert_eq!(engine.game_logic.get_player(0).unwrap().general_name, "Alpha General");
    assert_eq!(acquired(&engine.game_logic), vec![base_science, base_science]);
    drop(attempt);
    drop(rank_ui);
    assert!(engine.control_bar.is_ui_dirty());
    Ok(())
}


pub(super) fn run() -> anyhow::Result<()> {
    // Engine startup needs a real Main bootstrap owner before each match
    // installs its independently owned Main/World services.
    let _main = crate::main_services::MainServicesScope::new();
    let root = std::path::PathBuf::from(std::env::var_os("GENERALS_RANK_PROBE_PRIVATE_ROOT").ok_or_else(|| anyhow::anyhow!("private rank probe root required"))?);
    let event_loop = EventLoop::new()?;
    let args = Arc::new(crate::command_line::CommandLineArgs::parse_from_args(vec!["native-rank-probe".into(), "--runtime_host=headless".into(), "--noaudio".into(), format!("--userdata={}", root.join("userdata").display())])?);
    let mut services_a = crate::subsystem_manager::ApplicationServices::new();
    let mut services_b = crate::subsystem_manager::ApplicationServices::new();
    let window_a = Arc::new(event_loop.create_window(WindowAttributes::default().with_visible(false).with_title("Rank owner A"))?);
    let window_b = Arc::new(event_loop.create_window(WindowAttributes::default().with_visible(false).with_title("Rank owner B"))?);
    let main_a = Arc::new(crate::main_services::MainServices::new());
    let main_b = Arc::new(crate::main_services::MainServices::new());
    let mut a = crate::main_services::with_main_services(&main_a, || {
        pollster::block_on(CnCGameEngine::new(window_a, Arc::clone(&args), &mut services_a))
    })?;
    let mut b = crate::main_services::with_main_services(&main_b, || {
        pollster::block_on(CnCGameEngine::new(window_b, args, &mut services_b))
    })?;
    for engine in [&mut a, &mut b] {
        game_client::message_stream::meta_event::apply_command_map_source(
            engine.game_client.meta_map_mut(), COMMANDS,
        ).map_err(|error| anyhow::anyhow!("fixture CommandMap: {error:?}"))?;
        if engine.game_client.in_game_ui_subsystem_mut().is_none() {
            engine.game_client.set_in_game_ui_subsystem(
                game_client::core::subsystems::InGameUISubsystem::default(),
            );
        }
    }
    a.current_state = GameState::InGame;
    b.current_state = GameState::InGame;
    let ids_a = fixture(&mut a.game_logic, "Alpha", 2, 7)?;
    let ids_b = fixture(&mut b.game_logic, "Beta", 3, 13)?;
    let mut foreign = GameLogic::new();
    let ids_f = fixture(&mut foreign, "Foreign", 1, 19)?;
    for (engine, file, label, previous) in [
        (&mut a, "alpha.str", "Alpha General", "Old Alpha"),
        (&mut b, "beta.str", "Beta General", "Old Beta"),
    ] {
        let path = root.join(file);
        std::fs::write(&path, format!("SCIENCE:GeneralName\n\"{label}\"\nEND\n"))?;
        let mut text = engine.game_client.game_text_store().write().unwrap();
        // C++ resolves CSF before map.str. This fixture intentionally uses
        // independent empty base catalogs so its map-only labels are selected.
        *text = game_client::game_text::GameText::default();
        text.load_map_strings(&path)
            .map_err(|error| anyhow::anyhow!("map text: {error}"))?;
        assert_eq!(text.fetch_owned("SCIENCE:GeneralName"), label);
        drop(text);
        engine.game_logic.get_player_mut(0).unwrap().general_name = previous.into();
    }
    foreign.get_player_mut(0).unwrap().general_name = "Foreign General".into();
    assert_eq!(a.game_logic.get_player(0).unwrap().general_name, "Old Alpha");
    assert_eq!(b.game_logic.get_player(0).unwrap().general_name, "Old Beta");
    let foreign_services = Arc::clone(&foreign.world_services);
    gamelogic::system::engine_stores::with_world_services(&foreign_services, || -> anyhow::Result<()> {
        for engine in [&mut a, &mut b] {
            let owner = engine.game_client.window_manager_owner();
            owner.with_manager(|manager| engine.control_bar.reset(manager))
                .map_err(|error| anyhow::anyhow!("control bar reset: {error}"))?;
            assert!(!engine.control_bar.is_ui_dirty());
        }
        press(&mut a, 'u', KeyCode::KeyU)?;
        assert_eq!(state(&a.game_logic), (2, 137, 7));
        assert_eq!(acquired(&a.game_logic), vec![ids_a[1]]);
        assert_eq!(state(&b.game_logic), (1, 0, 0));
        assert!(!b.control_bar.is_ui_dirty());
        assert!(a.control_bar.is_ui_dirty());
        assert_eq!(a.game_logic.take_eva_requests(), vec![gamelogic::helpers::EvaEvent::GeneralLevelUp]);
        press(&mut b, 'u', KeyCode::KeyU)?;
        press(&mut a, 'u', KeyCode::KeyU)?;
        press(&mut b, 'u', KeyCode::KeyU)?;
        assert_eq!(state(&a.game_logic), (2, 137, 7));
        assert_eq!(state(&b.game_logic), (3, 419, 24));
        assert_eq!(acquired(&b.game_logic), vec![ids_b[1], ids_b[2]]);
        press(&mut a, 'p', KeyCode::KeyP)?;
        assert_eq!(state(&a.game_logic), (2, 137, 8));
        press(&mut a, 'g', KeyCode::KeyG)?;
        assert!(a.game_logic.get_player(0).unwrap().unlocked_sciences.contains("SCIENCE_Alpha_Extra"));
        assert!(!a.game_logic.get_player(0).unwrap().unlocked_sciences.contains("SCIENCE_Beta_Extra"));
        assert_eq!(acquired(&a.game_logic), vec![ids_a[1], ids_a[2]]);
        press(&mut a, 'd', KeyCode::KeyD)?;
        assert_eq!(state(&a.game_logic), (1, 0, 2));
        assert_eq!(a.game_logic.get_player(0).unwrap().general_name, "Alpha General");
        assert_eq!(b.game_logic.get_player(0).unwrap().general_name, "Old Beta");
        assert_eq!(foreign.get_player(0).unwrap().general_name, "Foreign General");
        let player = a.game_logic.get_player(0).unwrap();
        assert!(player.unlocked_sciences.contains("SCIENCE_Alpha_Base"));
        assert!(!player.unlocked_sciences.contains("SCIENCE_Alpha_Rank"));
        assert!(!player.unlocked_sciences.contains("SCIENCE_Alpha_Extra"));
        assert_eq!(acquired(&a.game_logic), vec![ids_a[1], ids_a[2], ids_a[0], ids_a[0]]);
        assert_eq!(state(&b.game_logic), (3, 419, 24));
        assert_eq!(acquired(&b.game_logic), vec![ids_b[1], ids_b[2]]);
        assert_eq!(
            b.game_logic.take_eva_requests(),
            vec![gamelogic::helpers::EvaEvent::GeneralLevelUp; 2]
        );
        press(&mut b, 'p', KeyCode::KeyP)?;
        assert_eq!(state(&b.game_logic), (3, 419, 25));
        press(&mut b, 'g', KeyCode::KeyG)?;
        assert_eq!(acquired(&b.game_logic), vec![ids_b[1], ids_b[2]]);
        press(&mut b, 'd', KeyCode::KeyD)?;
        assert_eq!(state(&b.game_logic), (2, 137, 15));
        assert_eq!(b.game_logic.get_player(0).unwrap().general_name, "Beta General");
        assert_eq!(a.game_logic.get_player(0).unwrap().general_name, "Alpha General");
        assert_eq!(foreign.get_player(0).unwrap().general_name, "Foreign General");
        assert_eq!(
            acquired(&b.game_logic),
            vec![ids_b[1], ids_b[2], ids_b[0], ids_b[0], ids_b[1]]
        );
        assert_eq!(state(&a.game_logic), (1, 0, 2));
        assert_eq!(a.game_logic.take_eva_requests(), vec![gamelogic::helpers::EvaEvent::GeneralLevelUp]);
        assert!(foreign.take_eva_requests().is_empty());
        assert_eq!(state(&foreign), (1, 0, 0));
        assert!(acquired(&foreign).is_empty());
        assert!(!foreign.get_player(0).unwrap().unlocked_sciences.contains("SCIENCE_Foreign_Extra"));
        assert_ne!(ids_f[0], -1);
        let messages = a.game_client.in_game_ui_subsystem_mut().expect("in-game UI").drain_hud_messages();
        for expected in ["Adding a RankLevel", "Adding a SciencePurchasePoint", "Granting all sciences!", "Subtracting a RankLevel"] {
            assert!(messages.iter().any(|m| m == expected), "missing HUD message {expected}: {messages:?}");
        }
        press(&mut a, 'u', KeyCode::KeyU)?;
        a.game_logic.get_player_mut(0).unwrap().general_name = "Before on-create".into();
        on_create_rank_down(&mut a, ids_a[0])?;
        assert_eq!(b.game_logic.get_player(0).unwrap().general_name, "Beta General");
        assert_eq!(foreign.get_player(0).unwrap().general_name, "Foreign General");
        a.game_logic.reset();
        assert_eq!(a.game_logic.rank_level_limit(), 1000);
        assert!(acquired(&a.game_logic).is_empty());
        assert_eq!(
            acquired(&b.game_logic),
            vec![ids_b[1], ids_b[2], ids_b[0], ids_b[0], ids_b[1]]
        );
        assert_eq!(state(&b.game_logic), (2, 137, 15));
        assert_eq!(state(&foreign), (1, 0, 0));
        Ok(())
    })?;
    Ok(())
}
