//! Original model-condition names and INI equality separators at typed Draw admission.
use super::*;
use crate::common::ModelConditionFlags;
use crate::object::draw::W3DModelDrawModuleData;

#[test]
fn registered_draw_uses_original_condition_bits_for_aliases_and_retail_states() {
    let mut factory = catalog();
    let data = parse(&mut factory, ModuleType::Draw, "W3DModelDraw", "DefaultConditionState\nModel = OriginalModel\nEnd\nConditionState = MOVING\nEnd\nAliasConditionState = MOVING FIRING_A\nConditionState = FIRING_A\nEnd\nConditionState = FRONTCRUSHED BACKCRUSHED\nEnd\nConditionState = TURRET_ROTATE\nEnd\nConditionState = RAISING_FLAG\nEnd\nConditionState = CONTINUOUS_FIRE_SLOW ATTACKING\nEnd\nEnd\n").unwrap();
    let data = data
        .as_any()
        .downcast_ref::<W3DModelDrawModuleData>()
        .unwrap();
    assert_eq!(data.condition_states.len(), 7);
    assert_eq!(
        data.condition_states[1].conditions_yes[1].bits(),
        (1u128 << 49) | (1u128 << 36)
    );
    assert_eq!(
        data.condition_states[2].conditions_yes[0].bits(),
        1u128 << 36
    );
    assert_eq!(
        data.condition_states[3].conditions_yes[0].bits(),
        (1u128 << 1) | (1u128 << 2)
    );
    assert_eq!(
        data.condition_states[4].conditions_yes[0].bits(),
        1u128 << 47
    );
    assert_eq!(
        data.condition_states[5].conditions_yes[0],
        ModelConditionFlags::RAISING_FLAG
    );
    assert_eq!(
        data.condition_states[6].conditions_yes[0].bits(),
        (1u128 << 84) | (1u128 << 34)
    );
    for (bit, name) in game_engine::common::bit_flags::ModelConditionFlags::BIT_NAMES
        .iter()
        .enumerate()
    {
        assert_eq!(
            ModelConditionFlags::from_ini_name(name).unwrap().bits(),
            1u128 << bit,
            "original bit name {name}"
        );
    }
    assert_eq!(ModelConditionFlags::DISGUISED.bits(), 1u128 << 116);
    assert_eq!(
        ModelConditionFlags::from_ini_name("DISGUISED"),
        Some(ModelConditionFlags::DISGUISED),
        "authored parser and live stealth state must share canonical identity"
    );
    assert_eq!(ModelConditionFlags::SELECTED.bits(), 1u128 << 118);
    assert_eq!(ModelConditionFlags::WEAPON_UPGRADED.bits(), 1u128 << 119);
    assert_eq!(ModelConditionFlags::ARMOR_UPGRADED.bits(), 1u128 << 120);
    assert!(ModelConditionFlags::from_ini_name("NOT_AN_ORIGINAL_CONDITION").is_none());
}

#[test]
fn registered_draw_accepts_original_attached_equals_in_state_and_particle_fields() {
    let mut factory = catalog();
    let data = parse(&mut factory, ModuleType::Draw, "W3DModelDraw", "DefaultConditionState\nModel=OriginalModel\nParticleSysBone= Fire01 SmolderingFire\nEnd\nConditionState =RUBBLE\nModel=None\nEnd\nEnd\n").unwrap();
    let data = data
        .as_any()
        .downcast_ref::<W3DModelDrawModuleData>()
        .unwrap();
    assert_eq!(
        data.condition_states[0].model_name.as_str(),
        "originalmodel"
    );
    let row = &data.condition_states[0].particle_sys_bones[0];
    assert_eq!(row.bone_name.as_str(), "fire01");
    assert_eq!(row.particle_system.as_str(), "SmolderingFire");
    assert_eq!(
        data.condition_states[1].conditions_yes,
        [ModelConditionFlags::RUBBLE]
    );
    assert!(data.condition_states[1].model_name.is_empty());
    assert!(
        parse(
            &mut factory,
            ModuleType::Draw,
            "W3DModelDraw",
            "UnknownRetailDrawField=3\nEnd\n"
        )
        .is_err()
    );
}
