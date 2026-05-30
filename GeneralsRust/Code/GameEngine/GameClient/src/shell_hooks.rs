use gamelogic::helpers::TheScriptEngine;

pub const SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_SELECTED: i32 = 0;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_HIGHLIGHTED: i32 = 1;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_UNHIGHLIGHTED: i32 = 2;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_SELECTED: i32 = 3;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_HIGHLIGHTED: i32 = 4;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_UNHIGHLIGHTED: i32 = 5;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_SELECTED: i32 = 6;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_HIGHLIGHTED: i32 = 7;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_UNHIGHLIGHTED: i32 = 8;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_SELECTED: i32 = 9;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_HIGHLIGHTED: i32 = 10;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_UNHIGHLIGHTED: i32 = 11;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_SELECTED: i32 = 12;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_HIGHLIGHTED: i32 = 13;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_UNHIGHLIGHTED: i32 = 14;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_SELECTED: i32 = 15;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_HIGHLIGHTED: i32 = 16;
pub const SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_UNHIGHLIGHTED: i32 = 17;
pub const SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGIN: i32 = 18;
pub const SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGOUT: i32 = 19;
pub const SHELL_SCRIPT_HOOK_GENERALS_ONLINE_ENTERED_FROM_GAME: i32 = 20;
pub const SHELL_SCRIPT_HOOK_OPTIONS_OPENED: i32 = 21;
pub const SHELL_SCRIPT_HOOK_OPTIONS_CLOSED: i32 = 22;
pub const SHELL_SCRIPT_HOOK_SKIRMISH_OPENED: i32 = 23;
pub const SHELL_SCRIPT_HOOK_SKIRMISH_CLOSED: i32 = 24;
pub const SHELL_SCRIPT_HOOK_SKIRMISH_ENTERED_FROM_GAME: i32 = 25;
pub const SHELL_SCRIPT_HOOK_LAN_OPENED: i32 = 26;
pub const SHELL_SCRIPT_HOOK_LAN_CLOSED: i32 = 27;
pub const SHELL_SCRIPT_HOOK_LAN_ENTERED_FROM_GAME: i32 = 28;
pub const SHELL_SCRIPT_HOOK_TOTAL: i32 = 29;

pub const THE_SHELL_HOOK_NAMES: [&str; SHELL_SCRIPT_HOOK_TOTAL as usize] = [
    "ShellMainMenuCampaignPushed",
    "ShellMainMenuCampaignHighlighted",
    "ShellMainMenuCampaignUnhighlighted",
    "ShellMainMenuSkirmishPushed",
    "ShellMainMenuSkirmishHighlighted",
    "ShellMainMenuSkirmishUnhighlighted",
    "ShellMainMenuOptionsPushed",
    "ShellMainMenuOptionsHighlighted",
    "ShellMainMenuOptionsUnhighlighted",
    "ShellMainMenuOnlinePushed",
    "ShellMainMenuOnlineHighlighted",
    "ShellMainMenuOnlineUnhighlighted",
    "ShellMainMenuNetworkPushed",
    "ShellMainMenuNetworkHighlighted",
    "ShellMainMenuNetworkUnhighlighted",
    "ShellMainMenuExitPushed",
    "ShellMainMenuExitHighlighted",
    "ShellMainMenuExitUnhighlighted",
    "ShellGeneralsOnlineLogin",
    "ShellGeneralsOnlineLogout",
    "ShellGeneralsOnlineEnteredFromGame",
    "ShellOptionsOpened",
    "ShellOptionsClosed",
    "ShellSkirmishOpened",
    "ShellSkirmishClosed",
    "ShellSkirmishEnteredFromGame",
    "ShellLANOpened",
    "ShellLANClosed",
    "ShellLANEnteredFromGame",
];

pub fn signal_ui_interaction(interaction: i32) {
    if let Some(hook_name) = THE_SHELL_HOOK_NAMES.get(interaction as usize) {
        // PARITY_NOTE: script hooks in Rust are ultimately keyed by their canonical script names.
        // This compatibility table preserves the legacy integer-to-name mapping expected by shell code.
        TheScriptEngine::signal_ui_interact(hook_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_hook_names_match_cpp_script_table() {
        assert_eq!(THE_SHELL_HOOK_NAMES.len(), SHELL_SCRIPT_HOOK_TOTAL as usize);
        assert_eq!(
            THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_MAIN_MENU_CAMPAIGN_SELECTED as usize],
            "ShellMainMenuCampaignPushed"
        );
        assert_eq!(
            THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_SELECTED as usize],
            "ShellMainMenuSkirmishPushed"
        );
        assert_eq!(
            THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_HIGHLIGHTED as usize],
            "ShellMainMenuOptionsHighlighted"
        );
        assert_eq!(
            THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_UNHIGHLIGHTED as usize],
            "ShellMainMenuNetworkUnhighlighted"
        );
        assert_eq!(
            THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_GENERALS_ONLINE_ENTERED_FROM_GAME as usize],
            "ShellGeneralsOnlineEnteredFromGame"
        );
        assert_eq!(
            THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_GENERALS_ONLINE_LOGIN as usize],
            "ShellGeneralsOnlineLogin"
        );
        assert_eq!(
            THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_SKIRMISH_ENTERED_FROM_GAME as usize],
            "ShellSkirmishEnteredFromGame"
        );
        assert_eq!(
            THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_LAN_CLOSED as usize],
            "ShellLANClosed"
        );
    }
}
