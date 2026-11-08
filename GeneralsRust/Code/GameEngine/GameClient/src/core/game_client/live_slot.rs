// Client bone/pose queries, drawable-id counter, and LOD texture reduction.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).
//
// The former live GameClient snapshot slot (raw address + unsafe deref) is gone:
// every consumer receives its receiving GameClient or narrower owner explicitly.

const TEXTURE_REDUCTION_MIN: i32 = 0;
const TEXTURE_REDUCTION_MAX: i32 = 4;
const WW3D_TEXTURE_REDUCTION_MIN_DIMENSION: u32 = 32;

impl GameClient {
    /// Actual receiving drawable lookup, without publishing or selecting a client.
    pub fn query_current_client_bone_positions(
        &self,
        object_id: ObjectID,
        bone_name: &str,
        start: i32,
        max_bones: usize,
    ) -> Vec<(gamelogic::common::Coord3D, gamelogic::common::Matrix3D)> {
        if max_bones == 0 || bone_name.is_empty() {
            return Vec::new();
        }
        let Some(drawable_id) = self.get_drawable_for_object(object_id) else {
            return Vec::new();
        };
        let Some(drawable) = self.find_drawable_by_id(drawable_id) else {
            return Vec::new();
        };
        let Some(basic) = drawable
            .as_any()
            .downcast_ref::<crate::drawable::BasicDrawable>()
        else {
            return Vec::new();
        };
        let mut positions = vec![crate::drawable::Vector3::zero(); max_bones];
        let mut transforms = vec![crate::drawable::Matrix4::identity(); max_bones];
        let count = basic.get_current_client_bone_positions(
            bone_name,
            start,
            &mut positions,
            &mut transforms,
        );
        (0..count as usize)
            .map(|i| {
                (
                    gamelogic::common::Coord3D::new(positions[i].x, positions[i].y, positions[i].z),
                    transforms[i].to_glam(),
                )
            })
            .collect()
    }
}

impl GameClient {
    /// Capture C++ `BasicDrawable::xfer` visuals from this client.
    pub fn capture_drawable_xfer_visuals(
        &self,
        object_id: ObjectID,
    ) -> Option<crate::drawable::DrawableXferVisualSnapshot> {
        let drawable_id = self.get_drawable_for_object(object_id)?;
        let drawable = self.find_drawable_by_id(drawable_id)?;
        drawable
            .as_any()
            .downcast_ref::<crate::drawable::BasicDrawable>()
            .map(|basic| basic.capture_xfer_visuals())
    }

    /// Restore C++ `BasicDrawable::xfer` visuals onto this client.
    pub fn restore_drawable_xfer_visuals(
        &mut self,
        object_id: ObjectID,
        visuals: &crate::drawable::DrawableXferVisualSnapshot,
    ) -> bool {
        let Some(drawable_id) = self.get_drawable_for_object(object_id) else {
            return false;
        };
        let Some(drawable) = self.find_drawable_by_id_mut(drawable_id) else {
            return false;
        };
        if let Some(basic) = drawable
            .as_any_mut()
            .downcast_mut::<crate::drawable::BasicDrawable>()
        {
            basic.apply_xfer_visuals(visuals);
            true
        } else {
            false
        }
    }
}

impl GameClient {
    /// Apply queued logic-fire recoil on this client's presentation drawables.
    /// `update` already owns `&mut self`, so routing back through the live
    /// pointer would create an overlapping mutable borrow of this same value.
    fn apply_queued_weapon_recoils(&mut self) {
        let Some(visual_world) = self.visual_world.as_ref() else {
            return;
        };
        let queued = visual_world.take_weapon_recoils();
        for (object_id, amount, aim_angle) in queued {
            let Some(drawable_id) = self.get_drawable_for_object(object_id) else {
                continue;
            };
            let Some(drawable) = self.find_drawable_by_id_mut(drawable_id) else {
                continue;
            };
            if let Some(basic) = drawable
                .as_any_mut()
                .downcast_mut::<crate::drawable::BasicDrawable>()
            {
                basic.apply_recoil_impulse(amount, aim_angle);
            }
        }
    }
}

impl GameClient {
    /// Recoil on this client's presentation drawable, if it is not identity.
    fn recoil_glam_for_object(&self, object_id: u32) -> Option<glam::Mat4> {
        let drawable_id = self.get_drawable_for_object(object_id)?;
        let drawable = self.find_drawable_by_id(drawable_id)?;
        let basic = drawable
            .as_any()
            .downcast_ref::<crate::drawable::BasicDrawable>()?;
        let matrix = basic.recoil_visual();
        if matrix == crate::drawable::Matrix4::identity() {
            None
        } else {
            Some(matrix.to_glam())
        }
    }
}

/// Store a texture-reduction target in the definitions (C++ `adjustLOD`'s
/// `TheWritableGlobalData->m_textureReductionFactor`), clamped to [0, 4].
/// Returns `(clamped, previous)`. Used directly when no renderer is installed.
fn store_texture_reduction(target_factor: i32) -> Option<(i32, i32)> {
    let global_data = get_global_data()?;
    let clamped = target_factor.clamp(TEXTURE_REDUCTION_MIN, TEXTURE_REDUCTION_MAX);
    let previous = {
        let mut global = global_data.write();
        let previous = global
            .texture_reduction_factor
            .clamp(TEXTURE_REDUCTION_MIN, TEXTURE_REDUCTION_MAX);
        global.texture_reduction_factor = clamped;
        previous
    };
    if let Ok(mut runtime_global) = runtime_global_data::write_safe() {
        runtime_global.texture_reduction_factor = clamped;
    }
    Some((clamped, previous))
}

/// Texture-reduction target without an installed renderer: only the
/// definitions change. Returns the clamped factor.
pub fn store_lod_texture_reduction(target_factor: i32) -> Option<i32> {
    store_texture_reduction(target_factor).map(|(clamped, _)| clamped)
}

/// Apply a texture-reduction target to the definitions and the caller's
/// renderer, then re-texture its terrain, mirroring C++
/// `W3DGameClient::adjustLOD` (clamp [0,4], `WW3D::Set_Texture_Reduction`,
/// `TheTerrainRenderObject->setTextureLOD`). The caller lends its own
/// renderer runtime; no process or service slot is consulted.
pub fn apply_lod_texture_reduction_to_session(
    r3d: &ww3d_renderer_3d::Renderer3dRuntime,
    target_factor: i32,
    terrain: &mut dyn FnMut(i32),
) -> Option<i32> {
    let (clamped, previous) = store_texture_reduction(target_factor)?;

    let renderer_previous = ww3d_renderer_3d::rendering::texture_quality::texture_reduction(r3d);
    if renderer_previous != clamped as u32 {
        ww3d_renderer_3d::rendering::texture_quality::set_texture_reduction(
            r3d,
            clamped as u32,
            WW3D_TEXTURE_REDUCTION_MIN_DIMENSION,
        );
    }

    if previous != clamped || renderer_previous != clamped as u32 {
        terrain(clamped);
    }

    Some(clamped)
}

/// Delta-based texture reduction adjustment (C++ `adjustLOD(adj)` semantics).
pub fn adjust_lod_texture_reduction(
    r3d: &ww3d_renderer_3d::Renderer3dRuntime,
    delta: i32,
    terrain: &mut dyn FnMut(i32),
) -> Option<i32> {
    let current = get_global_data()?.read().texture_reduction_factor;
    apply_lod_texture_reduction_to_session(r3d, current.saturating_add(delta), terrain)
}


#[cfg(test)]
#[path = "fx_client_bone_tests.rs"]
mod fx_client_bone_tests;

/// Disjoint frame loan of this client's admitted frame.
pub struct ClientFrameWriter<'a> {
    frame: &'a mut u32,
}
impl ClientFrameWriter<'_> {
    /// Read the admitted frame from this exact receiving Client loan.
    pub fn frame(&self) -> u32 {
        *self.frame
    }

    pub fn set_frame(&mut self, frame: u32) {
        *self.frame = frame;
    }
}
/// Read-only loan of this client's actual admitted Drawable associations.
/// No registry copy or presentation snapshot can substitute for this owner.
pub struct NativeDrawablePresence<'a> {
    drawables: &'a drawable_world::DrawableWorld,
}
impl NativeDrawablePresence<'_> {
    pub fn binding_key(
        &self,
        host_epoch: u64,
        object_id: ObjectID,
    ) -> Option<PresentationDirectDrawableBindingKey> {
        self.drawables
            .presentation_direct_drawable_state(host_epoch, object_id)
            .map(|state| state.binding_key)
    }
}
impl GameClient {
    pub fn split_frame_and_snow(
        &mut self,
    ) -> (ClientFrameWriter<'_>, &mut crate::snow::SnowManager) {
        (
            ClientFrameWriter {
                frame: &mut self.frame,
            },
            &mut self.snow,
        )
    }
    pub fn split_frame_snow_and_drawable_presence(
        &mut self,
    ) -> (
        ClientFrameWriter<'_>,
        &mut crate::snow::SnowManager,
        NativeDrawablePresence<'_>,
    ) {
        (
            ClientFrameWriter {
                frame: &mut self.frame,
            },
            &mut self.snow,
            NativeDrawablePresence {
                drawables: &self.drawables,
            },
        )
    }
    pub fn split_frame_snow_and_drawable_owner(
        &mut self,
    ) -> (
        ClientFrameWriter<'_>,
        &mut crate::snow::SnowManager,
        NativeDrawableOwner<'_>,
    ) {
        let mut drawable_owner = self.drawables.borrow_native();
        drawable_owner.audio_device = self.native_audio_device.as_ref();
        (
            ClientFrameWriter {
                frame: &mut self.frame,
            },
            &mut self.snow,
            drawable_owner,
        )
    }
    pub fn split_frame_snow_drawable_and_campaign(
        &mut self,
    ) -> (
        ClientFrameWriter<'_>,
        &mut crate::snow::SnowManager,
        NativeDrawableOwner<'_>,
        &mut crate::gui::campaign_manager::CampaignManager,
    ) {
        let mut drawable_owner = self.drawables.borrow_native();
        drawable_owner.audio_device = self.native_audio_device.as_ref();
        (
            ClientFrameWriter {
                frame: &mut self.frame,
            },
            &mut self.snow,
            drawable_owner,
            &mut self.campaign_manager,
        )
    }
    pub fn split_frame_snow_drawable_campaign_and_hints(
        &mut self,
    ) -> (
        ClientFrameWriter<'_>,
        &mut crate::snow::SnowManager,
        NativeDrawableOwner<'_>,
        &mut crate::gui::campaign_manager::CampaignManager,
        &mut crate::gui::command_hints::CommandHintState,
    ) {
        let mut drawable_owner = self.drawables.borrow_native();
        drawable_owner.audio_device = self.native_audio_device.as_ref();
        (
            ClientFrameWriter {
                frame: &mut self.frame,
            },
            &mut self.snow,
            drawable_owner,
            &mut self.campaign_manager,
            &mut self.command_hints,
        )
    }
    /// Split the same Client's runtime fields for one script-bearing host phase.
    /// The caller owns the transient message vector; callbacks drain into this
    /// exact Client's stream before another authored action is examined.
    pub fn split_frame_snow_drawable_script_ui_and_hints<'a>(
        &'a mut self,
        emitted: &'a mut Vec<crate::message_stream::game_message::GameMessage>,
    ) -> (
        ClientFrameWriter<'a>,
        &'a mut crate::snow::SnowManager,
        NativeDrawableOwner<'a>,
        &'a mut crate::gui::command_hints::CommandHintState,
        crate::gui::game_window::WindowEventEffects<'a>,
        &'a mut crate::message_stream::message_stream::MessageStream,
        &'a std::sync::RwLock<crate::game_text::GameText>,
    ) {
        let client_frame = self.frame;
        let mut drawable_owner = self.drawables.borrow_native();
        drawable_owner.audio_device = self.native_audio_device.as_ref();
        self.script_display
            .bind(self.subsystem_manager.display.as_ref());
        let text = self.game_text_store.get_or_insert_with(|| {
            std::sync::Arc::new(std::sync::RwLock::new(crate::game_text::GameText::default()))
        });
        (
            ClientFrameWriter {
                frame: &mut self.frame,
            },
            &mut self.snow,
            drawable_owner,
            &mut self.command_hints,
            crate::gui::game_window::WindowEventEffects {
                cursor_mouse: self.subsystem_manager.input_mouse.as_mut(),
                control_bar_observation: None,
                menu_audio: None,
                input_provenance:
                    crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
                standalone_view: None,
                client_frame: Some(client_frame),
                logic_frame: None,
                ui: self.subsystem_manager.in_game_ui.as_mut(),
                payloads: Default::default(),
                pending_window_callbacks: Default::default(),
                diplomacy_world: None,
                menus: Some(self.menu_manager.receiving_menus(self.input_authority)),
                manager: None,
                ime: Some(&mut self.ime),
                window_manager_owner: Some(&self.window_manager_owner),
                selected_window: None,
                lent_windows: Vec::new(),
                emitted_messages: emitted,
                pending_control_bar_clicks: None,
                campaign_manager: Some(&mut self.campaign_manager),
                main_menu: Some(&mut self.main_menu),
                menu_world: None,
                menu_host: Some(&mut self.menu_host),
                script_display: Some(&mut self.script_display),
                window_videos: Some(&mut self.window_videos.challenge),
                pending_main_menu_actions: Vec::new(),
            },
            &mut self.message_stream,
            text,
        )
    }

    pub fn command_hint_snapshot(&self) -> Vec<crate::gui::command_hints::CommandHint> {
        self.command_hints.snapshot(self.frame)
    }
    pub fn snow_mut(&mut self) -> &mut crate::snow::SnowManager {
        &mut self.snow
    }
    pub fn snow_render_state(&self) -> crate::snow::SnowRenderState {
        crate::snow::SnowRenderState {
            manager: self.snow.clone(),
            definition: self.snow_definition.clone(),
        }
    }
    /// C++ GameLogic::reset deletes definition overrides without refreshing
    /// SnowManager's cached parameters or noise. Only a parsed Weather block
    /// calls updateIniSettings; immutable definition admission is separate.
    pub fn admit_base_snow_definition(&mut self) {
        self.snow_definition = self.snow_base_definition.clone();
    }
    /// Admit one successfully parsed Weather block, including its runtime refresh.
    pub fn admit_snow_definition(&mut self, definition: &crate::snow::WeatherSetting) {
        self.snow_definition = definition.clone();
        let client_random = &mut self.client_random;
        self.snow
            .update_ini_settings_with_definition(definition, &mut |low, high| {
                crate::GameClientRandomValue!(client_random, low, high)
            });
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod receiving_lod_tests {
    #[test]
    fn receiving_lod_callbacks_keep_peers_separate_and_follow_definition_renderer_updates() {
        crate::test_support::ensure_ambient_host();
        const NAME: &str =
            "receiving_lod_callbacks_keep_peers_separate_and_follow_definition_renderer_updates";
        const MARKER: &str = "GENERALS_RECEIVING_LOD_CHILD";
        if std::env::var(MARKER).as_deref() != Ok(NAME) {
            let module = module_path!().split_once("::").unwrap().1;
            let exact = format!("{module}::{NAME}");
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &exact, "--test-threads=1", "--nocapture"])
                .env(MARKER, NAME)
                .output()
                .unwrap();
            assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let definitions = game_engine::common::ini::ini_game_data::ensure_global_data();
        definitions.write().texture_reduction_factor = 0;
        // The receiving renderer is lent to every apply; nothing is published.
        let r3d = ww3d_renderer_3d::Renderer3dRuntime::new();
        ww3d_renderer_3d::rendering::texture_quality::set_texture_reduction(&r3d, 0, 32);
        // Publish a client's terrain visual the way standalone init does.
        let _live = crate::test_support::park_live_game_client();
        // A real foreign native holder remains unavailable throughout. The
        // receiving operation must not rediscover or try to borrow that slot.
        let _foreign = crate::terrain::terrain_visual::get_terrain_visual().unwrap();
        let mut a = Vec::new();
        let mut b = Vec::new();
        let observe = |value| {
            assert_eq!(definitions.read().texture_reduction_factor, value);
            assert_eq!(
                super::runtime_global_data::read().texture_reduction_factor,
                value
            );
            assert_eq!(
                ww3d_renderer_3d::rendering::texture_quality::texture_reduction(&r3d),
                value as u32
            );
        };
        assert_eq!(
            super::apply_lod_texture_reduction_to_session(&r3d, 2, &mut |value| {
                observe(value);
                a.push(value);
            }),
            Some(2)
        );
        assert_eq!(a, vec![2]);
        assert!(b.is_empty());
        assert_eq!(
            super::apply_lod_texture_reduction_to_session(&r3d, 4, &mut |value| {
                observe(value);
                b.push(value);
            }),
            Some(4)
        );
        assert_eq!(a, vec![2]);
        assert_eq!(b, vec![4]);
        assert_eq!(
            super::apply_lod_texture_reduction_to_session(&r3d, 4, &mut |value| a.push(value)),
            Some(4)
        );
        assert_eq!(
            a,
            vec![2],
            "unchanged target preserves existing no-invalidation branch"
        );
    }
}
