use super::*;
use game_engine::common::view_impulse::ViewImpulse;
use std::sync::mpsc::{Sender, channel};

struct Observe(&'static str, Sender<String>);
impl FXNugget for Observe {
    fn do_fx_pos(
        &self,
        _: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        self.1.send(self.0.into()).unwrap();
    }
}

fn marker(list: &mut FXList, label: &'static str, events: &Sender<String>) {
    list.add_fx_nugget(Box::new(Observe(label, events.clone())));
}

fn shake(list: &mut FXList, kind: CameraShakeType) {
    // Use the actual authored-definition -> executable-nugget compiler.
    list.add_fx_nugget(executable_nugget(
        &game_engine::common::ini::ini_fx_list::FXNugget::ViewShake { shake_type: kind },
    ));
}

fn host() -> gamelogic::helpers::HostFxObjectPose {
    gamelogic::helpers::HostFxObjectPose {
        id: 7,
        position: Coord3D::new(10.0, 0.0, 0.0),
        transform: Matrix3D::from_translation(Vec3::new(10.0, 0.0, 0.0)),
        player_index: 0,
        bounding_circle_radius: 1.0,
        is_shrouded: false,
    }
}

#[test]
fn authored_host_bonefx_keeps_receiving_services_and_immediate_nested_order() {
    let (events, observed) = channel();
    let mut catalog = FXListStore::new();
    let mut child = FXList::new();
    marker(&mut child, "child-before", &events);
    shake(&mut child, CameraShakeType::Subtle);
    marker(&mut child, "child-after", &events);
    catalog.add_fx_list("child".into(), child);
    let mut outer = FXList::new();
    marker(&mut outer, "outer-before", &events);
    shake(&mut outer, CameraShakeType::Normal);
    outer.add_fx_nugget(Box::new(FXListAtBonePosFXNugget {
        fx_name: "child".into(),
        bone_name: "Muzzle".into(),
        orient_to_bone: false, // C++ parses this but always uses bone transform.
    }));
    marker(&mut outer, "outer-after", &events);

    let mut rules = game_engine::common::global_data::GlobalData::default();
    rules.max_shake_range = 100.0;
    rules.max_shake_intensity = 20.0;
    rules.shake_normal_intensity = 2.0;
    rules.shake_subtle_intensity = 1.0;
    let (mut intensity, mut cosine, mut sine) = (0.0, 0.0, 0.0);
    let mut impulse = ViewImpulse {
        intensity: &mut intensity,
        cosine: &mut cosine,
        sine: &mut sine,
    };
    let mut sequence = 0;
    let mut shake = |position: &Coord3D, kind: ViewShakeKind| {
        events.send(format!("shake:{}", position.x)).unwrap();
        impulse.enqueue(
            [position.x, position.y],
            [10.0, 0.0],
            kind.into(),
            &rules,
            &mut |_, _| {
                sequence += 1;
                events.send(format!("angle:{sequence}")).unwrap();
                sequence as f32
            },
        );
    };
    let mut find = |name: &str| {
        events.send(format!("find:{name}")).unwrap();
        catalog.find_fx_list(name)
    };
    let mut bones = |id, name: &str, start, limit| {
        assert_eq!((id, name, limit), (7, "Muzzle", 40));
        events.send(format!("bones:{start}")).unwrap();
        let xs: &[f32] = if start == 0 { &[1.0] } else { &[2.0, 3.0] };
        xs.iter()
            .map(|x| Matrix3D::from_translation(Vec3::new(*x, 0.0, 0.0)))
            .collect()
    };
    let mut visible = |position: Option<&Coord3D>| {
        let x = position.unwrap().x;
        events.send(format!("visible:{x}")).unwrap();
        x != 12.0
    };
    outer.do_fx_obj_host_with_view(
        &host(),
        None,
        &mut FxViewServices {
            add_scorch: &mut |_, _, _| {},
            shake: &mut shake,
            find_list: &mut find,
            bone_transforms: &mut bones,
            position_visible: &mut visible,
        },
    );
    assert_eq!(
        observed.try_iter().collect::<Vec<_>>(),
        [
            "outer-before",
            "shake:10",
            "angle:1",
            "find:child",
            "bones:0",
            "visible:11",
            "child-before",
            "shake:11",
            "angle:2",
            "child-after",
            "bones:1",
            "visible:12",
            "visible:13",
            "child-before",
            "shake:13",
            "angle:3",
            "child-after",
            "outer-after",
        ]
    );
    assert_eq!(sequence, 3);
    assert_eq!(intensity, 2.0_f32 + 0.99 + 0.97);
    assert_eq!(cosine, 3.0_f64.cos() as f32);
    assert_eq!(sine, 3.0_f64.sin() as f32);
}

#[test]
fn hidden_position_and_host_skip_all_receiving_callbacks() {
    let mut list = FXList::new();
    shake(&mut list, CameraShakeType::Normal);
    let mut shake = |_: &Coord3D, _| panic!("hidden shake");
    let mut find = |_: &str| panic!("hidden catalog");
    let mut bones = |_, _: &str, _, _| panic!("hidden bones");
    let mut visible = |_: Option<&Coord3D>| false;
    let mut view = FxViewServices {
        add_scorch: &mut |_, _, _| {},
        shake: &mut shake,
        find_list: &mut find,
        bone_transforms: &mut bones,
        position_visible: &mut visible,
    };
    list.do_fx_pos_with_view(Some(&Coord3D::ZERO), None, 0.0, None, 0.0, &mut view);
    let mut hidden = host();
    hidden.is_shrouded = true;
    list.do_fx_obj_host_with_view(&hidden, None, &mut view);
}

#[test]
fn same_id_receivers_keep_their_own_view_and_impulse_without_tactical_publication() {
    let mut list = FXList::new();
    shake(&mut list, CameraShakeType::Normal);
    let mut rules = game_engine::common::global_data::GlobalData::default();
    rules.max_shake_range = 10.0;
    rules.shake_normal_intensity = 2.0;
    rules.max_shake_intensity = 3.0;
    let run = |position: [f32; 2], fields: &mut [f32; 3]| {
        let [intensity, cosine, sine] = fields;
        let mut impulse = ViewImpulse {
            intensity,
            cosine,
            sine,
        };
        let mut shake = |epicenter: &Coord3D, kind: ViewShakeKind| {
            impulse.enqueue(
                [epicenter.x, epicenter.y],
                position,
                kind.into(),
                &rules,
                &mut |_, _| 0.0,
            );
        };
        let mut find = |_: &str| None;
        let mut bones = |_, _: &str, _, _| Vec::new();
        let mut visible = |_: Option<&Coord3D>| true;
        list.do_fx_obj_host_with_view(
            &host(),
            None,
            &mut FxViewServices {
                add_scorch: &mut |_, _, _| {},
                shake: &mut shake,
                find_list: &mut find,
                bone_transforms: &mut bones,
                position_visible: &mut visible,
            },
        );
    };
    let mut a = [0.0; 3];
    let mut b = [0.0; 3];
    run([10.0, 0.0], &mut a);
    run([1000.0, 0.0], &mut b);
    assert_eq!(a, [2.0, 1.0, 0.0]);
    assert_eq!(b, [0.0, 1.0, 0.0]); // out-of-range still consumes direction.
    run([10.0, 0.0], &mut a);
    assert_eq!(a, [3.0, 1.0, 0.0]); // original fixed overflow, not max=4.
    assert_eq!(b, [0.0, 1.0, 0.0]);
}

#[test]
fn terrain_scorch_host_nugget_uses_receiving_callback_at_authored_list_position() {
    let (events, observed) = channel();
    let mut list = FXList::new();
    marker(&mut list, "before", &events);
    list.add_fx_nugget(Box::new(TerrainScorchFXNugget {
        scorch: 2,
        radius: 18.0,
    }));
    marker(&mut list, "after", &events);
    let mut a = crate::terrain::scorch_mesh::TerrainScorchBuffer::new();
    let mut b = crate::terrain::scorch_mesh::TerrainScorchBuffer::new();
    b.add_scorch([50.0, 60.0, 7.0], 12.0, 1);
    let peer = b.marks().to_vec();
    let mut add_scorch = |position, radius, kind| {
        events.send("scorch".into()).unwrap();
        a.add_scorch(position, radius, kind);
    };
    let mut shake = |_: &Coord3D, _: ViewShakeKind| panic!("no shake nugget");
    let mut find = |_: &str| None;
    let mut bones = |_, _: &str, _, _| Vec::new();
    let mut visible = |_: Option<&Coord3D>| true;
    list.do_fx_obj_host_with_view(
        &host(),
        None,
        &mut FxViewServices {
            add_scorch: &mut add_scorch,
            shake: &mut shake,
            find_list: &mut find,
            bone_transforms: &mut bones,
            position_visible: &mut visible,
        },
    );
    assert_eq!(
        observed.try_iter().collect::<Vec<_>>(),
        ["before", "scorch", "after"]
    );
    assert_eq!(
        a.marks(),
        [crate::terrain::scorch_mesh::ScorchMark {
            location: [10.0, 0.0, 0.0],
            radius: 18.0,
            scorch_type: 2
        }]
    );
    assert_eq!(b.marks(), peer);
}
