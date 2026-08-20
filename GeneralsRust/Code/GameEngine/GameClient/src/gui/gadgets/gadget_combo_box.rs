//! C++ parity wrapper for GadgetComboBox.cpp
//!
//! Body LeftUp is `GadgetComboBoxInput` `GWM_LEFT_UP` (`GadgetComboBox.cpp:115-181`):
//! play `GUIClick`, `winSetLoneWindow`, then show/hide the list. Outside mouse-up
//! closes via the lone-window `GGM_CLOSE` path (`GameWindowManager.cpp:1117-1134`).

pub use super::combobox::{ComboBox, ComboBoxCallback, ComboBoxItem, ComboBoxRenderCommand};
