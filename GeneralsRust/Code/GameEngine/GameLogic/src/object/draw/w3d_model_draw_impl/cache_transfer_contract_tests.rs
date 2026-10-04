use super::*;

struct CacheContractWrites<'a> {
    cursor: std::io::Cursor<&'a mut Vec<u8>>,
    widths: &'a mut Vec<usize>,
}

impl std::io::Write for CacheContractWrites<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.widths.push(bytes.len());
        std::io::Write::write(&mut self.cursor, bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.cursor)
    }
}

impl std::io::Seek for CacheContractWrites<'_> {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        std::io::Seek::seek(&mut self.cursor, position)
    }
}

// These CPU codec controls use an owned cache record. They never invoke
// ModuleFactory::xfer on shared Arcs or claim installed cache restoration.
fn cache_contract_matrix(first: f32) -> Matrix3D {
    Matrix3D::from_cols_array(&[
        first,
        first + 4.0,
        first + 8.0,
        0.0,
        first + 1.0,
        first + 5.0,
        first + 9.0,
        0.0,
        first + 2.0,
        first + 6.0,
        first + 10.0,
        0.0,
        first + 3.0,
        first + 7.0,
        first + 11.0,
        1.0,
    ])
}

fn cache_contract_definition() -> W3DModelDrawModuleData {
    assert_eq!(MAX_TURRETS, 2, "C++ W3DModelDraw.h MAX_TURRETS");
    assert_eq!(WEAPONSLOT_COUNT, 3, "C++ WeaponSlotType slot count");
    let mut data = W3DModelDrawModuleData::new();
    data.module_tag_name_key = 123;
    data.track_file = AsciiString::from("authored-track");
    let mut state = ModelConditionInfo::new();
    state.model_name = AsciiString::from("authored-model");
    state.valid_stuff = 0x0b;
    // Insert out of order: C++ std::map transfers ascending NameKey values.
    state.pristine_bones.insert(
        7,
        PristineBoneInfo {
            bone_index: 9,
            transform: cache_contract_matrix(21.0),
        },
    );
    state.pristine_bones.insert(
        3,
        PristineBoneInfo {
            bone_index: 5,
            transform: cache_contract_matrix(1.0),
        },
    );
    for (angle, pitch) in [(11, 12), (21, 22)] {
        let mut turret = TurretInfo::new();
        turret.turret_angle_bone = angle;
        turret.turret_pitch_bone = pitch;
        state.turrets.push(turret);
    }
    for first in [41.0, 61.0] {
        let mut barrel = WeaponBarrelInfo::new();
        barrel.projectile_offset_mtx = cache_contract_matrix(first);
        state.weapon_barrels[0].push(barrel);
    }
    let mut barrel = WeaponBarrelInfo::new();
    barrel.projectile_offset_mtx = cache_contract_matrix(81.0);
    state.weapon_barrels[2].push(barrel);
    data.condition_states.push(state);
    let mut transition = ModelConditionInfo::new();
    transition.model_name = AsciiString::from("authored-transition");
    transition.valid_stuff = 0x01;
    transition.pristine_bones.insert(
        99,
        PristineBoneInfo {
            bone_index: 711,
            transform: cache_contract_matrix(101.0),
        },
    );
    data.transition_states.push(transition);
    data
}

fn cache_contract_cpp_bytes() -> Vec<u8> {
    // W3DModelDraw.cpp4259-4296 retail payload: version, condition valid
    // byte, sorted pristine bone index + raw 3x4 matrix, two turret pairs,
    // then slot/barrel order matrices. No counts, tags, names or transitions.
    let mut bytes = vec![1, 0x0b];
    for (bone, first) in [(5_i32, 1_u32), (9_i32, 21_u32)] {
        bytes.extend_from_slice(&bone.to_le_bytes());
        for value in first..first + 12 {
            bytes.extend_from_slice(&(value as f32).to_le_bytes());
        }
    }
    for bone in [11_i32, 12, 21, 22] {
        bytes.extend_from_slice(&bone.to_le_bytes());
    }
    for first in [41_u32, 61, 81] {
        for value in first..first + 12 {
            bytes.extend_from_slice(&(value as f32).to_le_bytes());
        }
    }
    bytes
}

#[test]
fn w3d_cache_transfer_contract_crc_matches_cpp_payload_without_mutation() {
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    let data = cache_contract_definition();
    let mut bytes = Vec::new();
    let mut widths = Vec::new();
    let writer = CacheContractWrites {
        cursor: Cursor::new(&mut bytes),
        widths: &mut widths,
    };
    let mut save = XferSave::new(writer, 1);
    save.open("owned-w3d-cache-crc").unwrap();
    data.crc(&mut save).unwrap();
    save.close().unwrap();
    drop(save);
    assert_eq!(bytes, cache_contract_cpp_bytes());
    let mut expected_widths = vec![1, 1];
    // Two bone indices + 24 pristine floats + four turret integers +
    // 36 projectile floats. Each current scalar transfer writes separately.
    expected_widths.extend([4; 66]);
    assert_eq!(
        widths, expected_widths,
        "preserve actual transfer call boundaries"
    );
    // XferCRC.cpp74-86/116-142/155-159: original rotate/add fold,
    // preserving the separate one-byte version/valid transfer calls.
    let inner = XferSave::new(Cursor::new(Vec::<u8>::new()), 1);
    let mut crc = game_engine::common::system::xfer_crc::XferCRC::new(inner);
    crc.open("owned-w3d-cache-real-crc").unwrap();
    data.crc(&mut crc).unwrap();
    assert_eq!(crc.get_crc(), 0x7ccc_e248);
    crc.close().unwrap();
    assert_eq!(data.module_tag_name_key, 123);
    assert_eq!(data.track_file.as_str(), "authored-track");
    assert_eq!(data.condition_states[0].valid_stuff, 0x0b);
    assert_eq!(data.condition_states[0].pristine_bones[&3].bone_index, 5);
    assert_eq!(
        data.condition_states[0].pristine_bones[&7]
            .transform
            .to_cols_array(),
        cache_contract_matrix(21.0).to_cols_array()
    );
    assert_eq!(
        data.transition_states[0].pristine_bones[&99].bone_index,
        711
    );
}

#[test]
fn w3d_cache_transfer_contract_owned_load_restores_only_condition_cache() {
    use game_engine::common::system::xfer_load::XferLoad;
    use std::io::Cursor;
    let mut data = cache_contract_definition();
    let state = &mut data.condition_states[0];
    for bone in state.pristine_bones.values_mut() {
        bone.bone_index = -1;
        bone.transform = Matrix3D::IDENTITY;
    }
    for turret in &mut state.turrets {
        turret.turret_angle_bone = -1;
        turret.turret_pitch_bone = -2;
    }
    for slot in &mut state.weapon_barrels {
        for barrel in slot {
            barrel.projectile_offset_mtx = Matrix3D::IDENTITY;
        }
    }
    let mut bytes = cache_contract_cpp_bytes();
    bytes.extend_from_slice(&0x11223344_u32.to_le_bytes());
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("owned-w3d-cache-load").unwrap();
    data.xfer(&mut load).unwrap();
    let mut sentinel = 0_u32;
    game_engine::common::system::xfer::Xfer::xfer_unsigned_int(&mut load, &mut sentinel).unwrap();
    assert_eq!(sentinel, 0x11223344, "exact cache stream boundary");
    load.close().unwrap();
    assert_eq!(data.module_tag_name_key, 123);
    assert_eq!(data.track_file.as_str(), "authored-track");
    let state = &data.condition_states[0];
    assert_eq!(state.model_name.as_str(), "authored-model");
    assert_eq!(state.valid_stuff, 0x0b);
    for (key, index, first) in [(3, 5, 1.0), (7, 9, 21.0)] {
        assert_eq!(state.pristine_bones[&key].bone_index, index);
        assert_eq!(
            state.pristine_bones[&key].transform.to_cols_array(),
            cache_contract_matrix(first).to_cols_array()
        );
    }
    assert_eq!(state.turrets[0].turret_angle_bone, 11);
    assert_eq!(state.turrets[0].turret_pitch_bone, 12);
    assert_eq!(state.turrets[1].turret_angle_bone, 21);
    assert_eq!(state.turrets[1].turret_pitch_bone, 22);
    for (slot, index, first) in [(0, 0, 41.0), (0, 1, 61.0), (2, 0, 81.0)] {
        assert_eq!(
            state.weapon_barrels[slot][index]
                .projectile_offset_mtx
                .to_cols_array(),
            cache_contract_matrix(first).to_cols_array()
        );
    }
    assert_eq!(
        data.transition_states[0].model_name.as_str(),
        "authored-transition"
    );
    assert_eq!(
        data.transition_states[0].pristine_bones[&99].bone_index,
        711
    );
}

#[test]
fn w3d_cache_transfer_contract_registered_derived_crc_has_no_extra_prefix() {
    use game_engine::common::ini::{INI, INIError};
    use game_engine::common::system::xfer_save::XferSave;
    use game_engine::common::thing::module::ModuleType;
    use game_engine::common::thing::module_factory::ModuleFactory;
    use std::io::Cursor;
    let mut factory = ModuleFactory::new();
    crate::contain_module_overrides::register_module_overrides(&mut factory).unwrap();
    for name in [
        "W3DModelDraw",
        "W3DDependencyModelDraw",
        "W3DTankDraw",
        "W3DOverlordTankDraw",
        "W3DOverlordAircraftDraw",
        "W3DOverlordTruckDraw",
        "W3DPoliceCarDraw",
        "W3DScienceModelDraw",
        "W3DSupplyDraw",
        "W3DTankTruckDraw",
        "W3DTruckDraw",
    ] {
        let mut ini = INI::new();
        let data = ini
            .with_inline_source("DefaultConditionState\nModel = None\nEnd\nEnd\n", |ini| {
                factory
                    .try_new_module_data_from_ini(
                        Some(ini),
                        name,
                        ModuleType::Draw,
                        "CacheContractDraw",
                    )
                    .ok_or(INIError::InvalidData)
            })
            .unwrap();
        let alias = std::sync::Arc::clone(&data);
        let tag = data.get_module_tag_name_key();
        let mut bytes = Vec::new();
        let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
        save.open("registered-w3d-cache-crc").unwrap();
        data.crc(&mut save).unwrap();
        save.close().unwrap();
        drop(save);
        assert_eq!(
            bytes,
            [1, 0],
            "{name}: one inherited condition-cache payload"
        );
        assert!(std::sync::Arc::ptr_eq(&data, &alias));
        assert_eq!(alias.get_module_tag_name_key(), tag);
    }
}
