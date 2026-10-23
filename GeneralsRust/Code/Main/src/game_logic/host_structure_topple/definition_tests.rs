use super::*;
use crate::game_logic::{GameLogic, Team, ThingTemplate};

fn definition(fields: &str) -> crate::assets::ObjectDefinition {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&format!("Object FilteredTopple\nKindOf = STRUCTURE SELECTABLE\nBehavior = StructureToppleUpdate ModuleTag_Fall\n{fields}\nEnd\nEnd"),
        "topple-definition.ini").unwrap();
    parser.get_definition("FilteredTopple").unwrap().clone()
}

#[test]
fn authored_filter_defaults_and_original_flag_operations_are_retained() {
    let default = definition("");
    let parsed = StructureToppleDefinition::from_module(&default.behavior_modules[0]).unwrap();
    assert_eq!(
        parsed.die_mux.death_types,
        gamelogic::damage::DEATH_TYPE_FLAGS_ALL
    );
    assert_eq!(
        parsed.die_mux.veterancy_levels,
        gamelogic::object::die::VETERANCY_LEVEL_FLAGS_ALL
    );
    assert_eq!(
        parsed.damage_fx_types,
        gamelogic::damage::DamageTypeFlags::all_flags()
    );
    let authored = definition(
        "DeathTypes = NONE +NORMAL +CRUSHED -NORMAL\nVeterancyLevels = ALL -REGULAR\nDamageFXTypes = -FLAME\nRequiredStatus = HIJACKED\nExemptStatus = BOOBY_TRAPPED",
    );
    let parsed = StructureToppleDefinition::from_module(&authored.behavior_modules[0]).unwrap();
    assert!(!gamelogic::damage::get_death_type_flag(
        parsed.die_mux.death_types,
        gamelogic::damage::DeathType::Normal
    ));
    assert!(gamelogic::damage::get_death_type_flag(
        parsed.die_mux.death_types,
        gamelogic::damage::DeathType::Crushed
    ));
    assert!(
        !parsed
            .damage_fx_types
            .contains_damage_type(gamelogic::damage::DamageType::Flame)
    );
    assert!(
        parsed
            .damage_fx_types
            .contains_damage_type(gamelogic::damage::DamageType::Explosion)
    );
}

#[test]
fn invalid_authored_definition_is_rejected_before_actual_object_allocation() {
    for invalid in [
        "StructuralIntegrity = NaN",
        "StructuralDecay = infinity",
        "MinToppleDelay = -1",
        "MaxToppleBurstDelay = not-a-duration",
        "MinToppleDelay = 0.5",
        "AngleFX = NaN FX_Fall",
        "AngleFX = 10",
        "DeathTypes = NORMAL",
        "DeathTypes = +UNKNOWN",
        "VeterancyLevels = VETERAN",
        "DamageFXTypes = FLAME",
        "DamageFXTypes = +UNKNOWN",
        "RequiredStatus = UNKNOWN",
    ] {
        let invalid_definition = definition(invalid);
        let template = GameLogic::build_template_from_object_definition(
            "FilteredTopple",
            &invalid_definition,
            None,
        );
        assert!(
            matches!(&template.structure_topple_definition, Some(Err(_))),
            "{invalid}"
        );
        let mut owner = GameLogic::new();
        let mut peer = GameLogic::new();
        owner.templates.insert(template.name.clone(), template);
        assert!(
            owner
                .create_object("FilteredTopple", Team::USA, glam::Vec3::ZERO)
                .is_none(),
            "{invalid}"
        );
        assert_eq!(owner.host_objects().registered_len(), 0);
        let valid = ThingTemplate::new("ValidAfterRejection");
        owner.templates.insert(valid.name.clone(), valid.clone());
        peer.templates.insert(valid.name.clone(), valid);
        assert_eq!(
            owner.create_object("ValidAfterRejection", Team::USA, glam::Vec3::ZERO),
            peer.create_object("ValidAfterRejection", Team::USA, glam::Vec3::ZERO),
            "invalid definition must not consume an object ID: {invalid}"
        );
    }
}
