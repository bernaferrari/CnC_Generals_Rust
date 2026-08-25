//! GUIEdit library surface for C++-matching `.wnd` save/load and editor chrome.

pub mod chrome;
pub mod save;

pub use chrome::{
    ChromeEditor, DEFAULT_GRID_RESOLUTION, EDIT_MENU_LABELS, FILE_MENU_LABELS, GADGET_SIZE,
    GadgetType, LAYOUT_MENU_LABELS, VIEW_MENU_LABELS, WidgetInfo,
};
pub use save::{
    ComboBoxDataEdit, DrawDataSlot, GadgetData, ListBoxDataEdit, MAX_DRAW_DATA, NUM_TAB_PANES,
    SaveError, SliderDataEdit, TabControlDataEdit, TextEntryDataEdit, WndLayout, WndWindow,
    parse_layout, save_layout,
};
