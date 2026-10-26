use super::*;

#[test]
fn actual_movement_tick_delivers_tree_callbacks_only_to_its_receiving_world_at_owned_frame() {
    struct RecordingTerrain<'a> {
        visual: &'a mut game_client::terrain::terrain_visual::TerrainVisualImpl,
        moves: usize,
    }
    impl crate::game_logic::NativeTerrainEffects for RecordingTerrain<'_> {
        fn begin_tree_move(
            &mut self,
            unit: crate::game_logic::script_presentation::NativeTerrainUnitMove,
            frame: u32,
        ) -> Option<game_client::terrain::TreeMoveCursor> {
            self.moves += 1;
            crate::game_logic::NativeTerrainEffects::begin_tree_move(self.visual, unit, frame)
        }
        fn advance_tree_move(
            &mut self,
            cursor: &mut game_client::terrain::TreeMoveCursor,
            unit: crate::game_logic::script_presentation::NativeTerrainUnitMove,
        ) -> Option<game_client::terrain::TreeFxEvent> {
            crate::game_logic::NativeTerrainEffects::advance_tree_move(self.visual, cursor, unit)
        }
        fn finish_tree_move_fx(&mut self, cursor: &mut game_client::terrain::TreeMoveCursor) {
            crate::game_logic::NativeTerrainEffects::finish_tree_move_fx(self.visual, cursor);
        }

        fn unit_moved(
            &mut self,
            unit: crate::game_logic::script_presentation::NativeTerrainUnitMove,
            frame: u32,
        ) {
            self.moves += 1;
            crate::game_logic::NativeTerrainEffects::unit_moved(self.visual, unit, frame);
        }
        fn oversize_terrain(&mut self, _: i32) {
            panic!("unexpected terrain oversize")
        }
        fn remove_for_construction(&mut self, _: [f32; 3], _: f32, _: f32, _: bool, _: f32) {
            panic!("unexpected construction removal")
        }
        fn placement_bib(&mut self, _: u32, _: glam::Mat4, _: f32, _: f32, _: bool) {
            panic!("unexpected placement bib")
        }
        fn retain_placement_bibs(&mut self, _: &[u32]) {
            panic!("unexpected bib retention")
        }
        fn clear_placement_bibs(&mut self) {
            panic!("unexpected bib clear")
        }
        fn add_water_velocity(&mut self, _: f32, _: f32, _: f32, _: f32) {
            panic!("unexpected water velocity")
        }
        fn add_scorch(&mut self, _: [f32; 3], _: f32, _: i32) {
            panic!("unexpected scorch")
        }
        fn accepted_height(&mut self, _: gamelogic::terrain::TerrainHeightChange) {
            panic!("unexpected terrain height")
        }
        fn water_state(&mut self, _: &gamelogic::terrain_water::WaterGridState) {
            panic!("unexpected water state")
        }
    }
    fn trees() -> (
        game_client::terrain::terrain_visual::TerrainVisualImpl,
        [usize; 2],
    ) {
        let mut visual = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
        visual
            .tree_buffer_mut()
            .set_bounds(game_client::terrain::TreeRegion2D::new(
                glam::Vec2::ZERO,
                glam::Vec2::splat(100.0),
            ));
        let mut indexes = [0; 2];
        for (index, (name, topple)) in [("MovementOak", true), ("MovementBush", false)]
            .into_iter()
            .enumerate()
        {
            let mut data = game_client::terrain::TreeModuleData::default();
            data.model_name = name.into();
            data.do_topple = topple;
            data.frames_to_move_outward = 10;
            indexes[index] = visual
                .tree_buffer_mut()
                .add_tree(
                    701 + index as u32,
                    Vec3::new(3.0, 0.0, 0.0),
                    1.0,
                    0.0,
                    1.0,
                    data,
                    game_client::terrain::TreeSphere {
                        center: Vec3::ZERO,
                        radius: 5.0,
                    },
                )
                .unwrap();
        }
        (visual, indexes)
    }
    fn moving_unit() -> Object {
        let mut unit = ranger_at(9301, Vec3::ZERO);
        bind_fixture_locomotor(&mut unit, "BasicHumanLocomotor");
        unit.crusher_level = 2;
        unit.selection_radius = 8.0;
        unit.movement.max_speed = 60.0;
        unit.movement.acceleration = 1800.0;
        unit.movement.velocity = Vec3::new(60.0, 0.0, 0.0);
        unit.movement.target_position = Some(Vec3::new(20.0, 0.0, 0.0));
        unit
    }
    let mut driving = GameLogic::new();
    let mut peer = GameLogic::new();
    let id = ObjectId(9301);
    driving.objects.insert(id, moving_unit());
    peer.objects.insert(id, moving_unit());
    driving.frame = 37;
    peer.frame = 99;
    let (mut receiving, indexes) = trees();
    let (mut foreign, foreign_indexes) = trees();
    let mut recorded = RecordingTerrain {
        visual: &mut receiving,
        moves: 0,
    };
    driving.update_movement_with_effects(
        &[id],
        1.0 / 30.0,
        &mut crate::game_logic::ScriptPresentationEffects {
            terrain: Some(&mut recorded),
            ..Default::default()
        },
    );
    assert_eq!(
        recorded.moves, 1,
        "one actual integer crossing delivers exactly one Object callback"
    );
    drop(recorded);
    assert!(
        driving.objects[&id].get_position().x as i32 > 0,
        "actual locomotor pass must move across integer XY before checking effects"
    );
    assert_eq!(
        receiving.tree_buffer_mut().trees()[indexes[0]].topple_state,
        game_client::terrain::W3DToppleState::Falling,
        "actual movement tick must deliver crusher movement to its receiving tree owner"
    );
    assert_eq!(
        receiving.tree_buffer_mut().trees()[indexes[1]].last_frame_updated,
        37,
        "push-aside records the driving world frame at the immediate tree phase"
    );
    assert_ne!(
        foreign.tree_buffer_mut().trees()[foreign_indexes[0]].topple_state,
        game_client::terrain::W3DToppleState::Falling
    );
    assert_eq!(
        foreign.tree_buffer_mut().trees()[foreign_indexes[1]].last_frame_updated,
        0
    );
    assert_eq!(
        peer.objects[&id].get_position(),
        Vec3::ZERO,
        "same original object ID in another world remains independent"
    );
    assert_eq!((driving.frame, peer.frame), (37, 99));

    let mut fractional = GameLogic::new();
    let mut fractional_unit = moving_unit();
    fractional_unit.movement.max_speed = 0.3;
    fractional_unit.movement.velocity = Vec3::new(0.3, 0.0, 0.0);
    fractional_unit.movement.target_position = Some(Vec3::new(200.0, 0.0, 0.0));
    fractional.objects.insert(id, fractional_unit);
    fractional.frame = 37;
    let (mut fractional_visual, _) = trees();
    let mut fractional_recorded = RecordingTerrain {
        visual: &mut fractional_visual,
        moves: 0,
    };
    fractional.update_movement_with_effects(
        &[id],
        1.0 / 30.0,
        &mut crate::game_logic::ScriptPresentationEffects {
            terrain: Some(&mut fractional_recorded),
            ..Default::default()
        },
    );
    let x = fractional.objects[&id].get_position().x;
    assert!(
        0.0 < x && x < 1.0,
        "actual fractional movement must stay in the initial integer cell: {x}"
    );
    assert_eq!(
        fractional_recorded.moves, 0,
        "fractional movement must obey the original integer-position callback gate"
    );
}
