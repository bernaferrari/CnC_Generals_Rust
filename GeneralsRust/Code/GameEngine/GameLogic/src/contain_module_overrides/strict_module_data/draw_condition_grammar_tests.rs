//! Original BitFlagsIO token modes through the registered typed Draw constructors.
use super::*;
use crate::common::ModelConditionFlags;
use crate::object::draw::W3DModelDrawModuleData;
use game_engine::common::ini::INIError;

const MODEL_DRAW_CLASSES: &[&str] = &[
    "W3DModelDraw",
    "W3DDependencyModelDraw",
    "W3DTankDraw",
    "W3DOverlordTankDraw",
    "W3DPoliceCarDraw",
    "W3DScienceModelDraw",
];

fn condition_field_body(field: &str, value: &str) -> String {
    match field {
        "IgnoreConditionStates" => format!("{field} = {value}\nEnd\n"),
        "ConditionState" => {
            format!("DefaultConditionState\nModel=GrammarModel\nEnd\n{field} = {value}\nEnd\nEnd\n")
        }
        "AliasConditionState" => {
            format!("DefaultConditionState\nModel=GrammarModel\nEnd\n{field} = {value}\nEnd\n")
        }
        _ => unreachable!("model-condition field"),
    }
}

#[test]
fn registered_draw_rejects_unknown_condition_names_in_all_three_fields() {
    // BitFlagsIO delegates each name to the original table. scanIndexList
    // throws INVALID_DATA for an absent name, including a signed NONE.
    let mut factory = catalog();
    for &class in MODEL_DRAW_CLASSES {
        for field in [
            "IgnoreConditionStates",
            "ConditionState",
            "AliasConditionState",
        ] {
            for value in [
                "NOT_AN_ORIGINAL_MODEL_CONDITION",
                "INVALID",
                "PRISTINE",
                "MODELCONDITION_NIGHT",
                "SELECTED",
                "FIRING_PRIMARY",
                "NIGHT,SNOW",
                "NIGHT|SNOW",
                "+NONE",
                "-NONE",
                "+",
                "-",
            ] {
                let error = parse(
                    &mut factory,
                    ModuleType::Draw,
                    class,
                    &condition_field_body(field, value),
                )
                .expect_err("registered Draw must reject an unknown original condition name");
                assert!(error.contains(class), "{class} {field} {value}: {error}");
                assert!(error.contains("line"), "{class} {field} {value}: {error}");
                assert!(
                    error.ends_with("Invalid data"),
                    "{class} {field} {value} must preserve scanIndexList's error: {error}"
                );
            }
        }
        // Rejection must not poison the reusable registered constructor catalog.
        parse(
            &mut factory,
            ModuleType::Draw,
            class,
            "IgnoreConditionStates=NIGHT\nEnd\n",
        )
        .expect("a valid definition after the rejected definitions");
    }
}

#[test]
fn registered_draw_rejects_mixed_condition_modes_and_nonleading_none() {
    let mut factory = catalog();
    for &class in MODEL_DRAW_CLASSES {
        for field in [
            "IgnoreConditionStates",
            "ConditionState",
            "AliasConditionState",
        ] {
            for value in [
                "NIGHT +SNOW",
                "NIGHT -SNOW",
                "+NIGHT SNOW",
                "-NIGHT SNOW",
                "NIGHT NONE",
                "+NIGHT NONE",
                "-NIGHT NONE",
            ] {
                let error = parse(
                    &mut factory,
                    ModuleType::Draw,
                    class,
                    &condition_field_body(field, value),
                )
                .expect_err("registered Draw must reject mixed condition-list modes");
                assert!(
                    error.ends_with("Invalid name list"),
                    "{class} {field} {value} must preserve BitFlagsIO's mode error: {error}"
                );
            }
        }
    }
}

#[test]
fn registered_draw_preserves_ordered_mask_updates_aliases_and_none_shortcircuit() {
    let mut factory = catalog();
    let data = parse(
        &mut factory,
        ModuleType::Draw,
        "W3DModelDraw",
        "IgnoreConditionStates=NIGHT SNOW\nIgnoreConditionStates=+DAMAGED -SNOW\nIgnoreConditionStates=\nDefaultConditionState\nModel=GrammarModel\nEnd\nConditionState=MOVING FIRING_A\nEnd\nAliasConditionState=MOVING FIRING_B\nConditionState=+ATTACKING -NIGHT\nEnd\nEnd\n",
    )
    .unwrap();
    let data = data
        .as_any()
        .downcast_ref::<W3DModelDrawModuleData>()
        .unwrap();
    assert_eq!(
        data.ignore_condition_states,
        ModelConditionFlags::NIGHT | ModelConditionFlags::DAMAGED,
        "signed updates and an empty field retain the existing mask"
    );
    assert_eq!(data.default_state, 0);
    assert_eq!(data.condition_states.len(), 3);
    assert_eq!(
        data.condition_states[0].conditions_yes,
        [ModelConditionFlags::empty()]
    );
    assert_eq!(
        data.condition_states[1].conditions_yes,
        [
            ModelConditionFlags::MOVING | ModelConditionFlags::FIRING_A,
            ModelConditionFlags::MOVING | ModelConditionFlags::FIRING_B,
        ],
        "aliases remain attached to their preceding state in authored order"
    );
    assert_eq!(
        data.condition_states[2].conditions_yes,
        [ModelConditionFlags::ATTACKING]
    );

    let data = parse(
        &mut factory,
        ModuleType::Draw,
        "W3DModelDraw",
        "IgnoreConditionStates=NIGHT\nIgnoreConditionStates=SNOW\nDefaultConditionState\nModel=ReplacementModel\nEnd\nEnd\n",
    )
    .unwrap();
    let data = data
        .as_any()
        .downcast_ref::<W3DModelDrawModuleData>()
        .unwrap();
    assert_eq!(data.ignore_condition_states, ModelConditionFlags::SNOW);

    let data = parse(
        &mut factory,
        ModuleType::Draw,
        "W3DModelDraw",
        "IgnoreConditionStates=NIGHT\nIgnoreConditionStates=nOnE MOVING NOT_A_CONDITION\nConditionState=NONE MOVING NOT_A_CONDITION\nModel=NoneBreakModel\nEnd\nAliasConditionState=+MOVING -MOVING +FIRING_A\nEnd\n",
    )
    .unwrap();
    let data = data
        .as_any()
        .downcast_ref::<W3DModelDrawModuleData>()
        .unwrap();
    assert!(data.ignore_condition_states.is_empty());
    assert_eq!(data.default_state, -1);
    assert_eq!(data.condition_states.len(), 1);
    assert_eq!(
        data.condition_states[0].conditions_yes,
        [ModelConditionFlags::empty(), ModelConditionFlags::FIRING_A],
        "leading NONE clears and stops before later tokens, like the original break"
    );
}

#[test]
fn draw_condition_field_retains_original_prefix_when_later_token_rejects() {
    // C++ modifies the field as each accepted token arrives. A parse error
    // does not roll back that prefix; a discarded strict-admission candidate
    // still cannot become a published content definition.
    for (field, expected_error, expected_prefix) in [
        (
            "+SNOW +NOT_AN_ORIGINAL_MODEL_CONDITION",
            INIError::InvalidData,
            ModelConditionFlags::NIGHT | ModelConditionFlags::SNOW,
        ),
        (
            "MOVING NOT_AN_ORIGINAL_MODEL_CONDITION",
            INIError::InvalidData,
            ModelConditionFlags::MOVING,
        ),
        (
            "SNOW +DAMAGED",
            INIError::InvalidNameList,
            ModelConditionFlags::SNOW,
        ),
        (
            "+SNOW MOVING",
            INIError::InvalidNameList,
            ModelConditionFlags::NIGHT | ModelConditionFlags::SNOW,
        ),
        (
            "+SNOW NONE",
            INIError::InvalidNameList,
            ModelConditionFlags::NIGHT | ModelConditionFlags::SNOW,
        ),
        (
            "NOT_AN_ORIGINAL_MODEL_CONDITION",
            INIError::InvalidData,
            ModelConditionFlags::empty(),
        ),
    ] {
        let mut data = W3DModelDrawModuleData::new();
        let body = format!("IgnoreConditionStates=NIGHT\nIgnoreConditionStates={field}\nEnd\n");
        let result = INI::new().with_inline_source(&body, |ini| data.parse_from_ini(ini));
        assert_eq!(result, Err(expected_error), "{field}");
        assert_eq!(data.ignore_condition_states, expected_prefix, "{field}");
    }
}
