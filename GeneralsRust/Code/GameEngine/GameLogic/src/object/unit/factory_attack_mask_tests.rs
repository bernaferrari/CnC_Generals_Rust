//! C++ ActionManager.cpp:721-742 checks any nonzero slot mask only for players.

use crate::action_manager::TheActionManager;
use crate::ai::CommandSourceType;
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::{Coord3D, KindOf, ObjectStatusTypes, Relationship};
use crate::damage::DamageType;
use crate::helpers::TheGameLogic;
use crate::locomotor::{LOCOMOTOR_STORE, LocomotorTemplate, SURFACE_GROUND};
use crate::modules::AIUpdateInterface;
use crate::object::Object;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::player::{Player, ThePlayerList};
use crate::team::Team;
use crate::weapon::{WeaponSlotType, WeaponTemplate, WeaponTemplateSet};
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::{Arc, Mutex, RwLock};

const MASK_OBJECT_SOURCE: &str = "PlayerMaskAttackInfantry";
const MASK_OBJECT_TARGET: &str = "PlayerMaskAttackTarget";
const MASK_TEAM_SOURCE: u32 = 9601;
const MASK_TEAM_TARGET: u32 = 9602;
const MASK_PLAYER_SOURCE: u32 = 0;
const MASK_PLAYER_TARGET: u32 = 1;

fn mask_definitions() {
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();

    let mut locomotor = LocomotorTemplate::new("PlayerMaskAttackGround".into());
    locomotor.surfaces = SURFACE_GROUND;
    locomotor.max_speed = 3.0;
    locomotor.acceleration = 0.1;
    LOCOMOTOR_STORE.register_template(locomotor);

    assert_eq!(
        get_thing_factory()
            .unwrap()
            .as_mut()
            .unwrap()
            .load_ini_text(
                "Object PlayerMaskAttackInfantry\n KindOf = INFANTRY CAN_ATTACK\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface PlayerMaskAttackAI\n End\n Locomotor = SET_NORMAL PlayerMaskAttackGround\nEnd\nObject PlayerMaskAttackTarget\n KindOf = STRUCTURE IMMOBILE\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
            ),
        2
    );

    let mut players = ThePlayerList().write().unwrap();
    players.clear();
    for id in [MASK_PLAYER_SOURCE, MASK_PLAYER_TARGET] {
        let player_index = i32::try_from(id).unwrap();
        let mut player = Player::new(player_index);
        player.set_player_relationship_by_index(1 - player_index, Relationship::Enemies);
        players.add_player(Arc::new(RwLock::new(player)));
    }
}

fn mask_team(id: u32, player_id: u32) -> Arc<RwLock<Team>> {
    let team = Arc::new(RwLock::new(Team::new(
        format!("PlayerMaskTeam{id}").into(),
        id,
    )));
    team.write()
        .unwrap()
        .set_controlling_player_id(Some(player_id));
    team
}

#[derive(Clone, Copy)]
enum MaskWeapon {
    Explosion,
    Hack,
}

struct MaskAttackRuntime {
    _factory: ObjectFactory,
    source: Arc<RwLock<Object>>,
    target: Arc<RwLock<Object>>,
    ai: Arc<Mutex<dyn AIUpdateInterface>>,
}

impl MaskAttackRuntime {
    fn new(weapon: Option<MaskWeapon>, masks: [u32; 3]) -> Self {
        let mut factory = ObjectFactory::new();
        let source_id = factory
            .create_object(
                MASK_OBJECT_SOURCE,
                Coord3D::ZERO,
                Some(mask_team(MASK_TEAM_SOURCE, MASK_PLAYER_SOURCE)),
                ObjectCreationFlags::empty(),
            )
            .unwrap();
        let target_id = factory
            .create_object(
                MASK_OBJECT_TARGET,
                Coord3D::new(250.0, 0.0, 0.0),
                Some(mask_team(MASK_TEAM_TARGET, MASK_PLAYER_TARGET)),
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();

        let unit = factory.get_object(source_id).unwrap();
        assert!(unit.is_unit());
        let source = unit.get_base_object().unwrap();
        assert!(Arc::ptr_eq(
            &source,
            &TheGameLogic::find_object_by_id(source_id).unwrap()
        ));
        assert!(super::super::registry::get_unit_arc(source_id).is_none());
        let target = factory
            .get_object(target_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        assert!(source.read().unwrap().is_kind_of(KindOf::Infantry));
        assert!(target.read().unwrap().is_kind_of(KindOf::Structure));
        assert!(
            target.read().unwrap().get_body().is_some(),
            "authored ActiveBody"
        );
        assert_eq!(
            target
                .read()
                .unwrap()
                .get_body()
                .unwrap()
                .lock()
                .unwrap()
                .get_health(),
            100.0
        );
        let ai = source.read().unwrap().get_ai_update_interface().unwrap();

        if let Some(kind) = weapon {
            let mut template = WeaponTemplate::new("PlayerMaskAttackWeapon".into());
            template.attack_range = 1000.0;
            template.primary_damage = 10.0;
            template.damage_type = match kind {
                MaskWeapon::Explosion => DamageType::Explosion,
                MaskWeapon::Hack => DamageType::Hack,
            };
            let mut set = WeaponTemplateSet::new();
            set.set_weapon_template(WeaponSlotType::Primary, Arc::new(template));
            set.auto_choose_mask = masks;

            let mut source_guard = source.write().unwrap();
            source_guard.weapon_set.add_weapon_template_set(set);
            source_guard.refresh_weapon_set().unwrap();
            source_guard.reload_all_ammo(true).unwrap();
            let (current, slot) = source_guard
                .get_current_weapon()
                .expect("installed primary weapon instance");
            assert_eq!(slot, WeaponSlotType::Primary);
            assert_eq!(current.get_weapon_slot(), WeaponSlotType::Primary);
            assert_eq!(
                current.get_status(),
                crate::weapon::WeaponStatus::ReadyToFire
            );
        }
        {
            let mut ai = ai.lock().unwrap();
            ai.execute_command(&crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::Idle,
                CommandSourceType::FromAI,
            ))
            .unwrap();
            assert_eq!(
                ai.get_current_state_id(),
                Some(crate::ai::states::AIStateType::Idle as u32)
            );
            assert_eq!(
                ai.get_current_command(),
                Some(crate::ai::AiCommandType::Idle)
            );
        }

        Self {
            _factory: factory,
            source,
            target,
            ai,
        }
    }

    fn ai_wire(&self) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        assert!(
            self.ai
                .lock()
                .unwrap()
                .xfer_ai_update_state(&mut XferSave::new(&mut bytes, 1))
                .unwrap()
        );
        assert!(!bytes.get_ref().is_empty());
        bytes.into_inner()
    }

    fn weapon_wire(&self) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        self.source
            .write()
            .unwrap()
            .weapon_set
            .xfer_state(&mut XferSave::new(&mut bytes, 1))
            .unwrap();
        assert!(!bytes.get_ref().is_empty());
        bytes.into_inner()
    }
}

fn mask_lower(source: &Object, target: &Object, command: CommandSourceType) -> CanAttackResult {
    source.get_able_to_attack_specific_object_for_objects(
        AbleToAttackType::NewTarget,
        target,
        command,
    )
}

fn mask_action(source: &Object, target: &Object, command: CommandSourceType) -> CanAttackResult {
    TheActionManager::get_can_attack_object(source, target, command, AbleToAttackType::NewTarget)
}

fn assert_mask_query_pure(
    runtime: &MaskAttackRuntime,
    expected_lower: CanAttackResult,
    expected_player: CanAttackResult,
    expected_ai: CanAttackResult,
) {
    let ai_before = runtime.ai_wire();
    let weapon_before = runtime.weapon_wire();
    let rng_before = game_engine::common::random_value::get_game_logic_random_seed_state();
    let _ai = runtime.ai.lock().unwrap();
    let source = runtime.source.write().unwrap();
    let target = runtime.target.write().unwrap();
    assert_eq!(
        source.relationship_to(&target),
        Relationship::Enemies,
        "real factory owner/team relation"
    );
    assert!(!source.is_effectively_dead());
    assert!(!target.is_effectively_dead());
    assert!(!source.test_status(ObjectStatusTypes::NoAttack));
    assert_eq!(
        source.get_weapon_in_weapon_slot_command_source_mask(WeaponSlotType::Primary),
        source
            .weapon_set
            .get_nth_command_source_mask(WeaponSlotType::Primary)
    );

    let before = (
        source.get_status_bits(),
        target.get_status_bits(),
        *source.get_position(),
        *target.get_position(),
    );
    assert_eq!(
        mask_lower(&source, &target, CommandSourceType::FromPlayer),
        expected_lower
    );
    let player = mask_action(&source, &target, CommandSourceType::FromPlayer);
    let computer = mask_action(&source, &target, CommandSourceType::FromAI);
    let script = mask_action(&source, &target, CommandSourceType::FromScript);
    let after = (
        source.get_status_bits(),
        target.get_status_bits(),
        *source.get_position(),
        *target.get_position(),
    );
    drop(target);
    drop(source);
    drop(_ai);

    assert_eq!(player, expected_player, "ActionManager FromPlayer policy");
    assert_eq!(computer, expected_ai, "ActionManager FromAI policy");
    assert_eq!(
        script, expected_lower,
        "ActionManager FromScript keeps lower result"
    );
    assert_eq!(before, after, "query must not mutate status or positions");
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        rng_before,
        "query must not consume game logic RNG"
    );
    assert_eq!(
        runtime.ai_wire(),
        ai_before,
        "AI Xfer must be byte-identical"
    );
    assert_eq!(
        runtime.weapon_wire(),
        weapon_before,
        "weapon-set Xfer must be byte-identical"
    );
}

#[test]
fn factory_mask_all_zero_ordinary_damage_rejects_player_but_keeps_ai() {
    if !super::child(concat!(
        module_path!(),
        "::factory_mask_all_zero_ordinary_damage_rejects_player_but_keeps_ai"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    mask_definitions();
    let _frame = super::RestoreAmbientFrame::set(17);
    let runtime = MaskAttackRuntime::new(Some(MaskWeapon::Explosion), [0, 0, 0]);
    {
        let source = runtime.source.read().unwrap();
        assert!(source.has_any_damage_weapon());
        assert!(source.get_current_weapon().is_some());
        for slot in [
            WeaponSlotType::Primary,
            WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary,
        ] {
            assert_eq!(
                source.get_weapon_in_weapon_slot_command_source_mask(slot),
                0
            );
        }
    }
    assert_mask_query_pure(
        &runtime,
        CanAttackResult::Possible,
        CanAttackResult::NotPossible,
        CanAttackResult::Possible,
    );
}

#[test]
fn factory_mask_hack_default_nonzero_mask_preserves_invalid_shot_result() {
    if !super::child(concat!(
        module_path!(),
        "::factory_mask_hack_default_nonzero_mask_preserves_invalid_shot_result"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    mask_definitions();
    let _frame = super::RestoreAmbientFrame::set(17);
    let runtime = MaskAttackRuntime::new(Some(MaskWeapon::Hack), [0xffff_ffff; 3]);
    {
        let source = runtime.source.read().unwrap();
        assert!(!source.has_any_damage_weapon());
        assert!(source.get_current_weapon().is_some());
        assert_eq!(
            source.get_weapon_in_weapon_slot_command_source_mask(WeaponSlotType::Primary),
            0xffff_ffff
        );
        for slot in [WeaponSlotType::Secondary, WeaponSlotType::Tertiary] {
            assert_eq!(
                source.get_weapon_in_weapon_slot_command_source_mask(slot),
                0xffff_ffff,
                "empty slots preserve C++/Rust all-source defaults"
            );
        }
    }
    // C++ WeaponSet::getAbleToUseWeaponAgainstTarget skips the damage scan
    // for HACK-only weapons and returns INVALID_SHOT; the slot-mask gate
    // preserves that result rather than turning it into NOT_POSSIBLE.
    assert_mask_query_pure(
        &runtime,
        CanAttackResult::InvalidShot,
        CanAttackResult::InvalidShot,
        CanAttackResult::InvalidShot,
    );
}

fn check_truthy_mask_rows(default_masks: bool) {
    mask_definitions();
    let _frame = super::RestoreAmbientFrame::set(17);
    assert_eq!(CommandSourceType::FromPlayer as u32, 0);
    assert_eq!(CommandSourceType::FromScript as u32, 1);
    assert_eq!(CommandSourceType::FromAI as u32, 2);
    let player_bit = 1u32 << (CommandSourceType::FromPlayer as u32);
    let script_bit = 1u32 << (CommandSourceType::FromScript as u32);
    assert_ne!(script_bit, 0);
    assert_eq!(script_bit & player_bit, 0);
    let first = if default_masks { 1 } else { 0 };
    for slot_index in first..3 {
        let mut masks = [0; 3];
        let witness_mask = if default_masks { u32::MAX } else { script_bit };
        masks[slot_index] = witness_mask;
        let slots = [
            WeaponSlotType::Primary,
            WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary,
        ];
        let witness_slot = slots[slot_index];
        let runtime = MaskAttackRuntime::new(Some(MaskWeapon::Explosion), masks);
        {
            let source = runtime.source.read().unwrap();
            assert!(source.has_any_damage_weapon());
            for (index, slot) in slots.into_iter().enumerate() {
                assert_eq!(
                    source.get_weapon_in_weapon_slot_command_source_mask(slot),
                    masks[index]
                );
            }
            assert_ne!(witness_mask, 0);
            if !default_masks {
                assert_eq!(
                    witness_mask & player_bit,
                    0,
                    "C++ does not require the player bit"
                );
            }
            if witness_slot != WeaponSlotType::Primary {
                assert!(
                    source.get_weapon_in_weapon_slot(witness_slot).is_none(),
                    "nonzero mask on an empty slot still counts"
                );
            }
        }
        eprintln!("mask witness slot={witness_slot:?} default={default_masks}");
        assert_mask_query_pure(
            &runtime,
            CanAttackResult::Possible,
            CanAttackResult::Possible,
            CanAttackResult::Possible,
        );
    }
}

#[test]
fn factory_mask_script_only_truthy_mask_in_each_slot_accepts_player() {
    if !super::child(concat!(
        module_path!(),
        "::factory_mask_script_only_truthy_mask_in_each_slot_accepts_player"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    check_truthy_mask_rows(false);
}

#[test]
fn factory_mask_default_masks_on_empty_slots_accept_player() {
    if !super::child(concat!(
        module_path!(),
        "::factory_mask_default_masks_on_empty_slots_accept_player"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    check_truthy_mask_rows(true);
}
