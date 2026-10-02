use super::*;
use crate::common::{DefaultThingTemplate, WeaponBonusConditionFlags};
use crate::object::ModuleEntry;
use crate::object::registry::{OBJECT_REGISTRY, test_isolation_lock};
use crate::player::{Player, PlayerList, player_list};
use crate::team::Team;
use game_engine::common::thing::module::ModuleInterfaceType;
use game_engine::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

pub(super) struct PlayerListRestore(Option<PlayerList>);

impl PlayerListRestore {
    pub(super) fn new() -> Self {
        Self(Some(std::mem::replace(
            &mut *player_list().write().unwrap(),
            PlayerList::new(),
        )))
    }
}

impl Drop for PlayerListRestore {
    fn drop(&mut self) {
        *player_list().write().unwrap() = self.0.take().unwrap();
    }
}

struct Army {
    owner: Arc<RwLock<GameObject>>,
    member: Arc<RwLock<GameObject>>,
    excluded: Arc<RwLock<GameObject>>,
    enemy: Arc<RwLock<GameObject>>,
    player: Arc<RwLock<Player>>,
    _players: PlayerListRestore,
}

impl Army {
    fn new() -> Self {
        let players = PlayerListRestore::new();
        let player = Arc::new(RwLock::new(Player::new(0)));
        let enemy_player = Arc::new(RwLock::new(Player::new(1)));
        {
            let mut list = player_list().write().unwrap();
            list.add_player(player.clone());
            list.add_player(enemy_player);
        }
        let object = |id, kind, player_id| {
            let mut object = GameObject::new_test(id, 100.0);
            let mut template = DefaultThingTemplate::new("BattlePlanMember".into());
            template.add_kind_of(kind);
            object.thing_template = Arc::new(template);
            object.set_vision_range(80.0);
            object.set_shroud_clearing_range(60.0);
            let team = Arc::new(RwLock::new(Team::new("BattlePlanArmy".into(), id + 10)));
            team.write()
                .unwrap()
                .set_controlling_player_id(Some(player_id));
            let object = Arc::new(RwLock::new(object));
            OBJECT_REGISTRY.register_object(id, &object);
            object.write().unwrap().set_team(Some(team)).unwrap();
            object
        };
        Self {
            owner: object(0x00B0_7201, KindOf::Vehicle, 0),
            member: object(0x00B0_7202, KindOf::Vehicle, 0),
            excluded: object(0x00B0_7203, KindOf::Structure, 0),
            enemy: object(0x00B0_7204, KindOf::Vehicle, 1),
            player,
            _players: players,
        }
    }

    fn assert_bonus(&self, scalar: f32, sight_scalar: f32, flag: WeaponBonusConditionFlags) {
        for object in [&self.owner, &self.member] {
            let object = object.read().unwrap();
            assert_eq!(
                object
                    .get_body_module()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .get_damage_scalar(),
                scalar
            );
            assert_eq!(object.get_vision_range(), 80.0 * sight_scalar);
            assert_eq!(object.get_shroud_clearing_range(), 60.0 * sight_scalar);
            assert_eq!(object.get_weapon_bonus_condition(), flag);
        }
        for object in [&self.excluded, &self.enemy] {
            let object = object.read().unwrap();
            assert_eq!(
                object
                    .get_body_module()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .get_damage_scalar(),
                1.0
            );
            assert_eq!(object.get_vision_range(), 80.0);
            assert_eq!(object.get_shroud_clearing_range(), 60.0);
            assert!(object.get_weapon_bonus_condition().is_empty());
        }
    }
}

impl Drop for Army {
    fn drop(&mut self) {
        for object in [&self.owner, &self.member, &self.excluded, &self.enemy] {
            OBJECT_REGISTRY.unregister_object(object.read().unwrap().get_id());
        }
    }
}

#[test]
fn player_callback_releases_battle_plan_owner_before_mutation() {
    let _lock = test_isolation_lock().lock().unwrap();
    let army = Army::new();
    let module = super::tests::make_module(army.owner.clone(), Some(1));
    // This is the same callback boundary used by onDelete. Avoid a hanging
    // test by checking the required owner write borrow before mutating it.
    let can_borrow_owner = module
        .behavior
        .with_controlling_player_mut(|_| army.owner.try_write().is_ok());
    assert_eq!(can_borrow_owner, Some(true));
}

#[test]
fn restored_plan_deletion_updates_owned_army_at_last_plan_boundary() {
    let _lock = test_isolation_lock().lock().unwrap();
    // C++ Player.cpp3391-3460: 2->1 retains bonuses, 1->0 removes them.
    // Object.cpp onDestroy invokes onDelete while its owner is still present.
    for plan in [
        BattlePlanStatus::Bombardment,
        BattlePlanStatus::HoldTheLine,
        BattlePlanStatus::SearchAndDestroy,
    ] {
        for count in [1, 2] {
            let army = Army::new();
            let mut module = super::tests::make_module(army.owner.clone(), Some(1));
            module.behavior.plan_affecting_army = plan;
            let bonuses = &mut module.behavior.bonuses;
            bonuses.armor_scalar = 2.0;
            bonuses.sight_range_scalar = 1.5;
            bonuses.valid_kind_of = KindOf::Vehicle.cpp_mask();
            bonuses.invalid_kind_of = KindOf::Structure.cpp_mask();
            let (plan_type, flag) = match plan {
                BattlePlanStatus::Bombardment => {
                    bonuses.bombardment = 1;
                    (
                        BattlePlanType::Bombard,
                        WeaponBonusConditionFlags::BATTLEPLAN_BOMBARDMENT,
                    )
                }
                BattlePlanStatus::HoldTheLine => {
                    bonuses.hold_the_line = 1;
                    (
                        BattlePlanType::HoldTheLine,
                        WeaponBonusConditionFlags::BATTLEPLAN_HOLDTHELINE,
                    )
                }
                BattlePlanStatus::SearchAndDestroy => {
                    bonuses.search_and_destroy = 1;
                    (
                        BattlePlanType::SearchAndDestroy,
                        WeaponBonusConditionFlags::BATTLEPLAN_SEARCHANDDESTROY,
                    )
                }
                BattlePlanStatus::None => unreachable!(),
            };
            for _ in 0..count {
                army.player
                    .write()
                    .unwrap()
                    .change_battle_plan(plan_type, 1, bonuses);
            }
            army.assert_bonus(2.0, 1.5, flag);

            let mut bytes = Vec::new();
            module
                .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap();
            let mut loaded = super::tests::make_module(army.owner.clone(), Some(1));
            loaded
                .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
                .unwrap();
            loaded.load_post_process().unwrap();
            assert_eq!(loaded.behavior.plan_affecting_army, plan);
            let data: Arc<dyn EngineModuleData> = loaded.module_data.clone();
            let entry = Arc::new(ModuleEntry::new(
                "BattlePlanUpdate".into(),
                "ModuleTag_Plan".into(),
                ModuleInterfaceType::UPDATE,
                data,
                Box::new(loaded),
            ));
            // Take the actual Object destruction path, including its already
            // held owner write guard and the module-list callback dispatch.
            {
                let mut owner = army.owner.write().unwrap();
                owner.modules.push(entry);
                owner.on_destroy_internal();
            }
            assert_eq!(
                army.player.read().unwrap().get_battle_plan_count(plan_type),
                count - 1
            );
            if count == 1 {
                army.assert_bonus(1.0, 1.0, WeaponBonusConditionFlags::empty());
            } else {
                army.assert_bonus(2.0, 1.5, flag);
            }
        }
    }
}

#[test]
fn snapshot_enums_preserve_raw_i32_bytes_and_unknown_fallback() {
    for (mut plan, mut transition) in [
        (BattlePlanStatus::None, TransitionStatus::Idle),
        (BattlePlanStatus::Bombardment, TransitionStatus::Unpacking),
        (BattlePlanStatus::HoldTheLine, TransitionStatus::Active),
        (
            BattlePlanStatus::SearchAndDestroy,
            TransitionStatus::Packing,
        ),
    ] {
        let mut bytes = Vec::new();
        let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
        xfer_battle_plan_status(&mut save, &mut plan).unwrap();
        xfer_transition_status(&mut save, &mut transition).unwrap();
        let mut expected = (plan as i32).to_ne_bytes().to_vec();
        expected.extend_from_slice(&(transition as i32).to_ne_bytes());
        assert_eq!(bytes, expected);
        let mut loaded_plan = BattlePlanStatus::None;
        let mut loaded_transition = TransitionStatus::Idle;
        let mut load = XferLoad::new(Cursor::new(bytes), 1);
        xfer_battle_plan_status(&mut load, &mut loaded_plan).unwrap();
        xfer_transition_status(&mut load, &mut loaded_transition).unwrap();
        assert_eq!(loaded_plan, plan);
        assert_eq!(loaded_transition, transition);
    }
    let mut bytes = (-1i32).to_ne_bytes().to_vec();
    bytes.extend_from_slice(&99i32.to_ne_bytes());
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    let mut plan = BattlePlanStatus::SearchAndDestroy;
    let mut transition = TransitionStatus::Packing;
    xfer_battle_plan_status(&mut load, &mut plan).unwrap();
    xfer_transition_status(&mut load, &mut transition).unwrap();
    assert_eq!(plan, BattlePlanStatus::None);
    assert_eq!(transition, TransitionStatus::Idle);
}

#[test]
fn kind_mask_crc_preserves_version_and_native_integer_payload() {
    use game_engine::system::xfer_crc::XferCRC;
    let expected_mask = KindOf::Vehicle.cpp_mask() | KindOf::Structure.cpp_mask();
    let mut mask = expected_mask;
    let mut actual = XferCRC::new(XferSave::new(Cursor::new(Vec::<u8>::new()), 1));
    xfer_kind_of_mask(&mut actual, &mut mask).unwrap();
    let mut expected = XferCRC::new(XferSave::new(Cursor::new(Vec::<u8>::new()), 1));
    expected.xfer_version(&mut 1, 1).unwrap();
    expected
        .xfer_user_bytes(&mut expected_mask.to_ne_bytes())
        .unwrap();
    assert_eq!(actual.get_crc(), expected.get_crc());
    assert_eq!(mask, expected_mask);
}

#[test]
fn kind_mask_save_uses_each_retail_bit_name_once_in_cpp_order() {
    let mut mask = KindOf::Structure.cpp_mask() | KindOf::Vehicle.cpp_mask();
    let mut bytes = Vec::new();
    xfer_kind_of_mask(&mut XferSave::new(Cursor::new(&mut bytes), 1), &mut mask).unwrap();
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    let mut version = 0;
    load.xfer_version(&mut version, 1).unwrap();
    assert_eq!(version, 1);
    let mut count = 0;
    load.xfer_int(&mut count).unwrap();
    assert_eq!(count, 2, "STRUCTURE aliases share one C++ bit");
    for expected in ["STRUCTURE", "VEHICLE"] {
        let mut name = String::new();
        load.xfer_ascii_string(&mut name).unwrap();
        assert_eq!(name, expected);
    }
}

#[test]
fn kind_mask_roundtrips_all_retail_bits() {
    use game_engine::common::system::kind_of::{KIND_OF_BIT_NAMES, KINDOF_COUNT};
    assert_eq!(KINDOF_COUNT, KIND_OF_BIT_NAMES.len());
    let expected = (1u128 << KINDOF_COUNT) - 1;
    let mut saved = expected;
    let mut bytes = Vec::new();
    xfer_kind_of_mask(&mut XferSave::new(Cursor::new(&mut bytes), 1), &mut saved).unwrap();
    let mut loaded = 0;
    xfer_kind_of_mask(&mut XferLoad::new(Cursor::new(bytes), 1), &mut loaded).unwrap();
    assert_eq!(loaded, expected);
}

#[test]
fn battle_plan_applies_nearest_scalar_below_one_like_cpp() {
    let _lock = test_isolation_lock().lock().unwrap();
    let army = Army::new();
    let mut module = super::tests::make_module(army.owner.clone(), Some(1));
    // Player.cpp3524/3536 test scalar !=1.0f, not approximate equality.
    let scalar = f32::from_bits(1.0f32.to_bits() - 1);
    let bonus = &mut module.behavior.bonuses;
    bonus.armor_scalar = scalar;
    bonus.sight_range_scalar = scalar;
    bonus.valid_kind_of = KindOf::Vehicle.cpp_mask();
    bonus.invalid_kind_of = KindOf::Structure.cpp_mask();
    bonus.hold_the_line = 1;
    army.player
        .write()
        .unwrap()
        .change_battle_plan(BattlePlanType::HoldTheLine, 1, bonus);
    army.assert_bonus(
        scalar,
        scalar,
        WeaponBonusConditionFlags::BATTLEPLAN_HOLDTHELINE,
    );
}
