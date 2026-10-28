use super::*;
use crate::game_logic::script_presentation::{
    NativeTerrainEffects, NativeTerrainUnitMove, ScriptPresentationEffects,
};

#[derive(Default)]
struct Receiver<'a> {
    moves: Vec<(NativeTerrainUnitMove, u32)>,
    trigger: Option<&'a std::sync::Arc<std::sync::Mutex<gamelogic::scripting::HostTriggerWorld>>>,
}
impl NativeTerrainEffects for Receiver<'_> {
    fn unit_moved(&mut self, unit: NativeTerrainUnitMove, frame: u32) {
        if let Some(world) = self.trigger {
            assert!(
                world
                    .try_lock()
                    .expect("tree callback precedes trigger mutation loan")
                    .capture()
                    .is_empty(),
                "tree notification observes the pre-trigger position"
            );
        }
        self.moves.push((unit, frame));
    }
    fn oversize_terrain(&mut self, _: i32) {}
    fn remove_for_construction(&mut self, _: [f32; 3], _: f32, _: f32, _: bool, _: f32) {}
    fn placement_bib(&mut self, _: u32, _: Mat4, _: f32, _: f32, _: bool) {}
    fn retain_placement_bibs(&mut self, _: &[u32]) {}
    fn clear_placement_bibs(&mut self) {}
    fn add_water_velocity(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn add_scorch(&mut self, _: [f32; 3], _: f32, _: i32) {}
    fn accepted_height(&mut self, _: gamelogic::terrain::TerrainHeightChange) {}
    fn water_state(&mut self, _: &gamelogic::terrain_water::WaterGridState) {}
}
fn object(id: u32, kinds: &[KindOf]) -> Object {
    let mut template = ThingTemplate::new("OwnedTreeMover");
    for kind in kinds {
        template.add_kind_of(*kind);
    }
    Object::new(template, ObjectId(id), Team::Neutral)
}
#[test]
fn borrowed_pose_tree_callback_uses_receiving_frame_source_coordinates_and_precedes_trigger_update()
{
    let world = crate::game_logic::GameLogic::new();
    world
        .host_trigger_world
        .lock()
        .unwrap()
        .set_current_frame(37);
    let mut mover = object(801, &[KindOf::Vehicle]);
    mover.attach_host_trigger_world(&world.host_trigger_world, &world.team_factory);
    mover.selection_radius = 8.0;
    mover.crusher_level = 2;
    mover.set_orientation(std::f32::consts::FRAC_PI_2);
    let mut receiver = Receiver {
        trigger: Some(&world.host_trigger_world),
        ..Default::default()
    };
    mover.set_position_with_effects(
        Vec3::new(10.0, 5.0, 20.0),
        37,
        &mut ScriptPresentationEffects {
            terrain: Some(&mut receiver),
            ..Default::default()
        },
    );
    assert_eq!(receiver.moves.len(), 1);
    let (unit, frame) = receiver.moves[0];
    assert_eq!(frame, 37);
    assert_eq!(unit.object_id, 801);
    assert_eq!(unit.position, Vec3::new(10.0, 20.0, 5.0));
    assert_eq!(
        unit.direction_2d,
        glam::Vec2::new(
            mover.get_orientation().cos(),
            -mover.get_orientation().sin()
        )
    );
    assert_eq!(
        (unit.major_radius, unit.minor_radius, unit.crusher_level),
        (8.0, 8.0, 2)
    );
    let flags = world.host_trigger_world.lock().unwrap().capture();
    assert_eq!(flags.len(), 1);
    assert_eq!((flags[0].i_x, flags[0].i_y), (10, 20));
}
#[test]
fn equal_integer_pose_changes_do_not_notify_but_transform_translation_notifies_immediately() {
    let mut mover = object(802, &[KindOf::Infantry]);
    let mut receiver = Receiver::default();
    let initial = mover.native_pose_revision();
    let mut effects = ScriptPresentationEffects {
        terrain: Some(&mut receiver),
        ..Default::default()
    };
    mover.set_position_with_effects(Vec3::new(0.9, 10.0, 0.9), 41, &mut effects);
    mover.set_transform_matrix_with_effects(
        Mat4::from_translation(Vec3::new(1.1, 10.0, 0.9)),
        42,
        &mut effects,
    );
    drop(effects);
    assert_eq!(mover.native_pose_revision(), initial + 2);
    assert_eq!(receiver.moves.len(), 1);
    assert_eq!(receiver.moves[0].1, 42);
    assert_eq!(receiver.moves[0].0.position, Vec3::new(1.1, 0.9, 10.0));
}
#[test]
fn cpp_tree_move_kindof_gates_include_inert_and_projectile_combinations_without_drawable_gate() {
    for (index, kinds, expected) in [
        (0, vec![KindOf::Vehicle], 1),
        (1, vec![KindOf::Infantry], 1),
        (2, vec![KindOf::Vehicle, KindOf::Immobile], 0),
        (3, vec![KindOf::Vehicle, KindOf::Projectile], 0),
        (4, vec![KindOf::Vehicle, KindOf::Inert], 0),
        (5, vec![], 0),
    ] {
        let mut mover = object(900 + index, &kinds);
        let mut receiver = Receiver::default();
        mover.set_position_with_effects(
            Vec3::new(3.0, 0.0, 4.0),
            51,
            &mut ScriptPresentationEffects {
                terrain: Some(&mut receiver),
                ..Default::default()
            },
        );
        assert_eq!(receiver.moves.len(), expected, "authored kinds={kinds:?}");
    }
}
