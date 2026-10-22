// Parsed world-animation metadata and native image resources for one client.
// No Anim2D instance/Arc/Mutex or global definition lookup is used per draw.
use game_engine::common::ini::ini_animation::{Anim2DMode, OwnedAnim2DTemplates};
use game_engine::common::ini::ini_mapped_image::OwnedMappedImages;

#[derive(Debug, Clone)]
struct WorldAnimationDefinition {
    frames: Vec<Option<String>>,
    delay: u16,
    mode: Anim2DMode,
}
#[derive(Default)]
struct OwnedWorldAnimations {
    definitions: std::collections::BTreeMap<String, WorldAnimationDefinition>,
    images: crate::display::image::ImageCollection,
    instances: std::collections::BTreeMap<u64, WorldAnimationPlayback>,
    host_epoch: Option<u64>,
    paused: bool,
    logic_frame: u32,
}
impl OwnedWorldAnimations {
    fn admit(templates: &OwnedAnim2DTemplates, images: &OwnedMappedImages) -> Self {
        use crate::display::image::{ICoord2D, Image, ImageStatus, Region2D};
        let definitions = templates
            .iter()
            .map(|(name, template)| {
                (
                    name.to_owned(),
                    WorldAnimationDefinition {
                        frames: (0..template.get_num_frames())
                            .map(|frame| template.get_frame_name(frame).map(str::to_owned))
                            .collect(),
                        delay: template.get_num_frames_between_updates(),
                        mode: template.get_anim_mode(),
                    },
                )
            })
            .collect();
        let mut owned_images = crate::display::image::ImageCollection::new();
        for source in images.iter() {
            let mut image = Image::new();
            image.set_name(source.get_name());
            image.set_filename(source.get_filename());
            image.set_texture_width(source.get_texture_size().x);
            image.set_texture_height(source.get_texture_size().y);
            let uv = source.get_uv();
            image.set_uv(Region2D::from_coords(uv.left, uv.top, uv.right, uv.bottom));
            image.set_image_size(ICoord2D::new(
                source.get_image_size().x,
                source.get_image_size().y,
            ));
            if source.status.is_rotated_90_clockwise() {
                image.set_status(ImageStatus::ROTATED_90_CLOCKWISE);
            }
            owned_images.add_image(image);
        }
        Self {
            definitions,
            images: owned_images,
            instances: std::collections::BTreeMap::new(),
            host_epoch: None,
            paused: false,
            logic_frame: 0,
        }
    }
    /// Retire per-world playback only; immutable definitions and native image
    /// resources belong to the application Client and survive match reset.
    fn clear_runtime(&mut self) {
        self.instances.clear();
        self.host_epoch = None;
        self.paused = false;
        self.logic_frame = 0;
    }
    fn observe_owner(
        &mut self,
        epoch: u64,
        logic_frame: u32,
        paused: bool,
        entries: &[game_engine::common::world_animation::WorldAnimationPresentation],
    ) {
        if self.host_epoch != Some(epoch) {
            self.instances.clear();
            self.host_epoch = Some(epoch);
        }
        self.paused = paused;
        self.logic_frame = logic_frame;
        let live = entries
            .iter()
            .map(|entry| entry.identity)
            .collect::<std::collections::BTreeSet<_>>();
        self.instances.retain(|id, _| live.contains(id));
    }
    /// CPP world UI advances its live Z before shroud/projection gates.
    /// Matching immutable descriptors never reset this per-instance state.
    fn prepare_draw(
        &mut self,
        event: &game_engine::common::world_animation::WorldAnimationPresentation,
        now: u32,
    ) -> Option<[f32; 3]> {
        let definition = self.definitions.get(&event.template)?;
        if definition.frames.is_empty()
            || usize::from(event.initial_frame) >= definition.frames.len()
        {
            return None;
        }
        let state = self
            .instances
            .entry(event.identity)
            .or_insert_with(|| WorldAnimationPlayback::new(event));
        if state.template != event.template
            || state.spawn_frame != event.spawn_frame
            || state.initial_frame != event.initial_frame
            || state.expired
        {
            return None;
        }
        if !self.paused {
            let expire = game_engine::common::world_animation::expiry_frame(
                event.spawn_frame,
                event.duration,
            )?;
            if now >= expire {
                // Retain tombstone until source descriptor disappears so a
                // repeated snapshot cannot recreate an expired instance.
                state.expired = true;
                return None;
            }
            if event.rise != 0.0 {
                state.position[2] += event.rise / 30.0;
            }
        }
        Some(state.position)
    }
    /// CPP fade happens after shroud, before projection. Preserve prior alpha
    /// when unsigned frames-till-expire is outside the final fade window.
    fn visible_alpha(
        &mut self,
        event: &game_engine::common::world_animation::WorldAnimationPresentation,
        now: u32,
    ) -> f32 {
        let Some(state) = self.instances.get_mut(&event.identity) else {
            return 1.0;
        };
        if event.fades {
            if let Some(expire) = game_engine::common::world_animation::expiry_frame(
                event.spawn_frame,
                event.duration,
            ) {
                let remaining = expire.wrapping_sub(now);
                if remaining < 30 {
                    state.alpha = remaining as f32 / 30.0;
                }
            }
        }
        state.alpha
    }
    fn draw(
        &mut self,
        event: &game_engine::common::world_animation::WorldAnimationPresentation,
        x: f32,
        y: f32,
        zoom: f32,
        alpha: f32,
        now: u32,
        renderer: &mut crate::gui::ui_renderer::UIRenderer,
    ) -> bool {
        use crate::gui::ui_renderer::UIRect;
        let Some(definition) = self.definitions.get(&event.template) else {
            return false;
        };
        if definition.frames.is_empty()
            || usize::from(event.initial_frame) >= definition.frames.len()
        {
            return false;
        }
        let Some(state) = self.instances.get_mut(&event.identity) else {
            return false;
        };
        // One id cannot silently become a different animation in the same host epoch.
        if state.template != event.template
            || state.spawn_frame != event.spawn_frame
            || state.initial_frame != event.initial_frame
        {
            return false;
        }
        let Some(frame_name) = definition
            .frames
            .get(usize::from(state.current))
            .and_then(Option::as_deref)
        else {
            return false;
        };
        let Some(image) = self.images.find_image_by_name_mut(frame_name) else {
            return false;
        };
        let drawn = (|| {
            if image.get_gpu_texture().is_none()
                && image
                    .create_gpu_texture(renderer.device(), renderer.queue())
                    .is_err()
            {
                return false;
            }
            let Some(gpu) = image.get_gpu_texture() else {
                return false;
            };
            let Some((rect, alpha)) = world_animation_draw_geometry(
                x,
                y,
                image.get_image_width(),
                image.get_image_height(),
                zoom,
                alpha,
            ) else {
                return false;
            };
            let uv = image.get_uv();
            renderer.draw_textured_rect(
                rect,
                std::sync::Arc::new(gpu.view().clone()),
                [1.0, 1.0, 1.0, alpha],
                Some(UIRect::new(uv.min.x, uv.min.y, uv.width(), uv.height())),
                0.0,
            );
            true
        })();
        // CPP draw submits the CURRENT image, then advances at most once.
        // Pause gates world expiry/rise, not Anim2D's own per-draw delay check.
        state.after_draw(definition, now);
        drawn
    }
}

/// CPP InGameUI.cpp5401–5414 narrows scaled unsigned dimensions before integer
/// centering; Anim2D.cpp640 passes alpha through GameMakeColor's UnsignedByte.
fn world_animation_draw_geometry(
    screen_x: f32,
    screen_y: f32,
    image_width: i32,
    image_height: i32,
    zoom: f32,
    alpha: f32,
) -> Option<(crate::gui::ui_renderer::UIRect, f32)> {
    if !screen_x.is_finite()
        || !screen_y.is_finite()
        || !zoom.is_finite()
        || !alpha.is_finite()
        || screen_x < i32::MIN as f32
        || screen_x >= 2_147_483_648.0
        || screen_y < i32::MIN as f32
        || screen_y >= 2_147_483_648.0
        || image_width < 0
        || image_height < 0
        || zoom < 0.0
    {
        return None;
    }
    let width = image_width as f32 * zoom;
    let height = image_height as f32 * zoom;
    // Explicit range rejection precedes Rust's saturating float→integer cast.
    if !width.is_finite()
        || !height.is_finite()
        || width < 0.0
        || width >= 4_294_967_296.0
        || height < 0.0
        || height >= 4_294_967_296.0
    {
        return None;
    }
    let width = width as u32;
    let height = height as u32;
    if width == 0 || height == 0 {
        return None;
    }
    let left = (screen_x as i32).wrapping_sub((width / 2) as i32);
    let top = (screen_y as i32).wrapping_sub((height / 2) as i32);
    let alpha_byte = (255.0 * alpha.clamp(0.0, 1.0)) as u8;
    Some((
        crate::gui::ui_renderer::UIRect::new(left as f32, top as f32, width as f32, height as f32),
        f32::from(alpha_byte) / 255.0,
    ))
}

#[derive(Debug)]
struct WorldAnimationPlayback {
    template: String,
    spawn_frame: u32,
    initial_frame: u16,
    current: u16,
    last_update: u32,
    reversed: bool,
    complete: bool,
    position: [f32; 3],
    alpha: f32,
    expired: bool,
}
impl WorldAnimationPlayback {
    fn new(event: &game_engine::common::world_animation::WorldAnimationPresentation) -> Self {
        Self {
            template: event.template.clone(),
            spawn_frame: event.spawn_frame,
            initial_frame: event.initial_frame,
            current: event.initial_frame,
            // CPP constructor resets this to0 after reset/randomizeCurrentFrame.
            last_update: 0,
            reversed: false,
            complete: false,
            position: event.position,
            alpha: 1.0,
            expired: false,
        }
    }
    fn after_draw(&mut self, definition: &WorldAnimationDefinition, now: u32) {
        if now.wrapping_sub(self.last_update) < u32::from(definition.delay) {
            return;
        }
        let Some(last) = definition
            .frames
            .len()
            .checked_sub(1)
            .and_then(|value| u16::try_from(value).ok())
        else {
            return;
        };
        let next = match definition.mode {
            Anim2DMode::Once => {
                if self.current < last {
                    Some(self.current + 1)
                } else {
                    self.complete = true;
                    None
                }
            }
            Anim2DMode::OnceBackwards => {
                if self.current > 0 {
                    Some(self.current - 1)
                } else {
                    self.complete = true;
                    None
                }
            }
            Anim2DMode::Loop => Some(if self.current == last {
                0
            } else {
                self.current + 1
            }),
            Anim2DMode::LoopBackwards => Some(if self.current > 0 {
                self.current - 1
            } else {
                last
            }),
            Anim2DMode::PingPong | Anim2DMode::PingPongBackwards => {
                // Original subtracts below zero for a one-frame ping-pong.
                // Keep its only valid frame safely; this malformed-content edge
                // is an explicit bounded safety deviation, not a parity claim.
                if last == 0 {
                    Some(0)
                } else if self.reversed {
                    if self.current == 0 {
                        self.reversed = false;
                        Some(1)
                    } else {
                        Some(self.current - 1)
                    }
                } else if self.current == last {
                    self.reversed = true;
                    Some(last - 1)
                } else {
                    Some(self.current + 1)
                }
            }
            Anim2DMode::Invalid => None,
        };
        if let Some(next) = next {
            self.current = next;
            self.last_update = now;
        }
    }
}

impl GameClient {
    /// Admit the exact immutable template/image definitions captured for this
    /// application. Mutable GPU materialization stays local to this client.
    pub fn admit_owned_world_animations(
        &mut self,
        templates: &OwnedAnim2DTemplates,
        images: &OwnedMappedImages,
    ) {
        self.owned_world_animations = OwnedWorldAnimations::admit(templates, images);
    }
}

#[cfg(test)]
mod owned_world_animation_tests {
    use super::*;
    use game_engine::common::world_animation::WorldAnimationPresentation as Event;
    fn event() -> Event {
        Event {
            identity: 1,
            template: "SameName".into(),
            position: [3.0, 4.0, 5.0],
            duration: 2.0,
            rise: 30.0,
            fades: true,
            spawn_frame: 100,
            initial_frame: 0,
        }
    }
    fn definition(count: usize, delay: u16, mode: Anim2DMode) -> WorldAnimationDefinition {
        WorldAnimationDefinition {
            frames: (0..count).map(|n| Some(format!("Image{n}"))).collect(),
            delay,
            mode,
        }
    }
    fn owner(count: usize) -> OwnedWorldAnimations {
        let mut result = OwnedWorldAnimations::default();
        result
            .definitions
            .insert("SameName".into(), definition(count, 5, Anim2DMode::Loop));
        result
    }
    #[test]
    fn original_last_frame_zero_and_no_elapsed_catchup() {
        let event = event();
        let definition = definition(4, 5, Anim2DMode::Loop);
        let mut state = WorldAnimationPlayback::new(&event);
        assert_eq!(state.last_update, 0);
        assert_eq!(state.current, 0);
        state.after_draw(&definition, 100);
        assert_eq!((state.current, state.last_update), (1, 100));
        state.after_draw(&definition, 100);
        assert_eq!(state.current, 1);
        state.after_draw(&definition, 140);
        assert_eq!((state.current, state.last_update), (2, 140));
    }
    #[test]
    fn zero_delay_advances_once_per_call_at_same_paused_frame() {
        let event = event();
        let definition = definition(4, 0, Anim2DMode::Loop);
        let mut state = WorldAnimationPlayback::new(&event);
        for expected in [1, 2, 3, 0] {
            state.after_draw(&definition, 100);
            assert_eq!(state.current, expected);
        }
    }
    #[test]
    fn hidden_phase_advances_z_without_advancing_animation() {
        let mut owner = owner(4);
        let event = event();
        owner.observe_owner(9, 100, false, &[event.clone()]);
        assert_eq!(owner.prepare_draw(&event, 100), Some([3.0, 4.0, 6.0]));
        assert_eq!(owner.prepare_draw(&event, 101), Some([3.0, 4.0, 7.0]));
        // Simulate caller skipping actual draw at its shroud/projection gate.
        let state = &owner.instances[&1];
        assert_eq!(state.current, 0);
        assert_eq!(state.last_update, 0);
    }
    #[test]
    fn matching_snapshots_do_not_reset_live_position_or_initial_frame() {
        let mut owner = owner(4);
        let mut event = event();
        event.initial_frame = 2;
        owner.observe_owner(9, 100, false, &[event.clone()]);
        owner.prepare_draw(&event, 100);
        owner
            .instances
            .get_mut(&1)
            .unwrap()
            .after_draw(&definition(4, 0, Anim2DMode::Loop), 100);
        owner.observe_owner(9, 100, false, &[event.clone()]);
        assert_eq!(owner.prepare_draw(&event, 101), Some([3.0, 4.0, 7.0]));
        assert_eq!(owner.instances[&1].current, 3);
    }
    #[test]
    fn expired_descriptor_is_tombstoned_until_removed_or_epoch_replaced() {
        let mut owner = owner(4);
        let event = event();
        owner.observe_owner(9, 100, false, &[event.clone()]);
        assert!(owner.prepare_draw(&event, 160).is_none());
        assert!(owner.instances[&1].expired);
        owner.observe_owner(9, 100, true, &[event.clone()]);
        assert!(owner.prepare_draw(&event, 100).is_none());
        owner.observe_owner(9, 100, false, &[]);
        assert!(owner.instances.is_empty());
        owner.observe_owner(10, 100, false, &[event.clone()]);
        assert_eq!(owner.prepare_draw(&event, 100), Some([3.0, 4.0, 6.0]));
    }
    #[test]
    fn two_same_id_owners_keep_independent_z_and_playback() {
        let mut a = owner(4);
        let mut b = owner(7);
        let event = event();
        a.observe_owner(1, 100, false, &[event.clone()]);
        b.observe_owner(1, 100, true, &[event.clone()]);
        assert_eq!(a.prepare_draw(&event, 100), Some([3.0, 4.0, 6.0]));
        assert_eq!(b.prepare_draw(&event, 100), Some(event.position));
        a.instances
            .get_mut(&1)
            .unwrap()
            .after_draw(&definition(4, 0, Anim2DMode::Loop), 100);
        b.observe_owner(2, 100, false, &[]);
        assert_eq!(a.instances[&1].current, 1);
        assert_eq!(a.definitions["SameName"].frames.len(), 4);
    }
    #[test]
    fn pause_preserves_z_and_alpha_uses_unsigned_remaining_window() {
        let mut owner = owner(4);
        let event = event();
        owner.observe_owner(1, 100, true, &[event.clone()]);
        assert_eq!(owner.prepare_draw(&event, 150), Some(event.position));
        assert!((owner.visible_alpha(&event, 150) - 10.0 / 30.0).abs() < 0.000001);
        // CPP does not reset alpha when a paused observation wraps past expiry.
        assert!((owner.visible_alpha(&event, 161) - 10.0 / 30.0).abs() < 0.000001);
    }
    #[test]
    fn once_backwards_and_pingpong_keep_source_step_order() {
        let mut event = event();
        event.initial_frame = 2;
        let mut state = WorldAnimationPlayback::new(&event);
        let backward = definition(3, 0, Anim2DMode::OnceBackwards);
        for expected in [1, 0, 0] {
            state.after_draw(&backward, 100);
            assert_eq!(state.current, expected);
        }
        assert!(state.complete);
        let mut state = WorldAnimationPlayback::new(&event);
        let pingpong = definition(3, 0, Anim2DMode::PingPongBackwards);
        for expected in [1, 0, 1, 2, 1] {
            state.after_draw(&pingpong, 100);
            assert_eq!(state.current, expected);
        }
    }
}

#[cfg(test)]
mod admitted_game_client_animation_tests {
    use super::*;
    use game_engine::common::ini::ini_mapped_image::{
        ICoord2D as SourceSize, Image as SourceImage, ImageCollection as SourceImages,
    };
    use game_engine::common::world_animation::WorldAnimationPresentation as Event;

    fn content(
        prefix: &str,
        size: i32,
        delay_ms: u32,
    ) -> (OwnedAnim2DTemplates, OwnedMappedImages) {
        let mut source_images = SourceImages::new();
        for name in ["SharedFrame0", "SharedFrame1"] {
            let mut image = SourceImage::new();
            image.set_name(name.into());
            image.set_filename(format!("{prefix}-{name}.tga"));
            image.set_texture_width(size);
            image.set_texture_height(size);
            image.set_image_size(SourceSize::new(size, size));
            source_images.add_image(image);
        }
        let images = OwnedMappedImages::from_collection(&source_images);
        let templates = OwnedAnim2DTemplates::default().overlay_ini_text(
            &format!("Animation SharedAnimation\n NumberImages = 2\n Image = SharedFrame0\n Image = SharedFrame1\n AnimationMode = LOOP\n AnimationDelay = {delay_ms}\n End\n"),
            &|name| images.find(name).is_some(),
        ).expect("actual animation parser resolves images from this owned mapped collection");
        (templates, images)
    }

    #[test]
    fn actual_clients_admit_same_name_definitions_and_keep_cpu_playback_independent() {
        let (templates_a, images_a) = content("OwnerA", 32, 100);
        let (templates_b, images_b) = content("OwnerB", 64, 10000);
        let mut a = GameClient::new().expect("CPU-only GameClient constructor");
        let mut b = GameClient::new().expect("independent CPU-only GameClient constructor");
        a.admit_owned_world_animations(&templates_a, &images_a);
        b.admit_owned_world_animations(&templates_b, &images_b);
        drop((templates_a, images_a, templates_b, images_b));
        assert_eq!(
            a.owned_world_animations.definitions["SharedAnimation"].delay,
            3
        );
        assert_eq!(
            b.owned_world_animations.definitions["SharedAnimation"].delay,
            300
        );
        for (client, prefix, size) in [(&a, "OwnerA", 32), (&b, "OwnerB", 64)] {
            let image = client
                .owned_world_animations
                .images
                .find_image_by_name("SharedFrame0")
                .expect("actual admitted native mapped image");
            assert_eq!(image.get_filename(), format!("{prefix}-SharedFrame0.tga"));
            assert_eq!(image.get_image_width(), size);
            assert_eq!(image.get_image_height(), size);
            assert!(
                image.get_gpu_texture().is_none(),
                "CPU admission never requires a device or texture upload"
            );
        }
        let event = Event {
            identity: 7,
            template: "SharedAnimation".into(),
            position: [3.0, 4.0, 5.0],
            duration: 2.0,
            rise: 30.0,
            fades: true,
            spawn_frame: 100,
            initial_frame: 0,
        };
        // Exercise the actual public client presentation admission before its
        // private CPU draw-preparation/playback steps. No GPU renderer is made.
        a.apply_presentation_world_anims(9, 100, false, &[event.clone()]);
        b.apply_presentation_world_anims(9, 100, true, &[event.clone()]);
        assert_eq!(
            a.owned_world_animations.prepare_draw(&event, 100),
            Some([3.0, 4.0, 6.0])
        );
        assert_eq!(
            b.owned_world_animations.prepare_draw(&event, 100),
            Some(event.position)
        );
        let definition_a = a.owned_world_animations.definitions["SharedAnimation"].clone();
        let definition_b = b.owned_world_animations.definitions["SharedAnimation"].clone();
        a.owned_world_animations
            .instances
            .get_mut(&7)
            .unwrap()
            .after_draw(&definition_a, 100);
        b.owned_world_animations
            .instances
            .get_mut(&7)
            .unwrap()
            .after_draw(&definition_b, 100);
        assert_eq!(a.owned_world_animations.instances[&7].current, 1);
        assert_eq!(
            b.owned_world_animations.instances[&7].current, 0,
            "the peer's actual parsed delay controls only its own playback"
        );
        // Repeated matching snapshots cannot reset A's playback or live Z.
        a.apply_presentation_world_anims(9, 100, false, &[event.clone()]);
        assert_eq!(
            a.owned_world_animations.prepare_draw(&event, 101),
            Some([3.0, 4.0, 7.0])
        );
        assert_eq!(a.owned_world_animations.instances[&7].current, 1);
        b.apply_presentation_world_anims(10, 100, false, &[]);
        assert!(b.owned_world_animations.instances.is_empty());
        drop(b);
        assert_eq!(a.owned_world_animations.instances[&7].current, 1);
        assert_eq!(
            a.owned_world_animations
                .images
                .find_image_by_name("SharedFrame0")
                .unwrap()
                .get_filename(),
            "OwnerA-SharedFrame0.tga"
        );
        // Expiry remains tombstoned when an identical descriptor is observed again.
        assert!(a.owned_world_animations.prepare_draw(&event, 160).is_none());
        a.apply_presentation_world_anims(9, 100, true, &[event.clone()]);
        assert!(a.owned_world_animations.prepare_draw(&event, 100).is_none());
        assert!(a.owned_world_animations.instances[&7].expired);
    }

    #[test]
    fn original_integer_zoom_centering_and_byte_alpha_are_preserved() {
        let (rect, alpha) =
            world_animation_draw_geometry(100.0, 80.0, 3, 5, 1.5, 1.0 / 30.0).unwrap();
        assert_eq!(
            (rect.x, rect.y, rect.width, rect.height),
            (98.0, 77.0, 4.0, 7.0)
        );
        assert_eq!(alpha, 8.0 / 255.0);
        let (rect, alpha) = world_animation_draw_geometry(-10.0, -8.0, 5, 3, 1.0, 1.0).unwrap();
        assert_eq!(
            (rect.x, rect.y, rect.width, rect.height),
            (-12.0, -9.0, 5.0, 3.0)
        );
        assert_eq!(alpha, 1.0);
    }
    #[test]
    fn malformed_pixel_geometry_is_rejected_before_integer_conversion() {
        assert!(world_animation_draw_geometry(f32::NAN, 0.0, 3, 3, 1.0, 1.0).is_none());
        assert!(world_animation_draw_geometry(0.0, 0.0, 3, 3, f32::INFINITY, 1.0).is_none());
        assert!(world_animation_draw_geometry(2_147_483_648.0, 0.0, 3, 3, 1.0, 1.0).is_none());
        assert!(world_animation_draw_geometry(0.0, 0.0, i32::MAX, 3, 2.0, 1.0).is_none());
        assert!(world_animation_draw_geometry(0.0, 0.0, -1, 3, 1.0, 1.0).is_none());
        assert!(world_animation_draw_geometry(0.0, 0.0, 1, 1, 0.5, 1.0).is_none());
    }

    fn lifecycle_event() -> Event {
        Event {
            identity: 73,
            template: "SharedAnimation".into(),
            position: [3.0, 4.0, 5.0],
            duration: 2.0,
            rise: 30.0,
            fades: true,
            spawn_frame: 100,
            initial_frame: 0,
        }
    }
    #[test]
    fn actual_successful_retirement_clears_runtime_and_transport_but_keeps_definitions() {
        let (templates, images) = content("Retirement", 32, 100);
        let mut client = GameClient::new().unwrap();
        client.admit_owned_world_animations(&templates, &images);
        client.subsystem_manager.in_game_ui = Some(std::sync::Arc::new(std::sync::Mutex::new(
            crate::core::subsystems::InGameUISubsystem::default(),
        )));
        let event = lifecycle_event();
        client.apply_presentation_world_anims(11, 103, false, &[event.clone()]);
        client.owned_world_animations.prepare_draw(&event, 103);
        assert_eq!(client.owned_world_animations.instances.len(), 1);
        client.retire_native_drawable_world_for_replace().unwrap();
        assert!(client.owned_world_animations.instances.is_empty());
        assert_eq!(client.owned_world_animations.host_epoch, None);
        assert_eq!(client.owned_world_animations.logic_frame, 0);
        assert!(
            client
                .subsystem_manager
                .in_game_ui
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .presentation_world_anims()
                .is_empty()
        );
        assert_eq!(
            client.owned_world_animations.definitions["SharedAnimation"]
                .frames
                .len(),
            2
        );
        assert_eq!(
            client
                .owned_world_animations
                .images
                .find_image_by_name("SharedFrame0")
                .unwrap()
                .get_filename(),
            "Retirement-SharedFrame0.tga"
        );
    }
    #[test]
    fn failed_retirement_preflight_preserves_animation_runtime_and_transport() {
        let (templates, images) = content("Failure", 32, 100);
        let mut client = GameClient::new().unwrap();
        client.admit_owned_world_animations(&templates, &images);
        let ui = std::sync::Arc::new(std::sync::Mutex::new(
            crate::core::subsystems::InGameUISubsystem::default(),
        ));
        client.subsystem_manager.in_game_ui = Some(ui.clone());
        let event = lifecycle_event();
        client.apply_presentation_world_anims(11, 103, false, &[event.clone()]);
        client.owned_world_animations.prepare_draw(&event, 103);
        let failed = std::panic::catch_unwind({
            let ui = ui.clone();
            move || {
                let _guard = ui.lock().unwrap();
                panic!("animation retirement UI poison witness");
            }
        });
        assert!(failed.is_err());
        assert!(
            client
                .retire_native_drawable_world_for_replace()
                .unwrap_err()
                .to_string()
                .contains("UI lock poisoned before world-animation retirement")
        );
        assert_eq!(
            client.owned_world_animations.instances[&event.identity].position,
            [3.0, 4.0, 6.0]
        );
        assert_eq!(client.owned_world_animations.host_epoch, Some(11));
        let recovered = ui
            .lock()
            .err()
            .expect("witness poisoned this UI loan")
            .into_inner();
        assert_eq!(recovered.presentation_world_anims().len(), 1);
        drop(recovered);
        ui.clear_poison();
    }
    #[test]
    fn admitted_animation_clock_does_not_follow_independent_client_frame() {
        let (templates, images) = content("Clock", 32, 100);
        let mut client = GameClient::new().unwrap();
        client.admit_owned_world_animations(&templates, &images);
        client.set_frame(159);
        let event = lifecycle_event();
        client.apply_presentation_world_anims(11, 160, false, &[event.clone()]);
        let logic_frame = client.owned_world_animations.logic_frame;
        assert_eq!(logic_frame, 160);
        assert_eq!(client.get_frame(), 159);
        assert!(
            client
                .owned_world_animations
                .prepare_draw(&event, logic_frame)
                .is_none()
        );
        assert!(client.owned_world_animations.instances[&event.identity].expired);
    }
}
