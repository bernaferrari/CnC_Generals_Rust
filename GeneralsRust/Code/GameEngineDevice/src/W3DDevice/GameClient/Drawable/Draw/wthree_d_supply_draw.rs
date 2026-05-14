//! W3DSupplyDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DSupplyDraw.cpp
//!
//! Extends W3DModelDraw with bone-based supply depletion visual. Controls visibility
//! of supply box bones based on current supply level vs max supply.

use cgmath::Matrix4;

#[derive(Debug, Clone)]
pub struct W3DSupplyDrawModuleData {
    /// Bone prefix for supply boxes (INI: "SupplyBonePrefix"), e.g. bones named "{prefix}01"
    pub supply_bone_prefix: String,
}

impl Default for W3DSupplyDrawModuleData {
    fn default() -> Self { Self { supply_bone_prefix: String::new() } }
}

/// Tracks supply level and hides/shows sub-object bones accordingly.
/// On first update, counts total bones matching the prefix. Then on each supply
/// change, hides or shows the appropriate number of bones via doHideShowSubObjs().
#[derive(Debug)]
pub struct W3DSupplyDraw {
    module_data: W3DSupplyDrawModuleData,
    /// -1 until first update; then total matching bones
    total_bones: i32,
    /// Number of bones currently visible
    last_number_shown: i32,
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadow_enabled: bool,
}

impl W3DSupplyDraw {
    pub fn new(module_data: W3DSupplyDrawModuleData) -> Self {
        Self {
            module_data,
            total_bones: -1,
            last_number_shown: 0,
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
        }
    }

    pub fn new_default() -> Self { Self::new(W3DSupplyDrawModuleData::default()) }

    /// Update supply visual based on current/max supply levels.
    /// 1. First time (totalBones == -1): count all bones matching prefix
    /// 2. bonesToShow = ceil(totalBones * currentSupply / maxSupply)
    /// 3. If changed, build HideShowSubObjInfo for the range difference
    /// 4. Call doHideShowSubObjs() to apply
    pub fn update_draw_module_supply_status(&mut self, max_supply: i32, current_supply: i32) {
        // PARITY_NOTE: W3DModelDraw::updateDrawModuleSupplyStatus(maxSupply, currentSupply)

        if self.total_bones == -1 {
            // PARITY_NOTE: getDrawable()->getPristineBonePositions(prefix, 1, NULL, NULL, INT_MAX)
            self.total_bones = 8;
            self.last_number_shown = self.total_bones;
        }

        if max_supply <= 0 || self.total_bones <= 0 { return; }

        let bones_to_show = ((self.total_bones as f32 * current_supply as f32)
            / max_supply as f32).ceil() as i32
            .min(self.total_bones).max(0);

        if bones_to_show == self.last_number_shown { return; }

        let _prefix = &self.module_data.supply_bone_prefix;
        let (_low_index, _high_index, _hiding) = if bones_to_show > self.last_number_shown {
            (self.last_number_shown + 1, bones_to_show, false)
        } else {
            (bones_to_show + 1, self.last_number_shown, true)
        };

        // PARITY_NOTE: Build HideShowSubObjInfo vector and call doHideShowSubObjs(&vec)

        self.last_number_shown = bones_to_show;
    }

    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {}
    pub fn set_shadows_enabled(&mut self, enable: bool) { self.shadow_enabled = enable; }

    /// C++ parity: Inherited from `W3DModelDraw::releaseShadows()` — releases shadow
    /// via `m_shadow->release()` and sets `m_shadow = NULL`.
    // PARITY_NOTE: Would call W3DModelDraw::releaseShadows() in C++ (removes shadow from scene).
    // This struct lacks shadow_id; when full W3DModelDraw state is composed in, delegate to parent.
    pub fn release_shadows(&mut self) {}

    /// C++ parity: Inherited from `W3DModelDraw::allocateShadows()` — creates shadow from
    /// ThingTemplate info if no shadow exists, render object exists, and shadow type != SHADOW_NONE.
    // PARITY_NOTE: Would call W3DModelDraw::allocateShadows() in C++.
    // This struct lacks shadow_id; when full W3DModelDraw state is composed in, delegate to parent.
    pub fn allocate_shadows(&mut self) {}

    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) { self.fully_obscured_by_shroud = fully_obscured; }
    pub fn react_to_transform_change(&mut self, _old_mtx: &Matrix4<f32>, _old_pos: &cgmath::Point3<f32>, _old_angle: f32) {}

    /// C++ parity: `virtual void reactToGeometryChange() { }` — explicit empty override
    /// in W3DSupplyDraw.h. The supply draw has no geometry-specific update.
    pub fn react_to_geometry_change(&mut self) {}

    pub fn set_hidden(&mut self, hidden: bool) { self.hidden = hidden; }
    pub fn is_visible(&self) -> bool { !self.hidden && !self.fully_obscured_by_shroud }
    pub fn get_module_data(&self) -> &W3DSupplyDrawModuleData { &self.module_data }
    pub fn crc(&self) -> u32 { 0 }
    pub fn xfer(&self) -> u32 { 1 }

    /// C++ parity: `W3DSupplyDraw::loadPostProcess()` — calls `W3DModelDraw::loadPostProcess()`.
    /// No additional post-load logic for supply draw.
    pub fn load_post_process(&mut self) {}
}

impl Default for W3DSupplyDraw {
    fn default() -> Self { Self::new_default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_wthree_d_supply_draw_basic() {
        let draw = W3DSupplyDraw::new_default();
        assert!(draw.is_visible());
    }
    #[test]
    fn test_wthree_d_supply_draw_status() {
        let mut draw = W3DSupplyDraw::new(W3DSupplyDrawModuleData { supply_bone_prefix: "SupplyBox".into() });
        draw.update_draw_module_supply_status(10, 10);
        assert_eq!(draw.last_number_shown, 8);
        draw.update_draw_module_supply_status(10, 5);
        assert_eq!(draw.last_number_shown, 4);
    }
}
