use super::*;

struct Receiver {
    events: Vec<String>,
    transform: Matrix3D,
    writes: Vec<(u32, Coord3D, f32, Matrix3D)>,
}
impl ClientParticleBoneRuntime for Receiver {
    fn particle_system_exists(&mut self, id: u32) -> bool {
        self.events.push(format!("find:{id}"));
        id != 7
    }
    fn bone_transform(&mut self, model: &str, bone_index: i32, name: &str) -> Option<Matrix3D> {
        self.events
            .push(format!("bone:{model}:{bone_index}:{name}"));
        (name != "MISSING").then_some(self.transform)
    }
    fn set_particle_bone_transform(
        &mut self,
        id: u32,
        pos: &Coord3D,
        angle: f32,
        transform: &Matrix3D,
    ) {
        self.events.push(format!("write:{id}"));
        self.writes.push((id, *pos, angle, *transform));
    }
}
fn model() -> W3DModelDraw {
    let mut data = W3DModelDrawModuleData::new();
    let mut state = ModelConditionInfo::new();
    state.model_name = AsciiString::from("ReceivingModel");
    data.condition_states.push(state);
    let mut draw = W3DModelDraw::new(data);
    draw.cur_state = Some(ActiveModelState::Condition(0));
    for (id, index, name) in [
        (3, 0, "ROOT"),
        (7, 2, "DEAD"),
        (41, 3, "FXBONE"),
        (42, 4, "MISSING"),
        (43, 5, ""),
    ] {
        draw.particle_systems.push(ParticleSysTracker {
            id,
            bone_index: index,
            bone_name: AsciiString::from(name),
        });
    }
    draw
}
#[test]
fn receiving_particle_trackers_preserve_lookup_query_write_order_and_peer_identity() {
    let mut draw = model();
    let matrix =
        Matrix3D::from_translation(glam::Vec3::new(9.0, 8.0, 7.0)) * Matrix3D::from_rotation_z(0.7);
    let mut a = Receiver {
        events: Vec::new(),
        transform: matrix,
        writes: Vec::new(),
    };
    assert!(draw.update_particle_bones_with_receiver(&mut a));
    assert_eq!(
        a.events,
        [
            "find:3",
            "find:7",
            "find:41",
            "bone:ReceivingModel:3:FXBONE",
            "write:41",
            "find:42",
            "bone:ReceivingModel:4:MISSING",
            "find:43",
            "bone:ReceivingModel:5:",
            "write:43"
        ]
    );
    assert_eq!(a.writes.len(), 2);
    // Tracker indices address the selected render model directly; this test
    // intentionally has no public/pristine bone entries or nonempty name gate.
    assert!(draw.current_state().unwrap().pristine_bones.is_empty());
    assert_eq!(a.writes[1].0, 43);
    assert_eq!(a.writes[0].0, 41);
    assert_eq!(a.writes[0].1, Coord3D::new(9.0, 8.0, 7.0));
    assert!((a.writes[0].2 - 0.7).abs() < 0.00001);
    assert_eq!(a.writes[0].3, matrix);
    let before = a.writes.clone();
    let mut b = Receiver {
        events: Vec::new(),
        transform: Matrix3D::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
        writes: Vec::new(),
    };
    assert!(draw.update_particle_bones_with_receiver(&mut b));
    assert_eq!(a.writes, before);
    assert_eq!(b.writes[0].1, Coord3D::new(1.0, 2.0, 3.0));
    // These IDs belong only to the borrowed receiver fixture, not the
    // compatibility global manager used by the historical drop adapter.
    draw.particle_systems.clear();
}
#[test]
fn receiving_model_without_render_state_still_consumes_first_success() {
    let draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
    let mut receiver = Receiver {
        events: Vec::new(),
        transform: Matrix3D::IDENTITY,
        writes: Vec::new(),
    };
    assert!(draw.update_particle_bones_with_receiver(&mut receiver));
    assert!(receiver.events.is_empty());
}

#[test]
fn receiving_outer_dispatch_stops_at_first_model_even_without_render_state() {
    let mut tracer = crate::object::draw::W3DTracerDraw::new(
        crate::object::draw::W3DTracerDrawModuleData::new(),
    );
    let mut first_model = W3DModelDraw::new(W3DModelDrawModuleData::new());
    let mut later_model = model();
    let mut receiver = Receiver {
        events: Vec::new(),
        transform: Matrix3D::IDENTITY,
        writes: Vec::new(),
    };
    let modules: [&mut dyn game_engine::common::thing::module::Module; 3] =
        [&mut tracer, &mut first_model, &mut later_model];
    let mut visited = 0;
    for module in modules {
        visited += 1;
        if crate::object::drawable::update_client_particle_bones(module, &mut receiver) {
            break;
        }
    }
    assert_eq!(visited, 2);
    assert!(receiver.events.is_empty());
    later_model.particle_systems.clear();
}
