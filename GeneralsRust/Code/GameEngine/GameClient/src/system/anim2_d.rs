//! Anim2D system (ported from GameClient/System/Anim2D.cpp).

use crate::display::image::{
    ensure_client_mapped_image, get_mapped_image_collection as get_client_images,
};
use crate::gui::ui_globals::with_ui_renderer;
use crate::gui::ui_renderer::UIRect;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::ini::INILoadType;
use game_engine::common::ini::{get_anim2d_collection, Anim2DMode, Anim2DTemplate, INI};
use game_engine::common::random_value::get_game_client_random_value;
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use gamelogic::helpers::TheGameLogic;
use parking_lot::Mutex;
use std::sync::{Arc, Weak};

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Anim2DStatus: u8 {
        const NONE = 0x00;
        const FROZEN = 0x01;
        const REVERSED = 0x02;
        const COMPLETE = 0x04;
    }
}

/// 2D animation instance.
pub struct Anim2D {
    current_frame: u16,
    last_update_frame: u32,
    template: Arc<RwLock<Anim2DTemplate>>,
    status: Anim2DStatus,
    min_frame: u16,
    max_frame: u16,
    frames_between_updates: u32,
    alpha: f32,
    collection_system: Option<Weak<Mutex<Anim2DCollection>>>,
}

use parking_lot::RwLock;

impl Anim2D {
    pub fn new(
        template: Arc<RwLock<Anim2DTemplate>>,
        collection_system: Option<Arc<Mutex<Anim2DCollection>>>,
    ) -> Arc<Mutex<Self>> {
        let mut anim = Self {
            current_frame: 0,
            last_update_frame: 0,
            template: template.clone(),
            status: Anim2DStatus::NONE,
            min_frame: 0,
            max_frame: template.read().get_num_frames().saturating_sub(1),
            frames_between_updates: template.read().get_num_frames_between_updates() as u32,
            alpha: 1.0,
            collection_system: collection_system.as_ref().map(Arc::downgrade),
        };

        if template.read().is_randomized_start_frame() {
            anim.randomize_current_frame();
        } else {
            anim.reset();
        }

        let anim = Arc::new(Mutex::new(anim));
        if let Some(collection) = collection_system {
            collection.lock().register_animation(&anim);
        }
        anim
    }

    pub fn get_current_frame(&self) -> u16 {
        self.current_frame
    }

    pub fn set_current_frame(&mut self, frame: u16) {
        let template = self.template.read();
        if frame >= template.get_num_frames() {
            return;
        }

        self.current_frame = frame;
        self.last_update_frame = TheGameLogic::get_frame();
    }

    pub fn randomize_current_frame(&mut self) {
        let max = self.template.read().get_num_frames();
        if max == 0 {
            return;
        }
        let frame = get_game_client_random_value(0, (max - 1) as i32) as u16;
        self.set_current_frame(frame);
    }

    pub fn reset(&mut self) {
        let anim_mode = self.template.read().get_anim_mode();
        match anim_mode {
            Anim2DMode::Once | Anim2DMode::Loop | Anim2DMode::PingPong => {
                self.set_current_frame(self.min_frame);
            }
            Anim2DMode::OnceBackwards
            | Anim2DMode::LoopBackwards
            | Anim2DMode::PingPongBackwards => {
                self.set_current_frame(self.max_frame);
            }
            Anim2DMode::Invalid => {}
        }
    }

    pub fn set_status(&mut self, status: Anim2DStatus) {
        self.status.insert(status);
    }

    pub fn clear_status(&mut self, status: Anim2DStatus) {
        self.status.remove(status);
    }

    pub fn get_status(&self) -> Anim2DStatus {
        self.status
    }

    pub fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha;
    }

    pub fn get_alpha(&self) -> f32 {
        self.alpha
    }

    pub fn set_min_frame(&mut self, frame: u16) {
        self.min_frame = frame;
    }

    pub fn set_max_frame(&mut self, frame: u16) {
        self.max_frame = frame;
    }

    pub fn get_current_frame_width(&self) -> u32 {
        let name = match self.resolve_frame_image_name() {
            Some(name) => name,
            None => return 0,
        };
        self.ensure_client_image(&name);
        let collection = get_client_images();
        let collection = collection.read();
        collection
            .find_image_by_name(&name)
            .map(|image| image.get_image_width() as u32)
            .unwrap_or(0)
    }

    pub fn get_current_frame_height(&self) -> u32 {
        let name = match self.resolve_frame_image_name() {
            Some(name) => name,
            None => return 0,
        };
        self.ensure_client_image(&name);
        let collection = get_client_images();
        let collection = collection.read();
        collection
            .find_image_by_name(&name)
            .map(|image| image.get_image_height() as u32)
            .unwrap_or(0)
    }

    pub fn draw(&mut self, x: i32, y: i32) {
        let Some(name) = self.resolve_frame_image_name() else {
            return;
        };
        self.ensure_client_image(&name);
        let collection = get_client_images();
        let collection = collection.read();
        if let Some(image) = collection.find_image_by_name(&name) {
            let width = image.get_image_width();
            let height = image.get_image_height();
            drop(collection);
            self.draw_internal(&name, x, y, x + width, y + height);
        }
    }

    pub fn draw_sized(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let Some(name) = self.resolve_frame_image_name() else {
            return;
        };
        self.ensure_client_image(&name);
        self.draw_internal(&name, x, y, x + width, y + height);
    }

    fn draw_internal(
        &mut self,
        image_name: &str,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) {
        let color = argb_to_rgba_u8(0xFFFFFFFF, self.alpha);

        let rect = UIRect::new(
            start_x as f32,
            start_y as f32,
            (end_x - start_x) as f32,
            (end_y - start_y) as f32,
        );

        let _ = with_ui_renderer(|renderer| {
            let mut renderer = renderer.write().ok()?;
            let client_images = get_client_images();
            let mut collection = client_images.write();
            let image = collection.find_image_by_name_mut(image_name)?;
            if image.get_gpu_texture().is_none() {
                let _ = image.create_gpu_texture(renderer.device(), renderer.queue());
            }
            let gpu = image.get_gpu_texture()?;
            let uv = image.get_uv();
            renderer.draw_textured_rect(
                rect,
                Arc::new(gpu.view().clone()),
                color,
                Some(UIRect::new(uv.min.x, uv.min.y, uv.width(), uv.height())),
                0.0,
            );
            Some(())
        });

        if self.collection_system.is_none() && !self.status.contains(Anim2DStatus::FROZEN) {
            self.try_next_frame();
        }
    }

    fn try_next_frame(&mut self) {
        if self.frames_between_updates == 0 {
            return;
        }

        let now = TheGameLogic::get_frame();
        if now - self.last_update_frame < self.frames_between_updates {
            return;
        }

        let anim_mode = self.template.read().get_anim_mode();
        match anim_mode {
            Anim2DMode::Once => {
                if self.current_frame < self.max_frame {
                    self.set_current_frame(self.current_frame + 1);
                } else {
                    self.set_status(Anim2DStatus::COMPLETE);
                }
            }
            Anim2DMode::OnceBackwards => {
                if self.current_frame > self.min_frame {
                    self.set_current_frame(self.current_frame - 1);
                } else {
                    self.set_status(Anim2DStatus::COMPLETE);
                }
            }
            Anim2DMode::Loop => {
                if self.current_frame == self.max_frame {
                    self.set_current_frame(self.min_frame);
                } else {
                    self.set_current_frame(self.current_frame + 1);
                }
            }
            Anim2DMode::LoopBackwards => {
                if self.current_frame > self.min_frame {
                    self.set_current_frame(self.current_frame - 1);
                } else {
                    self.set_current_frame(self.max_frame);
                }
            }
            Anim2DMode::PingPong | Anim2DMode::PingPongBackwards => {
                if self.status.contains(Anim2DStatus::REVERSED) {
                    if self.current_frame == self.min_frame {
                        self.set_current_frame(self.current_frame + 1);
                        self.clear_status(Anim2DStatus::REVERSED);
                    } else {
                        self.set_current_frame(self.current_frame - 1);
                    }
                } else if self.current_frame == self.max_frame {
                    self.set_current_frame(self.current_frame - 1);
                    self.set_status(Anim2DStatus::REVERSED);
                } else {
                    self.set_current_frame(self.current_frame + 1);
                }
            }
            Anim2DMode::Invalid => {}
        }
    }

    fn resolve_frame_image_name(&self) -> Option<String> {
        let template = self.template.read();
        template
            .get_frame_name(self.current_frame)
            .map(|s| s.to_string())
    }

    fn ensure_client_image(&self, name: &str) -> Option<()> {
        ensure_client_mapped_image(name).then_some(())
    }
}

impl Snapshotable for Anim2D {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        let mut current_frame = self.current_frame;
        xfer.xfer_unsigned_short(&mut current_frame)
            .map_err(|e| format!("{:?}", e))?;
        self.current_frame = current_frame;

        let mut last_update_frame = self.last_update_frame;
        xfer.xfer_unsigned_int(&mut last_update_frame)
            .map_err(|e| format!("{:?}", e))?;
        self.last_update_frame = last_update_frame;

        let mut status = self.status.bits();
        xfer.xfer_unsigned_byte(&mut status)
            .map_err(|e| format!("{:?}", e))?;
        self.status = Anim2DStatus::from_bits_truncate(status);

        let mut min_frame = self.min_frame;
        xfer.xfer_unsigned_short(&mut min_frame)
            .map_err(|e| format!("{:?}", e))?;
        self.min_frame = min_frame;

        let mut max_frame = self.max_frame;
        xfer.xfer_unsigned_short(&mut max_frame)
            .map_err(|e| format!("{:?}", e))?;
        self.max_frame = max_frame;

        let mut frames_between_updates = self.frames_between_updates;
        xfer.xfer_unsigned_int(&mut frames_between_updates)
            .map_err(|e| format!("{:?}", e))?;
        self.frames_between_updates = frames_between_updates;

        let mut alpha = self.alpha;
        xfer.xfer_real(&mut alpha).map_err(|e| format!("{:?}", e))?;
        self.alpha = alpha;

        if xfer.get_xfer_mode() == XferMode::Load {
            // Ensure current frame is valid after load
            let max = self.template.read().get_num_frames();
            if max > 0 {
                let clamped = self.current_frame.min(max.saturating_sub(1));
                self.current_frame = clamped;
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Drop for Anim2D {
    fn drop(&mut self) {
        if let Some(collection) = self.collection_system.as_ref().and_then(|c| c.upgrade()) {
            collection
                .lock()
                .unregister_animation_ptr(self as *const Anim2D);
        }
    }
}

/// Anim2D collection system (tracks instances, templates are in common INI store).
pub struct Anim2DCollection {
    instances: Vec<Weak<Mutex<Anim2D>>>,
}

impl Anim2DCollection {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
        }
    }

    pub fn init(&mut self) {
        let mut ini = INI::default();
        let _ = ini.load("Data/INI/Animation2D.ini", INILoadType::Overwrite);
    }

    pub fn update(&mut self) {
        self.instances.retain(|weak| {
            if let Some(anim) = weak.upgrade() {
                let mut anim = anim.lock();
                if !anim.status.contains(Anim2DStatus::FROZEN) {
                    anim.try_next_frame();
                }
                true
            } else {
                false
            }
        });
    }

    pub fn find_template(&self, name: &AsciiString) -> Option<Arc<RwLock<Anim2DTemplate>>> {
        get_anim2d_collection().and_then(|collection| collection.read().find_template(name))
    }

    pub fn register_animation(&mut self, anim: &Arc<Mutex<Anim2D>>) {
        self.instances.push(Arc::downgrade(anim));
    }

    fn unregister_animation_ptr(&mut self, anim_ptr: *const Anim2D) {
        self.instances.retain(|weak| {
            weak.upgrade()
                .map(|anim| Arc::as_ptr(&anim) as *const Anim2D != anim_ptr)
                .unwrap_or(false)
        });
    }
}

fn argb_to_rgba_u8(color: u32, alpha: f32) -> [f32; 4] {
    let a = ((color >> 24) & 0xFF) as f32 / 255.0;
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    let a = (a * alpha).clamp(0.0, 1.0);
    [r, g, b, a]
}
