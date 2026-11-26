//! Typed `CHUNK_GameClient` archive capture and restore.
//!
//! The positional stream's template TOC is a compression/lookup artifact. The
//! typed archive stores template identity on each ordered drawable instead.
//! Drawable ordering, object bindings, briefing history, and module ordering
//! remain part of the semantic continuation.

use super::*;
use crate::drawable::drawable::BasicDrawableArchive;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameClientDrawableArchive {
    pub template_name: String,
    /// Object ID written immediately before the Drawable payload by C++.
    /// Valid archives keep this equal to the payload's own object binding;
    /// preserving both fields lets validation reject that malformed split.
    pub object_id: ObjectID,
    pub drawable: BasicDrawableArchive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameClientArchive {
    pub frame: u32,
    /// Saveable drawables in the native roster order used by C++.
    pub drawables: Vec<GameClientDrawableArchive>,
    pub briefing_history: Vec<String>,
}

impl GameClientArchive {
    pub fn validate(&self) -> Result<(), String> {
        let mut drawable_ids = std::collections::HashSet::with_capacity(self.drawables.len());
        for entry in &self.drawables {
            if entry.template_name.trim().is_empty() {
                return Err("GameClient archive contains an empty Drawable template".into());
            }
            if entry.object_id != entry.drawable.object_id {
                return Err(format!(
                    "GameClient Drawable '{}' object binding is split across the archive",
                    entry.template_name
                ));
            }
            if entry.drawable.drawable_id == 0 {
                return Err(format!(
                    "GameClient Drawable '{}' has invalid ID zero",
                    entry.template_name
                ));
            }
            if !drawable_ids.insert(entry.drawable.drawable_id) {
                return Err(format!(
                    "duplicate Drawable ID {} in GameClient archive",
                    entry.drawable.drawable_id
                ));
            }
            let mut icon_types = std::collections::HashSet::with_capacity(entry.drawable.icons.len());
            for icon in &entry.drawable.icons {
                if icon.template_name.trim().is_empty() {
                    return Err(format!(
                        "GameClient Drawable '{}' contains an empty icon template",
                        entry.template_name
                    ));
                }
                if !icon_types.insert(icon.icon_type) {
                    return Err(format!(
                        "GameClient Drawable '{}' contains duplicate {:?} icons",
                        entry.template_name, icon.icon_type
                    ));
                }
            }
            for module in &entry.drawable.modules {
                if module.module_type_index >= 2 {
                    return Err(format!(
                        "GameClient Drawable '{}' contains module bucket {}",
                        entry.template_name, module.module_type_index
                    ));
                }
                if module.identifier.trim().is_empty() {
                    return Err(format!(
                        "GameClient Drawable '{}' contains an unnamed module continuation",
                        entry.template_name
                    ));
                }
            }
        }
        Ok(())
    }
}

impl GameClient {
    /// Capture this client's typed GameClient continuation. Rust-owned
    /// auxiliary drawables are validated and excluded exactly as in the
    /// positional capture; they are persisted by their owner instead.
    pub fn capture_archive_with_owned_auxiliaries(
        &mut self,
        owned: &[NativeAuxiliaryReceipt],
    ) -> Result<GameClientArchive, String> {
        self.import_objectless_from_logic_client();
        // A native Drawable must carry the template admitted by its own
        // client. Do not resolve a missing name through another world's
        // compatibility object registry while writing this session's save.
        for &id in &self.drawables.native_drawable_order {
            if owned.iter().any(|receipt| receipt.drawable_id() == id) {
                continue;
            }
            let drawable = self
                .drawables
                .drawable_map
                .get(&id)
                .ok_or_else(|| format!("native Drawable roster ID {} is absent", id.0))?;
            if Self::should_save_drawable(drawable.as_ref())
                && drawable.get_template_name().is_none_or(str::is_empty)
            {
                return Err(format!(
                    "native Drawable '{}' missing its receiving template name for save",
                    id.0
                ));
            }
        }

        let save_entries = self.collect_saveable_drawables_excluding_owned(owned)?;
        let mut drawables = Vec::with_capacity(save_entries.len());
        for (drawable_id, template_name) in save_entries {
            let Some(drawable) = self.drawables.drawable_map.get_mut(&drawable_id) else {
                return Err(format!(
                    "Drawable '{}' disappeared during archive capture",
                    drawable_id.0
                ));
            };
            let object_id = drawable.get_object_id().unwrap_or(INVALID_ID);
            let Some(basic) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() else {
                return Err(format!(
                    "native Drawable '{}' is not a BasicDrawable",
                    drawable_id.0
                ));
            };
            let drawable_archive = basic.capture_archive()?;
            if drawable_archive.object_id != object_id {
                return Err(format!(
                    "Drawable '{}' object binding changed during archive capture",
                    drawable_id.0
                ));
            }
            drawables.push(GameClientDrawableArchive {
                template_name,
                object_id,
                drawable: drawable_archive,
            });
        }

        let archive = GameClientArchive {
            frame: self.frame,
            drawables,
            briefing_history: crate::gui::callbacks::diplomacy::get_briefing_text_list(
                self.menu_manager.diplomacy_mut(),
            ),
        };
        archive.validate()?;
        Ok(archive)
    }

    pub fn capture_archive(&mut self) -> Result<GameClientArchive, String> {
        self.capture_archive_with_owned_auxiliaries(&[])
    }

    pub fn restore_from_archive(&mut self, archive: &GameClientArchive) -> Result<(), String> {
        self.restore_from_archive_with_parts(archive, None, None, None)
    }

    pub fn restore_from_archive_with_receiving_content(
        &mut self,
        archive: &GameClientArchive,
        content: &mut game_engine::common::thing::receiving_module_content::ReceivingModuleContent,
        policy: ReceivingDrawablePolicy,
    ) -> Result<(), String> {
        self.restore_from_archive_with_parts(archive, Some(content), Some(policy), None)
    }

    /// Restore using the receiving world's color for each bound object. The
    /// lookup occurs in archive order before that Drawable's modules apply,
    /// matching the positional stream's object-binding position.
    fn restore_from_archive_with_parts(
        &mut self,
        archive: &GameClientArchive,
        mut receiving_content: Option<
            &mut game_engine::common::thing::receiving_module_content::ReceivingModuleContent,
        >,
        receiving_policy: Option<ReceivingDrawablePolicy>,
        mut receiving_indicator_color: Option<
            &mut dyn FnMut(ObjectID) -> Result<(u8, u8, u8), String>,
        >,
    ) -> Result<(), String> {
        self.restore_from_archive_impl(
            archive,
            receiving_content.as_deref_mut(),
            receiving_policy,
            receiving_indicator_color,
        )
    }

    /// Restore using the receiving world's color for each bound object. The
    /// lookup occurs in archive order before that Drawable's modules apply,
    /// matching the positional stream's object-binding position.
    pub fn restore_from_archive_with_receiving_content_and_indicator_colors(
        &mut self,
        archive: &GameClientArchive,
        content: Option<
            &mut game_engine::common::thing::receiving_module_content::ReceivingModuleContent,
        >,
        policy: ReceivingDrawablePolicy,
        mut indicator_color_for_object: impl FnMut(ObjectID) -> Result<(u8, u8, u8), String>,
    ) -> Result<(), String> {
        self.restore_from_archive_with_parts(
            archive,
            content,
            Some(policy),
            Some(&mut indicator_color_for_object),
        )
    }

    fn restore_from_archive_impl(
        &mut self,
        archive: &GameClientArchive,
        mut receiving_content: Option<
            &mut game_engine::common::thing::receiving_module_content::ReceivingModuleContent,
        >,
        receiving_policy: Option<ReceivingDrawablePolicy>,
        mut receiving_indicator_color: Option<
            &mut dyn FnMut(ObjectID) -> Result<(u8, u8, u8), String>,
        >,
    ) -> Result<(), String> {
        archive.validate()?;
        let native_receiving = receiving_policy.is_some();
        self.pending_drawable_post_process_order = None;
        self.invalidate_presentation_shell();
        self.frame = archive.frame;

        if native_receiving && !archive.drawables.is_empty() && receiving_content.is_none() {
            return Err("receiving Drawable template catalog was not admitted".into());
        }
        // Retire prior native resources only after the catalog required by
        // this archive has been admitted, preserving the positional restore's
        // commit boundary.
        self.try_invalidate_presentation_drawable_world()
            .map_err(|error| error.to_string())?;

        let mut decoded_drawable_order = Vec::new();
        let mut decoded_drawable_ids = std::collections::HashSet::new();

        for entry in &archive.drawables {
            let object_id = entry.object_id;
            let template = if native_receiving {
                receiving_content
                    .as_deref()
                    .ok_or("receiving Drawable template catalog was not admitted")?
                    .resolve_template(&entry.template_name)
                    .map(XferDrawableTemplate::Receiving)
            } else {
                let template = {
                    let guard = get_thing_factory().map_err(|_| "ThingFactory lock failed")?;
                    let factory = guard
                        .as_ref()
                        .ok_or_else(|| "ThingFactory not initialized".to_string())?;
                    factory.find_template(&entry.template_name, false)
                };
                template.map(XferDrawableTemplate::Compatibility)
            };
            // The positional loader skips a Drawable whose template is absent
            // from the receiving catalog. Preserve that admission behavior.
            let Some(template) = template else {
                continue;
            };

            let receiving_color = if native_receiving && object_id != INVALID_ID {
                let resolve = receiving_indicator_color.as_deref_mut().ok_or_else(|| {
                    format!(
                        "receiving object resolver is required for Drawable object {object_id}"
                    )
                })?;
                Some(resolve(object_id)?)
            } else {
                None
            };

            let mut drawable = None;
            if object_id != INVALID_ID {
                if let Some(existing_id) = self.get_drawable_for_object(object_id) {
                    let needs_replace =
                        if let Some(existing) = self.drawables.drawable_map.get(&existing_id) {
                            !template.matches_existing(existing.as_ref())?
                        } else {
                            true
                        };
                    if needs_replace {
                        self.destroy_drawable(existing_id)
                            .map_err(|error| error.to_string())?;
                    } else {
                        drawable = self.drawables.take_native_drawable_for_load(existing_id);
                    }
                }
            }

            if drawable.is_none() {
                self.advance_drawable_id_past_live_nodes()?;
                let created_id = match template.receiving_admission() {
                    Some(admission) => match receiving_content.as_deref_mut() {
                        Some(content) => self.create_drawable_from_receiving_template(
                            admission,
                            content,
                            receiving_policy.ok_or_else(|| {
                                "receiving template restore requires Drawable policy".to_owned()
                            })?,
                        ),
                        None => Err(GameClientError::InvalidOperation(
                            "receiving template resolution lost its ModuleFactory owner".into(),
                        )),
                    },
                    None => self.create_drawable_from_template(template.template()),
                }
                .map_err(|error| {
                    format!(
                        "GameClient archive - Unable to create drawable for '{}': {}",
                        template.template().get_name(),
                        error
                    )
                })?;
                let (mut created, receiving_position) = self
                    .drawables
                    .take_native_drawable_for_load(created_id)
                    .ok_or_else(|| {
                        format!(
                            "GameClient archive - Created drawable '{}' was not registered",
                            created_id.0
                        )
                    })?;
                self.drawables
                    .owned_laser_order
                    .retain(|owner| *owner != created_id);
                if object_id != INVALID_ID && !native_receiving {
                    created.set_object_id(Some(object_id));
                }
                drawable = Some((created, receiving_position));
            }

            let (mut drawable, receiving_position) = drawable.expect("drawable exists");
            if let Some(color) = receiving_color
                && drawable.get_object_id() != Some(object_id)
            {
                let Some(basic) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() else {
                    return Err(format!(
                        "receiving drawable '{}' is not a BasicDrawable",
                        entry.template_name
                    ));
                };
                basic.friend_bind_to_object_with_receiving_color(object_id, color);
            }
            if let Some(admission) = template.receiving_admission() {
                let basic = drawable
                    .as_any_mut()
                    .downcast_mut::<BasicDrawable>()
                    .ok_or_else(|| {
                        format!(
                            "receiving drawable '{}' is not a BasicDrawable",
                            entry.template_name
                        )
                    })?;
                basic.initialize_native_constructor_template(
                    Some(entry.template_name.clone()),
                    Some(admission),
                );
            }

            let Some(basic) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() else {
                return Err(format!(
                    "receiving drawable '{}' is not a BasicDrawable",
                    entry.template_name
                ));
            };
            basic.apply_archive(
                &entry.drawable,
                archive.frame,
                self.native_audio_device.as_ref(),
                native_receiving,
                self.owned_world_animations.disabled_icon_content(self.input_authority),
            )?;

            if native_receiving {
                let expected_object = (object_id != INVALID_ID).then_some(object_id);
                if drawable.get_object_id() != expected_object {
                    return Err(format!(
                        "Drawable '{}' archive object does not match its receiving binding",
                        entry.template_name
                    ));
                }
            }
            let id = drawable.get_id();
            if !decoded_drawable_ids.insert(id) {
                return Err(format!(
                    "duplicate Drawable ID {} in GameClient archive",
                    id.0
                ));
            }
            if self.drawables.drawable_map.contains_key(&id) {
                return Err(format!(
                    "Drawable ID {} in GameClient archive collides with a live receiver",
                    id.0
                ));
            }
            if let Some(object_id) = drawable.get_object_id() {
                self.drawables.drawable_object_map.insert(object_id, id);
            }
            let native_lasers = Self::owned_laser_count(drawable.as_mut());
            if native_lasers > 1 {
                return Err("multiple owned LaserUpdate callbacks in restored drawable".into());
            }
            Self::bind_owned_laser_module(drawable.as_mut(), id);
            self.drawables
                .owned_laser_order
                .retain(|owner| *owner != id);
            if native_lasers == 1 {
                self.drawables.owned_laser_order.insert(0, id);
            }
            self.drawables
                .restore_native_drawable_after_load(id, drawable, receiving_position);
            decoded_drawable_order.push(id);

            if !native_receiving && object_id != INVALID_ID {
                if OBJECT_REGISTRY.get_object(object_id).is_some() {
                    let _ = self.bind_drawable_to_object(id, object_id);
                }
            }
        }

        // C++ GameClient::xfer v2+ briefing history. Decode is already
        // detached in the typed archive; publish the clear-and-replay sequence
        // only after Drawable admission has succeeded.
        let diplomacy = self.menu_manager.diplomacy_mut();
        self.window_manager_owner.with_manager(|manager| {
            crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text(
                "", true, manager, diplomacy,
            )
        });
        for entry in &archive.briefing_history {
            self.window_manager_owner.with_manager(|manager| {
                crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text(
                    entry, false, manager, diplomacy,
                )
            });
        }

        self.load_post_process_with_registry(!native_receiving)?;
        self.pending_drawable_post_process_order = Some(decoded_drawable_order);
        self.export_objectless_to_logic_client();
        Ok(())
    }
}
