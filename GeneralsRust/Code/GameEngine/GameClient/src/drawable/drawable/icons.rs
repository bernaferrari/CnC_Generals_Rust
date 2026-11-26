//! Drawable icon types, Anim2D-backed icons, and icon snapshot/xfer.

use super::*;
use crate::system::{Anim2D, Anim2DCollection};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::{Anim2DTemplate, get_anim2d_collection};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

/// Types of drawable icons (converted from C++ DrawableIconType)
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub enum IconType {
    DefaultHeal,
    StructureHeal,
    VehicleHeal,
    Demoralized,
    BombTimed,
    BombRemote,
    Disabled,
    BattleplanBombard,
    BattleplanHoldTheLine,
    BattleplanSearchAndDestroy,
    Emoticon,
    Enthusiastic,
    EnthusiasticSubliminal,
    CarBomb,
}

impl IconType {
    /// C++ parity order used by Drawable::xfer icon serialization.
    /// C++ writes icon slots in fixed enum order; keep Rust stable too.
    pub const XFER_ORDER: [IconType; 14] = [
        IconType::DefaultHeal,
        IconType::StructureHeal,
        IconType::VehicleHeal,
        IconType::Demoralized,
        IconType::BombTimed,
        IconType::BombRemote,
        IconType::Disabled,
        IconType::BattleplanBombard,
        IconType::BattleplanHoldTheLine,
        IconType::BattleplanSearchAndDestroy,
        IconType::Emoticon,
        IconType::Enthusiastic,
        IconType::EnthusiasticSubliminal,
        IconType::CarBomb,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            IconType::DefaultHeal => "DefaultHeal",
            IconType::StructureHeal => "StructureHeal",
            IconType::VehicleHeal => "VehicleHeal",
            IconType::Demoralized => "Demoralized",
            IconType::BombTimed => "BombTimed",
            IconType::BombRemote => "BombRemote",
            IconType::Disabled => "Disabled",
            IconType::BattleplanBombard => "BattlePlanIcon_Bombard",
            IconType::BattleplanHoldTheLine => "BattlePlanIcon_HoldTheLine",
            IconType::BattleplanSearchAndDestroy => "BattlePlanIcon_SeekAndDestroy",
            IconType::Emoticon => "Emoticon",
            IconType::Enthusiastic => "Enthusiastic",
            IconType::EnthusiasticSubliminal => "Subliminal",
            IconType::CarBomb => "CarBomb",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "DefaultHeal" => Some(IconType::DefaultHeal),
            "StructureHeal" => Some(IconType::StructureHeal),
            "VehicleHeal" => Some(IconType::VehicleHeal),
            "Demoralized" => Some(IconType::Demoralized),
            "BombTimed" => Some(IconType::BombTimed),
            "BombRemote" => Some(IconType::BombRemote),
            "Disabled" => Some(IconType::Disabled),
            "BattlePlanIcon_Bombard" => Some(IconType::BattleplanBombard),
            "BattlePlanIcon_HoldTheLine" => Some(IconType::BattleplanHoldTheLine),
            "BattlePlanIcon_SeekAndDestroy" => Some(IconType::BattleplanSearchAndDestroy),
            "Emoticon" => Some(IconType::Emoticon),
            "Enthusiastic" => Some(IconType::Enthusiastic),
            "Subliminal" => Some(IconType::EnthusiasticSubliminal),
            "CarBomb" => Some(IconType::CarBomb),
            _ => None,
        }
    }
}

/// Definition available to this drawable's receiving HUD and archive reader.
#[derive(Clone, Copy)]
pub(crate) enum DisabledIconContent<'a> {
    Compatibility,
    Receiving(Option<&'a Arc<RwLock<Anim2DTemplate>>>),
}


/// Typed continuation for one Anim2D-backed Drawable icon.
///
/// The positional stream's icon-count byte and fixed enum order are container
/// artifacts. The archive stores each present icon explicitly in that same
/// fixed order, preserving template identity, expiry, and animation state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawableIconArchive {
    pub icon_type: IconType,
    pub keep_till_frame: u32,
    pub template_name: String,
    pub anim: crate::system::anim2_d::Anim2DArchiveState,
}

/// Icon information for drawable objects
#[derive(Debug, Clone)]
pub struct IconInfo {
    pub icons: HashMap<IconType, Arc<dyn Icon>>,
    pub keep_till_frame: HashMap<IconType, u32>,
}

impl Default for IconInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl IconInfo {
    pub fn new() -> Self {
        Self {
            icons: HashMap::new(),
            keep_till_frame: HashMap::new(),
        }
    }

    pub fn set_icon(
        &mut self,
        icon_type: IconType,
        icon: Arc<dyn Icon>,
        duration_frames: u32,
        current_frame: u32,
    ) {
        self.icons.insert(icon_type, icon);
        self.keep_till_frame
            .insert(icon_type, current_frame + duration_frames);
    }

    pub fn clear_icon(&mut self, icon_type: IconType) {
        self.icons.remove(&icon_type);
        self.keep_till_frame.remove(&icon_type);
    }

    pub fn update(&mut self, current_frame: u32) {
        let expired_icons: Vec<IconType> = self
            .keep_till_frame
            .iter()
            .filter(|(kind, frame)| {
                **frame <= current_frame && !(**kind == IconType::Disabled && **frame == 0)
            })
            .map(|(icon_type, _)| *icon_type)
            .collect();

        for icon_type in expired_icons {
            self.clear_icon(icon_type);
        }
    }


    /// Capture icons in the fixed C++ Drawable xfer order.
    pub fn capture_archive(&self) -> Result<Vec<DrawableIconArchive>, String> {
        let mut entries = Vec::with_capacity(self.icons.len());
        for icon_type in IconType::XFER_ORDER {
            let Some(icon) = self.icons.get(&icon_type) else {
                continue;
            };
            let anim_icon = icon
                .as_any()
                .downcast_ref::<Anim2DIcon>()
                .ok_or_else(|| "Icon is not Anim2D-backed".to_string())?;
            entries.push(DrawableIconArchive {
                icon_type,
                keep_till_frame: *self.keep_till_frame.get(&icon_type).unwrap_or(&0),
                template_name: anim_icon.template_name().to_string(),
                anim: anim_icon.anim.lock().capture_archive_state(),
            });
        }
        Ok(entries)
    }

    /// Reconstruct icon state using the same template-resolution split as the
    /// positional loader. Constructor frame/random observations still happen
    /// before the saved continuation is applied.
    pub fn from_archive(
        entries: &[DrawableIconArchive],
        disabled_content: DisabledIconContent<'_>,
    ) -> Result<Option<Self>, String> {
        if entries.is_empty() {
            return Ok(None);
        }
        let mut info = Self::new();
        for entry in entries {
            let mut icon = match (entry.icon_type, disabled_content) {
                (IconType::Disabled, DisabledIconContent::Receiving(template)) => {
                    let template = template
                        .filter(|template| template.read().get_name().as_str() == entry.template_name)
                        .ok_or_else(|| {
                            format!(
                                "Unknown receiving Anim2D template '{}'",
                                entry.template_name
                            )
                        })?;
                    Anim2DIcon {
                        anim: Anim2D::new_for_restore(Arc::clone(template)),
                        template_name: entry.template_name.clone(),
                    }
                }
                _ => Anim2DIcon::from_template_name(
                    &entry.template_name,
                    &mut game_engine::common::random_value::RandomState::default(),
                )?,
            };
            icon.anim.lock().apply_archive_state(entry.anim);
            info.icons.insert(entry.icon_type, Arc::new(icon));
            info.keep_till_frame.insert(entry.icon_type, entry.keep_till_frame);
        }
        Ok(Some(info))
    }

    pub(crate) fn xfer_cpp_layout(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.xfer_cpp_layout_with_disabled_content(xfer, DisabledIconContent::Compatibility)
    }

    pub(crate) fn xfer_cpp_layout_with_disabled_content(
        &mut self,
        xfer: &mut dyn Xfer,
        disabled_content: DisabledIconContent<'_>,
    ) -> Result<(), String> {
        let mut icon_count = self.icons.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut icon_count)
            .map_err(|e| format!("{:?}", e))?;

        self.xfer_icon_entries(xfer, icon_count, disabled_content)
    }

    pub(crate) fn xfer_versioned_with_disabled_content(
        &mut self,
        xfer: &mut dyn Xfer,
        disabled_content: DisabledIconContent<'_>,
    ) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        self.xfer_cpp_layout_with_disabled_content(xfer, disabled_content)
    }

    fn xfer_icon_entries(
        &mut self,
        xfer: &mut dyn Xfer,
        icon_count: u8,
        disabled_content: DisabledIconContent<'_>,
    ) -> Result<(), String> {
        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for icon_type in IconType::XFER_ORDER {
                    let Some(icon) = self.icons.get(&icon_type) else {
                        continue;
                    };

                    let mut icon_name = icon_type.name().to_string();
                    xfer.xfer_ascii_string(&mut icon_name)
                        .map_err(|e| format!("{:?}", e))?;

                    let mut keep = *self.keep_till_frame.get(&icon_type).unwrap_or(&0);
                    xfer.xfer_unsigned_int(&mut keep)
                        .map_err(|e| format!("{:?}", e))?;

                    let mut template_name = icon
                        .anim2d_template_name()
                        .ok_or_else(|| "Icon is not Anim2D-backed".to_string())?
                        .to_string();
                    xfer.xfer_ascii_string(&mut template_name)
                        .map_err(|e| format!("{:?}", e))?;

                    icon.xfer(xfer)?;
                }
            }
            XferMode::Load => {
                self.icons.clear();
                self.keep_till_frame.clear();

                for _ in 0..icon_count {
                    let mut icon_name = String::new();
                    xfer.xfer_ascii_string(&mut icon_name)
                        .map_err(|e| format!("{:?}", e))?;
                    let icon_type = IconType::from_name(&icon_name)
                        .ok_or_else(|| format!("Unknown icon type '{}'", icon_name))?;

                    let mut keep = 0u32;
                    xfer.xfer_unsigned_int(&mut keep)
                        .map_err(|e| format!("{:?}", e))?;

                    let mut template_name = String::new();
                    xfer.xfer_ascii_string(&mut template_name)
                        .map_err(|e| format!("{:?}", e))?;
                    let icon = match (icon_type, disabled_content) {
                        (IconType::Disabled, DisabledIconContent::Receiving(template)) => {
                            let template = template
                                .filter(|template| template.read().get_name().as_str() == template_name)
                                .ok_or_else(|| format!("Unknown receiving Anim2D template '{template_name}'"))?;
                            Anim2DIcon {
                                anim: Anim2D::new_for_restore(Arc::clone(template)),
                                template_name,
                            }
                        }
                        _ => Anim2DIcon::from_template_name(
                            &template_name,
                            &mut game_engine::common::random_value::RandomState::default(),
                        )?,
                    };
                    icon.xfer(xfer)?;

                    self.icons.insert(icon_type, Arc::new(icon));
                    self.keep_till_frame.insert(icon_type, keep);
                }
            }
            XferMode::Invalid => {
                return Err("IconInfo::xfer_icon_entries - invalid xfer mode".to_string());
            }
        }

        Ok(())
    }
}

impl Snapshotable for IconInfo {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut icon_count = self.icons.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut icon_count)
            .map_err(|e| format!("{:?}", e))?;

        for icon_type in IconType::XFER_ORDER {
            let Some(icon) = self.icons.get(&icon_type) else {
                continue;
            };

            let mut icon_name = icon_type.name().to_string();
            xfer.xfer_ascii_string(&mut icon_name)
                .map_err(|e| format!("{:?}", e))?;

            let mut keep = *self.keep_till_frame.get(&icon_type).unwrap_or(&0);
            xfer.xfer_unsigned_int(&mut keep)
                .map_err(|e| format!("{:?}", e))?;

            let mut template_name = icon
                .anim2d_template_name()
                .ok_or_else(|| "Icon is not Anim2D-backed".to_string())?
                .to_string();
            xfer.xfer_ascii_string(&mut template_name)
                .map_err(|e| format!("{:?}", e))?;

            icon.xfer(xfer)?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.xfer_versioned_with_disabled_content(xfer, DisabledIconContent::Compatibility)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl BasicDrawable {
    pub(crate) fn observe_disabled_icon(&mut self, disabled: bool) {
        self.overlay_data.show_disabled = disabled;
        if !disabled {
            if let Some(icons) = self.icon_info.as_mut() {
                icons.clear_icon(IconType::Disabled);
            }
        }
    }

    pub(crate) fn disabled_icon_for_draw(
        &mut self,
        collection: &Arc<Mutex<Anim2DCollection>>,
        content: DisabledIconContent<'_>,
        client_random: &mut game_engine::common::random_value::RandomState,
    ) -> Option<&dyn Icon> {
        use std::collections::hash_map::Entry;
        let icons = self.get_icon_info_mut();
        let stored = match icons.icons.entry(IconType::Disabled) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let icon = match content {
                    DisabledIconContent::Receiving(template) => Anim2DIcon::new(
                        Arc::clone(template?),
                        Some(Arc::clone(collection)),
                        client_random,
                    ),
                    DisabledIconContent::Compatibility => {
                        Anim2DIcon::from_template_name_in_collection(
                            "Disabled",
                            Some(Arc::clone(collection)),
                            client_random,
                        )
                        .ok()?
                    }
                };
                entry.insert(Arc::new(icon))
            }
        };
        Some(&**stored)
    }

    pub(crate) fn register_restored_disabled_icon(&self, collection: &Arc<Mutex<Anim2DCollection>>) {
        if let Some(icon) = self.icon_info.as_ref().and_then(|icons| icons.icons.get(&IconType::Disabled)) {
            icon.attach_to_anim2d_collection(collection);
        }
    }
}

/// Trait for drawable icons
pub trait Icon: std::fmt::Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn render(&self, position: Vector3, size: Vector3, renderer: &mut crate::gui::UIRenderer);
    fn render_with_receiving_images(
        &self,
        _rect: crate::gui::ui_renderer::UIRect,
        _images: &mut crate::display::image::ImageCollection,
        _renderer: &mut crate::gui::UIRenderer,
    ) {
    }
    fn anim2d_template_name(&self) -> Option<&str> {
        None
    }
    fn attach_to_anim2d_collection(&self, _collection: &Arc<Mutex<Anim2DCollection>>) {}
    fn xfer(&self, xfer: &mut dyn Xfer) -> Result<(), String>;
}

/// Anim2D-backed drawable icon (parity with C++ Anim2D icons).
pub struct Anim2DIcon {
    anim: Arc<Mutex<Anim2D>>,
    template_name: String,
}

impl std::fmt::Debug for Anim2DIcon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Anim2DIcon")
            .field("template_name", &self.template_name)
            .finish()
    }
}

impl Anim2DIcon {
    pub fn new(
        template: Arc<RwLock<Anim2DTemplate>>,
        collection: Option<Arc<Mutex<Anim2DCollection>>>,
        client_random: &mut game_engine::common::random_value::RandomState,
    ) -> Self {
        let template_name = template.read().get_name().as_str().to_string();
        let anim = Anim2D::new(template, collection, client_random);
        Self {
            anim,
            template_name,
        }
    }

    pub fn from_template_name(
        name: &str,
        client_random: &mut game_engine::common::random_value::RandomState,
    ) -> Result<Self, String> {
        Self::from_template_name_in_collection(name, None, client_random)
    }

    fn from_template_name_in_collection(
        name: &str,
        collection: Option<Arc<Mutex<Anim2DCollection>>>,
        client_random: &mut game_engine::common::random_value::RandomState,
    ) -> Result<Self, String> {
        let template_name = name.to_string();
        let name_key = AsciiString::from(name);
        let template = get_anim2d_collection()
            .and_then(|collection| collection.read().find_template(&name_key))
            .ok_or_else(|| format!("Unknown Anim2D template '{}'", template_name))?;
        Ok(Self::new(template, collection, client_random))
    }

    pub fn template_name(&self) -> &str {
        &self.template_name
    }
}

impl Icon for Anim2DIcon {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn render(&self, position: Vector3, size: Vector3, renderer: &mut crate::gui::UIRenderer) {
        let mut anim = self.anim.lock();
        anim.draw_sized(
            position.x as i32,
            position.y as i32,
            size.x as i32,
            size.y as i32,
            renderer,
        );
    }

    fn render_with_receiving_images(
        &self,
        rect: crate::gui::ui_renderer::UIRect,
        images: &mut crate::display::image::ImageCollection,
        renderer: &mut crate::gui::UIRenderer,
    ) {
        self.anim
            .lock()
            .draw_sized_with_receiving_images(rect, images, renderer);
    }

    fn anim2d_template_name(&self) -> Option<&str> {
        Some(self.template_name())
    }

    fn attach_to_anim2d_collection(&self, collection: &Arc<Mutex<Anim2DCollection>>) {
        Anim2D::attach_to_collection(&self.anim, collection);
    }

    fn xfer(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut anim = self.anim.lock();
        anim.xfer(xfer)
    }
}
