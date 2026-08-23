//! C++ `Drawable::updateHiddenStatus` / `setDrawableHidden`.

use super::*;

impl BasicDrawable {
    /// C++ `Drawable::updateHiddenStatus` (`Drawable.cpp:4629-4641`).
    ///
    /// Hidden = `m_hidden || m_hiddenByStealth`. When hidden, deselect this
    /// drawable (C++ `TheInGameUI->deselectDrawable`) and hide every draw
    /// module so the mesh and its shadow drop out of the scene.
    pub fn update_hidden_status(&mut self) {
        let hidden = gamelogic::object::draw::leftover_hidden_status_deselects(
            self.hidden,
            self.hidden_by_stealth,
        );
        if hidden && self.selected {
            self.selected = false;
        }
        for module in &mut self.draw_modules {
            module.set_hidden(hidden);
        }
    }
}
