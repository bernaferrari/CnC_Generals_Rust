use super::*;
use std::sync::Arc;

use crate::helpers::TheInGameUI;
use crate::language_filter::get_language_filter;
use game_engine::common::audio::dynamic_audio_event_info::DynamicAudioEventInfo;
use game_engine::common::bit_flags::ModelConditionFlags;
use game_engine::common::system::game_common::WhichTurretType;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::Module;
use gamelogic::common::Matrix3D;
use gamelogic::common::types::{INVALID_ID, ObjectShroudStatus, WeaponSlotType};
use gamelogic::helpers::{
    BoneOverrideState, ModelDrawSourceIdentity, ModelDrawState, TheGameClient,
};
use gamelogic::object::registry::OBJECT_REGISTRY;
use parking_lot::Mutex;

fn assert_near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.0001,
        "actual {actual} expected {expected}"
    );
}

#[derive(Debug)]
struct SnapshotTestDrawModule {
    identifier: &'static str,
    module_type: usize,
    payload: u32,
    observed_payload: Option<Arc<std::sync::atomic::AtomicU32>>,
}

impl DrawModule for SnapshotTestDrawModule {
    fn snapshot_module_identifier(&self) -> Option<&str> {
        Some(self.identifier)
    }

    fn drawable_module_type_index(&self) -> usize {
        self.module_type
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_unsigned_int(&mut self.payload)
            .map_err(|e| format!("{:?}", e))?;
        if let Some(observed_payload) = &self.observed_payload {
            observed_payload.store(self.payload, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct IndicatorDispatchTestDrawModule {
    observed_color: Arc<Mutex<Option<(u8, u8, u8)>>>,
    bind_count: Arc<std::sync::atomic::AtomicU32>,
}

#[derive(Debug)]
struct ShroudDispatchTestDrawModule {
    observed: Arc<Mutex<Vec<bool>>>,
}

impl DrawModule for ShroudDispatchTestDrawModule {
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.observed.lock().push(fully_obscured);
    }
}

impl DrawModule for IndicatorDispatchTestDrawModule {
    fn replace_indicator_color(&mut self, color: Option<(u8, u8, u8)>) {
        *self.observed_color.lock() = color;
    }

    fn on_drawable_bound_to_object(&mut self) {
        self.bind_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct ProjectileLaunchTestDrawModule {
    observed_pitch_pointer: Arc<Mutex<bool>>,
}

impl DrawModule for ProjectileLaunchTestDrawModule {
    fn get_projectile_launch_offset(
        &self,
        wslot: WeaponSlotType,
        barrel: i32,
        launch_pos: &mut Matrix4,
        turret: WhichTurretType,
        turret_rot_pos: &mut Vector3,
        turret_pitch_pos: Option<&mut Vector3>,
    ) -> bool {
        assert_eq!(wslot, WeaponSlotType::Primary);
        assert_eq!(barrel, 1);
        assert_eq!(turret, WhichTurretType::Main);

        *launch_pos = Matrix4::translation(Vector3::new(10.0, 20.0, 30.0));
        *turret_rot_pos = Vector3::new(1.0, 2.0, 3.0);
        if let Some(pitch) = turret_pitch_pos {
            *self.observed_pitch_pointer.lock() = true;
            *pitch = Vector3::new(4.0, 5.0, 6.0);
        }
        true
    }
}

#[derive(Debug)]
struct WeaponFireDispatchTestDrawModule {
    handled: bool,
    calls: Arc<Mutex<u32>>,
}

impl DrawModule for WeaponFireDispatchTestDrawModule {
    fn handle_weapon_fire_fx(
        &mut self,
        _wslot: WeaponSlotType,
        _barrel: i32,
        _fx_list: Option<&FXListRef>,
        _weapon_speed: f32,
        _victim_pos: Option<&Vector3>,
        _damage_radius: f32,
    ) -> bool {
        *self.calls.lock() += 1;
        self.handled
    }
}

#[test]
fn test_drawable_creation() {
    let drawable = BasicDrawable::new(DrawableId(1));
    assert_eq!(drawable.get_id(), DrawableId(1));
    assert_eq!(drawable.get_position(), Vector3::zero());
    assert!(drawable.is_visible());
    assert!(!drawable.is_selected());
    assert_eq!(drawable.get_opacity(), 1.0);
    // C++ Drawable.cpp: m_drawableInfo.m_drawable = this; m_ghostObject = NULL;
    assert_eq!(drawable.drawable_info().drawable_id, 1);
    assert_eq!(drawable.drawable_info().ghost_object_id, INVALID_ID);
    assert_eq!(drawable.drawable_info().shroud_status_object_id, INVALID_ID);
    assert_eq!(drawable.shroud_clear_frame(), 0);
    assert!(!drawable.fully_obscured_by_shroud());
}

#[test]
fn get_current_client_bone_positions_reads_w3d_bone_data() {
    let mut drawable = BasicDrawable::new(DrawableId(1));
    let mut bones = BoneData::default();
    bones.add_current_bone(
        "WeaponFireFXBone",
        Vector3::new(1.0, 2.0, 3.0),
        Matrix4::translation(Vector3::new(1.0, 2.0, 3.0)),
    );
    drawable.set_bone_data(bones);
    let mut positions = [Vector3::zero(); 4];
    let mut transforms = [Matrix4::identity(); 4];
    let count = drawable.get_current_client_bone_positions(
        "WeaponFireFXBone",
        0,
        &mut positions,
        &mut transforms,
    );
    assert_eq!(count, 1);
    assert_eq!(positions[0], Vector3::new(1.0, 2.0, 3.0));
}

#[test]
fn direct_shroud_history_is_volatile_but_survives_live_rebinds() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut saved = BasicDrawable::new(DrawableId(77));
    let _ =
        saved
            .shroud_clear_state
            .evaluate_scene_direct(17, ObjectShroudStatus::Clear, false, false);
    saved.set_fully_obscured_by_shroud(true);
    saved.set_object_id(Some(100));
    saved.set_object_id(Some(200));
    assert_eq!(saved.shroud_clear_frame(), 17);
    assert!(saved.fully_obscured_by_shroud());

    let mut saved_bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut saved_bytes);
        let mut xfer = XferSave::new(cursor, 1);
        xfer.open("drawable_volatile_shroud_state").unwrap();
        saved.xfer_snapshot(&mut xfer).unwrap();
        xfer.close().unwrap();
    }

    // Neither the clear timestamp nor fully-obscured state belongs in C++
    // Drawable::xfer, so otherwise identical drawables serialize byte-for-byte
    // the same data.
    let mut baseline = BasicDrawable::new(DrawableId(77));
    baseline.set_object_id(Some(200));
    let mut baseline_bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut baseline_bytes);
        let mut xfer = XferSave::new(cursor, 1);
        xfer.open("drawable_volatile_shroud_state").unwrap();
        baseline.xfer_snapshot(&mut xfer).unwrap();
        xfer.close().unwrap();
    }
    assert_eq!(saved_bytes, baseline_bytes);

    let mut loaded = BasicDrawable::new(DrawableId(0));
    let _ = loaded.shroud_clear_state.evaluate_scene_direct(
        29,
        ObjectShroudStatus::Clear,
        false,
        false,
    );
    loaded.set_fully_obscured_by_shroud(true);
    let mut xfer = XferLoad::new(Cursor::new(saved_bytes), 1);
    xfer.open("drawable_volatile_shroud_state").unwrap();
    loaded.xfer_snapshot(&mut xfer).unwrap();
    xfer.close().unwrap();

    assert_eq!(loaded.shroud_clear_frame(), 0);
    assert!(!loaded.fully_obscured_by_shroud());
}

#[test]
fn fade_in_reaches_full_opacity_after_requested_frames() {
    let mut drawable = BasicDrawable::new(DrawableId(10));
    drawable.fade_in(10);

    assert_eq!(drawable.fading_mode(), FadingMode::FadingIn);
    assert_eq!(drawable.time_to_fade(), 10);
    assert_eq!(drawable.time_elapsed_fade(), 0);
    assert_near(drawable.get_explicit_opacity(), 0.0);
    assert_near(drawable.get_opacity(), 0.0);
    assert!(drawable.is_fading());

    // C++ updateDrawable applies elapsed first (starts at 0 → opacity 0),
    // then increments. Completes when elapsed > timeToFade, so fade_in(N)
    // needs N+1 ticks to reach full opacity. After N ticks opacity is (N-1)/N.
    for i in 0..10 {
        drawable.update(0.0);
        assert_eq!(drawable.fading_mode(), FadingMode::FadingIn);
        assert_near(drawable.get_explicit_opacity(), i as f32 / 10.0);
    }

    drawable.update(0.0);
    assert_near(drawable.get_explicit_opacity(), 1.0);
    assert_near(drawable.get_opacity(), 1.0);
    assert_eq!(drawable.fading_mode(), FadingMode::None);
    assert!(!drawable.is_fading());
}

#[test]
fn fade_out_reaches_zero_opacity_after_requested_frames() {
    let mut drawable = BasicDrawable::new(DrawableId(11));
    drawable.fade_out(10);
    assert_near(drawable.get_explicit_opacity(), 1.0);
    assert_eq!(drawable.fading_mode(), FadingMode::FadingOut);

    for i in 0..10 {
        drawable.update(0.0);
        assert_near(drawable.get_explicit_opacity(), (10 - i) as f32 / 10.0);
    }

    drawable.update(0.0);
    assert_near(drawable.get_explicit_opacity(), 0.0);
    assert_eq!(drawable.fading_mode(), FadingMode::None);
}

#[test]
fn test_drawable_visibility() {
    let mut drawable = BasicDrawable::new(DrawableId(1));

    drawable.set_visible(false);
    assert!(!drawable.is_visible());

    drawable.set_visible(true);
    assert!(drawable.is_visible());

    drawable.set_stealth_look(StealthLook::Invisible);
    assert!(!drawable.is_visible());
}

#[test]
fn fully_obscured_state_dispatches_only_on_change() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.add_draw_module(Box::new(ShroudDispatchTestDrawModule {
        observed: Arc::clone(&observed),
    }));

    drawable.set_fully_obscured_by_shroud(true);
    drawable.set_fully_obscured_by_shroud(true);
    drawable.set_fully_obscured_by_shroud(false);

    assert_eq!(*observed.lock(), vec![true, false]);
}

#[test]
fn test_drawable_selection() {
    let mut drawable = BasicDrawable::new(DrawableId(1));

    assert!(!drawable.is_selected());

    drawable.set_selected(true);
    assert!(drawable.is_selected());
    assert!(drawable.selection_flash_envelope.is_some());

    drawable.set_selected(false);
    assert!(!drawable.is_selected());
    // C++ onUnselected is empty — envelope decays, it is not cut off.
    assert!(drawable.selection_flash_envelope.is_some());
}

#[test]
fn icon_pip_gate_matches_cpp_selected_or_moused_over_drawable() {
    TheInGameUI::set_moused_over_drawable_id(0);
    let mut drawable = BasicDrawable::new(DrawableId(424_242));

    assert!(!drawable.selected_or_moused_over_for_icon_pips());

    TheInGameUI::set_moused_over_drawable_id(424_242);
    assert!(drawable.selected_or_moused_over_for_icon_pips());

    TheInGameUI::set_moused_over_drawable_id(0);
    drawable.set_selected(true);
    assert!(drawable.selected_or_moused_over_for_icon_pips());

    TheInGameUI::set_moused_over_drawable_id(0);
}

#[test]
fn caption_text_is_language_filtered() {
    get_language_filter().set_words_for_test(["badword"]);

    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.set_caption_text("Pilot badword ready");

    assert_eq!(drawable.get_caption_text(), Some("Pilot ******* ready"));
}

#[test]
fn draw_caption_publishes_caption_overlay() {
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.set_caption_text("Beacon Alpha");

    drawable.draw_caption(&IRegion2D::new(
        ICoord2D::new(10, 20),
        ICoord2D::new(60, 40),
    ));

    assert_eq!(
        drawable.overlay_data.caption.as_deref(),
        Some("Beacon Alpha")
    );
    assert!(drawable.overlay_data.visible);
}

#[test]
fn draw_caption_clears_stale_overlay_without_caption_text() {
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.set_caption_text("Beacon Alpha");
    drawable.draw_caption(&IRegion2D::new(
        ICoord2D::new(10, 20),
        ICoord2D::new(60, 40),
    ));

    drawable.clear_caption_text();
    drawable.draw_caption(&IRegion2D::new(
        ICoord2D::new(10, 20),
        ICoord2D::new(60, 40),
    ));

    assert_eq!(drawable.overlay_data.caption, None);
}

#[test]
fn indicator_color_is_dispatched_to_draw_modules() {
    let observed_color = Arc::new(Mutex::new(None));
    let bind_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.add_draw_module(Box::new(IndicatorDispatchTestDrawModule {
        observed_color: Arc::clone(&observed_color),
        bind_count: Arc::clone(&bind_count),
    }));

    drawable.set_indicator_color(Some((10, 20, 30)));

    assert_eq!(*observed_color.lock(), Some((10, 20, 30)));
    assert_eq!(bind_count.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn object_binding_notifies_draw_modules() {
    let observed_color = Arc::new(Mutex::new(None));
    let bind_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.add_draw_module(Box::new(IndicatorDispatchTestDrawModule {
        observed_color,
        bind_count: Arc::clone(&bind_count),
    }));

    drawable.friend_bind_to_object(123);

    assert_eq!(drawable.object_id, Some(123));
    assert_eq!(bind_count.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn model_draw_bridge_uses_bound_object_not_client_drawable_id() {
    let client = TheGameClient::get().expect("logic game-client bridge");
    let object_id = 9_020_001;
    client.clear_object_model_draws(object_id);
    client.begin_object_model_draw_frame(object_id);
    client.begin_active_object_model_draw(
        object_id,
        ModelDrawSourceIdentity {
            runtime_draw_ordinal: 0,
            module_name: "W3DModelDraw".to_string(),
            module_tag: "Body".to_string(),
            module_tag_name_key: 123,
        },
    );
    client.set_active_object_model_draw(
        object_id,
        ModelDrawState {
            source: Default::default(),
            logic_drawable_id: 0,
            model_name: "BridgeIdentityTank".to_string(),
            world_transform: Matrix3D::IDENTITY,
            render_object_scale: Some(1.0),
            render_object_color: Some(0),
            condition_flags_bits: 0,
            bone_overrides: Vec::new(),
            animation_name: None,
            animation_time: 0.0,
            animation_mode: 0,
            mesh_uv_overrides: Vec::new(),
            sub_object_visibility: Vec::new(),
            weapon_bone_bindings: Default::default(),
        },
    );
    client.commit_active_object_model_draw(object_id, 777);

    let mut drawable = BasicDrawable::new(DrawableId(42));
    drawable.set_object_id(Some(object_id));
    let states = drawable.model_draw_states();

    assert_eq!(states.len(), 1);
    assert_eq!(states[0].model_name, "BridgeIdentityTank");
    assert_eq!(states[0].logic_drawable_id, 777);
    assert_eq!(states[0].source.runtime_draw_ordinal, 0);

    client.clear_object_model_draws(object_id);
}

#[test]
fn set_object_id_uses_binding_side_effects_once_per_object() {
    let observed_color = Arc::new(Mutex::new(None));
    let bind_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.add_draw_module(Box::new(IndicatorDispatchTestDrawModule {
        observed_color,
        bind_count: Arc::clone(&bind_count),
    }));

    drawable.set_object_id(Some(123));
    drawable.set_object_id(Some(123));
    drawable.set_object_id(None);

    assert_eq!(drawable.object_id, None);
    assert_eq!(bind_count.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn test_hotkey_squad_resolution() {
    let mut player = gamelogic::player::Player::new(0);
    player.init_from_dict_defaults();

    let squad = player
        .get_hotkey_squad(3)
        .expect("expected squad slot to exist after init");
    squad.add_object_id(77);

    assert_eq!(
        BasicDrawable::find_hotkey_squad_number(&mut player, 77),
        Some(3)
    );
    assert_eq!(
        BasicDrawable::find_hotkey_squad_number(&mut player, 99),
        None
    );
}

#[test]
fn test_tint_envelope() {
    let mut envelope = TintEnvelope::new();

    envelope.play(Vector3::new(1.0, 0.5, 0.0), 2, 2, 1);
    assert!(envelope.is_effective);
    assert_eq!(envelope.state, EnvelopeState::Attack);

    // Simulate updates
    envelope.update();
    envelope.update();
    assert_eq!(envelope.state, EnvelopeState::Sustain);

    envelope.update();
    assert_eq!(envelope.state, EnvelopeState::Sustain);

    envelope.update();
    assert_eq!(envelope.state, EnvelopeState::Decay);
}

#[test]
fn test_drawable_status_flags() {
    let mut status = DrawableStatus::NONE;

    assert!(!status.has(DrawableStatus::SHADOWS));

    status.set(DrawableStatus::SHADOWS);
    assert!(status.has(DrawableStatus::SHADOWS));

    status.clear(DrawableStatus::SHADOWS);
    assert!(!status.has(DrawableStatus::SHADOWS));
}

#[test]
fn test_icon_info() {
    let mut icon_info = IconInfo::new();

    // Mock icon implementation
    #[derive(Debug)]
    struct MockIcon;
    impl Icon for MockIcon {
        fn render(&self, _position: Vector3, _size: Vector3) {}
        fn xfer(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
            Ok(())
        }
    }

    let icon = Arc::new(MockIcon);
    icon_info.set_icon(IconType::DefaultHeal, icon, 10, 0);

    assert!(icon_info.icons.contains_key(&IconType::DefaultHeal));
    assert!(
        icon_info
            .keep_till_frame
            .contains_key(&IconType::DefaultHeal)
    );

    icon_info.clear_icon(IconType::DefaultHeal);
    assert!(!icon_info.icons.contains_key(&IconType::DefaultHeal));
}

#[test]
fn test_icon_xfer_order_matches_cpp_slot_order() {
    let names: Vec<&'static str> = IconType::XFER_ORDER
        .iter()
        .map(|icon_type| icon_type.name())
        .collect();
    assert_eq!(
        names,
        vec![
            "DefaultHeal",
            "StructureHeal",
            "VehicleHeal",
            "Demoralized",
            "BombTimed",
            "BombRemote",
            "Disabled",
            "BattlePlanIcon_Bombard",
            "BattlePlanIcon_HoldTheLine",
            "BattlePlanIcon_SeekAndDestroy",
            "Emoticon",
            "Enthusiastic",
            "Subliminal",
            "CarBomb",
        ]
    );
}

#[test]
fn test_icon_info_cpp_layout_empty_writes_only_count() {
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut icon_info = IconInfo::new();
    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("empty_icon_info").unwrap();
        icon_info.xfer_cpp_layout(&mut save).unwrap();
        save.close().unwrap();
    }

    assert_eq!(bytes, vec![0]);
}

#[test]
fn test_drawable_modules_save_writes_cpp_empty_type_buckets() {
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut modules: Vec<Box<dyn DrawModule>> = Vec::new();
    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_modules_empty").unwrap();
        xfer_drawable_modules(&mut save, &mut modules).unwrap();
        save.close().unwrap();
    }

    assert_eq!(bytes, vec![1, 2, 0, 0, 0, 0, 0]);
}

#[test]
fn test_drawable_modules_save_writes_named_snapshot_blocks() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut modules: Vec<Box<dyn DrawModule>> = vec![
        Box::new(SnapshotTestDrawModule {
            identifier: "DrawTag",
            module_type: 0,
            payload: 0x1122_3344,
            observed_payload: None,
        }),
        Box::new(SnapshotTestDrawModule {
            identifier: "ClientUpdateTag",
            module_type: 1,
            payload: 0x5566_7788,
            observed_payload: None,
        }),
    ];

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_modules_named").unwrap();
        xfer_drawable_modules(&mut save, &mut modules).unwrap();
        save.close().unwrap();
    }

    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_modules_named").unwrap();
    let mut version = 0;
    load.xfer_version(&mut version, 1).unwrap();
    assert_eq!(version, 1);
    let mut module_types = 0u16;
    load.xfer_unsigned_short(&mut module_types).unwrap();
    assert_eq!(module_types, 2);

    let mut draw_count = 0u16;
    load.xfer_unsigned_short(&mut draw_count).unwrap();
    assert_eq!(draw_count, 1);
    let mut draw_identifier = String::new();
    load.xfer_ascii_string(&mut draw_identifier).unwrap();
    assert_eq!(draw_identifier, "DrawTag");
    let draw_block_size = load.begin_block().unwrap();
    assert_eq!(draw_block_size, 4);
    let mut draw_payload = 0;
    load.xfer_unsigned_int(&mut draw_payload).unwrap();
    load.end_block().unwrap();
    assert_eq!(draw_payload, 0x1122_3344);

    let mut client_update_count = 0u16;
    load.xfer_unsigned_short(&mut client_update_count).unwrap();
    assert_eq!(client_update_count, 1);
    let mut client_update_identifier = String::new();
    load.xfer_ascii_string(&mut client_update_identifier)
        .unwrap();
    assert_eq!(client_update_identifier, "ClientUpdateTag");
    let client_update_block_size = load.begin_block().unwrap();
    assert_eq!(client_update_block_size, 4);
    let mut client_update_payload = 0;
    load.xfer_unsigned_int(&mut client_update_payload).unwrap();
    load.end_block().unwrap();
    assert_eq!(client_update_payload, 0x5566_7788);
    load.close().unwrap();
}

#[test]
fn test_logic_draw_module_adapter_saves_concrete_w3d_snapshot_block() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use gamelogic::object::draw::{W3DTreeDraw, W3DTreeDrawModuleData};
    use std::io::Cursor;

    let mut modules: Vec<Box<dyn DrawModule>> =
        vec![Box::new(LogicDrawModuleSnapshotAdapter::draw_module(
            "W3DTreeDraw",
            Box::new(W3DTreeDraw::new(W3DTreeDrawModuleData::new())),
        ))];

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_modules_w3d_tree").unwrap();
        xfer_drawable_modules(&mut save, &mut modules).unwrap();
        save.close().unwrap();
    }

    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_modules_w3d_tree").unwrap();
    let mut version = 0;
    load.xfer_version(&mut version, 1).unwrap();
    assert_eq!(version, 1);
    let mut module_types = 0u16;
    load.xfer_unsigned_short(&mut module_types).unwrap();
    assert_eq!(module_types, 2);

    let mut draw_count = 0u16;
    load.xfer_unsigned_short(&mut draw_count).unwrap();
    assert_eq!(draw_count, 1);
    let mut draw_identifier = String::new();
    load.xfer_ascii_string(&mut draw_identifier).unwrap();
    assert_eq!(draw_identifier, "W3DTreeDraw");
    let draw_block_size = load.begin_block().unwrap();
    assert_eq!(draw_block_size, 1);
    let mut tree_draw_version = 0;
    load.xfer_version(&mut tree_draw_version, 1).unwrap();
    load.end_block().unwrap();
    assert_eq!(tree_draw_version, 1);

    let mut client_update_count = 0u16;
    load.xfer_unsigned_short(&mut client_update_count).unwrap();
    assert_eq!(client_update_count, 0);
    load.close().unwrap();
}

#[test]
fn test_logic_draw_module_adapter_loads_matching_w3d_snapshot_block() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use gamelogic::object::draw::{W3DTreeDraw, W3DTreeDrawModuleData};
    use std::io::Cursor;

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_modules_w3d_tree_load").unwrap();
        let mut version = 1;
        save.xfer_version(&mut version, 1).unwrap();
        let mut module_types = 2u16;
        save.xfer_unsigned_short(&mut module_types).unwrap();

        let mut draw_count = 1u16;
        save.xfer_unsigned_short(&mut draw_count).unwrap();
        let mut module_identifier = "W3DTreeDraw".to_string();
        save.xfer_ascii_string(&mut module_identifier).unwrap();
        save.begin_block().unwrap();
        let mut tree_draw_version = 1;
        save.xfer_version(&mut tree_draw_version, 1).unwrap();
        save.end_block().unwrap();

        let mut client_update_count = 0u16;
        save.xfer_unsigned_short(&mut client_update_count).unwrap();
        save.close().unwrap();
    }

    let mut modules: Vec<Box<dyn DrawModule>> =
        vec![Box::new(LogicDrawModuleSnapshotAdapter::draw_module(
            "W3DTreeDraw",
            Box::new(W3DTreeDraw::new(W3DTreeDrawModuleData::new())),
        ))];

    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_modules_w3d_tree_load").unwrap();
    xfer_drawable_modules(&mut load, &mut modules).unwrap();
    load.close().unwrap();
}

#[test]
fn test_drawable_modules_load_applies_matching_snapshot_block() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU32, Ordering};

    let observed_payload = Arc::new(AtomicU32::new(0));
    let mut modules: Vec<Box<dyn DrawModule>> = vec![Box::new(SnapshotTestDrawModule {
        identifier: "ExistingDrawModule",
        module_type: 0,
        payload: 0,
        observed_payload: Some(Arc::clone(&observed_payload)),
    })];

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_modules_matching").unwrap();
        let mut version = 1;
        save.xfer_version(&mut version, 1).unwrap();
        let mut module_types = 2u16;
        save.xfer_unsigned_short(&mut module_types).unwrap();

        let mut draw_module_count = 1u16;
        save.xfer_unsigned_short(&mut draw_module_count).unwrap();
        let mut module_identifier = "ExistingDrawModule".to_string();
        save.xfer_ascii_string(&mut module_identifier).unwrap();
        save.begin_block().unwrap();
        let mut payload = 0xCAFE_BABE;
        save.xfer_unsigned_int(&mut payload).unwrap();
        save.end_block().unwrap();

        let mut client_update_count = 0u16;
        save.xfer_unsigned_short(&mut client_update_count).unwrap();
        save.close().unwrap();
    }

    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_modules_matching").unwrap();
    xfer_drawable_modules(&mut load, &mut modules).unwrap();
    load.close().unwrap();

    assert_eq!(observed_payload.load(Ordering::SeqCst), 0xCAFE_BABE);
}

#[test]
fn test_drawable_modules_load_skips_unknown_module_blocks() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_modules_with_unknown").unwrap();

        let mut version = 1;
        save.xfer_version(&mut version, 1).unwrap();
        let mut module_types = 2u16;
        save.xfer_unsigned_short(&mut module_types).unwrap();

        let mut draw_module_count = 1u16;
        save.xfer_unsigned_short(&mut draw_module_count).unwrap();
        let mut module_identifier = "UnknownDrawModule".to_string();
        save.xfer_ascii_string(&mut module_identifier).unwrap();
        save.begin_block().unwrap();
        let mut skipped_payload = 0x1234_5678;
        save.xfer_unsigned_int(&mut skipped_payload).unwrap();
        save.end_block().unwrap();

        let mut client_update_count = 0u16;
        save.xfer_unsigned_short(&mut client_update_count).unwrap();

        let mut marker = 0xAABB_CCDD;
        save.xfer_unsigned_int(&mut marker).unwrap();
        save.close().unwrap();
    }

    let mut modules: Vec<Box<dyn DrawModule>> = Vec::new();
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_modules_with_unknown").unwrap();
    xfer_drawable_modules(&mut load, &mut modules).unwrap();
    let mut marker = 0;
    load.xfer_unsigned_int(&mut marker).unwrap();
    load.close().unwrap();

    assert_eq!(marker, 0xAABB_CCDD);
}

#[test]
fn test_matrix3d_save_layout_matches_cpp_xfer_matrix3d() {
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut matrix = Matrix4::identity();
    matrix.elements[0] = [1.0, 2.0, 3.0, 4.0];
    matrix.elements[1] = [5.0, 6.0, 7.0, 8.0];
    matrix.elements[2] = [9.0, 10.0, 11.0, 12.0];
    matrix.elements[3] = [13.0, 14.0, 15.0, 16.0];

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("matrix3d").unwrap();
        xfer_matrix3d(&mut save, &mut matrix).unwrap();
        save.close().unwrap();
    }

    assert_eq!(bytes.len(), 1 + 12 * std::mem::size_of::<f32>());
    assert_eq!(bytes[0], 1);
    assert_eq!(&bytes[1..5], &1.0f32.to_le_bytes());
    assert_eq!(&bytes[45..49], &12.0f32.to_le_bytes());
}

#[test]
fn test_matrix3d_user_load_restores_identity_bottom_row() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut saved = Matrix4::identity();
    saved.elements[0] = [1.0, 2.0, 3.0, 4.0];
    saved.elements[1] = [5.0, 6.0, 7.0, 8.0];
    saved.elements[2] = [9.0, 10.0, 11.0, 12.0];

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("matrix3d_user").unwrap();
        xfer_matrix3d_user(&mut save, &mut saved).unwrap();
        save.close().unwrap();
    }

    assert_eq!(bytes.len(), 12 * std::mem::size_of::<f32>());

    let mut loaded = Matrix4 {
        elements: [[99.0; 4]; 4],
    };
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("matrix3d_user").unwrap();
    xfer_matrix3d_user(&mut load, &mut loaded).unwrap();
    load.close().unwrap();

    assert_eq!(loaded.elements[0], [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(loaded.elements[1], [5.0, 6.0, 7.0, 8.0]);
    assert_eq!(loaded.elements[2], [9.0, 10.0, 11.0, 12.0]);
    assert_eq!(loaded.elements[3], [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn test_tint_envelope_xfer_order_matches_cpp() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut saved = TintEnvelope {
        attack_rate: Vector3::new(1.0, 2.0, 3.0),
        decay_rate: Vector3::new(4.0, 5.0, 6.0),
        peak_color: Vector3::new(7.0, 8.0, 9.0),
        current_color: Vector3::new(10.0, 11.0, 12.0),
        sustain_counter: 13,
        state: EnvelopeState::Sustain,
        is_effective: true,
    };

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("tint_envelope").unwrap();
        saved.xfer(&mut save).unwrap();
        save.close().unwrap();
    }

    assert_eq!(
        bytes.len(),
        1 + 12 * std::mem::size_of::<f32>() + 4 + std::mem::size_of::<i32>() + 1
    );
    assert_eq!(bytes[0], 1);
    assert_eq!(&bytes[1..5], &1.0f32.to_le_bytes());
    assert_eq!(&bytes[45..49], &12.0f32.to_le_bytes());
    assert_eq!(&bytes[49..53], &13u32.to_le_bytes());
    assert_eq!(&bytes[53..57], &1i32.to_le_bytes());
    assert_eq!(bytes[57], 3);

    let mut loaded = TintEnvelope::new();
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("tint_envelope").unwrap();
    loaded.xfer(&mut load).unwrap();
    load.close().unwrap();

    assert_eq!(loaded.attack_rate, Vector3::new(1.0, 2.0, 3.0));
    assert_eq!(loaded.current_color, Vector3::new(10.0, 11.0, 12.0));
    assert_eq!(loaded.sustain_counter, 13);
    assert!(loaded.is_effective);
    assert_eq!(loaded.state, EnvelopeState::Sustain);
}

#[test]
fn test_drawable_enum_fields_use_cpp_u32_layout() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_enum_layout").unwrap();

        let mut terrain_decal = terrain_decal_to_u32(TerrainDecalType::ShadowTexture);
        save.xfer_unsigned_int(&mut terrain_decal).unwrap();
        let mut fading_mode = fading_mode_to_u32(FadingMode::FadingOut);
        save.xfer_unsigned_int(&mut fading_mode).unwrap();
        let mut stealth_look = stealth_look_to_u32(StealthLook::Invisible);
        save.xfer_unsigned_int(&mut stealth_look).unwrap();

        save.close().unwrap();
    }

    assert_eq!(bytes.len(), 3 * std::mem::size_of::<u32>());
    assert_eq!(&bytes[0..4], &9u32.to_le_bytes());
    assert_eq!(&bytes[4..8], &2u32.to_le_bytes());
    assert_eq!(&bytes[8..12], &5u32.to_le_bytes());

    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_enum_layout").unwrap();
    let mut terrain_decal = 0;
    load.xfer_unsigned_int(&mut terrain_decal).unwrap();
    let mut fading_mode = 0;
    load.xfer_unsigned_int(&mut fading_mode).unwrap();
    let mut stealth_look = 0;
    load.xfer_unsigned_int(&mut stealth_look).unwrap();
    load.close().unwrap();

    assert_eq!(
        terrain_decal_from_u32(terrain_decal),
        TerrainDecalType::ShadowTexture
    );
    assert_eq!(fading_mode_from_u32(fading_mode), FadingMode::FadingOut);
    assert_eq!(stealth_look_from_u32(stealth_look), StealthLook::Invisible);
}

#[test]
fn test_loco_info_uses_inline_cpp_layout() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut saved = LocoInfo {
        pitch: 1.0,
        pitch_rate: 2.0,
        roll: 3.0,
        roll_rate: 4.0,
        yaw: 5.0,
        acceleration_pitch: 6.0,
        acceleration_pitch_rate: 7.0,
        acceleration_roll: 8.0,
        acceleration_roll_rate: 9.0,
        overlap_z_velocity: 10.0,
        overlap_z: 11.0,
        wobble: 12.0,
        yaw_modulator: 99.0,
        pitch_modulator: 100.0,
        wheel_info: WheelInfo {
            front_left_height_offset: 13.0,
            front_right_height_offset: 14.0,
            rear_left_height_offset: 15.0,
            rear_right_height_offset: 16.0,
            wheel_angle: 17.0,
            frames_airborne_counter: 18,
            frames_airborne: 19,
        },
    };

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("loco_info").unwrap();
        saved.xfer(&mut save).unwrap();
        save.close().unwrap();
    }

    assert_eq!(
        bytes.len(),
        17 * std::mem::size_of::<f32>() + 2 * std::mem::size_of::<i32>()
    );
    assert_eq!(&bytes[0..4], &1.0f32.to_le_bytes());
    assert_eq!(&bytes[44..48], &12.0f32.to_le_bytes());
    assert_eq!(&bytes[48..52], &13.0f32.to_le_bytes());
    assert_eq!(&bytes[64..68], &17.0f32.to_le_bytes());
    assert_eq!(&bytes[68..72], &18i32.to_le_bytes());
    assert_eq!(&bytes[72..76], &19i32.to_le_bytes());

    let mut loaded = LocoInfo::default();
    loaded.yaw_modulator = -1.0;
    loaded.pitch_modulator = -2.0;
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("loco_info").unwrap();
    loaded.xfer(&mut load).unwrap();
    load.close().unwrap();

    assert_eq!(loaded.pitch, 1.0);
    assert_eq!(loaded.wobble, 12.0);
    assert_eq!(loaded.wheel_info.front_left_height_offset, 13.0);
    assert_eq!(loaded.wheel_info.wheel_angle, 17.0);
    assert_eq!(loaded.wheel_info.frames_airborne_counter, 18);
    assert_eq!(loaded.wheel_info.frames_airborne, 19);
    assert_eq!(loaded.yaw_modulator, -1.0);
    assert_eq!(loaded.pitch_modulator, -2.0);
}

#[test]
fn test_drawable_xfer_preserves_instance_matrix() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let instance = Matrix4::translation(Vector3::new(11.0, 22.0, 33.0)).mul(&Matrix4::scale(2.5));
    let mut saved = BasicDrawable::new(DrawableId(77));
    saved.set_instance_transform(instance);
    saved.set_instance_scale(3.0);

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_instance_matrix").unwrap();
        saved.xfer_snapshot(&mut save).unwrap();
        save.close().unwrap();
    }

    let mut loaded = BasicDrawable::new(DrawableId(0));
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_instance_matrix").unwrap();
    loaded.xfer_snapshot(&mut load).unwrap();
    load.close().unwrap();

    assert_eq!(loaded.get_id(), DrawableId(77));
    assert_eq!(loaded.instance_transform, instance);
    assert_eq!(loaded.get_instance_scale(), 3.0);
    assert!(!loaded.is_instance_identity());
}

#[test]
fn test_drawable_xfer_preserves_shroud_status_object_id() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut saved = BasicDrawable::new(DrawableId(88));
    saved.set_shroud_status_object_id(1234);

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_shroud_status_object_id").unwrap();
        saved.xfer_snapshot(&mut save).unwrap();
        save.close().unwrap();
    }

    let mut loaded = BasicDrawable::new(DrawableId(0));
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_shroud_status_object_id").unwrap();
    loaded.xfer_snapshot(&mut load).unwrap();
    load.close().unwrap();

    assert_eq!(loaded.get_id(), DrawableId(88));
    assert_eq!(loaded.shroud_status_object_id(), 1234);
    assert_eq!(loaded.drawable_info().drawable_id, 88);
    assert_eq!(loaded.drawable_info().ghost_object_id, INVALID_ID);
}

#[test]
fn test_drawable_xfer_preserves_ambient_sound_flags() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut saved = BasicDrawable::new(DrawableId(99));
    saved.ambient_sound_enabled = false;
    saved.ambient_sound_enabled_from_script = true;
    saved.custom_sound_ambient_off = true;

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_ambient_sound_flags").unwrap();
        saved.xfer_snapshot(&mut save).unwrap();
        save.close().unwrap();
    }

    let mut loaded = BasicDrawable::new(DrawableId(0));
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_ambient_sound_flags").unwrap();
    loaded.xfer_snapshot(&mut load).unwrap();
    load.close().unwrap();

    assert_eq!(loaded.get_id(), DrawableId(99));
    assert!(!loaded.ambient_sound_enabled);
    assert!(loaded.ambient_sound_enabled_from_script);
    assert!(loaded.custom_sound_ambient_off);
}

#[test]
fn test_drawable_xfer_preserves_custom_ambient_sound_info() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut custom_info = DynamicAudioEventInfo::new();
    custom_info.override_volume(0.75);
    custom_info.override_loop_count(4);

    let mut saved = BasicDrawable::new(DrawableId(100));
    saved.custom_sound_ambient_base_name = Some("UnitAmbientBase".to_string());
    saved.custom_sound_ambient_dynamic_info = Some(custom_info);

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_custom_ambient_sound_info").unwrap();
        saved.xfer_snapshot(&mut save).unwrap();
        save.close().unwrap();
    }

    let mut loaded = BasicDrawable::new(DrawableId(0));
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_custom_ambient_sound_info").unwrap();
    loaded.xfer_snapshot(&mut load).unwrap();
    load.close().unwrap();

    assert_eq!(loaded.get_id(), DrawableId(100));
    assert_eq!(
        loaded.custom_sound_ambient_base_name.as_deref(),
        Some("UnitAmbientBase")
    );
    assert!(!loaded.custom_sound_ambient_off);
    let loaded_info = loaded.custom_sound_ambient_dynamic_info.as_ref().unwrap();
    assert!((loaded_info.get_audio_event_info().volume - 0.75).abs() < f32::EPSILON);
    assert_eq!(loaded_info.get_audio_event_info().loop_count, 4);
}

#[test]
fn drawable_load_post_process_restarts_only_permanent_ambient() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut permanent = DynamicAudioEventInfo::new();
    permanent.override_audio_name("PermanentAmbient");
    permanent.override_loop_flag(true);
    permanent.override_loop_count(0);

    let mut saved = BasicDrawable::new(DrawableId(101));
    saved.custom_sound_ambient_base_name = Some("PermanentAmbient".to_string());
    saved.custom_sound_ambient_dynamic_info = Some(permanent);
    saved.start_ambient_sound(false);
    assert!(saved.ambient_sound_is_active());

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_ambient_post").unwrap();
        saved.xfer_snapshot(&mut save).unwrap();
        save.close().unwrap();
    }

    let mut loaded = BasicDrawable::new(DrawableId(0));
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_ambient_post").unwrap();
    loaded.xfer_snapshot(&mut load).unwrap();
    load.close().unwrap();
    assert!(!loaded.ambient_sound_is_active());
    Snapshotable::load_post_process(&mut loaded).unwrap();
    assert!(loaded.ambient_sound_is_active());

    let mut oneshot = DynamicAudioEventInfo::new();
    oneshot.override_audio_name("OneShotAmbient");
    oneshot.override_loop_flag(false);
    oneshot.override_loop_count(1);
    let mut oneshot_drawable = BasicDrawable::new(DrawableId(102));
    oneshot_drawable.custom_sound_ambient_dynamic_info = Some(oneshot);
    oneshot_drawable.start_ambient_sound(true);
    assert!(!oneshot_drawable.ambient_sound_is_active());
}

#[test]
fn drawable_load_post_process_stops_when_ambient_disabled() {
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    let mut saved = BasicDrawable::new(DrawableId(103));
    saved.ambient_sound_enabled = false;
    saved.ambient_sound_enabled_from_script = true;

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut save = XferSave::new(cursor, 1);
        save.open("drawable_ambient_disabled").unwrap();
        saved.xfer_snapshot(&mut save).unwrap();
        save.close().unwrap();
    }

    let mut loaded = BasicDrawable::new(DrawableId(0));
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    load.open("drawable_ambient_disabled").unwrap();
    loaded.xfer_snapshot(&mut load).unwrap();
    load.close().unwrap();
    Snapshotable::load_post_process(&mut loaded).unwrap();
    assert!(!loaded.ambient_sound_is_active());
    assert!(!loaded.ambient_sound_enabled);
}

#[test]
fn test_model_condition_flags_flow_into_render_flags() {
    use crate::render_bridge::RenderConditionFlags;
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.set_model_condition_state(ModelConditionFlags::DAMAGED);
    drawable.set_model_condition_state(ModelConditionFlags::SNOW);
    drawable.set_model_condition_state(ModelConditionFlags::AFLAME);
    drawable.set_model_condition_state(ModelConditionFlags::TOPPLED);

    let render_flags = drawable.compute_render_condition_flags();
    assert!(render_flags.contains(RenderConditionFlags::DAMAGED));
    assert!(render_flags.contains(RenderConditionFlags::SNOW));
    assert!(render_flags.contains(RenderConditionFlags::AFLAME));
    assert!(render_flags.contains(RenderConditionFlags::TOPPLED));
}

#[test]
fn react_to_body_damage_sets_exclusive_damage_condition_bits() {
    use gamelogic::common::types::BodyDamageType;

    let mut drawable = BasicDrawable::new(DrawableId(42));
    // Seed a non-damage flag that must survive.
    drawable.set_model_condition_state(ModelConditionFlags::MOVING);
    // Seed a stale damage bit that must clear on transition.
    drawable.set_model_condition_state(ModelConditionFlags::DAMAGED);

    drawable.react_to_body_damage_state_change(BodyDamageType::ReallyDamaged);
    let flags = drawable.get_model_condition_flags();
    assert!(flags.test(ModelConditionFlags::REALLYDAMAGED));
    assert!(!flags.test(ModelConditionFlags::DAMAGED));
    assert!(!flags.test(ModelConditionFlags::RUBBLE));
    assert!(
        flags.test(ModelConditionFlags::MOVING),
        "non-damage flags survive clear-and-set"
    );

    drawable.react_to_body_damage_state_change(BodyDamageType::Pristine);
    let flags = drawable.get_model_condition_flags();
    assert!(!flags.test(ModelConditionFlags::DAMAGED));
    assert!(!flags.test(ModelConditionFlags::REALLYDAMAGED));
    assert!(!flags.test(ModelConditionFlags::RUBBLE));
    assert!(flags.test(ModelConditionFlags::MOVING));
}

#[test]
fn is_object_kind_of_fail_closed_when_dual_world_empty_and_no_presentation_kinds() {
    let drawable = BasicDrawable::new(DrawableId(11));
    assert!(
        drawable.presentation_kind_names.is_empty(),
        "new drawable has no presentation KindOf residual"
    );
    assert!(
        !drawable.is_object_kind_of(gamelogic::common::types::KindOf::Structure),
        "Wave 270: empty dual-world + unstamped kinds must fail-closed"
    );
}

#[test]
fn draw_construct_percent_fail_closed_on_sold_not_dead_health() {
    // C++ drawConstructPercent: OBJECT_STATUS_SOLD clears the overlay;
    // isEffectivelyDead is commented out, so 0 health still shows UC text.
    let region = IRegion2D::new(ICoord2D::new(10, 20), ICoord2D::new(74, 32));

    let mut building = BasicDrawable::new(DrawableId(8));
    building.overlay_data.health_region = Some(region);
    building.presentation_under_construction = true;
    building.presentation_construction_percent = 0.42;
    building.presentation_sold = false;
    building.presentation_health_pct = 0.0;
    building.overlay_data.is_under_construction = true;
    building.overlay_data.construction_percent = 0.42;
    building.draw_icon_ui();
    assert!(
        building.overlay_data.is_under_construction,
        "dead health must not clear construct overlay (C++ dead check commented out)"
    );
    assert!(
        (building.overlay_data.construction_percent - 0.42).abs() < 0.0001,
        "construct percent stays at presentation residual when not sold"
    );

    building.presentation_sold = true;
    building.draw_icon_ui();
    assert!(
        !building.overlay_data.is_under_construction,
        "sold must fail-closed construct overlay"
    );
    assert_eq!(building.overlay_data.construction_percent, 0.0);

    let mut complete = BasicDrawable::new(DrawableId(9));
    complete.overlay_data.health_region = Some(region);
    complete.presentation_under_construction = false;
    complete.presentation_construction_percent = 0.8;
    complete.presentation_sold = false;
    complete.presentation_health_pct = 1.0;
    complete.overlay_data.is_under_construction = true;
    complete.overlay_data.construction_percent = 0.8;
    complete.draw_icon_ui();
    assert!(!complete.overlay_data.is_under_construction);
    assert_eq!(complete.overlay_data.construction_percent, 0.0);

    let mut sold_setter = BasicDrawable::new(DrawableId(10));
    sold_setter.overlay_data.is_under_construction = true;
    sold_setter.overlay_data.construction_percent = 0.3;
    sold_setter.set_presentation_sold(true);
    assert!(!sold_setter.overlay_data.is_under_construction);
    assert_eq!(sold_setter.overlay_data.construction_percent, 0.0);
}

#[test]
fn compute_health_region_falls_back_to_seeded_region_without_object() {
    let mut drawable = BasicDrawable::new(DrawableId(7));
    assert!(drawable.compute_health_region().is_none());

    let seeded = IRegion2D::new(ICoord2D::new(10, 20), ICoord2D::new(74, 32));
    drawable.overlay_data.health_region = Some(seeded);
    assert_eq!(drawable.compute_health_region(), Some(seeded));

    // draw_icon_ui must leave overlay visible when a region is present + caption set.
    drawable.set_caption_text("UnitHealth");
    drawable.draw_icon_ui();
    assert!(drawable.overlay_data.visible);
    assert_eq!(drawable.overlay_data.caption.as_deref(), Some("UnitHealth"));
    assert!(
        drawable.overlay_data.health_region.is_some(),
        "health bar region remains observable after icon UI"
    );
}

#[test]
fn test_model_draw_bits_flow_into_render_flags() {
    use crate::render_bridge::RenderConditionFlags;
    use gamelogic::common::ModelConditionFlags as LogicModelConditionFlags;

    let bits = (LogicModelConditionFlags::DAMAGED | LogicModelConditionFlags::SNOW).bits();
    let render_flags = BasicDrawable::render_condition_flags_from_bits(bits);

    assert!(render_flags.contains(RenderConditionFlags::DAMAGED));
    assert!(render_flags.contains(RenderConditionFlags::SNOW));
}

#[test]
fn test_model_draw_animation_mode_mapping_matches_logic_discriminants() {
    assert_eq!(
        BasicDrawable::animation_mode_from_model_draw(0),
        Some(ww3d_core::animation::AnimationMode::Manual)
    );
    assert_eq!(
        BasicDrawable::animation_mode_from_model_draw(1),
        Some(ww3d_core::animation::AnimationMode::Loop)
    );
    assert_eq!(
        BasicDrawable::animation_mode_from_model_draw(2),
        Some(ww3d_core::animation::AnimationMode::Once)
    );
    assert_eq!(
        BasicDrawable::animation_mode_from_model_draw(3),
        Some(ww3d_core::animation::AnimationMode::LoopPingPong)
    );
    assert_eq!(
        BasicDrawable::animation_mode_from_model_draw(4),
        Some(ww3d_core::animation::AnimationMode::LoopBackward)
    );
    assert_eq!(
        BasicDrawable::animation_mode_from_model_draw(5),
        Some(ww3d_core::animation::AnimationMode::OnceBackward)
    );
    assert_eq!(BasicDrawable::animation_mode_from_model_draw(99), None);
}

#[test]
fn test_model_draw_bone_override_preserves_index_and_transform() {
    let transform = glam::Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0));
    let override_state = BoneOverrideState {
        bone_index: 7,
        transform,
    };

    let render_override = BasicDrawable::bone_override_from_model_draw(&override_state);

    assert_eq!(render_override.bone_index, 7);
    assert_eq!(render_override.transform, transform);
}

#[test]
fn test_render_state_from_flags_preserves_condition_overrides() {
    use crate::render_bridge::RenderConditionFlags;

    let flags = RenderConditionFlags::NIGHT
        | RenderConditionFlags::SNOW
        | RenderConditionFlags::DAMAGED
        | RenderConditionFlags::PARTIALLY_CONSTRUCTED
        | RenderConditionFlags::AFLAME;

    let state =
        BasicDrawable::render_state_from_flags(flags, 0.75, Vector3::new(0.2, 0.8, 0.1), true);

    assert!(state.apply_night_map);
    assert!(state.apply_snow_map);
    assert_eq!(state.construction_tint, Some([0.5, 0.5, 0.5]));
    assert!((state.damage_overlay - 0.5).abs() < f32::EPSILON);
    assert!((state.opacity - 0.7).abs() < f32::EPSILON);
    assert!(state.selected);
    assert!((state.emissive_tint[0] - 1.2).abs() < f32::EPSILON);
    assert!((state.emissive_tint[1] - 1.2).abs() < f32::EPSILON);
    assert!((state.emissive_tint[2] - 0.15).abs() < f32::EPSILON);
}

#[test]
fn test_shadow_status_enabled_after_create() {
    // Residual: shadow enable is observable after create without GPU mesh alloc.
    let drawable = BasicDrawable::new(DrawableId(1));
    assert!(
        drawable.get_shadows_enabled(),
        "create-time DRAWABLE_STATUS_SHADOWS must be set"
    );
    assert!(drawable.get_status().has(DrawableStatus::SHADOWS));
}

#[test]
fn test_shadow_toggle_helpers() {
    let mut drawable = BasicDrawable::new(DrawableId(1));
    assert!(drawable.get_shadows_enabled());

    drawable.set_shadows_enabled(false);
    assert!(!drawable.get_shadows_enabled());
    assert!(!drawable.get_status().has(DrawableStatus::SHADOWS));

    drawable.set_shadows_enabled(true);
    assert!(drawable.get_shadows_enabled());
    assert!(drawable.get_status().has(DrawableStatus::SHADOWS));

    // allocate/release are Options-screen resource hooks and must not flip status
    // (C++ Drawable::allocateShadows/releaseShadows only touch draw modules).
    drawable.set_shadows_enabled(false);
    drawable.allocate_shadows();
    assert!(
        !drawable.get_shadows_enabled(),
        "allocate_shadows must not set status bits"
    );
    drawable.set_shadows_enabled(true);
    drawable.release_shadows();
    assert!(
        drawable.get_shadows_enabled(),
        "release_shadows must not clear status bits"
    );
}

#[test]
fn test_model_condition_change_preserves_shadow_status() {
    // Model condition swaps must not clear shadow enable (C++ keeps m_shadowEnabled
    // and only reallocates mesh resources on condition change).
    let mut drawable = BasicDrawable::new(DrawableId(42));
    assert!(drawable.get_shadows_enabled());

    drawable.react_to_body_damage_state_change(gamelogic::common::types::BodyDamageType::Damaged);
    assert!(
        drawable.get_shadows_enabled(),
        "damage model condition must not clear SHADOWS status"
    );

    drawable.react_to_body_damage_state_change(gamelogic::common::types::BodyDamageType::Rubble);
    assert!(drawable.get_shadows_enabled());

    drawable.set_shadows_enabled(false);
    drawable.react_to_body_damage_state_change(gamelogic::common::types::BodyDamageType::Pristine);
    assert!(
        !drawable.get_shadows_enabled(),
        "explicit disable must survive model condition updates"
    );
}

#[test]
fn test_set_shadows_enabled_dispatches_to_draw_modules() {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    static ENABLED: AtomicBool = AtomicBool::new(true);
    static ALLOC_CALLS: AtomicU32 = AtomicU32::new(0);
    static RELEASE_CALLS: AtomicU32 = AtomicU32::new(0);

    ENABLED.store(true, Ordering::SeqCst);
    ALLOC_CALLS.store(0, Ordering::SeqCst);
    RELEASE_CALLS.store(0, Ordering::SeqCst);

    #[derive(Debug)]
    struct ShadowDispatchModule;
    impl DrawModule for ShadowDispatchModule {
        fn set_shadows_enabled(&mut self, enable: bool) {
            ENABLED.store(enable, Ordering::SeqCst);
        }
        fn allocate_shadows(&mut self) {
            ALLOC_CALLS.fetch_add(1, Ordering::SeqCst);
        }
        fn release_shadows(&mut self) {
            RELEASE_CALLS.fetch_add(1, Ordering::SeqCst);
        }
    }

    let mut drawable = BasicDrawable::new(DrawableId(7));
    drawable.add_draw_module(Box::new(ShadowDispatchModule));

    drawable.set_shadows_enabled(false);
    assert!(!ENABLED.load(Ordering::SeqCst));
    assert!(!drawable.get_shadows_enabled());

    drawable.set_shadows_enabled(true);
    assert!(ENABLED.load(Ordering::SeqCst));
    assert!(drawable.get_shadows_enabled());

    drawable.allocate_shadows();
    drawable.release_shadows();
    assert_eq!(ALLOC_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(RELEASE_CALLS.load(Ordering::SeqCst), 1);
    // Status remains enable-controlled, not allocate/release-controlled.
    assert!(drawable.get_shadows_enabled());
}

#[test]
fn weapon_fire_recoil_subtracts_bound_object_orientation() {
    use gamelogic::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use gamelogic::object::Object;
    use std::sync::Arc;

    let object_id = 900_001;
    let template = Arc::new(DefaultThingTemplate::new("DrawableRecoilTest".to_string()));
    let object =
        Object::new_with_id(template, object_id, ObjectStatusMaskType::none(), None).unwrap();
    object
        .write()
        .unwrap()
        .set_orientation(std::f32::consts::FRAC_PI_2)
        .unwrap();

    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.set_object_id(Some(object_id));
    drawable.loco_info = Some(LocoInfo::default());

    drawable.handle_weapon_fire_fx(
        WeaponSlotType::Primary,
        0,
        None,
        0.0,
        2.0,
        std::f32::consts::FRAC_PI_2,
        None,
        0.0,
    );

    let loco = drawable.get_loco_info().unwrap();
    assert_near(loco.acceleration_pitch_rate, -2.0);
    assert_near(loco.acceleration_roll_rate, 0.0);

    OBJECT_REGISTRY.unregister_object(object_id);
}

#[test]
fn weapon_fire_dispatch_reports_first_handled_draw_module_index() {
    let first_calls = Arc::new(Mutex::new(0));
    let second_calls = Arc::new(Mutex::new(0));
    let mut drawable = BasicDrawable::new(DrawableId(900_003));
    drawable.add_draw_module(Box::new(WeaponFireDispatchTestDrawModule {
        handled: false,
        calls: Arc::clone(&first_calls),
    }));
    drawable.add_draw_module(Box::new(WeaponFireDispatchTestDrawModule {
        handled: true,
        calls: Arc::clone(&second_calls),
    }));

    let fx_name = "FireFX_Test";
    let handled_by = drawable.handle_weapon_fire_fx_with_module_index(
        WeaponSlotType::Primary,
        0,
        Some(&fx_name),
        0.0,
        0.0,
        0.0,
        None,
        0.0,
    );

    assert_eq!(handled_by, Some(1));
    assert_eq!(*first_calls.lock(), 1);
    assert_eq!(*second_calls.lock(), 1);
}

#[test]
fn weapon_fire_dispatch_without_handler_visits_all_modules_in_order() {
    let first_calls = Arc::new(Mutex::new(0));
    let second_calls = Arc::new(Mutex::new(0));
    let mut drawable = BasicDrawable::new(DrawableId(900_004));
    drawable.add_draw_module(Box::new(WeaponFireDispatchTestDrawModule {
        handled: false,
        calls: Arc::clone(&first_calls),
    }));
    drawable.add_draw_module(Box::new(WeaponFireDispatchTestDrawModule {
        handled: false,
        calls: Arc::clone(&second_calls),
    }));

    let handled_by = drawable.handle_weapon_fire_fx_with_module_index(
        WeaponSlotType::Primary,
        0,
        None,
        0.0,
        0.0,
        0.0,
        None,
        0.0,
    );

    assert_eq!(handled_by, None);
    assert_eq!(*first_calls.lock(), 1);
    assert_eq!(*second_calls.lock(), 1);
}

#[test]
fn render_does_not_apply_persisted_loco_info_without_exact_frozen_source() {
    #[derive(Debug)]
    struct TransformCaptureModule {
        observed: Arc<Mutex<Option<Matrix4>>>,
    }

    impl DrawModule for TransformCaptureModule {
        fn do_draw(&mut self, transform: &Matrix4, _view: &Matrix4, _projection: &Matrix4) {
            *self.observed.lock() = Some(*transform);
        }
    }

    let observed = Arc::new(Mutex::new(None));
    let mut drawable = BasicDrawable::new(DrawableId(900_002));
    drawable.set_position(Vector3::new(12.0, -4.0, 7.0));
    drawable.set_instance_transform(Matrix4::rotation_z(0.25));
    drawable.loco_info = Some(LocoInfo {
        pitch: 0.4,
        roll: -0.3,
        yaw: 0.2,
        acceleration_pitch: 0.15,
        acceleration_roll: -0.1,
        overlap_z: 5.0,
        ..LocoInfo::default()
    });
    drawable.add_draw_module(Box::new(TransformCaptureModule {
        observed: observed.clone(),
    }));

    let expected = drawable.get_transform();
    drawable.render(&Matrix4::identity(), &Matrix4::identity());

    assert_eq!(
        *observed.lock(),
        Some(expected),
        "persisted LocoInfo is client history, not a complete frozen calcPhysicsXform input"
    );
}

#[test]
fn projectile_launch_offset_forwards_pitch_pointer_to_draw_module() {
    let observed_pitch_pointer = Arc::new(Mutex::new(false));
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.add_draw_module(Box::new(ProjectileLaunchTestDrawModule {
        observed_pitch_pointer: observed_pitch_pointer.clone(),
    }));

    let mut launch_pos = Matrix4::identity();
    let mut turret_rot_pos = Vector3::zero();
    let mut turret_pitch_pos = Vector3::zero();

    assert!(drawable.get_projectile_launch_offset(
        WeaponSlotType::Primary,
        1,
        &mut launch_pos,
        WhichTurretType::Main,
        &mut turret_rot_pos,
        Some(&mut turret_pitch_pos),
    ));

    assert!(*observed_pitch_pointer.lock());
    assert_eq!(
        launch_pos,
        Matrix4::translation(Vector3::new(10.0, 20.0, 30.0))
    );
    assert_eq!(turret_rot_pos, Vector3::new(1.0, 2.0, 3.0));
    assert_eq!(turret_pitch_pos, Vector3::new(4.0, 5.0, 6.0));
}

#[test]
fn draw_bombed_carbomb_icon_requires_local_player_not_status() {
    use gamelogic::common::{ObjectStatusMaskType, ObjectStatusTypes};
    use gamelogic::object::Object;
    use gamelogic::weapon::WeaponSetType;
    use std::sync::{Arc, RwLock};

    let object_id = 900_002;
    let object = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
    object
        .write()
        .unwrap()
        .set_weapon_set_flag(WeaponSetType::CarBomb);
    object.write().unwrap().set_status(
        ObjectStatusMaskType::from_status(ObjectStatusTypes::IsCarBomb),
        true,
    );
    OBJECT_REGISTRY.register_object(object_id, &object);

    let mut drawable = BasicDrawable::new(DrawableId(2));
    drawable.set_object_id(Some(object_id));
    drawable.overlay_data.show_bombed = true;
    drawable.overlay_data.bomb_type = 3;

    drawable.draw_bombed(&IRegion2D::default());

    // C++ also requires controllingPlayer == localPlayer. new_test objects
    // are not locally controlled, so the icon must stay off (no intel leak).
    assert!(!drawable.overlay_data.show_bombed);
    assert_eq!(drawable.overlay_data.bomb_type, 0);

    OBJECT_REGISTRY.unregister_object(object_id);
}

#[test]
fn visible_friendly_stealth_uses_module_friendly_opacity_like_cpp() {
    use gamelogic::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use gamelogic::object::Object;
    use gamelogic::stealth_update::{StealthUpdateModule, StealthUpdateModuleData};

    let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
    let original_global_opacity = global_data.read().stealth_friendly_opacity;
    global_data.write().stealth_friendly_opacity = 0.25;

    let object_id = 900_003;
    let template = Arc::new(DefaultThingTemplate::new(
        "DrawableStealthOpacityTest".to_string(),
    ));
    let _object =
        Object::new_with_id(template, object_id, ObjectStatusMaskType::none(), None).unwrap();

    let mut stealth_module =
        StealthUpdateModule::new(0, Arc::new(StealthUpdateModuleData::default()), object_id);
    stealth_module.on_object_created();

    let mut drawable = BasicDrawable::new(DrawableId(3));
    drawable.set_object_id(Some(object_id));
    drawable.apply_stealth_look(StealthLook::VisibleFriendly);

    assert_near(drawable.stealth_opacity, 0.5);
    assert!(!drawable.hidden_by_stealth);
    assert_eq!(drawable.second_material_pass_opacity, 0.0);

    OBJECT_REGISTRY.unregister_object(object_id);
    global_data.write().stealth_friendly_opacity = original_global_opacity;
}

#[test]
fn disguised_friendly_stealth_leaves_opacity_and_second_pass_like_cpp_break() {
    use gamelogic::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use gamelogic::object::Object;
    use gamelogic::stealth_update::{StealthUpdateModule, StealthUpdateModuleData};

    let object_id = 900_004;
    let template = Arc::new(DefaultThingTemplate::new(
        "DrawableStealthDisguiseTest".to_string(),
    ));
    let _object =
        Object::new_with_id(template, object_id, ObjectStatusMaskType::none(), None).unwrap();

    let mut stealth_module =
        StealthUpdateModule::new(0, Arc::new(StealthUpdateModuleData::default()), object_id);
    stealth_module.on_object_created();
    stealth_module
        .get_stealth_disguise_control_interface()
        .unwrap()
        .disguise_as_template(Some("ChinaVehicleTroopCrawler".to_string()), 0);

    let mut drawable = BasicDrawable::new(DrawableId(4));
    drawable.set_object_id(Some(object_id));
    drawable.second_material_pass_opacity = 0.75;
    drawable.apply_stealth_look(StealthLook::VisibleFriendlyDetected);

    assert_near(drawable.stealth_opacity, 1.0);
    assert!(!drawable.hidden_by_stealth);
    assert_eq!(drawable.stealth_look, StealthLook::VisibleFriendlyDetected);
    assert_near(drawable.second_material_pass_opacity, 0.75);

    OBJECT_REGISTRY.unregister_object(object_id);
}

#[test]
fn test_vector3_operations() {
    let v1 = Vector3::new(1.0, 2.0, 3.0);
    let v2 = Vector3::zero();
    let v3 = Vector3::one();

    assert_eq!(v2, Vector3::new(0.0, 0.0, 0.0));
    assert_eq!(v3, Vector3::new(1.0, 1.0, 1.0));
    assert_ne!(v1, v2);
}

#[test]
fn test_matrix4_operations() {
    let identity = Matrix4::identity();
    let translation = Matrix4::translation(Vector3::new(1.0, 2.0, 3.0));
    let scale = Matrix4::scale(2.0);

    // Check identity matrix
    for i in 0..4 {
        for j in 0..4 {
            if i == j {
                assert_eq!(identity.elements[i][j], 1.0);
            } else {
                assert_eq!(identity.elements[i][j], 0.0);
            }
        }
    }

    // Check translation matrix
    assert_eq!(translation.elements[0][3], 1.0);
    assert_eq!(translation.elements[1][3], 2.0);
    assert_eq!(translation.elements[2][3], 3.0);

    // Check scale matrix
    assert_eq!(scale.elements[0][0], 2.0);
    assert_eq!(scale.elements[1][1], 2.0);
    assert_eq!(scale.elements[2][2], 2.0);
}

#[test]
fn legacy_matrix4_bridge_conversion_preserves_cpp_affine_semantics() {
    let legacy = Matrix4::translation(Vector3::new(1.0, 2.0, 3.0))
        .mul(&Matrix4::rotation_z(std::f32::consts::FRAC_PI_2));

    let rendered = legacy.to_glam();
    let transformed = rendered.transform_point3(glam::Vec3::X);
    assert_near(transformed.x, 1.0);
    assert_near(transformed.y, 3.0);
    assert_near(transformed.z, 3.0);

    let recovered = Matrix4::from_glam(rendered);
    assert_eq!(
        recovered, legacy,
        "bridge conversion must round-trip Xfer layout"
    );
}

#[derive(Debug)]
struct ConditionAndHiddenTestModule {
    hidden: Arc<Mutex<Option<bool>>>,
    conditions: Arc<Mutex<Option<u32>>>,
}

impl DrawModule for ConditionAndHiddenTestModule {
    fn set_hidden(&mut self, hidden: bool) {
        *self.hidden.lock() = Some(hidden);
    }

    fn replace_model_condition_state(
        &mut self,
        flags: &game_engine::common::bit_flags::ModelConditionBitFlags,
    ) {
        *self.conditions.lock() = Some(flags.count() as u32);
    }
}

#[test]
fn set_effective_opacity_does_not_overwrite_explicit_opacity() {
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.set_opacity(1.0);
    drawable.set_effective_opacity(0.0, Some(0.5));
    assert_near(drawable.get_explicit_opacity(), 1.0);
    assert_near(drawable.get_effective_opacity(), 0.5);
}

#[test]
fn detected_stealth_does_not_invent_first_pass_opacity() {
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.apply_stealth_look(StealthLook::VisibleDetected);
    assert!(!drawable.is_effectively_hidden());
    assert_near(drawable.get_opacity(), 1.0);
    assert_near(drawable.second_material_pass_opacity, 1.0);
}

#[test]
fn stealth_invisible_hides_modules_and_deselects() {
    let hidden = Arc::new(Mutex::new(None));
    let conditions = Arc::new(Mutex::new(None));
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.add_draw_module(Box::new(ConditionAndHiddenTestModule {
        hidden: Arc::clone(&hidden),
        conditions: Arc::clone(&conditions),
    }));
    drawable.set_selected(true);
    assert!(drawable.is_selected());

    drawable.apply_stealth_look(StealthLook::Invisible);
    assert!(drawable.hidden_by_stealth);
    assert!(!drawable.is_selected());
    assert_eq!(*hidden.lock(), Some(true));
}

#[test]
fn condition_flags_dirty_apply_to_draw_modules() {
    let hidden = Arc::new(Mutex::new(None));
    let conditions = Arc::new(Mutex::new(None));
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.add_draw_module(Box::new(ConditionAndHiddenTestModule {
        hidden: Arc::clone(&hidden),
        conditions: Arc::clone(&conditions),
    }));

    let mut set = game_engine::common::bit_flags::create_model_condition_flags();
    set.set(ModelConditionFlags::NIGHT, true);
    set.set(ModelConditionFlags::SNOW, true);
    drawable.clear_and_set_model_condition_flags(
        &game_engine::common::bit_flags::create_model_condition_flags(),
        &set,
    );
    assert!(conditions.lock().is_none());

    let _ = drawable.get_draw_modules_mut();
    assert_eq!(*conditions.lock(), Some(2));

    drawable.clear_model_condition_state(ModelConditionFlags::NIGHT);
    let _ = drawable.get_draw_modules_mut();
    assert_eq!(*conditions.lock(), Some(1));
}

#[test]
fn signed_disabled_tint_fades_in_instead_of_snapping() {
    let mut envelope = TintEnvelope::new();
    envelope.play(DARK_GRAY_DISABLED_COLOR, 30, 30, SUSTAIN_INDEFINITELY);
    envelope.update();
    let color = envelope.color();
    assert!(color.x < 0.0);
    assert!(color.x > DARK_GRAY_DISABLED_COLOR.x);
    assert!(envelope.is_effective);

    let state = BasicDrawable::render_state_from_flags(
        crate::render_bridge::RenderConditionFlags::empty(),
        1.0,
        DARK_GRAY_DISABLED_COLOR,
        false,
    );
    assert!((state.emissive_tint[0] + 0.5).abs() < f32::EPSILON);
    assert!((state.emissive_tint[1] + 0.5).abs() < f32::EPSILON);
    assert!((state.emissive_tint[2] + 0.5).abs() < f32::EPSILON);
}

#[test]
fn selection_flash_uses_saturated_white_and_survives_deselect() {
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.flash_as_selected(None);
    let envelope = drawable.selection_flash_envelope.as_ref().unwrap();
    // saturateRGB(white, 0.5) = (0.25, 0.25, 0.25)
    assert_near(envelope.peak_color.x, 0.25);
    assert_near(envelope.peak_color.y, 0.25);
    assert_near(envelope.peak_color.z, 0.25);

    drawable.set_selected(true);
    drawable.set_selected(true);
    drawable.set_selected(false);
    assert!(drawable.selection_flash_envelope.is_some());
}

#[test]
fn color_tint_envelope_ticks_and_is_sampled_by_get_tint_color() {
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.color_flash_envelope(Some(Vector3::new(1.0, 0.0, 0.0)), 4, 1, 0);
    drawable.update(0.0);
    let tint = drawable.get_tint_color();
    assert!(tint.x > 0.0);
    assert!(drawable.tint_color_effect().is_some());
}

#[test]
fn frenzy_status_plays_vehicle_frenzy_color_without_infantry_kind() {
    let mut drawable = BasicDrawable::new(DrawableId(1));
    drawable.set_tint_status(TintStatus::FRENZY);
    drawable.update(0.0);
    let envelope = drawable.tint_envelope.as_ref().expect("frenzy envelope");
    assert_near(envelope.peak_color.x, FRENZY_COLOR.x);
    assert_near(envelope.peak_color.y, FRENZY_COLOR.y);
    assert_near(envelope.peak_color.z, FRENZY_COLOR.z);
}

#[test]
fn frenzy_infantry_color_matches_cpp_authored_values() {
    assert_near(FRENZY_COLOR_INFANTRY.x, 0.0);
    assert_near(FRENZY_COLOR_INFANTRY.y, -0.7);
    assert_near(FRENZY_COLOR_INFANTRY.z, -0.7);
}

#[test]
fn start_ambient_sound_uses_template_when_custom_info_absent() {
    // C++ Drawable::startAmbientSound (`Drawable.cpp:4459-4468`) reads
    // getAmbientSoundByDamage / template SoundAmbient when custom info is null.
    let mut drawable = BasicDrawable::new(DrawableId(7));
    drawable.custom_sound_ambient_dynamic_info = None;
    drawable.start_ambient_sound(false);
    // No template registered → no event, but must not require custom_info.
    assert!(!drawable.ambient_sound_is_active());

    let mut custom = DynamicAudioEventInfo::new();
    custom.override_audio_name("FactoryHum");
    custom.override_loop_flag(true);
    custom.override_loop_count(0);
    drawable.custom_sound_ambient_dynamic_info = Some(custom);
    drawable.start_ambient_sound(false);
    assert!(drawable.ambient_sound_is_active());
}

#[test]
fn set_time_of_day_queues_ambient_restart() {
    // C++ Drawable::setTimeOfDay (`Drawable.cpp:4344-4350`) restarts ambient.
    let mut drawable = BasicDrawable::new(DrawableId(8));
    let mut custom = DynamicAudioEventInfo::new();
    custom.override_audio_name("NightHum");
    custom.override_loop_flag(true);
    custom.override_loop_count(0);
    drawable.custom_sound_ambient_dynamic_info = Some(custom);
    drawable
        .set_time_of_day(crate::system::TimeOfDay::Night)
        .unwrap();
    drawable.update(0.0);
    assert!(drawable.ambient_sound_is_active());
    assert!(
        drawable
            .get_model_condition_flags()
            .test(ModelConditionFlags::NIGHT)
    );
}

#[test]
fn health_bar_requires_selected_and_show_object_health() {
    game_engine::common::ini::init_global_data();

    // C++ Drawable::drawHealthBar (`Drawable.cpp:3834-3849`).
    let region = IRegion2D::new(ICoord2D::new(10, 20), ICoord2D::new(74, 32));
    let mut drawable = BasicDrawable::new(DrawableId(9));
    drawable.overlay_data.health_region = Some(region);
    drawable.presentation_health_pct = 0.8;
    drawable.presentation_selected = false;
    drawable.selected = false;
    drawable.draw_icon_ui();
    assert!(
        !drawable.overlay_data.health_bar_visible,
        "unselected drawable must not draw a health bar"
    );

    drawable.selected = true;
    drawable.presentation_selected = true;
    drawable.draw_icon_ui();
    // ShowObjectHealth defaults false (C++ GlobalData.cpp:795).
    assert!(!drawable.overlay_data.health_bar_visible);

    if let Some(data) = game_engine::common::ini::get_global_data() {
        data.write().show_object_health = true;
    }
    drawable.draw_icon_ui();
    assert!(drawable.overlay_data.health_bar_visible);
    assert!((drawable.overlay_data.health_ratio - 0.8).abs() < 0.0001);
    if let Some(data) = game_engine::common::ini::get_global_data() {
        data.write().show_object_health = false;
    }
}

#[test]
fn health_bar_force_attackable_is_hidden() {
    game_engine::common::ini::init_global_data();

    if let Some(data) = game_engine::common::ini::get_global_data() {
        data.write().show_object_health = true;
    }
    let region = IRegion2D::new(ICoord2D::new(10, 20), ICoord2D::new(74, 32));
    let mut drawable = BasicDrawable::new(DrawableId(10));
    drawable.overlay_data.health_region = Some(region);
    drawable.presentation_health_pct = 1.0;
    drawable.selected = true;
    drawable.presentation_selected = true;
    drawable.presentation_kind_names = vec!["ForceAttackable".to_string()];
    drawable.draw_icon_ui();
    assert!(!drawable.overlay_data.health_bar_visible);
    if let Some(data) = game_engine::common::ini::get_global_data() {
        data.write().show_object_health = false;
    }
}

#[test]
fn health_bar_construction_uses_cyan_channel() {
    // C++ GameMakeColor(0, healthRatio*255, 255, 255) (`Drawable.cpp:3872-3875`).
    let (fill, _) = health_bar_colors(0.5, true, false, false);
    assert_eq!(fill[0], 0.0);
    assert!((fill[1] - 0.5).abs() < 0.0001);
    assert_eq!(fill[2], 1.0);
    let (pristine, _) = health_bar_colors(1.0, false, false, false);
    assert!(pristine[1] > pristine[0], "pristine averages toward green");
    let (really, _) = health_bar_colors(0.6, false, true, true);
    assert!(really[0] > really[1], "really-damaged averages toward red");
}

#[test]
fn construct_percent_uses_under_construction_desc() {
    // C++ Drawable::drawConstructPercent (`Drawable.cpp:3707`).
    let text = format_under_construction_desc(0.42);
    assert!(
        text.contains("42"),
        "formatted construct text must include percent: {text}"
    );
    assert!(
        !text.eq("42%"),
        "must not be the raw percent-only live HUD string"
    );
}

#[test]
fn ammo_pips_require_show_object_health_and_local() {
    game_engine::common::ini::init_global_data();

    let region = IRegion2D::new(ICoord2D::new(10, 20), ICoord2D::new(74, 32));
    let mut drawable = BasicDrawable::new(DrawableId(11));
    drawable.overlay_data.health_region = Some(region);
    drawable.presentation_health_pct = 1.0;
    drawable.presentation_ammo_pip_total = 5;
    drawable.presentation_ammo_pip_full = 2;
    drawable.selected = true;
    drawable.presentation_selected = true;
    drawable.draw_icon_ui();
    assert!(!drawable.overlay_data.show_ammo);

    if let Some(data) = game_engine::common::ini::get_global_data() {
        data.write().show_object_health = true;
    }
    drawable.draw_icon_ui();
    assert!(drawable.overlay_data.show_ammo);
    assert_eq!(drawable.overlay_data.ammo_total, 5);
    if let Some(data) = game_engine::common::ini::get_global_data() {
        data.write().show_object_health = false;
    }
}

#[test]
fn draw_icon_ui_honors_draw_icon_ui_gate() {
    // C++ Drawable::drawIconUI (`Drawable.cpp:2740`).
    let region = IRegion2D::new(ICoord2D::new(10, 20), ICoord2D::new(74, 32));
    let mut drawable = BasicDrawable::new(DrawableId(12));
    drawable.overlay_data.health_region = Some(region);
    drawable.set_caption_text("Keep");
    drawable.overlay_data.visible = true;
    gamelogic::helpers::TheGameLogic::set_draw_icon_ui(false);
    drawable.draw_icon_ui();
    assert!(
        !drawable.overlay_data.visible,
        "icon UI must no-op when getDrawIconUI is false"
    );
    gamelogic::helpers::TheGameLogic::set_draw_icon_ui(true);
}

#[test]
fn draws_any_ui_text_for_selected_hotkey_group() {
    // C++ Drawable::drawsAnyUIText (`Drawable.cpp:2709-2729`).
    let mut drawable = BasicDrawable::new(DrawableId(13));
    drawable.selected = true;
    drawable.presentation_selected = true;
    drawable.set_presentation_hotkey_group(2);
    assert!(drawable.draws_any_ui_text());
    drawable.draw_icon_ui();
    assert!(drawable.overlay_data.queue_ui_text);
    assert_eq!(drawable.overlay_data.group_numeral.as_deref(), Some("2"));
}

#[test]
fn status_icons_are_submitted_on_overlay_data() {
    // C++ drawHealing/drawDisabled/drawEnthusiastic/drawBombed/drawEmoticon.
    let region = IRegion2D::new(ICoord2D::new(10, 20), ICoord2D::new(74, 32));
    let mut drawable = BasicDrawable::new(DrawableId(14));
    drawable.overlay_data.health_region = Some(region);
    drawable.presentation_health_pct = 0.5;
    drawable.presentation_show_healing = true;
    drawable.presentation_disabled = true;
    drawable.presentation_weapon_bonus_enthusiastic = true;
    drawable.presentation_is_carbomb = true;
    drawable.draw_icon_ui();
    assert!(drawable.overlay_data.show_healing);
    assert!(drawable.overlay_data.show_disabled);
    assert!(drawable.overlay_data.show_enthusiastic);
    assert!(drawable.overlay_data.show_bombed);
}
