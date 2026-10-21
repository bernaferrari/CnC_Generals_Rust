use super::*;
use crate::game_logic::{KindOf, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

fn sync_entry(
    object_id: u32,
    host_epoch: u64,
    visual_template_name: &str,
    resident: bool,
    destroyed: bool,
    position: [f32; 3],
    orientation: f32,
) -> game_client::core::PresentationDrawableSync {
    game_client::core::PresentationDrawableSync {
        receiving_science_player: None,
        object_id,
        host_epoch,
        resident,
        visual_template_name: visual_template_name.to_string(),
        template_name: "UnderlyingTemplate".to_string(),
        position,
        orientation,
        float_yaw: 0.0,
        float_pitch: 0.0,
        destroyed,
        model_condition_bits: 0,
        body_damage_state: 0,
        kind_names: std::sync::Arc::new(Vec::new()),
        team_color: [1.0, 1.0, 1.0, 1.0],
        effectively_stealthed: false,
        scene_hidden_by_stealth: false,
        health_current: 0.0,
        health_max: 0.0,
        selected: false,
        veterancy_level: 0,
        under_construction: false,
        construction_percent: 0.0,
        sold: false,
        ammo_pip_total: 0,
        ammo_pip_full: 0,
        occupant_count: 0,
        max_garrison: 0,
        disabled: false,
        is_carbomb: false,
        bomb_type: 0,
        bomb_timer_seconds: 0,
        weapon_bonus_enthusiastic: false,
        show_healing: false,
        healing_icon_type: 0,
        garrisoned_ids: Vec::new(),
        emoticon_name: String::new(),
        emoticon_frames_left: 0,
        formation_id: 0,
        caption: String::new(),
        module_admission:
            game_client::core::game_client::PresentationModuleAdmission::Compatibility,
        draw_logic_frame: 0,
        draw_indicator_color: [0, 0, 0],
        draw_physics: Default::default(),
        draw_velocity_magnitude_squared: 0.0,
        draw_module_names: std::sync::Arc::new(Vec::new()),
        health_box_width: 0.0,
        health_box_z_offset: 0.0,
    }
}

struct World {
    logic: GameLogic,
    client: GameClient,
    id: ObjectId,
    epoch: u64,
}
impl World {
    fn new(epoch: u64) -> Self {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "Local", true));
        let mut template = ThingTemplate::new("MaterialAdmissionTank");
        template.add_kind_of(KindOf::Vehicle);
        template.set_health(100.);
        logic.templates.insert(template.name.clone(), template);
        let id = logic
            .create_object("MaterialAdmissionTank", Team::USA, Vec3::ZERO)
            .unwrap();
        let mut client = GameClient::new().unwrap();
        client
            .sync_presentation_drawables([sync_entry(
                id.0,
                epoch,
                "MaterialAdmissionTank",
                true,
                false,
                [0.; 3],
                0.,
            )])
            .unwrap();
        Self {
            logic,
            client,
            id,
            epoch,
        }
    }
    fn frame(&mut self) -> PresentationFrame {
        let frame = PresentationFrame::build_from_logic(&mut self.logic, 0);
        // Match the real engine sequence: synchronize each current visual
        // identity before entering the shared material admission operation.
        self.client
            .sync_presentation_drawables(frame.direct_host_drawables.iter().map(|direct| {
                sync_entry(
                    direct.object.id.0,
                    self.epoch,
                    &direct.visual_template_name,
                    direct.resident,
                    direct.object.destroyed,
                    [0.; 3],
                    0.,
                )
            }))
            .unwrap();
        frame
    }
    fn admit(
        &mut self,
        frame: &PresentationFrame,
    ) -> Result<Vec<NativeMaterialObservation>, String> {
        prepare(&mut self.logic, Some(frame), &mut self.client, self.epoch)
    }
    fn opacity(&self) -> f32 {
        let key = self
            .client
            .presentation_direct_drawable_state(self.epoch, self.id.0)
            .unwrap()
            .binding_key;
        self.client
            .capture_presentation_material_opacities()
            .into_iter()
            .find(|(current, _)| *current == key)
            .unwrap()
            .1
    }
}

#[test]
fn native_material_admission_acknowledges_receiver_only_and_preserves_logic_write_order() {
    let mut first = World::new(11);
    let mut peer = World::new(12);
    assert_eq!(first.id, peer.id);
    for world in [&mut first, &mut peer] {
        let object = world.logic.host_object_mut(world.id).unwrap();
        object.camo_stealth_look = 3;
        object.queue_changed_stealth_material(3, false);
    }
    let peer_frame = peer.frame();
    let frame = first.frame();
    let facts = first.admit(&frame).unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].look, StealthLook::VisibleDetected);
    assert_eq!(facts[0].pending_opacity, Some(1.));
    assert_eq!(first.opacity(), 1.);
    assert_eq!(
        first
            .logic
            .host_object(first.id)
            .unwrap()
            .pending_second_material_pass(),
        None
    );
    assert_eq!(
        peer.logic
            .host_object(peer.id)
            .unwrap()
            .pending_second_material_pass(),
        Some(1.)
    );
    assert_eq!(peer.opacity(), 0.);
    // Hint precedes changed-look assignment; the last write is admitted exactly.
    let object = first.logic.host_object_mut(first.id).unwrap();
    object.queue_second_material_pass(1.);
    object.queue_changed_stealth_material(0, false);
    object.camo_stealth_look = 0;
    let frame = first.frame();
    let facts = first.admit(&frame).unwrap();
    assert_eq!(facts[0].pending_opacity, Some(0.));
    assert_eq!(first.opacity(), 0.);
    // An unchanged NONE look must not overwrite a subsequent explicit hint.
    first
        .logic
        .host_object_mut(first.id)
        .unwrap()
        .queue_second_material_pass(1.);
    let frame = first.frame();
    first.admit(&frame).unwrap();
    assert_eq!(first.opacity(), 1.);
    peer.admit(&peer_frame).unwrap();
    assert_eq!(peer.opacity(), 1.);
    assert_eq!(
        first
            .logic
            .host_object(first.id)
            .unwrap()
            .pending_second_material_pass(),
        None
    );
}

#[test]
fn native_material_observations_use_frozen_death_exact_frenzy_mine_and_disguise_facts() {
    let mut world = World::new(13);
    let object = world.logic.host_object_mut(world.id).unwrap();
    object.health.current = 0.;
    object.status.effectively_dead = false;
    object.drawable_tint_status = 0x20;
    object.weapon_bonus_enthusiastic = true;
    object.template_mut().kind_of.insert(KindOf::DemoTrap);
    object.camo_friendly_opacity = 0.375;
    object.disguise_pending_template = Some("PendingMask".into());
    let frame = world.frame();
    let facts = world.admit(&frame).unwrap();
    assert!(
        !facts[0].effectively_dead,
        "zero HP is not the C++ effectively-dead flag"
    );
    assert!(
        !facts[0].frenzy,
        "a weapon bonus or other tint bit is not FRENZY"
    );
    assert!(!facts[0].is_mine, "KINDOF_DEMOTRAP is not KINDOF_MINE");
    assert!(
        facts[0].is_disguised,
        "pending disguise participates before the visual halfway point"
    );
    assert_eq!(facts[0].friendly_opacity, 0.375);
    let object = world.logic.host_object_mut(world.id).unwrap();
    object.health.current = 100.;
    object.status.effectively_dead = true;
    object.drawable_tint_status = 0x10;
    object.template_mut().kind_of.insert(KindOf::Mine);
    object.disguise_pending_template = None;
    object.disguise_as_template = Some("CommittedMask".into());
    let frame = world.frame();
    // The completed frame owns this phase's death observation, even if a host
    // mutation occurs before the client handoff.
    world
        .logic
        .host_object_mut(world.id)
        .unwrap()
        .status
        .effectively_dead = false;
    let facts = world.admit(&frame).unwrap();
    assert!(
        facts[0].effectively_dead,
        "positive HP cannot cancel frozen effective death"
    );
    assert!(facts[0].frenzy);
    assert!(facts[0].is_mine);
    assert!(facts[0].is_disguised);
    let object = world.logic.host_object_mut(world.id).unwrap();
    object.disguise_as_template = None;
    object.status.disguised = true;
    let frame = world.frame();
    assert!(world.admit(&frame).unwrap()[0].is_disguised);
}

#[test]
fn failed_material_admission_preserves_pending_commands_and_receiving_owner() {
    let mut world = World::new(14);
    world
        .logic
        .host_object_mut(world.id)
        .unwrap()
        .queue_second_material_pass(0.625);
    let frame = world.frame();
    let error = prepare(
        &mut world.logic,
        Some(&frame),
        &mut world.client,
        world.epoch + 1,
    )
    .unwrap_err();
    assert!(error.contains("lacks its current Drawable binding"));
    assert_eq!(world.opacity(), 0.);
    assert_eq!(
        world
            .logic
            .host_object(world.id)
            .unwrap()
            .pending_second_material_pass(),
        Some(0.625)
    );
    let object = world.logic.host_objects_mut().remove(&world.id).unwrap();
    let error = world.admit(&frame).unwrap_err();
    assert!(error.contains("lacks its host owner"));
    assert_eq!(object.pending_second_material_pass(), Some(0.625));
    assert_eq!(world.opacity(), 0.);
    world.logic.host_objects_mut().admit(world.id, object);
    world.admit(&frame).unwrap();
    assert_eq!(world.opacity(), 0.625);
    assert_eq!(
        world
            .logic
            .host_object(world.id)
            .unwrap()
            .pending_second_material_pass(),
        None
    );
}
