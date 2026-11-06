//! Host GameLogic tests — `shells_and_missiles`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

/// Model a C++ armor-less target dummy in this world.
///
/// C++: ThingTemplate::findArmorTemplateSet returns NULL for a template with no
/// ArmorSet rows and ActiveBody keeps the default ArmorTemplate (every
/// coefficient 1.0 — Armor.cpp:25-33 clear()), so an armor-less template takes
/// full damage from every damage type. The Rust residual instead resolves
/// retail-typical armor by KindOf for armor-less templates
/// (host_armor_residual::residual_armor_for_object), which zeroes e.g. SNIPER
/// vs KindOf::Vehicle and therefore (correctly, per C++ WeaponSet.cpp:834-836
/// zero-damage elimination) blocks auto-fire against it. Fixtures that model a
/// C++ armor-less dummy author an explicit all-1.0 ArmorSet — identical
/// observable damage in C++, no production change.
fn stamp_cpp_armorless_dummy_armor(game_logic: &mut GameLogic, template_name: &str) {
    use gamelogic::object::armor::{ArmorDefinitions, ArmorTemplate};
    const TEST_DUMMY_ALL_ONES_ARMOR: &str = "TestDummyAllOnesArmor";
    if let Some(tpl) = game_logic.templates.get_mut(template_name) {
        let mut definitions = tpl
            .armor_definitions
            .as_deref()
            .cloned()
            .unwrap_or_else(ArmorDefinitions::new);
        definitions.register_template(TEST_DUMMY_ALL_ONES_ARMOR, ArmorTemplate::new());
        tpl.armor_definitions = Some(std::sync::Arc::new(definitions));
        tpl.armor_sets.push(crate::game_logic::HostArmorSet {
            conditions: 0,
            armor: Some(TEST_DUMMY_ALL_ONES_ARMOR.to_string()),
            damage_fx: None,
        });
    }
}

mod authored_module_keying;
mod authored_weapon_set_bindings;
mod bomb_truck_and_camouflage_residuals;
mod ground_unit_weapon_residuals;
mod named_weapon_skip_residuals;
mod salvage_and_missile_weapon_residuals;
