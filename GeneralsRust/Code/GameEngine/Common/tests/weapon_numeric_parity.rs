use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_weapon::get_weapon_store;

#[test]
fn authored_weapon_speeds_preserve_original_float_rounding() {
    let mut ini = INI::new();
    ini.with_inline_source(
        "Weapon Numeric400\n WeaponSpeed = 400.0\nEnd\n\
         Weapon Numeric999\n WeaponSpeed = 999.0\nEnd\n",
        |ini| ini.parse_current_file(),
    )
    .unwrap();

    // Exact outputs of the original-source oracle, also used by the corresponding
    // internal Weapon tests. These public parser cases need no internal feature.
    let store = get_weapon_store().unwrap();
    for (name, bits) in [("Numeric400", 0x4155_5556), ("Numeric999", 0x4205_3334)] {
        let template = store.find_template(&AsciiString::from(name)).unwrap();
        assert_eq!(template.projectile_speed.to_bits(), bits, "{name}");
    }
}
