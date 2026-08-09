//! GUIEdit library surface for C++-matching `.wnd` save/load and editor chrome.

pub mod chrome;
pub mod save;

pub use chrome::{
    ChromeEditor, GadgetType, WidgetInfo, DEFAULT_GRID_RESOLUTION, EDIT_MENU_LABELS,
    FILE_MENU_LABELS, GADGET_SIZE, LAYOUT_MENU_LABELS, VIEW_MENU_LABELS,
};
pub use save::{
    parse_layout, save_layout, ComboBoxDataEdit, DrawDataSlot, GadgetData, ListBoxDataEdit,
    SaveError, SliderDataEdit, TabControlDataEdit, TextEntryDataEdit, WndLayout, WndWindow,
    MAX_DRAW_DATA, NUM_TAB_PANES,
};
