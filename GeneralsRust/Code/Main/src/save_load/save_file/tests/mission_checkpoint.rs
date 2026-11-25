use super::*;

use game_client::core::game_client::ClientInputAuthority;
use game_client::core::GameClient;
use game_client::gui::callbacks::score_screen::ScoreScreenAcademySnapshot;
use game_client::gui::campaign_launch_host_bridge::{MenuWorldFacts, ShellMapSettings};
use game_client::system::SubsystemInterface;
use game_engine::common::ini::ini_campaign::CampaignStore;
use game_engine::common::{common_services::CommonServicesScope, ini};
use std::time::Duration;

const FIRST: &str = "Maps\\MD_USA01\\MD_USA01.map";
const NEXT: &str = "Maps\\MD_USA02\\MD_USA02.map";
const CONTINUE: &str = "ScoreScreen.wnd:ButtonContinue";
const SAVED: &str = "ScoreScreen.wnd:StaticTextGameSaveComplete";

fn load_campaigns() -> CampaignStore {
    let archive = crate::fixture_content::retail_content_root()
        .join("Command & Conquer Generals Zero Hour/Data/INIZH.big");
    let mut files = crate::assets::archive::ArchiveFileSystem::new();
    files.set_local_search_roots(Vec::new());
    futures::executor::block_on(files.load_big_file(archive)).expect("mount retail INIZH.big");
    let bytes = files
        .read_optional_definition_source("Data/INI/Campaign.ini")
        .unwrap()
        .expect("retail Campaign.ini");
    CampaignStore::from_ini_text(&String::from_utf8(bytes).unwrap()).unwrap()
}

fn mount_nonfinal_score(definitions: &CampaignStore, rank_points: i32) -> GameClient {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    client.mark_initialized_with_campaign_definitions(definitions);
    client.configure_shell_map(ShellMapSettings {
        enabled: false,
        ..ShellMapSettings::default()
    });
    let facts = MenuWorldFacts::default();
    client.with_window_event_effects(Some(facts), |_| {});
    let campaign = client.campaign_manager_mut();
    campaign.set_campaign("usa");
    campaign.set_victorious(true);
    assert_eq!(campaign.get_current_map().as_deref(), Some(FIRST));
    assert_eq!(campaign.get_next_mission().unwrap().map_name, NEXT);

    let mut outgoing = GameLogic::new();
    let mut player = Player::new(3, Team::USA, "Local", true);
    player.skill_points = rank_points;
    outgoing.add_player(player);
    assert!(outgoing.select_local_player(Some(3)));
    let roster = outgoing.score_screen_player_roster();
    assert!(roster
        .iter()
        .any(|player| player.is_local && player.rank_points == rank_points));

    client
        .with_receiving_shell(Some(facts), |shell, effects| {
            shell.init().unwrap();
            shell.push("Menus/MainMenu.wnd", false, effects).unwrap();
        })
        .expect("receiving Shell");
    client
        .with_receiving_shell(Some(facts), |shell, effects| {
            shell.hide_with_effects(true, effects);
        })
        .expect("receiving Shell");
    client
        .with_receiving_shell(Some(facts), |shell, effects| {
            assert!(shell.top().unwrap().is_hidden());
            shell
                .push_score_screen(
                    ScoreScreenAcademySnapshot {
                        end_frame: 900,
                        advice: Default::default(),
                        show_advice: false,
                        player_roster: Some(roster),
                        is_single_player: Some(true),
                        is_skirmish: Some(false),
                        session_difficulty: Some(2),
                        skirmish_outcome: None,
                    },
                    false,
                    effects,
                )
                .expect("mounted retail ScoreScreen WND");
            shell.show_shell(false, effects).unwrap();
        })
        .expect("receiving Shell");
    client.take_menu_host_requests();
    client
}

fn init_isolated_score_services(root: &Path) {
    ini::ini_game_data::init_global_data();
    ini::get_global_data()
        .unwrap()
        .write()
        .set_path_user_data(root.to_string_lossy().into_owned());
    game_engine::System::SaveGame::init_game_state(root.join("legacy"));
    let assets = PathBuf::from(std::env::var_os("GENERALS_ASSETS_DIR").expect("mount retail WNDs"));
    for wnd in ["ScoreScreen.wnd", "MainMenu.wnd", "BlankWindow.wnd"] {
        assert!(assets.join("Window/Menus").join(wnd).is_file(), "{wnd}");
    }
}

fn post_draw(client: &mut GameClient) {
    client
        .update_post_draw_ui(Some(MenuWorldFacts::default()), None)
        .unwrap();
}

fn hidden(client: &GameClient, name: &str) -> bool {
    client
        .window_manager_owner_ref()
        .with_manager_ref(|manager| {
            manager
                .find_window_by_name(name)
                .unwrap_or_else(|| panic!("retail widget {name}"))
                .borrow()
                .is_hidden()
        })
}

#[test]
#[ignore = "requires mounted Zero Hour Campaign.ini and retail WNDs"]
fn mounted_nonfinal_score_waits_on_receiving_checkpoint() {
    let _common = CommonServicesScope::new();
    let _main = crate::main_services::MainServicesScope::new();
    let root = tempfile::tempdir().unwrap();
    init_isolated_score_services(root.path());
    let campaigns = load_campaigns();
    let mut client = mount_nonfinal_score(&campaigns, 13);
    // Shell owns a 33 ms update cadence.
    std::thread::sleep(Duration::from_millis(40));
    post_draw(&mut client);
    assert_eq!(
        client.campaign_manager().get_current_map().as_deref(),
        Some(NEXT)
    );
    assert!(
        client.has_menu_host_requests(),
        "nonfinal ScoreScreen wrote through Common instead of requesting Main's save owner",
    );
    assert!(!root.path().join("legacy/00000000.sav").exists());
    assert!(hidden(&client, CONTINUE));
    assert!(hidden(&client, SAVED));
}

#[test]
#[ignore = "requires mounted Zero Hour Campaign.ini and retail WNDs"]
fn mounted_nonfinal_score_persists_next_campaign_mission_before_continue() {
    use game_client::gui::callbacks::score_screen::complete_mission_checkpoint;
    use game_client::gui::campaign_launch_host_bridge::{MenuHostRequest, MenuLaunchSource};

    let _common = CommonServicesScope::new();
    let _main = crate::main_services::MainServicesScope::new();
    let root = tempfile::tempdir().unwrap();
    init_isolated_score_services(root.path());
    let campaigns = load_campaigns();
    let facts = MenuWorldFacts::default();
    let mut source = mount_nonfinal_score(&campaigns, 13);
    let mut peer = mount_nonfinal_score(&campaigns, 29);
    std::thread::sleep(Duration::from_millis(40));
    post_draw(&mut source);
    post_draw(&mut peer);
    let [MenuHostRequest::SaveCampaignCheckpoint(checkpoint)]: [MenuHostRequest; 1] = source
        .take_menu_host_requests()
        .try_into()
        .expect("one mission save request")
    else {
        panic!("retail post-draw did not request its mission checkpoint");
    };
    assert_eq!(checkpoint.next_map, NEXT);
    assert_eq!(checkpoint.state.rank_points, 13);
    assert_eq!(checkpoint.state.difficulty, 2);
    assert!(hidden(&source, CONTINUE));
    assert!(!source.with_window_event_effects(Some(facts), |effects| {
        game_client::gui::dispatch_os_click_named_window(CONTINUE, effects)
    }));

    let manager = SaveFileManager::with_save_directory(root.path().join("receiving"));
    std::fs::create_dir_all(manager.save_directory()).unwrap();
    let occupied = manager.get_save_path("00000000");
    std::fs::write(&occupied, b"previous checkpoint").unwrap();
    let (path, result) = manager.save_campaign_checkpoint(
        &checkpoint.next_map,
        &checkpoint.description,
        &checkpoint.state,
    );
    result.unwrap();
    assert_eq!(path, manager.get_save_path("00000001"));
    assert_eq!(std::fs::read(occupied).unwrap(), b"previous checkpoint");
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(
        &bytes[..VERSIONED_SAVE_MAGIC.len()],
        VERSIONED_SAVE_MAGIC,
        "mission checkpoints use the versioned archive container"
    );
    let checkpoint_archive = decode_versioned_archive(&bytes);
    assert_eq!(
        checkpoint_archive.save_info.save_type,
        SaveFileType::Mission
    );
    assert!(checkpoint_archive.campaign_state.is_some());
    let saved = manager.load_game_snapshot("00000001").unwrap();
    assert_eq!(saved.save_info.save_type, SaveFileType::Mission);
    assert_eq!(saved.save_info.map_name, NEXT);
    assert_eq!(saved.save_info.mission_number, Some(1));
    assert_eq!(saved.save_info.difficulty, GameDifficulty::Hard);
    assert_eq!(saved.save_info.description, checkpoint.description);
    assert!(saved.snapshot.objects.is_empty());
    let state = saved
        .campaign_state
        .expect("restorable CampaignManager Xfer");
    assert_eq!(state.campaign, checkpoint.state.campaign);
    assert_eq!(state.mission, checkpoint.state.mission);
    assert_eq!(state.rank_points, 13);

    peer.with_window_event_effects(Some(facts), |effects| {
        complete_mission_checkpoint(&checkpoint.score_parent, true, effects)
    });
    assert!(hidden(&peer, CONTINUE));
    assert_eq!(
        peer.campaign_manager()
            .capture_logic_chunk_state()
            .rank_points,
        29
    );
    let [MenuHostRequest::SaveCampaignCheckpoint(peer_checkpoint)]: [MenuHostRequest; 1] = peer
        .take_menu_host_requests()
        .try_into()
        .expect("one peer mission save request")
    else {
        panic!("peer post-draw did not request its mission checkpoint");
    };
    let unavailable = root.path().join("unavailable");
    std::fs::write(&unavailable, b"unrelated receiving file").unwrap();
    let rejected = SaveFileManager::with_save_directory(&unavailable);
    let (attempt, result) = rejected.save_campaign_checkpoint(
        &peer_checkpoint.next_map,
        &peer_checkpoint.description,
        &peer_checkpoint.state,
    );
    assert_eq!(attempt, unavailable);
    assert!(result.is_err());
    assert_eq!(
        std::fs::read(unavailable).unwrap(),
        b"unrelated receiving file"
    );
    peer.with_window_event_effects(Some(facts), |effects| {
        complete_mission_checkpoint(&peer_checkpoint.score_parent, result.is_ok(), effects)
    });
    assert!(!hidden(&peer, CONTINUE));
    assert!(hidden(&peer, SAVED));
    assert!(hidden(&source, CONTINUE));
    source.with_window_event_effects(Some(facts), |effects| {
        complete_mission_checkpoint(&checkpoint.score_parent, true, effects)
    });
    assert!(!hidden(&source, CONTINUE));
    assert!(!hidden(&source, SAVED));
    std::thread::sleep(Duration::from_millis(40));
    post_draw(&mut source);
    assert!(
        !source.has_menu_host_requests(),
        "checkpoint requested twice"
    );

    assert!(source.with_window_event_effects(Some(facts), |effects| {
        game_client::gui::dispatch_os_click_named_window(CONTINUE, effects)
    }));
    let [MenuHostRequest::StartCampaign {
        descriptor,
        clear_previous: false,
        source: MenuLaunchSource::ScoreScreen,
        ..
    }, MenuHostRequest::SetCursorVisibility(true)]: [MenuHostRequest; 2] = source
        .take_menu_host_requests()
        .try_into()
        .expect("one continuation followed by a cursor update")
    else {
        panic!("retail Continue did not queue the next mission");
    };
    assert_eq!(descriptor.map_name, NEXT);
    // CampaignManager.h deliberately returns zero for the next battle's
    // starting rank. The checkpoint still retains the prior player's rank.
    assert_eq!(descriptor.rank_points, 0);
    assert_eq!(descriptor.difficulty_code, 2);
    assert!(!manager.save_exists("00000002"));
}
