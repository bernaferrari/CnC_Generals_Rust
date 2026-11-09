use super::*;

#[derive(Default)]
struct Receiver {
    fail_template: Option<&'static str>,
    missing_model_bone: bool,
    events: Vec<String>,
    frame: Option<u32>,
    reentrant_data: Option<game_engine::common::thing::module::ModuleDataHandle>,
    configured: Vec<(u32, u32, i32, Matrix3D, bool)>,
}
impl ClientParticleStateRuntime for Receiver {
    fn standard_public_bones(&self) -> Vec<String> {
        Vec::new()
    }
    fn pristine_bone(
        &mut self,
        _model: &str,
        _scale: f32,
        _frame: i32,
        name: &str,
    ) -> Option<(i32, Matrix3D)> {
        (name == "fxbone").then_some((
            3,
            Matrix3D::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
        ))
    }
    fn sub_object_names(&mut self, _model: &str) -> Vec<String> {
        Vec::new()
    }
    fn destroy_particle_system(&mut self, id: u32) {
        self.events.push(format!("destroy:{id}"));
    }
    fn set_particle_system_running(&mut self, id: u32, running: bool) {
        self.events.push(format!("running:{id}:{running}"));
    }
    fn create_particle_system(
        &mut self,
        template: &str,
        frame: u32,
    ) -> Result<Option<u32>, String> {
        self.frame = Some(frame);
        self.events.push(format!("create:{template}"));
        if self.fail_template == Some(template) {
            return Err("receiving manager rejected creation".into());
        }
        Ok(Some(41))
    }
    fn model_space_bone(&mut self, model: &str, bone: &str) -> Option<(i32, Matrix3D)> {
        if let Some(data) = &self.reentrant_data {
            let data = data.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let mut guard = data.write();
                guard.set_module_tag_name_key(123);
                let _ = tx.send(());
            });
            rx.recv_timeout(std::time::Duration::from_secs(2))
                .expect("model-space callback must not hold the model-data read lock");
        }
        assert_eq!(model, "AuthoredModel");
        assert_eq!(bone, "FXBONE");
        if self.missing_model_bone {
            return None;
        }
        Some((
            3,
            Matrix3D::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
        ))
    }
    fn configure_particle_system(
        &mut self,
        id: u32,
        drawable: u32,
        index: i32,
        local: &Matrix3D,
        hidden: bool,
    ) {
        self.events.push(format!("configure:{id}"));
        self.configured.push((id, drawable, index, *local, hidden));
    }
}
fn model() -> W3DModelDraw {
    let mut data = W3DModelDrawModuleData::new();
    let mut state = ModelConditionInfo::new();
    state.model_name = AsciiString::from("AuthoredModel");
    state.conditions_yes.push(ModelConditionFlags::empty());
    state.public_bones.push(AsciiString::from("FXBONE"));
    state.particle_sys_bones.push(ParticleSysBoneInfo {
        bone_name: AsciiString::from("FXBONE"),
        particle_system: AsciiString::from("Smoke"),
    });
    data.condition_states.push(state);
    W3DModelDraw::new(data)
}
fn context(frame: u32) -> NativeStateParticleContext {
    NativeStateParticleContext {
        conditions: ModelConditionFlags::empty(),
        instance_scale: 1.0,
        state_particles: true,
        drawable_id: 91,
        hidden: false,
        fully_obscured: false,
        creation_frame: frame,
    }
}
#[test]
fn model_space_callback_can_write_shared_model_data() {
    let mut draw = model();
    let mut runtime = Receiver {
        reentrant_data: Some(draw.data.clone()),
        ..Receiver::default()
    };
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&context(37), &mut runtime),
        Ok(NativeStateParticlePreparation::Prepared(true))
    );
    assert_eq!(draw.data.read().get_module_tag_name_key(), 123);
    draw.particle_systems.clear();
}

#[test]
fn receiving_static_state_creates_installed_trackers_at_actual_client_frame_once() {
    let mut draw = model();
    let mut runtime = Receiver::default();
    let mut facts = context(37);
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&facts, &mut runtime),
        Ok(NativeStateParticlePreparation::Prepared(true))
    );
    assert_eq!(runtime.frame, Some(37));
    assert_eq!(runtime.events, ["create:Smoke", "configure:41"]);
    assert_eq!(runtime.configured[0].1, 91);
    assert_eq!(runtime.configured[0].2, 3);
    assert!(!runtime.configured[0].4);
    assert_eq!(draw.particle_systems.len(), 1);
    assert_eq!(draw.particle_systems[0].id, 41);
    facts.creation_frame = 38;
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&facts, &mut runtime),
        Ok(NativeStateParticlePreparation::Prepared(true))
    );
    assert_eq!(runtime.configured.len(), 1);
    facts.hidden = true;
    draw.prepare_state_particles_with_receiver(&facts, &mut runtime)
        .unwrap();
    assert_eq!(runtime.events.last().unwrap(), "running:41:false");
    let mut peer = model();
    let mut peer_runtime = Receiver::default();
    peer.prepare_state_particles_with_receiver(&context(900), &mut peer_runtime)
        .unwrap();
    assert_eq!(peer_runtime.frame, Some(900));
    assert_eq!(runtime.frame, Some(37));
    draw.particle_systems.clear();
    peer.particle_systems.clear();
}
#[test]
fn receiving_state_rejects_unowned_terrain_before_mutating_installed_runtime() {
    let mut draw = model();
    draw.model_data_mut().track_file = AsciiString::from("AuthoredTrack");
    let mut runtime = Receiver::default();
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&context(7), &mut runtime),
        Ok(NativeStateParticlePreparation::Unsupported {
            animation_pose: false,
            terrain_tracks: true
        })
    );
    assert!(draw.cur_state.is_none());
    assert!(draw.particle_systems.is_empty());
    assert!(runtime.events.is_empty());
}

#[test]
fn receiving_visibility_and_retirement_touch_only_installed_emitters() {
    let mut draw = model();
    let mut runtime = Receiver::default();
    let mut hidden = context(37);
    hidden.hidden = true;
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&hidden, &mut runtime),
        Ok(NativeStateParticlePreparation::Prepared(false))
    );
    assert!(draw.cur_state.is_none());
    assert!(runtime.events.is_empty());
    draw.prepare_state_particles_with_receiver(&context(37), &mut runtime)
        .unwrap();
    assert_eq!(draw.native_state_particle_count(), 1);
    draw.set_native_particle_visibility(true, false, &mut runtime);
    draw.set_native_particle_visibility(false, true, &mut runtime);
    draw.set_native_particle_visibility(false, false, &mut runtime);
    assert_eq!(
        &runtime.events[2..],
        ["running:41:false", "running:41:true"]
    );
    draw.retire_native_state_particles(&mut runtime);
    assert_eq!(draw.native_state_particle_count(), 0);
    assert_eq!(runtime.events.last().unwrap(), "destroy:41");
    let count = runtime.events.len();
    draw.retire_native_state_particles(&mut runtime);
    assert_eq!(runtime.events.len(), count);
}

#[test]
fn receiving_non_emitter_models_do_not_enter_state_particle_continuation() {
    let mut draw = model();
    draw.model_data_mut().condition_states[0].particle_sys_bones.clear();
    draw.model_data_mut().track_file = AsciiString::from("UnrelatedAuthoredTrack");
    let mut runtime = Receiver::default();
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&context(37), &mut runtime),
        Ok(NativeStateParticlePreparation::Prepared(false))
    );
    assert!(draw.cur_state.is_none());
    assert!(runtime.events.is_empty());
}

#[test]
fn receiving_factory_error_stops_before_later_rows_and_retains_recalc_request() {
    let mut draw = model();
    draw.model_data_mut().condition_states[0]
        .particle_sys_bones
        .push(ParticleSysBoneInfo {
            bone_name: AsciiString::from("FXBONE"),
            particle_system: AsciiString::from("LaterRow"),
        });
    let mut runtime = Receiver {
        fail_template: Some("Smoke"),
        ..Receiver::default()
    };
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&context(37), &mut runtime),
        Err("receiving manager rejected creation".into())
    );
    assert_eq!(runtime.events, ["create:Smoke"]);
    assert!(runtime.configured.is_empty());
    assert!(draw.particle_systems.is_empty());
    assert!(draw.need_recalc_bone_particle_systems);
}

#[test]
fn receiving_successful_prefix_is_not_duplicated_after_later_factory_error() {
    let mut draw = model();
    draw.model_data_mut().condition_states[0]
        .particle_sys_bones
        .push(ParticleSysBoneInfo {
            bone_name: AsciiString::from("FXBONE"),
            particle_system: AsciiString::from("LaterRow"),
        });
    let mut runtime = Receiver {
        fail_template: Some("LaterRow"),
        ..Receiver::default()
    };
    let error = Err("receiving manager rejected creation".into());
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&context(37), &mut runtime),
        error
    );
    assert_eq!(
        runtime.events,
        ["create:Smoke", "configure:41", "create:LaterRow"]
    );
    assert_eq!(draw.native_state_particle_count(), 1);
    let events = runtime.events.clone();
    runtime.fail_template = None;
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&context(38), &mut runtime),
        error
    );
    assert_eq!(runtime.events, events);
    assert_eq!(runtime.frame, Some(37));
    let mut bytes = Vec::new();
    {
        let mut save = game_engine::common::system::xfer_save::XferSave::new(
            std::io::Cursor::new(&mut bytes),
            1,
        );
        assert!(
            draw.xfer(&mut save)
                .unwrap_err()
                .starts_with("cannot save failed native state-particle admission:")
        );
    }
    assert!(bytes.is_empty());
    draw.retire_native_state_particles(&mut runtime);
    assert_eq!(runtime.events.last().unwrap(), "destroy:41");
    assert!(draw.native_state_particle_failure.is_none());
    assert_eq!(draw.native_state_particle_count(), 0);
    draw.model_data_mut().condition_states[0].particle_sys_bones.pop();
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&context(39), &mut runtime),
        Ok(NativeStateParticlePreparation::Prepared(true))
    );
    assert_eq!(runtime.frame, Some(39));
    assert_eq!(draw.native_state_particle_count(), 1);
    draw.retire_native_state_particles(&mut runtime);
}

#[test]
fn receiving_actual_state_transition_retires_failed_prefix_before_new_birth() {
    let mut draw = model();
    draw.model_data_mut().condition_states[0]
        .particle_sys_bones
        .push(ParticleSysBoneInfo {
            bone_name: AsciiString::from("FXBONE"),
            particle_system: AsciiString::from("LaterRow"),
        });
    let mut damaged = draw.model_data().condition_states[0].clone();
    damaged.conditions_yes = vec![ModelConditionFlags::DAMAGED];
    damaged.particle_sys_bones.pop();
    draw.model_data_mut().condition_states.push(damaged);
    let mut runtime = Receiver {
        fail_template: Some("LaterRow"),
        ..Receiver::default()
    };
    assert!(
        draw.prepare_state_particles_with_receiver(&context(37), &mut runtime)
            .is_err()
    );
    let mut facts = context(38);
    facts.conditions = ModelConditionFlags::DAMAGED;
    assert_eq!(
        draw.prepare_state_particles_with_receiver(&facts, &mut runtime),
        Ok(NativeStateParticlePreparation::Prepared(true))
    );
    assert_eq!(
        &runtime.events[3..],
        ["destroy:41", "create:Smoke", "configure:41"]
    );
    assert!(draw.native_state_particle_failure.is_none());
    assert_eq!(draw.native_state_particle_count(), 1);
    draw.retire_native_state_particles(&mut runtime);
}

#[test]
fn receiving_missing_model_bone_does_not_adopt_a_pristine_cache_transform() {
    let mut draw = model();
    draw.model_data_mut().condition_states[0].pristine_bones.insert(
        name_key_generate("FXBONE"),
        PristineBoneInfo {
            bone_index: 19,
            transform: Matrix3D::from_translation(glam::Vec3::splat(999.)),
        },
    );
    let mut receiver = Receiver {
        missing_model_bone: true,
        ..Default::default()
    };
    draw.prepare_state_particles_with_receiver(&context(37), &mut receiver)
        .unwrap();
    let configured = receiver.configured.clone();
    draw.particle_systems.clear();
    assert_eq!(configured.len(), 1);
    assert_eq!(
        configured[0].2, 0,
        "C++ missing bone uses index0, not another cache"
    );
    assert_eq!(configured[0].3, Matrix3D::IDENTITY);
}

#[test]
fn receiving_missing_animation_and_track_capabilities_are_typed_and_inert() {
    for terrain_tracks in [false, true] {
        let mut draw = model();
        draw.model_data_mut().condition_states[0]
            .animations
            .push(W3DAnimationInfo::new(
                AsciiString::from("AuthoredAnimation"),
                false,
                0.0,
            ));
        if terrain_tracks {
            draw.model_data_mut().track_file = AsciiString::from("AuthoredTrack");
        }
        let mut runtime = Receiver::default();
        let before_conditions = draw.last_model_conditions;
        assert_eq!(
            draw.prepare_state_particles_with_receiver(&context(7), &mut runtime),
            Ok(NativeStateParticlePreparation::Unsupported {
                animation_pose: true,
                terrain_tracks
            })
        );
        assert!(draw.cur_state.is_none());
        assert!(!draw.need_recalc_bone_particle_systems);
        assert_eq!(draw.last_model_conditions, before_conditions);
        assert!(draw.native_state_particle_failure.is_none());
        assert!(draw.particle_systems.is_empty());
        assert!(runtime.events.is_empty());
        assert_eq!(runtime.frame, None);
    }
}

#[test]
fn receiver_validation_exposes_incremental_cache_without_holding_definition_lock() {
    struct Observer {
        data: game_engine::common::thing::module::ModuleDataHandle,
    }
    impl Observer {
        fn inspect(&self, f: impl FnOnce(&mut W3DModelDrawModuleData) + Send + 'static) {
            let data = self.data.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let mut guard = data.write();
                f(project_model_data_mut(&mut *guard));
                let _ = tx.send(());
            });
            rx.recv_timeout(std::time::Duration::from_secs(2))
                .expect("validation callback must permit reentrant model-data writes");
        }
    }
    impl ClientParticleStateRuntime for Observer {
        fn standard_public_bones(&self) -> Vec<String> {
            self.inspect(|data| {
                assert_ne!(data.condition_states[0].valid_stuff & MODEL_CONDITION_PUBLIC_BONES_VALID, 0);
            });
            vec!["root".into()]
        }
        fn pristine_bone(&mut self, _: &str, _: f32, _: i32, name: &str) -> Option<(i32, Matrix3D)> {
            if name == "root01" {
                self.inspect(|data| {
                    let state = &data.condition_states[0];
                    assert_ne!(state.valid_stuff & MODEL_CONDITION_PRISTINE_BONES_VALID, 0);
                    assert_eq!(state.pristine_bones.get(&11).map(|bone| bone.bone_index), Some(7));
                });
            }
            (name == "root").then_some((7, Matrix3D::IDENTITY))
        }
        fn name_to_key(&mut self, name: &str) -> NameKeyType {
            if name == "ROOT01" {
                self.inspect(|data| {
                    let state = &mut data.condition_states[0];
                    assert_ne!(state.valid_stuff & MODEL_CONDITION_TURRETS_VALID, 0);
                    state.pristine_bones.insert(12, PristineBoneInfo {
                        bone_index: 9,
                        transform: Matrix3D::IDENTITY,
                    });
                });
                12
            } else if name == "root" {
                11
            } else {
                13
            }
        }
        fn sub_object_names(&mut self, _: &str) -> Vec<String> { Vec::new() }
        fn destroy_particle_system(&mut self, _: u32) {}
        fn set_particle_system_running(&mut self, _: u32, _: bool) {}
        fn create_particle_system(&mut self, _: &str, _: u32) -> Result<Option<u32>, String> { Ok(None) }
        fn model_space_bone(&mut self, _: &str, _: &str) -> Option<(i32, Matrix3D)> { None }
        fn configure_particle_system(&mut self, _: u32, _: u32, _: i32, _: &Matrix3D, _: bool) {}
    }

    let mut data = W3DModelDrawModuleData::new();
    let mut state = ModelConditionInfo::new();
    state.model_name = AsciiString::from("model");
    state.weapon_recoil_bone[0] = AsciiString::from("ROOT");
    data.condition_states.push(state);
    let handle = game_engine::common::thing::module::ModuleDataHandle::new(data);
    let draw = W3DModelDraw::new_with_module_data(handle.clone());
    let mut receiver = Observer { data: handle };
    draw.validate_state_with_receiver(ActiveModelState::Condition(0), &[], 1.0, &mut receiver);
    let state = draw.resolve_state(ActiveModelState::Condition(0)).unwrap();
    assert_eq!(state.weapon_barrels[0][0].recoil_bone, 9);
}
