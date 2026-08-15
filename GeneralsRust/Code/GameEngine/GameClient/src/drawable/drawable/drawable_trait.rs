//! Drawable trait surface plus downcast helpers.

use super::*;
use crate::system::TimeOfDay;
use game_engine::common::system::Xfer;
use std::any::Any;
use std::error::Error;

pub trait DrawableDowncast {
    /// Get a reference to the object as Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Get a mutable reference to the object as Any for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Extension trait for Drawable downcasting operations
pub trait DrawableExt {
    /// Try to downcast to a specific drawable type
    fn downcast_ref<T: 'static>(&self) -> Option<&T>;

    /// Try to downcast to a specific drawable type (mutable)
    fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T>;
}

/// Main drawable trait that all renderable objects must implement
pub trait Drawable: std::fmt::Debug + Send + Sync + DrawableDowncast {
    /// Get unique identifier for this drawable
    fn get_id(&self) -> DrawableId;

    /// Assign a unique identifier to this drawable (default no-op)
    fn set_id(&mut self, _id: DrawableId) {}

    /// Get current world position
    fn get_position(&self) -> Vector3;

    /// Set world position
    fn set_position(&mut self, position: Vector3);

    /// Get current world transformation matrix
    fn get_transform(&self) -> Matrix4;

    /// Set instance transformation matrix
    fn set_instance_transform(&mut self, transform: Matrix4);

    fn is_instance_identity(&self) -> bool;

    fn get_instance_scale(&self) -> f32;

    /// Set instance scale factor
    fn set_instance_scale(&mut self, scale: f32);

    /// Get drawable status flags
    fn get_status(&self) -> DrawableStatus;

    /// Set drawable status flags
    fn set_status(&mut self, status: DrawableStatus);

    /// Check if drawable is currently visible
    fn is_visible(&self) -> bool;

    /// Set drawable visibility
    fn set_visible(&mut self, visible: bool);

    /// C++ Drawable::setFullyObscuredByShroud residual.
    /// Default no-op for drawables without shroud state.
    fn set_fully_obscured_by_shroud(&mut self, _fully_obscured: bool) {}

    /// C++ `Drawable::getFullyObscuredByShroud` for presentation readers.
    ///
    /// `None` keeps custom/objectless implementations from being interpreted
    /// as a direct-object shroud decision.
    fn fully_obscured_by_shroud(&self) -> Option<bool> {
        None
    }

    /// Exact C++ `Drawable::isDrawableEffectivelyHidden` value for the W3D
    /// `Visibility_Check`/scene path.
    ///
    /// This must remain narrower than generic presentation visibility: C++
    /// checks only `m_hidden || m_hiddenByStealth` before scene dispatch.
    /// `None` keeps non-direct/objectless implementations out of Main's
    /// direct-host sidecar.
    fn scene_effectively_hidden(&self) -> Option<bool> {
        None
    }

    /// Apply C++ GameClient's frozen direct-object shroud decision.
    ///
    /// `None` means this drawable has no base Drawable clear-frame state.
    /// Implementations must only update fully-obscured state here; direct W3D
    /// scene dispatch remains the sole owner of clear-frame refreshes.
    fn apply_frozen_direct_shroud_status(
        &mut self,
        _logic_frame: u32,
        _raw_status: gamelogic::common::types::ObjectShroudStatus,
        _effectively_dead: bool,
    ) -> Option<crate::drawable::ClientShroudVisibility> {
        None
    }

    /// Evaluate the C++ W3D direct-scene shroud branch for one already
    /// selected frozen visual candidate.
    ///
    /// This is the sole trait hook allowed to refresh a Drawable's volatile
    /// clear-frame history. Callers must have completed frozen frustum and
    /// render-model eligibility first; implementations retain their own
    /// effective-hidden check before mutating that history.
    fn evaluate_frozen_direct_scene_candidate(
        &mut self,
        _logic_frame: u32,
        _raw_status: gamelogic::common::types::ObjectShroudStatus,
        _effectively_dead: bool,
    ) -> Option<crate::drawable::SceneShroudDecision> {
        None
    }

    /// Reset volatile direct-object shroud state during world replacement.
    ///
    /// Ordinary object rebinds must not use this hook.
    fn reset_volatile_shroud_state(&mut self) {}

    /// Check if drawable is selected
    fn is_selected(&self) -> bool;

    /// Set drawable selection state
    fn set_selected(&mut self, selected: bool);

    /// Get current opacity (0.0 = transparent, 1.0 = opaque)
    fn get_opacity(&self) -> f32;

    /// Set drawable opacity
    fn set_opacity(&mut self, opacity: f32);

    /// Get stealth visualization mode
    fn get_stealth_look(&self) -> StealthLook;

    /// Set stealth visualization mode
    fn set_stealth_look(&mut self, stealth_look: StealthLook);

    /// Get tint color for visual effects
    fn get_tint_color(&self) -> Vector3;

    /// Set tint color
    fn set_tint_color(&mut self, color: Vector3);

    /// Flash drawable with specified color and duration
    fn flash_color(&mut self, color: Vector3, duration_frames: u32);

    /// Update drawable (called each frame)
    fn update(&mut self, delta_time: f32);

    /// Render drawable to screen.
    /// Takes &mut self because rendering may toggle shadow state per-frame
    /// based on stealth look (C++ parity: Drawable::draw() is non-const).
    fn render(&mut self, view_matrix: &Matrix4, projection_matrix: &Matrix4);

    /// Get bounding sphere for culling
    fn get_bounding_sphere(&self) -> (Vector3, f32); // center, radius

    /// Check if drawable should receive dynamic lighting
    fn receives_dynamic_lights(&self) -> bool;

    /// Set whether drawable receives dynamic lighting
    fn set_receives_dynamic_lights(&mut self, receives: bool);

    /// Get terrain decal type
    fn get_terrain_decal_type(&self) -> TerrainDecalType;

    /// Set terrain decal type
    fn set_terrain_decal_type(&mut self, decal_type: TerrainDecalType);

    /// Get the owning object ID if this drawable is bound to a GameLogic object.
    fn get_object_id(&self) -> Option<u32> {
        None
    }

    /// Set the owning object ID (default no-op).
    fn set_object_id(&mut self, _object_id: Option<u32>) {}

    /// Get the template name used to create this drawable, if available.
    fn get_template_name(&self) -> Option<&str> {
        None
    }

    /// Set the template name used to create this drawable (default no-op).
    fn set_template_name(&mut self, _name: Option<String>) {}

    /// Render UI overlays/text associated with this drawable (default noop)
    fn draw_ui_text(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Populate icon UI overlay state after the drawable's 3D pass.
    /// Wave 270: trait default fail-closed (no dual-world walk). BasicDrawable overrides.
    fn draw_icon_ui(&mut self) {
        // Wave 270: default icon UI is empty; registry-bound types override.
    }

    /// Update drawable based on current time-of-day (default noop)
    fn set_time_of_day(&self, _time_of_day: TimeOfDay) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Preload any assets needed by this drawable (default noop)
    fn preload_assets(&self, _time_of_day: TimeOfDay) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Set the frame index used by time-based drawable logic (default noop).
    fn set_current_frame(&mut self, _frame: u32) {}

    /// Whether this drawable should be auto-destroyed at the current frame.
    fn is_expired(&self, _current_frame: u32) -> bool {
        false
    }

    /// Snapshot transfer hook for drawable-specific save/load state.
    fn xfer_snapshot(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Err("Drawable type does not support snapshot serialization".to_string())
    }
}

impl<T: Drawable + ?Sized> DrawableExt for T {
    fn downcast_ref<U: 'static>(&self) -> Option<&U> {
        self.as_any().downcast_ref::<U>()
    }

    fn downcast_mut<U: 'static>(&mut self) -> Option<&mut U> {
        let any = DrawableDowncast::as_any_mut(self);
        any.downcast_mut::<U>()
    }
}
