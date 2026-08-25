//! C++ `populateSkirmishBattleHonors` / `InsertBattleHonor` medal rows.
//!
//! Sibling of `skirmish_game_options_menu.rs` so the options menu stays a
//! callback shell instead of another honors-table god file.

use crate::game_text::GameText;
use crate::gui::gadgets::{ListBox, ListBoxItemData};
use crate::gui::with_window_manager;
use crate::gui::{Color, GameWindow, WindowInstanceData, WindowWidget};
use crate::input::mouse::with_mouse;
use crate::map_util::get_map_cache_manager;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::skirmish_battle_honors::{
    BATTLE_HONOR_AIR_WING, BATTLE_HONOR_APOCALYPSE, BATTLE_HONOR_BATTLE_TANK, BATTLE_HONOR_BLITZ5,
    BATTLE_HONOR_BLITZ10, BATTLE_HONOR_CAMPAIGN_CHINA, BATTLE_HONOR_CAMPAIGN_GLA,
    BATTLE_HONOR_CAMPAIGN_USA, BATTLE_HONOR_CHALLENGE, BATTLE_HONOR_CHALLENGE_MODE,
    BATTLE_HONOR_DOMINATION, BATTLE_HONOR_DOMINATION_ONLINE, BATTLE_HONOR_ENDURANCE,
    BATTLE_HONOR_FAIR_PLAY, BATTLE_HONOR_GLOBAL_GENERAL, BATTLE_HONOR_LOYALTY_CHINA,
    BATTLE_HONOR_LOYALTY_GLA, BATTLE_HONOR_LOYALTY_USA, BATTLE_HONOR_NOT_GAINED,
    BATTLE_HONOR_OFFICERSCLUB, BATTLE_HONOR_STREAK, BATTLE_HONOR_STREAK_ONLINE,
    BATTLE_HONOR_ULTIMATE, MAX_BATTLE_HONOR_COLUMNS, MAX_BATTLE_HONOR_IMAGE_HEIGHT,
    MAX_BATTLE_HONOR_IMAGE_WIDTH, SkirmishBattleHonors,
};
use game_engine::common::system::get_unsigned_int_from_registry;
use game_network::SlotState;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const DIFFICULTY_EASY: i32 = 0;
const DIFFICULTY_NORMAL: i32 = 1;
const DIFFICULTY_HARD: i32 = 2;
const MAX_GLOBAL_GENERAL_TYPES: usize = 9;

const ENABLED_COLOR: Color = Color {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};
const DISABLED_COLOR: Color = Color {
    r: 80,
    g: 80,
    b: 80,
    a: 255,
};

thread_local! {
    static ROWS_TO_SKIP: Cell<i32> = const { Cell::new(0) };
}

fn reset_battle_honor_insertion() {
    ROWS_TO_SKIP.with(|rows| rows.set(0));
}

fn ensure_listbox_row(listbox: &mut ListBox, row: usize) {
    while listbox.items().len() <= row {
        listbox.add_item("");
    }
}

fn insert_battle_honor(
    listbox: &mut ListBox,
    image_name: &str,
    enabled: bool,
    item_data: u32,
    row: &mut usize,
    column: &mut usize,
    extra: i32,
) {
    let color = if enabled {
        ENABLED_COLOR
    } else {
        DISABLED_COLOR
    };
    let stored = if enabled {
        item_data
    } else {
        item_data | BATTLE_HONOR_NOT_GAINED
    };

    ensure_listbox_row(listbox, *row);
    let _ = listbox.set_item_column_data(
        *row,
        *column,
        ListBoxItemData::Image {
            name: image_name.to_string(),
            width: MAX_BATTLE_HONOR_IMAGE_WIDTH,
            height: MAX_BATTLE_HONOR_IMAGE_HEIGHT,
            text: None,
        },
    );
    let _ = listbox.set_item_column_color(*row, *column, Some(color));
    let _ = listbox.set_item_column_user_data(
        *row,
        *column,
        Some(ListBoxItemData::Integer(stored as i32)),
    );
    if *row > 0 {
        ensure_listbox_row(listbox, *row - 1);
        let _ = listbox.set_item_column_user_data(
            *row - 1,
            *column,
            Some(ListBoxItemData::Integer(extra)),
        );
    }

    *column += 1;
    let columns = listbox.columns().max(MAX_BATTLE_HONOR_COLUMNS) as usize;
    if *column >= columns {
        *column = 0;
        let skip = ROWS_TO_SKIP.with(Cell::get);
        *row = *row + 1 + skip.max(0) as usize;
        ROWS_TO_SKIP.with(|rows| rows.set((skip - 1).max(0)));
    }
}

fn campaign_image(
    side: &str,
    stats: &SkirmishBattleHonors,
    honors: u32,
    flag: u32,
) -> (&'static str, bool) {
    let (gold, silver, bronze) = match side {
        "China" => ("ChinaCampaign_G", "ChinaCampaign_S", "ChinaCampaign_B"),
        "GLA" => ("GLACampaign_G", "GLACampaign_S", "GLACampaign_B"),
        _ => ("USACampaign_G", "USACampaign_S", "USACampaign_B"),
    };
    let complete = |difficulty: i32| match side {
        "China" => stats.get_china_campaign_complete(difficulty),
        "GLA" => stats.get_gla_campaign_complete(difficulty),
        _ => stats.get_usa_campaign_complete(difficulty),
    };
    if complete(DIFFICULTY_HARD) {
        (gold, true)
    } else if complete(DIFFICULTY_NORMAL) {
        (silver, true)
    } else if complete(DIFFICULTY_EASY) {
        (bronze, true)
    } else {
        (bronze, honors & flag != 0)
    }
}

fn endurance_image(stats: &SkirmishBattleHonors) -> (&'static str, bool) {
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let mut missing_easy = false;
    let mut missing_medium = false;
    let mut missing_brutal = false;
    for (name, meta) in cache_guard.iter_maps() {
        if !meta.is_official || !meta.is_multiplayer {
            continue;
        }
        let easy = stats.get_endurance_medal(&name, SlotState::EasyAI as i32) != 0;
        let med = stats.get_endurance_medal(&name, SlotState::MedAI as i32) != 0;
        let hard = stats.get_endurance_medal(&name, SlotState::BrutalAI as i32) != 0;
        if !easy && !med && !hard {
            missing_easy = true;
        }
        if !med && !hard {
            missing_medium = true;
        }
        if !hard {
            missing_brutal = true;
        }
    }
    if !missing_brutal {
        ("Endurance_G", true)
    } else if !missing_medium {
        ("Endurance_S", true)
    } else if !missing_easy {
        ("Endurance_B", true)
    } else {
        ("Endurance_B", false)
    }
}

fn ultimate_perfect(stats: &SkirmishBattleHonors) -> bool {
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    for (name, meta) in cache_guard.iter_maps() {
        if !meta.is_official || !meta.is_multiplayer {
            continue;
        }
        let total_opponent_slots = meta.num_players.saturating_sub(1);
        let beaten = stats.get_endurance_medal(&name, SlotState::BrutalAI as i32);
        if beaten < total_opponent_slots {
            return false;
        }
    }
    true
}

fn set_static_text(name: &str, value: i32) {
    let id = NameKeyGenerator::name_to_key(name) as i32;
    if let Some(window) = with_window_manager(|manager| manager.get_window_by_id(id)) {
        let _ = window.borrow_mut().set_text(&format!("{}", value));
    }
}
pub fn battle_honor_tooltip(window: &GameWindow, _inst: &WindowInstanceData, mouse: u32) {
    let x = (mouse & 0xFFFF) as i16 as i32;
    let y = (mouse >> 16) as i16 as i32;
    let Some(WindowWidget::ListBox(listbox)) = window.widget() else {
        set_cursor_tooltip("TOOLTIP:BattleHonors");
        return;
    };
    let (row, col) = listbox.entry_from_xy(x, y);
    if row < 0 || col < 0 {
        set_cursor_tooltip("TOOLTIP:BattleHonors");
        return;
    }
    let honor = match listbox.get_item_data_at(row, col) {
        Some(ListBoxItemData::Integer(value)) => *value as u32,
        _ => 0,
    };
    if honor == 0 {
        set_cursor_tooltip("TOOLTIP:BattleHonors");
        return;
    }
    let extra = match listbox.get_item_data_at(row - 1, col) {
        Some(ListBoxItemData::Integer(value)) => *value,
        _ => 0,
    };
    let key = honor_tooltip_key(honor, extra);
    set_cursor_tooltip(key);
}

fn set_cursor_tooltip(key: &str) {
    let text = GameText::fetch(key);
    with_mouse(|mouse| mouse.set_cursor_tooltip(text, Some(-1), None, None));
}

fn honor_tooltip_key(honor: u32, extra: i32) -> &'static str {
    let gained = honor & BATTLE_HONOR_NOT_GAINED == 0;
    if !gained {
        return if honor & BATTLE_HONOR_LOYALTY_USA != 0 {
            "TOOLTIP:BattleHonorLoyaltyUSADisabled"
        } else if honor & BATTLE_HONOR_LOYALTY_CHINA != 0 {
            "TOOLTIP:BattleHonorLoyaltyChinaDisabled"
        } else if honor & BATTLE_HONOR_LOYALTY_GLA != 0 {
            "TOOLTIP:BattleHonorLoyaltyGLADisabled"
        } else if honor & BATTLE_HONOR_BATTLE_TANK != 0 {
            "TOOLTIP:BattleHonorBattleTankDisabled"
        } else if honor & BATTLE_HONOR_AIR_WING != 0 {
            "TOOLTIP:BattleHonorAirWingDisabled"
        } else if honor & BATTLE_HONOR_ENDURANCE != 0 {
            "TOOLTIP:BattleHonorEnduranceDisabled"
        } else if honor & BATTLE_HONOR_CAMPAIGN_USA != 0 {
            "TOOLTIP:BattleHonorCampaignUSADisabled"
        } else if honor & BATTLE_HONOR_CAMPAIGN_CHINA != 0 {
            "TOOLTIP:BattleHonorCampaignChinaDisabled"
        } else if honor & BATTLE_HONOR_CAMPAIGN_GLA != 0 {
            "TOOLTIP:BattleHonorCampaignGLADisabled"
        } else if honor & BATTLE_HONOR_BLITZ10 != 0 {
            "TOOLTIP:BattleHonorBlitzDisabled"
        } else if honor & BATTLE_HONOR_FAIR_PLAY != 0 {
            "TOOLTIP:BattleHonorFairPlayDisabled"
        } else if honor & BATTLE_HONOR_APOCALYPSE != 0 {
            "TOOLTIP:BattleHonorApocalypseDisabled"
        } else if honor & BATTLE_HONOR_CHALLENGE_MODE != 0 {
            "TOOLTIP:BattleHonorCampaignChallengeDisabled"
        } else if honor & BATTLE_HONOR_ULTIMATE != 0 {
            "TOOLTIP:BattleHonorUltimateDisabled"
        } else if honor & BATTLE_HONOR_GLOBAL_GENERAL != 0 {
            "TOOLTIP:BattleHonorGlobalGeneralDisabled"
        } else if honor & BATTLE_HONOR_CHALLENGE != 0 {
            "TOOLTIP:BattleHonorChallengeDisabled"
        } else if honor & BATTLE_HONOR_STREAK != 0 {
            "TOOLTIP:BattleHonorStreakDisabled"
        } else if honor & BATTLE_HONOR_STREAK_ONLINE != 0 {
            "TOOLTIP:BattleHonorStreakOnlineDisabled"
        } else if honor & BATTLE_HONOR_DOMINATION != 0 {
            "TOOLTIP:BattleHonorDominationDisabled"
        } else if honor & BATTLE_HONOR_DOMINATION_ONLINE != 0 {
            "TOOLTIP:BattleHonorDominationOnlineDisabled"
        } else {
            "TOOLTIP:BattleHonors"
        };
    }

    if honor & BATTLE_HONOR_LOYALTY_USA != 0 {
        "TOOLTIP:BattleHonorLoyaltyUSA"
    } else if honor & BATTLE_HONOR_LOYALTY_CHINA != 0 {
        "TOOLTIP:BattleHonorLoyaltyChina"
    } else if honor & BATTLE_HONOR_LOYALTY_GLA != 0 {
        "TOOLTIP:BattleHonorLoyaltyGLA"
    } else if honor & BATTLE_HONOR_BATTLE_TANK != 0 {
        "TOOLTIP:BattleHonorBattleTank"
    } else if honor & BATTLE_HONOR_AIR_WING != 0 {
        "TOOLTIP:BattleHonorAirWing"
    } else if honor & BATTLE_HONOR_ENDURANCE != 0 {
        "TOOLTIP:BattleHonorEndurance"
    } else if honor & BATTLE_HONOR_CAMPAIGN_USA != 0 {
        "TOOLTIP:BattleHonorCampaignUSA"
    } else if honor & BATTLE_HONOR_CAMPAIGN_CHINA != 0 {
        "TOOLTIP:BattleHonorCampaignChina"
    } else if honor & BATTLE_HONOR_CAMPAIGN_GLA != 0 {
        "TOOLTIP:BattleHonorCampaignGLA"
    } else if honor & BATTLE_HONOR_BLITZ5 != 0 {
        "TOOLTIP:BattleHonorBlitz5"
    } else if honor & BATTLE_HONOR_BLITZ10 != 0 {
        "TOOLTIP:BattleHonorBlitz10"
    } else if honor & BATTLE_HONOR_FAIR_PLAY != 0 {
        "TOOLTIP:BattleHonorFairPlay"
    } else if honor & BATTLE_HONOR_APOCALYPSE != 0 {
        "TOOLTIP:BattleHonorApocalypse"
    } else if honor & BATTLE_HONOR_OFFICERSCLUB != 0 {
        "TOOLTIP:BattleHonorOfficersClub"
    } else if honor & BATTLE_HONOR_CHALLENGE_MODE != 0 {
        "TOOLTIP:BattleHonorCampaignChallenge"
    } else if honor & BATTLE_HONOR_ULTIMATE != 0 {
        "TOOLTIP:BattleHonorUltimate"
    } else if honor & BATTLE_HONOR_GLOBAL_GENERAL != 0 {
        "TOOLTIP:BattleHonorGlobalGeneral"
    } else if honor & BATTLE_HONOR_CHALLENGE != 0 {
        "TOOLTIP:BattleHonorChallenge"
    } else if honor & BATTLE_HONOR_STREAK != 0 {
        if extra >= 1000 {
            "TOOLTIP:BattleHonorStreak1000"
        } else if extra >= 500 {
            "TOOLTIP:BattleHonorStreak500"
        } else if extra >= 100 {
            "TOOLTIP:BattleHonorStreak100"
        } else if extra >= 25 {
            "TOOLTIP:BattleHonorStreak25"
        } else if extra >= 10 {
            "TOOLTIP:BattleHonorStreak10"
        } else if extra >= 3 {
            "TOOLTIP:BattleHonorStreak3"
        } else {
            "TOOLTIP:BattleHonorStreakDisabled"
        }
    } else if honor & BATTLE_HONOR_DOMINATION != 0 {
        if extra >= 10000 {
            "TOOLTIP:BattleHonorDomination10000"
        } else if extra >= 1000 {
            "TOOLTIP:BattleHonorDomination1000"
        } else if extra >= 500 {
            "TOOLTIP:BattleHonorDomination500"
        } else if extra >= 100 {
            "TOOLTIP:BattleHonorDomination100"
        } else {
            "TOOLTIP:BattleHonorDominationDisabled"
        }
    } else {
        "TOOLTIP:BattleHonors"
    }
}

/// C++ `populateSkirmishBattleHonors`.
pub fn populate(listbox_info: Option<&Rc<RefCell<GameWindow>>>) {
    let stats = SkirmishBattleHonors::new();
    let honors = stats.get_honors();

    set_static_text(
        "SkirmishGameOptionsMenu.wnd:StaticTextStreakValue",
        stats.get_win_streak(),
    );
    set_static_text(
        "SkirmishGameOptionsMenu.wnd:StaticTextBestStreakValue",
        stats.get_best_win_streak(),
    );
    set_static_text(
        "SkirmishGameOptionsMenu.wnd:StaticTextWinsValue",
        stats.get_wins(),
    );
    set_static_text(
        "SkirmishGameOptionsMenu.wnd:StaticTextLossesValue",
        stats.get_losses(),
    );

    let Some(listbox_window) = listbox_info else {
        return;
    };
    listbox_window
        .borrow_mut()
        .set_tooltip_callback(battle_honor_tooltip);
    let mut guard = listbox_window.borrow_mut();
    let Some(listbox) = guard.list_box_mut() else {
        return;
    };
    listbox.clear();
    if listbox.columns() < MAX_BATTLE_HONOR_COLUMNS {
        listbox.set_columns(MAX_BATTLE_HONOR_COLUMNS);
    }

    reset_battle_honor_insertion();
    ensure_listbox_row(listbox, 0);
    let mut row = 1usize;
    let mut column = 0usize;

    let (china_image, china_on) =
        campaign_image("China", &stats, honors, BATTLE_HONOR_CAMPAIGN_CHINA);
    insert_battle_honor(
        listbox,
        china_image,
        china_on,
        BATTLE_HONOR_CAMPAIGN_CHINA,
        &mut row,
        &mut column,
        0,
    );
    let (gla_image, gla_on) = campaign_image("GLA", &stats, honors, BATTLE_HONOR_CAMPAIGN_GLA);
    insert_battle_honor(
        listbox,
        gla_image,
        gla_on,
        BATTLE_HONOR_CAMPAIGN_GLA,
        &mut row,
        &mut column,
        0,
    );
    let (usa_image, usa_on) = campaign_image("USA", &stats, honors, BATTLE_HONOR_CAMPAIGN_USA);
    insert_battle_honor(
        listbox,
        usa_image,
        usa_on,
        BATTLE_HONOR_CAMPAIGN_USA,
        &mut row,
        &mut column,
        0,
    );

    let mut completed_hard = false;
    let mut completed_normal = false;
    let mut completed_easy = false;
    for i in 0..MAX_GLOBAL_GENERAL_TYPES {
        if stats.get_challenge_campaign_complete(i, DIFFICULTY_HARD) {
            completed_hard = true;
        }
        if stats.get_challenge_campaign_complete(i, DIFFICULTY_NORMAL) {
            completed_normal = true;
        }
        if stats.get_challenge_campaign_complete(i, DIFFICULTY_EASY) {
            completed_easy = true;
        }
    }
    let (challenge_image, challenge_on) = if completed_hard {
        ("Challenge_Gold", true)
    } else if completed_normal {
        ("Challenge_Silver", true)
    } else if completed_easy {
        ("Challenge_Bronz", true)
    } else {
        ("Challenge_Bronz", false)
    };
    insert_battle_honor(
        listbox,
        challenge_image,
        challenge_on,
        BATTLE_HONOR_CHALLENGE_MODE,
        &mut row,
        &mut column,
        0,
    );

    insert_battle_honor(
        listbox,
        "HonorAirWing",
        honors & BATTLE_HONOR_AIR_WING != 0,
        BATTLE_HONOR_AIR_WING,
        &mut row,
        &mut column,
        0,
    );
    insert_battle_honor(
        listbox,
        "HonorBattleTank",
        honors & BATTLE_HONOR_BATTLE_TANK != 0,
        BATTLE_HONOR_BATTLE_TANK,
        &mut row,
        &mut column,
        0,
    );

    ensure_listbox_row(listbox, 2);
    row = 3;
    column = 0;

    let (endurance_image_name, endurance_on) = endurance_image(&stats);
    insert_battle_honor(
        listbox,
        endurance_image_name,
        endurance_on,
        BATTLE_HONOR_ENDURANCE,
        &mut row,
        &mut column,
        0,
    );
    insert_battle_honor(
        listbox,
        "Apocalypse",
        honors & BATTLE_HONOR_APOCALYPSE != 0,
        BATTLE_HONOR_APOCALYPSE,
        &mut row,
        &mut column,
        0,
    );

    if honors & BATTLE_HONOR_BLITZ5 != 0 {
        insert_battle_honor(
            listbox,
            "HonorBlitz5",
            true,
            BATTLE_HONOR_BLITZ5,
            &mut row,
            &mut column,
            0,
        );
    } else if honors & BATTLE_HONOR_BLITZ10 != 0 {
        insert_battle_honor(
            listbox,
            "HonorBlitz10",
            true,
            BATTLE_HONOR_BLITZ10,
            &mut row,
            &mut column,
            0,
        );
    } else {
        insert_battle_honor(
            listbox,
            "HonorBlitz10",
            false,
            BATTLE_HONOR_BLITZ10,
            &mut row,
            &mut column,
            0,
        );
    }

    let streak = stats.get_best_win_streak();
    let streak_image = if streak >= 1000 {
        "HonorStreak_1000"
    } else if streak >= 500 {
        "HonorStreak_500"
    } else if streak >= 100 {
        "HonorStreak_100"
    } else if streak >= 25 {
        "HonorStreak_G"
    } else if streak >= 10 {
        "HonorStreak_S"
    } else {
        "HonorStreak_B"
    };
    insert_battle_honor(
        listbox,
        streak_image,
        streak >= 3,
        BATTLE_HONOR_STREAK,
        &mut row,
        &mut column,
        streak,
    );

    let total_wins = stats.get_wins();
    let domination_image = if total_wins >= 10000 {
        "Domination_10000"
    } else if total_wins >= 1000 {
        "Domination_1000"
    } else if total_wins >= 500 {
        "Domination_500"
    } else {
        "Domination_100"
    };
    insert_battle_honor(
        listbox,
        domination_image,
        total_wins >= 100,
        BATTLE_HONOR_DOMINATION,
        &mut row,
        &mut column,
        total_wins,
    );

    insert_battle_honor(
        listbox,
        "Ultimate",
        ultimate_perfect(&stats),
        BATTLE_HONOR_ULTIMATE,
        &mut row,
        &mut column,
        0,
    );

    if get_unsigned_int_from_registry("", "Preorder").unwrap_or(0) != 0 {
        insert_battle_honor(
            listbox,
            "OfficersClub",
            true,
            BATTLE_HONOR_OFFICERSCLUB,
            &mut row,
            &mut column,
            0,
        );
    }
}
