//! ActiveBody.h:27-41 inherits the empty ModuleData snapshot hooks through
//! BodyModuleData and BehaviorModuleData. Authored rules are not runtime health.
//! Factory CRC is borrowed safely; Save/Load exercises uniquely owned typed
//! data. Shared factory Xfer remains a separate, unresolved ownership boundary.
use super::*;
use game_engine::common::system::{xfer_crc::XferCRC, xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module_factory::ModuleFactory;
use std::io::Cursor;

fn authored_data(factory: &mut ModuleFactory, tag: &str) -> Arc<dyn ModuleData> {
    let mut ini = INI::new();
    ini.with_inline_source(
        "MaxHealth = 500\nInitialHealth = 450\nSubdualDamageCap = 90\n\
         SubdualDamageHealRate = 800\nSubdualDamageHealAmount = 6\nEnd\n",
        |ini| {
            Ok(
                factory.new_module_data_from_ini(
                    Some(ini),
                    "ActiveBody",
                    ModuleType::Behavior,
                    tag,
                ),
            )
        },
    )
    .unwrap()
    .unwrap()
}

fn definition(data: &dyn ModuleData) -> &ActiveBodyModuleData {
    data.as_any()
        .downcast_ref()
        .expect("registered ActiveBody data")
}

fn assert_rules(data: &ActiveBodyModuleData, tag: NameKeyType) {
    assert_eq!(data.get_module_tag_name_key(), tag);
    assert_eq!(data.max_health, 500.0);
    assert_eq!(data.initial_health, 450.0);
    assert_eq!(data.subdual_damage_cap, 90.0);
    assert_eq!(data.subdual_damage_heal_rate, 24);
    assert_eq!(data.subdual_damage_heal_amount, 6.0);
    assert!(data.default_armor_template.is_none());
}

#[test]
fn registered_active_definition_save_has_no_configuration_payload() {
    let mut factory = ModuleFactory::new();
    register_module_overrides(&mut factory).unwrap();
    let shared = authored_data(&mut factory, "DefinitionSnapshotSave");
    let retained = Arc::clone(&shared);
    let tag = shared.get_module_tag_name_key();
    assert_rules(definition(shared.as_ref()), tag);
    let mut owned = definition(shared.as_ref()).clone();
    owned.default_armor_template = Some("DefinitionOnlyArmor".into());
    let mut bytes = Vec::new();
    owned
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    assert!(
        bytes.is_empty(),
        "C++ ActiveBodyModuleData inherits empty Xfer"
    );
    assert_eq!(
        owned.default_armor_template.as_ref().unwrap().as_str(),
        "DefinitionOnlyArmor"
    );
    assert!(Arc::ptr_eq(&shared, &retained));
    assert_rules(definition(retained.as_ref()), tag);
}

#[test]
fn registered_active_definition_load_consumes_no_runtime_record() {
    let mut factory = ModuleFactory::new();
    register_module_overrides(&mut factory).unwrap();
    let shared = authored_data(&mut factory, "DefinitionSnapshotLoad");
    let tag = shared.get_module_tag_name_key();
    let mut owned = definition(shared.as_ref()).clone();
    owned.default_armor_template = Some("ReceivingDefinitionArmor".into());
    // Enough bytes for the former invented data payload, so OLD reaches the
    // zero-consumption assertion rather than failing on an unrelated EOF.
    let sentinel = 0x7856_3401_u32;
    let mut bytes = sentinel.to_le_bytes().to_vec();
    bytes.resize(64, 0);
    let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
    owned.xfer(&mut xfer).unwrap();
    assert_eq!(xfer.bytes_read(), 0, "C++ definition hook reads no state");
    owned.load_post_process().unwrap();
    let mut next = 0;
    xfer.xfer_unsigned_int(&mut next).unwrap();
    assert_eq!(next, sentinel);
    assert_eq!(owned.max_health, 500.0);
    assert_eq!(owned.initial_health, 450.0);
    assert_eq!(owned.subdual_damage_cap, 90.0);
    assert_eq!(owned.subdual_damage_heal_rate, 24);
    assert_eq!(owned.subdual_damage_heal_amount, 6.0);
    assert_eq!(owned.get_module_tag_name_key(), tag);
    assert_eq!(
        owned.default_armor_template.as_ref().unwrap().as_str(),
        "ReceivingDefinitionArmor"
    );
    assert_rules(definition(shared.as_ref()), tag);
}

#[test]
fn registered_active_definition_crc_keeps_shared_rules_and_empty_crc() {
    let mut factory = ModuleFactory::new();
    register_module_overrides(&mut factory).unwrap();
    let first = authored_data(&mut factory, "DefinitionSnapshotCRC1");
    let second = authored_data(&mut factory, "DefinitionSnapshotCRC2");
    let retained = Arc::clone(&first);
    let mut xfer = XferCRC::new(XferSave::new(Cursor::new(Vec::new()), 1));
    let mut sentinel = 0x1234_5678;
    xfer.xfer_unsigned_int(&mut sentinel).unwrap();
    let before = xfer.get_crc();
    factory.crc(&mut xfer).unwrap();
    assert_eq!(xfer.get_crc(), before, "C++ inherited data CRC is empty");
    assert!(Arc::ptr_eq(&first, &retained));
    assert_rules(definition(first.as_ref()), first.get_module_tag_name_key());
    assert_rules(
        definition(second.as_ref()),
        second.get_module_tag_name_key(),
    );
}
