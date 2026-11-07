use super::*;

struct DoorBones { missing: bool }
impl crate::assets::pristine_bones::PristineBoneAssets for DoorBones {
    fn pristine_bones_for_draws(
        &self, _: &crate::assets::pristine_bones::PristineBoneSource<'_>,
        name: &str, _: i32, _: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        if self.missing { return Ok(Vec::new()); }
        let position = match name {
            "ExitStart01" => glam::Vec3::new(2.0, 0.0, 3.0),
            "ExitEnd01" => glam::Vec3::new(6.0, 0.0, 3.0),
            _ => return Ok(Vec::new()),
        };
        Ok(vec![glam::Mat4::from_translation(position)])
    }
}

fn fixture(paths: i32) -> (GameLogic, ObjectId, ObjectId) {
    let ini = format!(r#"
Object ContainSpawnOwner
 KindOf = STRUCTURE
 Draw = W3DModelDraw DrawTag
  DefaultConditionState
   Model = ContainModel
  End
 End
 Behavior = TunnelContain Cargo
  NumberOfExitPaths = {paths}
  DoorOpenTime = 100ms
 End
 Behavior = SpawnBehavior Spawn
  SpawnNumber = 1
  SpawnTemplateName = ContainSpawnChild
  OneShot = Yes
 End
End
Object ContainSpawnChild
 KindOf = INFANTRY
End
"#);
    let content = crate::assets::ReceivingObjectContent::from_sources([("contain-spawn.ini".into(), ini)]).unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    let parent = world.create_object("ContainSpawnOwner", Team::GLA, Vec3::new(10.0, 4.0, 20.0)).unwrap();
    let child = world.create_object("ContainSpawnChild", Team::GLA, Vec3::new(1.0, 2.0, 3.0)).unwrap();
    (world, parent, child)
}

#[test]
fn zero_path_uses_authored_door_without_moving_child_or_requiring_assets() {
    let (mut world, parent, child) = fixture(0);
    let original = world.host_object(child).unwrap().get_position();
    assert!(world.canonical_open_contain_exit_available(parent, &Default::default()));
    assert!(world.canonical_open_contain_spawn_exit(parent, child, &mut Default::default()).unwrap());
    assert_eq!(world.host_object(child).unwrap().get_position(), original);
    assert_eq!(world.host_object(parent).unwrap().door_close_countdown, 3);
}

#[test]
fn real_bones_drive_world_start_and_per_parent_rotation() {
    let (mut world, parent, child) = fixture(2);
    let (mut peer, peer_parent, peer_child) = fixture(2);
    assert_eq!((parent, child), (peer_parent, peer_child));
    let bones = DoorBones { missing: false };
    let mut effects = ScriptPresentationEffects { pristine_bones: Some(&bones), ..Default::default() };
    assert!(world.canonical_open_contain_exit_available(parent, &effects));
    assert!(world.canonical_open_contain_spawn_exit(parent, child, &mut effects).unwrap());
    assert_eq!(world.host_object(child).unwrap().get_position(), Vec3::new(12.0, 7.0, 20.0));
    assert_eq!(world.host_object(parent).unwrap().which_exit_path, 2);
    assert_eq!(peer.host_object(peer_parent).unwrap().which_exit_path, 0);
    assert!(peer.canonical_open_contain_spawn_exit(peer_parent, peer_child, &mut effects).unwrap());
    assert_eq!(peer.host_object(peer_parent).unwrap().which_exit_path, 2);
}

#[test]
fn unavailable_source_refuses_and_actual_missing_bone_uses_cpp_parent_position() {
    let (mut world, parent, child) = fixture(2);
    let old_position = world.host_object(child).unwrap().get_position();
    assert!(!world.canonical_open_contain_exit_available(parent, &Default::default()));
    assert!(!world.canonical_open_contain_spawn_exit(parent, child, &mut Default::default()).unwrap());
    assert_eq!(world.host_object(child).unwrap().get_position(), old_position);
    assert_eq!(world.host_object(parent).unwrap().door_close_countdown, 0);
    let bones = DoorBones { missing: true };
    let mut effects = ScriptPresentationEffects { pristine_bones: Some(&bones), ..Default::default() };
    assert!(world.canonical_open_contain_exit_available(parent, &effects));
    assert!(world.canonical_open_contain_spawn_exit(parent, child, &mut effects).unwrap());
    assert_eq!(world.host_object(child).unwrap().get_position(), world.host_object(parent).unwrap().get_position());
}

#[test]
fn authored_spawn_scheduler_uses_tunnel_exit_in_receiving_world() {
    let (mut world, parent, _) = fixture(2);
    let (mut peer, peer_parent, _) = fixture(2);
    assert_eq!(parent, peer_parent);
    let bones = DoorBones { missing: false };
    let mut effects = ScriptPresentationEffects { pristine_bones: Some(&bones), ..Default::default() };
    world.set_current_frame(1);
    world.update_spawn_behavior_with_effects(&mut effects).unwrap();
    let children: Vec<_> = world.host_object(parent).unwrap().spawn_behavior_children().into_iter().map(|(id, _)| id).collect();
    assert_eq!(children.len(), 1);
    assert_eq!(world.host_object(children[0]).unwrap().get_position(), Vec3::new(12.0, 7.0, 20.0));
    assert_eq!(world.host_object(parent).unwrap().which_exit_path, 2);
    assert!(peer.host_object(peer_parent).unwrap().spawn_behavior_children().is_empty());
    peer.set_current_frame(1);
    peer.update_spawn_behavior_with_effects(&mut effects).unwrap();
    let peer_children: Vec<_> = peer.host_object(peer_parent).unwrap().spawn_behavior_children().into_iter().map(|(id, _)| id).collect();
    assert_eq!(peer_children, children);
}
