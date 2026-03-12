// FILE: mod.rs
// GameClient GUI Module
// Rust port of C&C Generals Zero Hour GUI subsystems

pub mod diplomacy;
pub mod credits;
pub mod credits_menu;

// Re-export main types
pub use diplomacy::{
    Color,
    GameWindow,
    WindowLayout,
    WindowManager,
    NameKeyGenerator,
    AnimateWindowManager,
    WindowMsgHandledType,
    WindowMsgData,
    NameKeyType,
    BriefingList,
    NAMEKEY_INVALID,
    game_make_color,
    get_briefing_text_list,
    update_diplomacy_briefing_text,
    show_diplomacy,
    reset_diplomacy,
    hide_diplomacy,
    toggle_diplomacy,
    diplomacy_input,
    diplomacy_system,
    populate_in_game_diplomacy_popup,
};

// Re-export credits types
pub use credits::{
    CreditsManager,
    CreditsLine,
    CreditStyle,
    FontDesc,
    GameFont,
    DisplayString,
    DisplayStringManager,
    FontLibrary,
    Display,
    GlobalLanguageData,
    GameText,
    INI,
    get_the_credits,
    set_the_credits,
};

// Re-export credits menu functions
pub use credits_menu::{
    credits_menu_init,
    credits_menu_shutdown,
    credits_menu_update,
    credits_menu_input,
    credits_menu_system,
    credits_menu_draw,
    is_credits_active,
    parse_credits_ini,
    Shell,
    AudioManager,
};
