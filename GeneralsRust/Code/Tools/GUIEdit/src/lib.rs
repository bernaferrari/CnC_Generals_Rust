//! GUIEdit library surface for C++-matching `.wnd` save/load.

pub mod save;

pub use save::{
    parse_layout, save_layout, ComboBoxDataEdit, DrawDataSlot, GadgetData, ListBoxDataEdit,
    SaveError, SliderDataEdit, TabControlDataEdit, TextEntryDataEdit, WndLayout, WndWindow,
    MAX_DRAW_DATA, NUM_TAB_PANES,
};
