use super::*;
use game_engine::common::audio::game_audio::{
    get_global_audio_manager, initialize_global_audio_manager,
};
use game_engine::common::audio::{AudioEventInfo, AudioManager, AudioType, ST_PLAYER};

// Restore the actual global services rather than leave this fixture's roster,
// sounds, or pending requests behind for the rest of the serial behavior suite.
struct Services {
    players: Option<PlayerList>,
    audio: Option<AudioManager>,
}
impl Services {
    fn install() -> Self {
        crate::helpers::TheAudio::get().unwrap();
        let players = std::mem::replace(&mut *player_list().write().unwrap(), PlayerList::new());
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let mut replacement = AudioManager::new();
        replacement.init();
        let audio = std::mem::replace(&mut *manager.lock().unwrap(), replacement);
        Self {
            players: Some(players),
            audio: Some(audio),
        }
    }
}
impl Drop for Services {
    fn drop(&mut self) {
        *player_list().write().unwrap() = self.players.take().unwrap();
        let manager = get_global_audio_manager().unwrap();
        *manager.lock().unwrap() = self.audio.take().unwrap();
    }
}
fn admit(list: &mut PlayerList, index: Int, cash: Int) -> Arc<RwLock<Player>> {
    let mut player = Player::new(index);
    player.set_default_team(Some(Arc::new(RwLock::new(Team::new(
        format!("money-team-{index}").into(),
        index as u32,
    )))));
    player.get_money_mut().set_money(cash);
    let player = Arc::new(RwLock::new(player));
    list.add_player(player.clone());
    player
}
fn register_money_sounds() -> (String, String) {
    let misc = crate::helpers::TheAudio::get_misc_audio();
    let withdraw = misc.money_withdraw.get_event_name().to_string();
    let deposit = misc.money_deposit.get_event_name().to_string();
    let manager = get_global_audio_manager().unwrap();
    let mut manager = manager.lock().unwrap();
    for name in [&withdraw, &deposit] {
        manager.register_audio_event_info(AudioEventInfo {
            audio_name: name.clone(),
            sound_type: AudioType::SoundEffect,
            sound_type_field: AudioType::SoundEffect,
            type_field: ST_PLAYER,
            sounds: vec![format!("{name}.wav")],
            volume: 1.0,
            pitch_shift_min: 1.0,
            pitch_shift_max: 1.0,
            loop_count: 1,
            ..Default::default()
        });
    }
    (withdraw, deposit)
}
fn requests(name: &str) -> usize {
    get_global_audio_manager()
        .unwrap()
        .lock()
        .unwrap()
        .pending_play_request_count_for(name)
}

#[test]
fn registered_money_audio_and_income_do_not_reenter_write_borrowed_player() {
    let _test_guard = crate::test_sync::lock();
    let _services = Services::install();
    let player = {
        let mut list = player_list().write().unwrap();
        let player = admit(&mut list, 0, 100);
        list.set_local_player_index(0);
        player
    };
    let (withdraw, deposit) = register_money_sounds();
    let mut owner = player.write().unwrap();
    // Actual classic path: the installed resolver would read this same Player
    // again in the old implementation. No try-lock or disabled audio fallback.
    assert_eq!(owner.get_money_mut().withdraw(30).unwrap(), 30);
    assert_eq!(owner.get_money().count_money(), 70);
    assert_eq!(requests(&withdraw), 1);
    owner.get_money_mut().deposit(12).unwrap();
    assert_eq!(owner.get_money().count_money(), 82);
    assert_eq!(owner.academy_stats.total_income, 12);
    assert_eq!(requests(&deposit), 1);
    assert_eq!(owner.score_keeper.get_total_money_earned(), 0);
    owner.get_money_mut().add_money_earned(12);
    assert_eq!(owner.score_keeper.get_total_money_earned(), 12);
    assert!(!owner.get_money_mut().subtract_money(83));
    assert_eq!(owner.get_money_mut().withdraw(0).unwrap(), 0);
    owner.get_money_mut().deposit(0).unwrap();
    assert_eq!(requests(&withdraw), 1);
    assert_eq!(requests(&deposit), 1);
    assert_eq!(
        owner
            .get_money_mut()
            .withdraw_with_sound(1000, false)
            .unwrap(),
        82
    );
    owner.get_money_mut().deposit_with_sound(8, false).unwrap();
    assert_eq!(owner.get_money().count_money(), 8);
    assert_eq!(owner.academy_stats.total_income, 20);
    assert_eq!(requests(&withdraw), 1);
    assert_eq!(requests(&deposit), 1);
}

#[test]
fn explicit_money_locality_keeps_same_index_worlds_and_income_independent() {
    let _test_guard = crate::test_sync::lock();
    let _services = Services::install();
    let (withdraw, deposit) = register_money_sounds();
    let mut first = PlayerList::new();
    let mut second = PlayerList::new();
    let a = admit(&mut first, 0, 100);
    let b = admit(&mut second, 0, 200);
    admit(&mut second, 1, 0);
    first.set_local_player_index(0);
    second.set_local_player_index(1);
    for (list, player, amount) in [(&first, &a, 10), (&second, &b, 20)] {
        let mut player = player.write().unwrap();
        let facts = crate::helpers::capture_player_audio_locality(&player, list);
        assert_eq!(
            player
                .get_money_mut_with_locality(facts)
                .withdraw(amount)
                .unwrap(),
            amount
        );
        player
            .get_money_mut_with_locality(facts)
            .deposit(amount)
            .unwrap();
    }
    assert_eq!(requests(&withdraw), 1); // second world's source is not its local player
    assert_eq!(requests(&deposit), 1);
    assert_eq!(a.read().unwrap().get_money().count_money(), 100);
    assert_eq!(b.read().unwrap().get_money().count_money(), 200);
    assert_eq!(a.read().unwrap().academy_stats.total_income, 10);
    assert_eq!(b.read().unwrap().academy_stats.total_income, 20);
    // A command processor owns the actual list's write borrow while modifying
    // resources; its explicit path must not rediscover the global list.
    use crate::commands::command_processor::PlayerManager;
    first.modify_player_resources(0, 5, 0);
    assert_eq!(a.read().unwrap().get_money().count_money(), 105);
    assert_eq!(a.read().unwrap().academy_stats.total_income, 15);
}

#[test]
fn money_effect_observes_balance_before_commit_and_income_after_credit() {
    let mut player = Player::new(0);
    player.get_money_mut().set_money(100);
    assert_eq!(
        player
            .get_money_mut()
            .withdraw_with_submission(30, true, |money| {
                assert_eq!(money.count_money(), 100);
                assert_eq!(money.player.academy_stats.total_income, 0);
            })
            .unwrap(),
        30
    );
    assert_eq!(player.get_money().count_money(), 70);
    player
        .get_money_mut()
        .deposit_with_submission(5, true, |money| {
            assert_eq!(money.count_money(), 70);
            assert_eq!(money.player.academy_stats.total_income, 0);
        })
        .unwrap();
    assert_eq!(player.get_money().count_money(), 75);
    assert_eq!(player.academy_stats.total_income, 5);
    player
        .get_money_mut()
        .withdraw_with_submission(0, true, |_| panic!("zero withdrawal sound"))
        .unwrap();
    player
        .get_money_mut()
        .deposit_with_submission(0, true, |_| panic!("zero deposit sound"))
        .unwrap();
    player
        .get_money_mut()
        .withdraw_with_submission(1, false, |_| panic!("silent withdrawal sound"))
        .unwrap();
    player
        .get_money_mut()
        .deposit_with_submission(1, false, |_| panic!("silent deposit sound"))
        .unwrap();
    assert_eq!(player.academy_stats.total_income, 6);
}
