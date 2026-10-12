//! Actual CPP Player roster / Object Underpowered effects, without registry publication.
use super::*;
use crate::common::{Coord3D, DefaultThingTemplate, KindOf};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::player::{
    OwnedPowerBrownout, Player, PlayerList, PowerBrownoutServices, RadarEdgeServices,
};
use crate::team::TeamFactory;

struct QuietRadar(u32);
impl RadarEdgeServices for QuietRadar {
    fn has_single_player_been_defeated(&self, _: &Player) -> bool {
        false
    }
    fn client_quiet(&self) -> bool {
        true
    }
    fn in_logic_update(&self) -> bool {
        true
    }
    fn frame(&self) -> u32 {
        self.0
    }
    fn play_radar_edge(&mut self, _: i32, _: bool) {
        panic!("quiet client emits no edge audio");
    }
}
struct Match {
    factory: ObjectFactory,
    teams: TeamFactory,
    players: PlayerList,
    producer: Arc<RwLock<Object>>,
    consumer: Arc<RwLock<Object>>,
}
impl Match {
    fn new(powered_producer: bool) -> Self {
        let mut players = PlayerList::new();
        players.add_player(Arc::new(RwLock::new(Player::new(0))));
        let mut teams = TeamFactory::new();
        teams
            .init_team_owned("PowerTeam".into(), "Owner".into(), true, None, Some(0))
            .unwrap();
        let team = teams
            .find_team_instances("PowerTeam")
            .into_iter()
            .next()
            .unwrap();
        let mut factory = ObjectFactory::new();
        let mut create = |name: &str, power: i32, powered: bool| {
            let mut template = DefaultThingTemplate::new(name.into());
            template.add_kind_of(KindOf::Structure);
            if powered {
                template.add_kind_of(KindOf::Powered);
            }
            template.set_energy_production(power);
            let id = factory
                .prepare_object(
                    Arc::new(template),
                    Coord3D::ZERO,
                    Some(team.clone()),
                    ObjectCreationFlags::NO_AI,
                )
                .unwrap();
            let object = factory.get_object(id).unwrap().get_base_object().unwrap();
            team.write().unwrap().add_member(id);
            players
                .get_player(0)
                .unwrap()
                .write()
                .unwrap()
                .change_owned_membership_power(&object.read().unwrap(), true);
            object
        };
        let producer = create("ActualPowerProducer", 10, powered_producer);
        let consumer = create("ActualPoweredConsumer", -15, true);
        Self {
            factory,
            teams,
            players,
            producer,
            consumer,
        }
    }
    fn energy(&self) -> (i32, i32) {
        let player = self.players.get_player(0).unwrap().read().unwrap();
        (
            player.get_energy().production(),
            player.get_energy().consumption(),
        )
    }
    fn apply(&self, brownout: bool, frame: u32) {
        let mut radar = QuietRadar(frame);
        let mut services = OwnedPowerBrownout {
            factory: &self.factory,
            teams: &self.teams,
            players: &self.players,
            radar: &mut radar,
            frame,
            special_power_uses_delay: true,
            ai_world: None,
            misc_audio: None,
            audio: None,
        };
        services
            .apply_brownout(0, brownout, &mut self.consumer.write().unwrap())
            .unwrap();
    }
}
#[test]
fn actual_power_roster_disables_only_owned_powered_members_and_preserves_peer() {
    let first = Match::new(false);
    let peer = Match::new(false);
    assert_eq!(
        first.consumer.read().unwrap().get_id(),
        peer.consumer.read().unwrap().get_id()
    );
    assert_eq!(first.energy(), (10, 15));
    first.apply(true, 20);
    assert!(
        first
            .consumer
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::DisabledUnderpowered)
    );
    assert!(!first.producer.read().unwrap().is_disabled());
    assert!(!peer.consumer.read().unwrap().is_disabled());
    assert_eq!(peer.energy(), (10, 15));
    first
        .players
        .get_player(0)
        .unwrap()
        .write()
        .unwrap()
        .get_energy_mut()
        .add_power_production(10);
    first.apply(false, 21);
    assert!(!first.consumer.read().unwrap().is_disabled());
    assert_eq!(first.energy(), (20, 15));
}
#[test]
fn actual_powered_producer_feedback_reenters_after_object_loan_is_released() {
    let world = Match::new(true);
    world.apply(true, 20);
    assert_eq!(world.energy(), (0, 15));
    assert!(
        world
            .producer
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::DisabledUnderpowered)
    );
    assert!(
        world
            .consumer
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::DisabledUnderpowered)
    );
    world.apply(true, 21);
    assert_eq!(
        world.energy(),
        (0, 15),
        "repeat disables have no second power edge"
    );
    world
        .players
        .get_player(0)
        .unwrap()
        .write()
        .unwrap()
        .get_energy_mut()
        .add_power_production(20);
    world.apply(false, 22);
    assert_eq!(world.energy(), (30, 15));
    assert!(!world.producer.read().unwrap().is_disabled());
    assert!(!world.consumer.read().unwrap().is_disabled());
}

#[test]
fn foreign_same_id_receiver_is_rejected_before_radar_or_disable_mutation() {
    let world = Match::new(false);
    let peer = Match::new(false);
    let mut radar = QuietRadar(20);
    let mut services = OwnedPowerBrownout {
        factory: &world.factory,
        teams: &world.teams,
        players: &world.players,
        radar: &mut radar,
        frame: 20,
        special_power_uses_delay: true,
        ai_world: None,
        misc_audio: None,
        audio: None,
    };
    assert!(
        services
            .apply_brownout(0, true, &mut peer.consumer.write().unwrap())
            .is_err()
    );
    assert!(!world.consumer.read().unwrap().is_disabled());
    assert!(!peer.consumer.read().unwrap().is_disabled());
    assert_eq!(world.energy(), (10, 15));
    assert_eq!(peer.energy(), (10, 15));
}

#[test]
fn actual_installed_special_power_pauses_on_brownout_and_extends_ready_deadline() {
    use crate::object::special_power_module::{SpecialPowerModule, SpecialPowerModuleData};
    use crate::object::special_power_template::SpecialPowerTemplate;
    use game_engine::common::system::{Snapshotable, xfer_save::XferSave};
    use std::io::Cursor;
    let world = Match::new(false);
    let data = SpecialPowerModuleData {
        special_power_template: Some(Arc::new(
            SpecialPowerTemplate::new("OwnedPower".into(), 7).with_reload_time(100),
        )),
        ..Default::default()
    };
    let entry = Arc::new(ModuleEntry::new(
        "SpecialPowerModule".into(),
        "OwnedPower".into(),
        ModuleInterfaceType::SPECIAL_POWER,
        Arc::new(data.clone()),
        Box::new(SpecialPowerModule::new(
            world.consumer.read().unwrap().get_id(),
            data,
        )),
    ));
    world.consumer.write().unwrap().modules.push(entry.clone());
    let saved = || {
        let mut bytes = Vec::new();
        entry.with_module(|module| {
            module
                .as_any_mut()
                .downcast_mut::<SpecialPowerModule>()
                .unwrap()
                .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap()
        });
        bytes
    };
    world.apply(true, 20);
    let bytes = saved();
    assert_eq!(i32::from_le_bytes(bytes[5..9].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(bytes[9..13].try_into().unwrap()), 20);
    world.apply(true, 30);
    assert_eq!(saved(), bytes, "repeat disable does not pause twice");
    world
        .players
        .get_player(0)
        .unwrap()
        .write()
        .unwrap()
        .get_energy_mut()
        .add_power_production(10);
    world.apply(false, 50);
    let bytes = saved();
    assert_eq!(i32::from_le_bytes(bytes[5..9].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(bytes[1..5].try_into().unwrap()), 30);
}

#[test]
fn injected_real_spy_name_is_not_a_supplied_module_capability() {
    let world = Match::new(false);
    let id = world.consumer.read().unwrap().get_id();
    let spy = crate::object::update::spy_vision_update::SpyVisionUpdate::new(
        0,
        Arc::new(crate::object::update::spy_vision_update::SpyVisionUpdateModuleData::default()),
        id,
    );
    world
        .consumer
        .write()
        .unwrap()
        .push_behavior_module_for_test(Arc::new(std::sync::Mutex::new(spy)));
    let mut radar = QuietRadar(20);
    let mut power = OwnedPowerBrownout {
        factory: &world.factory,
        teams: &world.teams,
        players: &world.players,
        radar: &mut radar,
        frame: 20,
        special_power_uses_delay: true,
        ai_world: None,
        misc_audio: None,
        audio: None,
    };
    assert!(
        power
            .apply_brownout_from_handle(0, true, &world.consumer)
            .is_err()
    );
    assert!(
        !world
            .consumer
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::DisabledUnderpowered)
    );
    assert_eq!(world.energy(), (10, 15));
}
