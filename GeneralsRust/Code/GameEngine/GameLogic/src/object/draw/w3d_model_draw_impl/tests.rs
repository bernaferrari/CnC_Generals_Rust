#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_sub_object_is_case_insensitive() {
        let mut draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
        draw.show_sub_object("Gun_Barrel", true);
        draw.show_sub_object("gun_barrel", false);
        draw.update_sub_objects();

        assert_eq!(draw.sub_object_vec.len(), 1);
        assert_eq!(draw.sub_object_vec[0].sub_obj_name.as_str(), "gun_barrel");
        assert!(draw.sub_object_vec[0].hide);
    }

    #[test]
    fn update_sub_objects_deduplicates_by_normalized_name() {
        let mut draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
        draw.sub_object_vec.push(HideShowSubObjInfo {
            sub_obj_name: AsciiString::from("Wheel_L"),
            hide: false,
        });
        draw.sub_object_vec.push(HideShowSubObjInfo {
            sub_obj_name: AsciiString::from("wheel_l"),
            hide: true,
        });
        draw.sub_objects_dirty = true;

        draw.update_sub_objects();

        assert_eq!(draw.sub_object_vec.len(), 1);
        assert_eq!(draw.sub_object_vec[0].sub_obj_name.as_str(), "wheel_l");
        assert!(draw.sub_object_vec[0].hide);
        assert!(!draw.sub_objects_dirty);
    }

    #[test]
    fn module_name_key_is_model_draw() {
        let draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
        assert_eq!(
            draw.get_module_name_key(),
            NameKeyGenerator::name_to_key("W3DModelDraw")
        );
    }

    #[test]
    fn react_to_geometry_change_is_noop_like_cpp() {
        let mut draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
        draw.need_recalc_bone_particle_systems = false;

        draw.react_to_geometry_change();

        assert!(!draw.need_recalc_bone_particle_systems);
    }

    #[test]
    fn state_change_stops_client_particle_trackers_immediately() {
        let mut data = W3DModelDrawModuleData::new();
        data.condition_states.push(ModelConditionInfo::new());
        data.condition_states.push(ModelConditionInfo::new());
        let mut draw = W3DModelDraw::new(data);
        draw.cur_state = Some(ActiveModelState::Condition(0));
        draw.particle_systems.push(ParticleSysTracker {
            id: 41,
            bone_index: 3,
            bone_name: AsciiString::from("FXBONE"),
        });

        draw.set_model_state(1);

        assert!(draw.particle_systems.is_empty());
        assert!(draw.need_recalc_bone_particle_systems);
    }

    #[test]
    fn particle_bone_update_returns_true_like_cpp_even_without_owner() {
        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.particle_sys_bones.push(ParticleSysBoneInfo {
            bone_name: AsciiString::from("FXBONE"),
            particle_system: AsciiString::from("Dust"),
        });
        data.condition_states.push(state);
        let mut draw = W3DModelDraw::new(data);
        draw.cur_state = Some(ActiveModelState::Condition(0));

        assert!(draw.update_bones_for_client_particle_systems());
    }

    #[test]
    fn particle_bone_matrix_helpers_use_cpp_translation_and_z_rotation() {
        let angle = 0.7_f32;
        let (sin_a, cos_a) = angle.sin_cos();
        let matrix = Matrix3D::from_cols_array(&[
            cos_a, sin_a, 0.0, 0.0, -sin_a, cos_a, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 9.0, 8.0, 7.0, 1.0,
        ]);

        assert_eq!(
            W3DModelDraw::matrix_translation(&matrix),
            Coord3D::new(9.0, 8.0, 7.0)
        );
        assert!((W3DModelDraw::matrix_z_rotation(&matrix) - angle).abs() < 0.0001);
    }

    #[test]
    fn model_condition_valid_stuff_preserves_cpp_bit_layout() {
        let mut state = ModelConditionInfo::new();
        state.pristine_bones.insert(
            1,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 7,
            },
        );
        state.turrets.push(TurretInfo::new());
        state.weapon_projectile_launch_bone[0] = AsciiString::from("MUZZLE");
        state.weapon_barrels[0].push(WeaponBarrelInfo::new());
        state.public_bones.push(AsciiString::from("HEAD"));

        assert_eq!(
            model_condition_valid_stuff(&state),
            MODEL_CONDITION_PRISTINE_BONES_VALID
                | MODEL_CONDITION_TURRETS_VALID
                | MODEL_CONDITION_HAS_PROJECTILE_BONES
                | MODEL_CONDITION_BARRELS_VALID
                | MODEL_CONDITION_PUBLIC_BONES_VALID
        );
    }

    #[test]
    fn projectile_launch_bone_updates_stored_valid_stuff_bit() {
        let mut state = ModelConditionInfo::new();
        state.weapon_projectile_launch_bone[0] = AsciiString::from("muzzle");

        state.refresh_projectile_valid_bit();

        assert_eq!(
            state.valid_stuff & MODEL_CONDITION_HAS_PROJECTILE_BONES,
            MODEL_CONDITION_HAS_PROJECTILE_BONES
        );
    }

    #[test]
    fn runtime_validation_merges_extra_public_bones_once() {
        let mut state = ModelConditionInfo::new();
        let extra = [AsciiString::from("extra_bone")];

        state.validate_runtime_caches(&extra);
        state.validate_runtime_caches(&extra);

        assert_eq!(
            state.valid_stuff & MODEL_CONDITION_PUBLIC_BONES_VALID,
            MODEL_CONDITION_PUBLIC_BONES_VALID
        );
        assert_eq!(
            state
                .public_bones
                .iter()
                .filter(|bone| bone.as_str() == "extra_bone")
                .count(),
            1
        );
    }

    #[test]
    fn matrix3d_user_xfer_uses_cpp_raw_3x4_row_layout() {
        use game_engine::common::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut matrix = Matrix3D::from_cols_array(&[
            1.0, 5.0, 9.0, 13.0, 2.0, 6.0, 10.0, 14.0, 3.0, 7.0, 11.0, 15.0, 4.0, 8.0, 12.0, 16.0,
        ]);

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("w3d_model_draw_matrix3d_user").unwrap();
            xfer_matrix3d_values(&mut save, &mut matrix).unwrap();
            save.close().unwrap();
        }

        assert_eq!(bytes.len(), 12 * std::mem::size_of::<f32>());
        for (index, expected) in (1..=12).map(|value| value as f32).enumerate() {
            let start = index * std::mem::size_of::<f32>();
            assert_eq!(&bytes[start..start + 4], &expected.to_le_bytes());
        }
    }

    #[test]
    fn matrix3d_user_xfer_load_restores_glam_bottom_row() {
        use game_engine::common::system::xfer_load::XferLoad;
        use std::io::Cursor;

        let bytes = (1..=12)
            .flat_map(|value| (value as f32).to_le_bytes())
            .collect::<Vec<_>>();
        let mut matrix = Matrix3D::from_cols_array(&[99.0; 16]);
        let mut load = XferLoad::new(Cursor::new(bytes), 1);

        load.open("w3d_model_draw_matrix3d_user").unwrap();
        xfer_matrix3d_values(&mut load, &mut matrix).unwrap();
        load.close().unwrap();

        assert_eq!(
            matrix.to_cols_array(),
            [1.0, 5.0, 9.0, 0.0, 2.0, 6.0, 10.0, 0.0, 3.0, 7.0, 11.0, 0.0, 4.0, 8.0, 12.0, 1.0]
        );
    }

    #[test]
    fn exposes_object_draw_interface_from_draw_module() {
        let mut draw = W3DModelDraw::new(W3DModelDrawModuleData::new());

        assert!(DrawModule::get_object_draw_interface(&draw).is_some());
        assert!(DrawModule::get_object_draw_interface_mut(&mut draw).is_some());
    }

    #[test]
    fn bound_model_draw_initializes_best_empty_condition_state() {
        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        data.condition_states.push(state);
        let mut draw = W3DModelDraw::new(data);

        draw.on_drawable_bound_to_object();

        assert_eq!(draw.cur_state, Some(ActiveModelState::Condition(0)));
    }

    #[test]
    fn bone_overrides_use_cpp_pitch_and_recoil_signs() {
        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.turrets.push(TurretInfo {
            turret_pitch_bone: 11,
            turret_art_pitch: 0.25,
            ..TurretInfo::new()
        });
        state.weapon_barrels[0].push(WeaponBarrelInfo {
            recoil_bone: 12,
            ..WeaponBarrelInfo::new()
        });
        data.condition_states.push(state);
        let mut draw = W3DModelDraw::new(data);
        draw.cur_state = Some(ActiveModelState::Condition(0));
        draw.weapon_recoil_info[0].push(WeaponRecoilInfo {
            state: RecoilState::Recoil,
            shift: 0.5,
            recoil_rate: 0.0,
        });

        let overrides = draw.collect_bone_overrides();

        let pitch = overrides
            .iter()
            .find(|override_state| override_state.bone_index == 11)
            .expect("pitch override");
        let recoil = overrides
            .iter()
            .find(|override_state| override_state.bone_index == 12)
            .expect("recoil override");

        let pitch_cols = pitch.transform.to_cols_array();
        assert!(pitch_cols[2] < 0.0);
        assert_eq!(recoil.transform.w_axis.x, -0.5);
    }

    #[test]
    fn weapon_fire_fx_starts_recoil_but_does_not_fabricate_handled_fx() {
        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.weapon_barrels[0].push(WeaponBarrelInfo {
            fx_bone: 7,
            recoil_bone: 12,
            muzzle_flash_bone: 13,
            ..WeaponBarrelInfo::new()
        });
        data.condition_states.push(state);
        data.initial_recoil = 1.75;

        let mut draw = W3DModelDraw::new(data);
        draw.cur_state = Some(ActiveModelState::Condition(0));
        draw.weapon_recoil_info[0].push(WeaponRecoilInfo::new());

        let handled = ObjectDrawInterface::handle_weapon_fire_fx(
            &mut draw,
            0,
            0,
            &Coord3D::new(1.0, 2.0, 3.0),
        );

        assert!(!handled);
        assert!(matches!(
            draw.weapon_recoil_info[0][0].state,
            RecoilState::RecoilStart
        ));
        assert_eq!(draw.weapon_recoil_info[0][0].recoil_rate, 1.75);
    }

    #[test]
    fn state_activation_populates_turret_bones_from_pristine_bones() {
        let turret_key = name_key_generate("turret");
        let pitch_key = name_key_generate("pitch");

        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.turrets.push(TurretInfo {
            turret_angle_name_key: turret_key,
            turret_pitch_name_key: pitch_key,
            ..TurretInfo::new()
        });
        state.pristine_bones.insert(
            turret_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 21,
            },
        );
        state.pristine_bones.insert(
            pitch_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 22,
            },
        );
        data.condition_states.push(state);

        let mut draw = W3DModelDraw::new(data);
        draw.set_model_state(0);

        let state = draw.current_state().expect("active state");
        assert_eq!(state.turrets[0].turret_angle_bone, 21);
        assert_eq!(state.turrets[0].turret_pitch_bone, 22);
    }

    #[test]
    fn state_activation_populates_numbered_weapon_barrels_like_cpp() {
        let fx01_key = name_key_generate("fx01");
        let recoil01_key = name_key_generate("recoil01");
        let muzzle01_key = name_key_generate("muzzle01");
        let muzzle02_key = name_key_generate("muzzle02");
        let projectile01_key = name_key_generate("projectile01");
        let projectile02_key = name_key_generate("projectile02");
        let projectile01 = Matrix3D::from_translation(glam::Vec3::new(1.0, 0.0, 0.0));
        let projectile02 = Matrix3D::from_translation(glam::Vec3::new(2.0, 0.0, 0.0));

        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.weapon_fire_fx_bone[0] = AsciiString::from("fx");
        state.weapon_recoil_bone[0] = AsciiString::from("recoil");
        state.weapon_muzzle_flash[0] = AsciiString::from("muzzle");
        state.weapon_projectile_launch_bone[0] = AsciiString::from("projectile");
        state.pristine_bones.insert(
            fx01_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 31,
            },
        );
        state.pristine_bones.insert(
            recoil01_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 32,
            },
        );
        state.pristine_bones.insert(
            muzzle01_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 33,
            },
        );
        state.pristine_bones.insert(
            muzzle02_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 34,
            },
        );
        state.pristine_bones.insert(
            projectile01_key,
            PristineBoneInfo {
                transform: projectile01,
                bone_index: 35,
            },
        );
        state.pristine_bones.insert(
            projectile02_key,
            PristineBoneInfo {
                transform: projectile02,
                bone_index: 36,
            },
        );
        data.condition_states.push(state);

        let mut draw = W3DModelDraw::new(data);
        draw.set_model_state(0);

        let barrels = &draw.current_state().expect("active state").weapon_barrels[0];
        assert_eq!(barrels.len(), 2);
        assert_eq!(barrels[0].fx_bone, 31);
        assert_eq!(barrels[0].recoil_bone, 32);
        assert_eq!(barrels[0].muzzle_flash_bone, 33);
        assert_eq!(barrels[0].projectile_offset_mtx.w_axis.x, 1.0);
        assert_eq!(barrels[1].fx_bone, 31);
        assert_eq!(barrels[1].muzzle_flash_bone, 34);
        assert_eq!(barrels[1].projectile_offset_mtx.w_axis.x, 2.0);
    }

    #[test]
    fn state_activation_uses_unadorned_weapon_bone_fallback() {
        let fx_key = name_key_generate("fx");
        let projectile_key = name_key_generate("projectile");
        let projectile = Matrix3D::from_translation(glam::Vec3::new(3.0, 0.0, 0.0));

        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.conditions_yes.push(ModelConditionFlags::empty());
        state.weapon_fire_fx_bone[0] = AsciiString::from("fx");
        state.weapon_projectile_launch_bone[0] = AsciiString::from("projectile");
        state.pristine_bones.insert(
            fx_key,
            PristineBoneInfo {
                transform: Matrix3D::IDENTITY,
                bone_index: 41,
            },
        );
        state.pristine_bones.insert(
            projectile_key,
            PristineBoneInfo {
                transform: projectile,
                bone_index: 42,
            },
        );
        data.condition_states.push(state);

        let mut draw = W3DModelDraw::new(data);
        draw.set_model_state(0);

        let barrels = &draw.current_state().expect("active state").weapon_barrels[0];
        assert_eq!(barrels.len(), 1);
        assert_eq!(barrels[0].fx_bone, 41);
        assert_eq!(barrels[0].projectile_offset_mtx.w_axis.x, 3.0);
    }

    #[test]
    fn construction_percent_z_delta_matches_cpp_translate_z() {
        assert_eq!(
            W3DModelDraw::construction_percent_z_delta(0.0, 40.0),
            Some(-40.0)
        );
        assert_eq!(
            W3DModelDraw::construction_percent_z_delta(25.0, 40.0),
            Some(-30.0)
        );
        assert_eq!(
            W3DModelDraw::construction_percent_z_delta(50.0, 40.0),
            Some(-20.0)
        );
        assert_eq!(
            W3DModelDraw::construction_percent_z_delta(100.0, 40.0),
            Some(0.0)
        );
        assert_eq!(
            W3DModelDraw::construction_percent_z_delta(-1.0, 40.0),
            None,
            "C++ CONSTRUCTION_COMPLETE = -1 skips the sink"
        );
    }

    #[test]
    fn partial_construction_lowers_local_z() {
        let mut mtx = Matrix3D::from_translation(Coord3D::new(10.0, 20.0, 50.0));
        let dz = W3DModelDraw::construction_percent_z_delta(25.0, 40.0).expect("partial");
        W3DModelDraw::translate_z(&mut mtx, dz);
        assert!((mtx.w_axis.x - 10.0).abs() < 1e-5);
        assert!((mtx.w_axis.y - 20.0).abs() < 1e-5);
        assert!(
            (mtx.w_axis.z - 20.0).abs() < 1e-5,
            "25% of height 40 sinks Z by 30 (50 -> 20), got {}",
            mtx.w_axis.z
        );
    }

    #[test]
    fn attach_to_drawable_bone_adds_rotated_pristine_offset() {
        let mut data = W3DModelDrawModuleData::new();
        data.attach_to_drawable_bone = AsciiString::from("turret");
        data.attach_to_drawable_bone_offset = Coord3D::new(4.0, 0.0, 2.0);

        let mut state = ModelConditionInfo::new();
        state.flags = 1u32 << ACBIT_ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT;
        data.condition_states.push(state);

        let mut draw = W3DModelDraw::new(data);
        draw.set_model_state(0);

        let yaw = std::f32::consts::FRAC_PI_2;
        let mtx = Matrix3D::from_rotation_z(yaw)
            * Matrix3D::from_translation(Coord3D::new(10.0, 20.0, 30.0));
        let out = draw.adjust_transform_mtx(&mtx);

        // Rotate_Vector((4,0,2)) about +Z 90° → (0,4,2), then add to translation.
        let expected = mtx.transform_vector3(Coord3D::new(4.0, 0.0, 2.0));
        assert!((out.w_axis.x - (mtx.w_axis.x + expected.x)).abs() < 1e-4);
        assert!((out.w_axis.y - (mtx.w_axis.y + expected.y)).abs() < 1e-4);
        assert!((out.w_axis.z - (mtx.w_axis.z + expected.z)).abs() < 1e-4);
    }

    #[test]
    fn construction_sink_without_owner_leaves_transform() {
        let mut data = W3DModelDrawModuleData::new();
        let mut state = ModelConditionInfo::new();
        state.flags = 1u32 << ACBIT_ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT;
        data.condition_states.push(state);
        let mut draw = W3DModelDraw::new(data);
        draw.set_model_state(0);

        let mtx = Matrix3D::from_translation(Coord3D::new(1.0, 2.0, 3.0));
        let out = draw.adjust_transform_mtx(&mtx);
        assert_eq!(out, mtx);
    }
}
