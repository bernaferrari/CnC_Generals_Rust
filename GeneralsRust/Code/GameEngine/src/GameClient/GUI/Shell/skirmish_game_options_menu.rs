// FILE: skirmish_game_options_menu.rs
// Author: Chris Brue, August 2002 (original C++), Rust port
// Description: Skirmish Game Options Menu
//
// Ported from: GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/SkirmishGameOptionsMenu.cpp

use std::collections::HashMap;

// Constants matching C++ implementation
pub const MAX_SLOTS: usize = 8;
const GREATER_NO_FPS_LIMIT: i32 = 60;
const DEFAULT_FPS_LIMIT: i32 = 30;
const MIN_FPS: i32 = 15;
const MAX_FPS: i32 = 1000;

// Player slot states - matches C++ SlotState enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Open = 0,
    Closed = 1,
    EasyAI = 2,
    MediumAI = 3,
    HardAI = 4,
    Player = 5,
}

impl SlotState {
    pub fn is_ai(&self) -> bool {
        matches!(self, SlotState::EasyAI | SlotState::MediumAI | SlotState::HardAI)
    }

    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => SlotState::Open,
            1 => SlotState::Closed,
            2 => SlotState::EasyAI,
            3 => SlotState::MediumAI,
            4 => SlotState::HardAI,
            5 => SlotState::Player,
            _ => SlotState::Open,
        }
    }
}

// Player template constants
pub const PLAYERTEMPLATE_RANDOM: i32 = -1;
pub const PLAYERTEMPLATE_MIN: i32 = -1;

// Game slot structure
#[derive(Debug, Clone)]
pub struct GameSlot {
    name: String,
    state: SlotState,
    color: i32,
    player_template: i32,
    team_number: i32,
    start_pos: i32,
    apparent_start_pos: i32,
    ip: u32,
    is_preorder: bool,
}

impl GameSlot {
    pub fn new() -> Self {
        GameSlot {
            name: String::new(),
            state: SlotState::Open,
            color: -1,
            player_template: PLAYERTEMPLATE_RANDOM,
            team_number: -1,
            start_pos: -1,
            apparent_start_pos: -1,
            ip: 0,
            is_preorder: false,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn get_state(&self) -> SlotState {
        self.state
    }

    pub fn set_state(&mut self, state: SlotState, title: String) {
        self.state = state;
        if state == SlotState::Player {
            self.name = title;
        }
    }

    pub fn get_color(&self) -> i32 {
        self.color
    }

    pub fn set_color(&mut self, color: i32) {
        self.color = color;
    }

    pub fn get_player_template(&self) -> i32 {
        self.player_template
    }

    pub fn set_player_template(&mut self, template: i32) {
        self.player_template = template;
    }

    pub fn get_team_number(&self) -> i32 {
        self.team_number
    }

    pub fn set_team_number(&mut self, team: i32) {
        self.team_number = team;
    }

    pub fn get_start_pos(&self) -> i32 {
        self.start_pos
    }

    pub fn set_start_pos(&mut self, pos: i32) {
        self.start_pos = pos;
    }

    pub fn get_apparent_start_pos(&self) -> i32 {
        self.apparent_start_pos
    }

    pub fn get_ip(&self) -> u32 {
        self.ip
    }

    pub fn set_ip(&mut self, ip: u32) {
        self.ip = ip;
    }

    pub fn is_ai(&self) -> bool {
        self.state.is_ai()
    }

    pub fn has_map(&self) -> bool {
        true // Simplified for now
    }

    pub fn mark_as_preorder(&mut self) {
        self.is_preorder = true;
    }
}

impl Default for GameSlot {
    fn default() -> Self {
        Self::new()
    }
}

// Money structure - matches C++ Money class
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money {
    amount: u32,
}

impl Money {
    pub fn new() -> Self {
        Money { amount: 0 }
    }

    pub fn deposit(&mut self, amount: u32, _verify: bool) {
        self.amount = amount;
    }

    pub fn count_money(&self) -> u32 {
        self.amount
    }
}

impl Default for Money {
    fn default() -> Self {
        Self::new()
    }
}

// Map metadata structure
#[derive(Debug, Clone)]
pub struct MapMetaData {
    pub display_name: String,
    pub num_players: usize,
    pub is_multiplayer: bool,
    pub is_official: bool,
    pub crc: u32,
    pub filesize: u64,
    pub extent_lo_x: f32,
    pub extent_lo_y: f32,
    pub extent_hi_x: f32,
    pub extent_hi_y: f32,
    pub waypoints: HashMap<String, (f32, f32, f32)>,
    pub supply_positions: Vec<(f32, f32, f32)>,
    pub tech_positions: Vec<(f32, f32, f32)>,
}

impl MapMetaData {
    pub fn new() -> Self {
        MapMetaData {
            display_name: String::new(),
            num_players: 0,
            is_multiplayer: false,
            is_official: false,
            crc: 0,
            filesize: 0,
            extent_lo_x: 0.0,
            extent_lo_y: 0.0,
            extent_hi_x: 0.0,
            extent_hi_y: 0.0,
            waypoints: HashMap::new(),
            supply_positions: Vec::new(),
            tech_positions: Vec::new(),
        }
    }
}

impl Default for MapMetaData {
    fn default() -> Self {
        Self::new()
    }
}

// Game Info structure - main game state
#[derive(Debug, Clone)]
pub struct SkirmishGameInfo {
    slots: Vec<GameSlot>,
    map_name: String,
    map_crc: u32,
    map_size: u64,
    seed: u32,
    starting_cash: Money,
    superweapon_restriction: i32,
    local_slot_num: i32,
    local_ip: u32,
    in_game: bool,
}

impl SkirmishGameInfo {
    pub fn new() -> Self {
        let mut slots = Vec::with_capacity(MAX_SLOTS);
        for _ in 0..MAX_SLOTS {
            slots.push(GameSlot::new());
        }

        SkirmishGameInfo {
            slots,
            map_name: String::new(),
            map_crc: 0,
            map_size: 0,
            seed: 0,
            starting_cash: Money::new(),
            superweapon_restriction: 0,
            local_slot_num: 0,
            local_ip: 0,
            in_game: false,
        }
    }

    pub fn init(&mut self) {
        // Initialize game info
    }

    pub fn reset(&mut self) {
        for slot in &mut self.slots {
            *slot = GameSlot::new();
        }
        self.map_name.clear();
        self.map_crc = 0;
        self.map_size = 0;
    }

    pub fn clear_slot_list(&mut self) {
        for slot in &mut self.slots {
            *slot = GameSlot::new();
        }
    }

    pub fn get_slot(&mut self, index: usize) -> Option<&mut GameSlot> {
        if index < MAX_SLOTS {
            Some(&mut self.slots[index])
        } else {
            None
        }
    }

    pub fn get_const_slot(&self, index: usize) -> Option<&GameSlot> {
        if index < MAX_SLOTS {
            Some(&self.slots[index])
        } else {
            None
        }
    }

    pub fn set_slot(&mut self, index: usize, slot: GameSlot) {
        if index < MAX_SLOTS {
            self.slots[index] = slot;
        }
    }

    pub fn get_map(&self) -> &str {
        &self.map_name
    }

    pub fn set_map(&mut self, map: String) {
        self.map_name = map;
    }

    pub fn get_map_crc(&self) -> u32 {
        self.map_crc
    }

    pub fn set_map_crc(&mut self, crc: u32) {
        self.map_crc = crc;
    }

    pub fn set_map_size(&mut self, size: u64) {
        self.map_size = size;
    }

    pub fn get_seed(&self) -> u32 {
        self.seed
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
    }

    pub fn get_starting_cash(&self) -> Money {
        self.starting_cash
    }

    pub fn set_starting_cash(&mut self, cash: Money) {
        self.starting_cash = cash;
    }

    pub fn get_superweapon_restriction(&self) -> i32 {
        self.superweapon_restriction
    }

    pub fn set_superweapon_restriction(&mut self, restriction: i32) {
        self.superweapon_restriction = restriction;
    }

    pub fn get_local_slot_num(&self) -> i32 {
        self.local_slot_num
    }

    pub fn set_local_ip(&mut self, ip: u32) {
        self.local_ip = ip;
    }

    pub fn enter_game(&mut self) {
        self.in_game = true;
    }

    pub fn end_game(&mut self) {
        self.in_game = false;
    }

    pub fn is_in_game(&self) -> bool {
        self.in_game
    }

    pub fn am_i_host(&self) -> bool {
        true // In skirmish mode, player is always host
    }

    pub fn get_num_players(&self) -> usize {
        self.slots.iter()
            .filter(|s| s.state != SlotState::Open && s.state != SlotState::Closed)
            .count()
    }

    pub fn start_game(&mut self, _frame: i32) {
        // Start game logic
    }

    pub fn mark_player_as_preorder(&mut self, index: usize) {
        if let Some(slot) = self.get_slot(index) {
            slot.mark_as_preorder();
        }
    }

    pub fn is_game_in_progress(&self) -> bool {
        self.in_game
    }
}

impl Default for SkirmishGameInfo {
    fn default() -> Self {
        Self::new()
    }
}

// Skirmish Game Options Menu state
pub struct SkirmishGameOptionsMenu {
    game_info: SkirmishGameInfo,
    button_pushed: bool,
    sandbox_ok: bool,
    just_entered: bool,
    initial_gadget_delay: i32,
    still_needs_to_set_options: bool,
    do_update_slot_list: bool,
    game_speed_slider_pos: i32,
}

impl SkirmishGameOptionsMenu {
    pub fn new() -> Self {
        SkirmishGameOptionsMenu {
            game_info: SkirmishGameInfo::new(),
            button_pushed: false,
            sandbox_ok: false,
            just_entered: false,
            initial_gadget_delay: 2,
            still_needs_to_set_options: false,
            do_update_slot_list: true,
            game_speed_slider_pos: DEFAULT_FPS_LIMIT,
        }
    }

    // Initialize menu
    pub fn init(&mut self) {
        self.game_info.init();
        self.game_info.clear_slot_list();
        self.game_info.reset();

        self.just_entered = true;
        self.initial_gadget_delay = 2;
        self.button_pushed = false;
        self.sandbox_ok = false;
        self.do_update_slot_list = true;
    }

    // Shutdown menu
    pub fn shutdown(&mut self, pop_immediate: bool) {
        if !pop_immediate {
            // Animate shutdown
        }
    }

    // Update menu state
    pub fn update(&mut self) {
        if self.just_entered {
            if self.initial_gadget_delay == 1 {
                self.still_needs_to_set_options = true;
                self.initial_gadget_delay = 2;
                self.just_entered = false;
            } else {
                self.initial_gadget_delay -= 1;
            }
        }
    }

    // Handle player selection change
    pub fn handle_player_selection(&mut self, index: usize, player_type: SlotState, title: String) {
        if index == 0 || index >= MAX_SLOTS {
            return;
        }

        if let Some(slot) = self.game_info.get_slot(index) {
            slot.set_state(player_type, title);
        }
        self.update_slot_list();
    }

    // Handle color selection change
    pub fn handle_color_selection(&mut self, index: usize, color: i32, num_colors: usize) {
        // Check if color is already set
        if let Some(slot) = self.game_info.get_const_slot(index) {
            if color == slot.get_color() {
                return;
            }
        }

        if color >= -1 && (color as usize) < num_colors {
            let mut color_available = true;
            if color != -1 {
                for i in 0..MAX_SLOTS {
                    if let Some(check_slot) = self.game_info.get_const_slot(i) {
                        if color == check_slot.get_color() && i != index {
                            color_available = false;
                            break;
                        }
                    }
                }
            }
            if color_available {
                if let Some(slot) = self.game_info.get_slot(index) {
                    slot.set_color(color);
                }
            }
        }
        self.update_slot_list();
    }

    // Handle player template (faction) selection change
    pub fn handle_player_template_selection(&mut self, index: usize, player_template: i32) {
        if let Some(slot) = self.game_info.get_slot(index) {
            if player_template == slot.get_player_template() {
                return;
            }
            slot.set_player_template(player_template);
        }
        self.update_slot_list();
    }

    // Handle team selection change
    pub fn handle_team_selection(&mut self, index: usize, team: i32) {
        if let Some(slot) = self.game_info.get_slot(index) {
            if team == slot.get_team_number() {
                return;
            }
            slot.set_team_number(team);
        }
        self.update_slot_list();
    }

    // Handle start position selection
    pub fn handle_start_position_selection(&mut self, index: usize, position: i32, _num_players: usize) {
        if let Some(slot) = self.game_info.get_slot(index) {
            if position == slot.get_start_pos() {
                return;
            }

            if position < 0 {
                slot.set_start_pos(position);
                return;
            }

            let mut is_available = true;
            for i in 0..MAX_SLOTS {
                if i != index {
                    if let Some(other_slot) = self.game_info.get_const_slot(i) {
                        if other_slot.get_start_pos() == position {
                            is_available = false;
                            break;
                        }
                    }
                }
            }

            if is_available {
                slot.set_start_pos(position);
            }
        }
    }

    // Handle starting cash selection
    pub fn handle_starting_cash_selection(&mut self, amount: u32) {
        let mut cash = Money::new();
        cash.deposit(amount, false);
        self.game_info.set_starting_cash(cash);
    }

    // Handle limit superweapons checkbox
    pub fn handle_limit_superweapons_click(&mut self, is_checked: bool) {
        self.game_info.set_superweapon_restriction(if is_checked { 1 } else { 0 });
    }

    // Handle game speed slider
    pub fn set_fps_slider(&mut self, slider_pos: i32) -> String {
        self.game_speed_slider_pos = slider_pos;

        if slider_pos > GREATER_NO_FPS_LIMIT {
            "--".to_string()
        } else {
            format!("{:2}", slider_pos)
        }
    }

    // Get next selectable player slot (for AI or local player)
    fn get_next_selectable_player(&self, start: usize) -> Option<usize> {
        if !self.game_info.am_i_host() {
            return None;
        }

        for j in start..MAX_SLOTS {
            if let Some(slot) = self.game_info.get_const_slot(j) {
                if slot.get_start_pos() == -1 &&
                   (j == self.game_info.get_local_slot_num() as usize || slot.is_ai()) {
                    return Some(j);
                }
            }
        }
        None
    }

    // Handle start position button click
    pub fn handle_map_start_position_click(&mut self, position: usize, num_players: usize) {
        let mut player_idx_in_pos: Option<usize> = None;

        for j in 0..MAX_SLOTS {
            if let Some(slot) = self.game_info.get_const_slot(j) {
                if slot.get_start_pos() == position as i32 {
                    player_idx_in_pos = Some(j);
                    break;
                }
            }
        }

        if let Some(idx) = player_idx_in_pos {
            if let Some(slot) = self.game_info.get_const_slot(idx) {
                if idx == self.game_info.get_local_slot_num() as usize ||
                   (self.game_info.am_i_host() && slot.is_ai()) {
                    // It's controllable - try to change it
                    let next_player = self.get_next_selectable_player(idx + 1);
                    self.handle_start_position_selection(idx, -1, num_players);
                    if let Some(next_idx) = next_player {
                        self.handle_start_position_selection(next_idx, position as i32, num_players);
                    }
                }
            }
        } else {
            // Nobody in the slot - put us in
            let next_player = self.get_next_selectable_player(0)
                .unwrap_or(self.game_info.get_local_slot_num() as usize);
            self.handle_start_position_selection(next_player, position as i32, num_players);
        }

        self.update_slot_list();
        self.sandbox_ok = false;
    }

    // Handle right-click on start position (to remove player from position)
    pub fn handle_map_start_position_right_click(&mut self, position: usize) {
        for j in 0..MAX_SLOTS {
            if let Some(slot) = self.game_info.get_const_slot(j) {
                if slot.get_start_pos() == position as i32 {
                    if j == self.game_info.get_local_slot_num() as usize ||
                       (self.game_info.am_i_host() && slot.is_ai()) {
                        self.handle_start_position_selection(j, -1, 0);
                    }
                    self.update_slot_list();
                    self.sandbox_ok = false;
                    break;
                }
            }
        }
    }

    // Update slot list UI
    pub fn update_slot_list(&mut self) {
        if self.do_update_slot_list {
            self.do_update_slot_list = false;
            // Update UI elements here
            self.do_update_slot_list = true;
        }
    }

    // Check if game can start
    pub fn can_start_game(&self, map_metadata: Option<&MapMetaData>) -> Result<(), String> {
        let player_count = self.game_info.get_num_players();

        if let Some(mmd) = map_metadata {
            if player_count > mmd.num_players {
                return Err(format!("Too many players for this map (max: {})", mmd.num_players));
            }
        } else {
            return Err("Cannot find map".to_string());
        }

        Ok(())
    }

    // Start the game
    pub fn start_game(&mut self) -> Result<i32, String> {
        let mut max_fps = self.game_speed_slider_pos;

        if max_fps > GREATER_NO_FPS_LIMIT {
            max_fps = MAX_FPS;
        }
        if max_fps < MIN_FPS {
            max_fps = MIN_FPS;
        }

        self.game_info.start_game(0);

        Ok(max_fps)
    }

    // Reset to defaults
    pub fn reset(&mut self) {
        self.game_info.reset();
        self.sandbox_ok = false;
        self.update_slot_list();
    }

    // Exit menu
    pub fn exit(&mut self) {
        self.button_pushed = true;
    }

    // Get game info reference
    pub fn get_game_info(&self) -> &SkirmishGameInfo {
        &self.game_info
    }

    // Get mutable game info reference
    pub fn get_game_info_mut(&mut self) -> &mut SkirmishGameInfo {
        &mut self.game_info
    }
}

impl Default for SkirmishGameOptionsMenu {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions for UI positioning

#[derive(Debug, Clone, Copy)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

pub const SUPPLY_TECH_SIZE: i32 = 16; // Size of supply/tech markers on map

// Position start spot controls on map preview
pub fn position_start_spot_controls(
    pos: (f32, f32, f32),
    mmd: &MapMetaData,
    map_window_size: ICoord2D,
    gadget_size: ICoord2D,
    ul: ICoord2D,
    lr: ICoord2D,
) -> ICoord2D {
    let small_width = lr.x - ul.x;
    let small_height = lr.y - ul.y;

    let position_x = (pos.0 - mmd.extent_lo_x) / (mmd.extent_hi_x - mmd.extent_lo_x);
    let mut gadget_pos_x = (position_x * small_width as f32) as i32 - gadget_size.x / 2 + ul.x;

    let position_y = (pos.1 - mmd.extent_lo_y) / (mmd.extent_hi_y - mmd.extent_lo_y);
    let mut gadget_pos_y = ((1.0 - position_y) * small_height as f32) as i32 - gadget_size.y / 2 + ul.y;

    ICoord2D { x: gadget_pos_x, y: gadget_pos_y }
}

// Find draw positions for map preview
pub fn find_draw_positions(
    _x: i32,
    _y: i32,
    width: i32,
    height: i32,
    extent_lo: (f32, f32),
    extent_hi: (f32, f32),
) -> (ICoord2D, ICoord2D) {
    // Simplified implementation - calculate centered position maintaining aspect ratio
    let extent_width = extent_hi.0 - extent_lo.0;
    let extent_height = extent_hi.1 - extent_lo.1;
    let aspect = extent_width / extent_height;

    let mut draw_width = width;
    let mut draw_height = height;

    if aspect > (width as f32 / height as f32) {
        draw_height = (width as f32 / aspect) as i32;
    } else {
        draw_width = (height as f32 * aspect) as i32;
    }

    let ul_x = (width - draw_width) / 2;
    let ul_y = (height - draw_height) / 2;
    let lr_x = ul_x + draw_width;
    let lr_y = ul_y + draw_height;

    (ICoord2D { x: ul_x, y: ul_y }, ICoord2D { x: lr_x, y: lr_y })
}

// Position additional map images (supply depots and tech buildings)
pub fn position_additional_images(
    mmd: &MapMetaData,
    map_window_size: ICoord2D,
    ul: ICoord2D,
    lr: ICoord2D,
) -> (Vec<ICoord2D>, Vec<ICoord2D>) {
    let mut supply_positions = Vec::new();
    let mut tech_positions = Vec::new();

    let small_width = lr.x - ul.x;
    let small_height = lr.y - ul.y;

    // Position supply depots
    for pos in &mmd.supply_positions {
        let position_x = (pos.0 - mmd.extent_lo_x) / (mmd.extent_hi_x - mmd.extent_lo_x);
        let marker_x = (position_x * small_width as f32) as i32 - SUPPLY_TECH_SIZE / 2 + ul.x;

        let position_y = (pos.1 - mmd.extent_lo_y) / (mmd.extent_hi_y - mmd.extent_lo_y);
        let marker_y = ((1.0 - position_y) * small_height as f32) as i32 - SUPPLY_TECH_SIZE / 2 + ul.y;

        supply_positions.push(ICoord2D { x: marker_x, y: marker_y });
    }

    // Position tech buildings
    for pos in &mmd.tech_positions {
        let position_x = (pos.0 - mmd.extent_lo_x) / (mmd.extent_hi_x - mmd.extent_lo_x);
        let marker_x = (position_x * small_width as f32) as i32 - SUPPLY_TECH_SIZE / 2 + ul.x;

        let position_y = (pos.1 - mmd.extent_lo_y) / (mmd.extent_hi_y - mmd.extent_lo_y);
        let marker_y = ((1.0 - position_y) * small_height as f32) as i32 - SUPPLY_TECH_SIZE / 2 + ul.y;

        tech_positions.push(ICoord2D { x: marker_x, y: marker_y });
    }

    (supply_positions, tech_positions)
}

// Update map start spots display
pub fn update_map_start_spots(
    game_info: &SkirmishGameInfo,
    mmd: &MapMetaData,
    on_load_screen: bool,
) -> Vec<Option<(usize, String)>> {
    let mut positions = vec![None; MAX_SLOTS];

    for i in 0..MAX_SLOTS {
        if let Some(slot) = game_info.get_const_slot(i) {
            let pos = if on_load_screen {
                slot.get_apparent_start_pos()
            } else {
                slot.get_start_pos()
            };

            if pos >= 0 && (pos as usize) < mmd.num_players && slot.get_player_template() > PLAYERTEMPLATE_MIN {
                let display_number = format!("{}", i + 1);
                positions[pos as usize] = Some((i, display_number));
            }
        }
    }

    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_slot_creation() {
        let slot = GameSlot::new();
        assert_eq!(slot.get_state(), SlotState::Open);
        assert_eq!(slot.get_color(), -1);
        assert_eq!(slot.get_player_template(), PLAYERTEMPLATE_RANDOM);
        assert_eq!(slot.get_start_pos(), -1);
    }

    #[test]
    fn test_slot_state_is_ai() {
        assert!(SlotState::EasyAI.is_ai());
        assert!(SlotState::MediumAI.is_ai());
        assert!(SlotState::HardAI.is_ai());
        assert!(!SlotState::Open.is_ai());
        assert!(!SlotState::Player.is_ai());
    }

    #[test]
    fn test_skirmish_game_info_creation() {
        let info = SkirmishGameInfo::new();
        assert_eq!(info.get_num_players(), 0);
        assert_eq!(info.get_map(), "");
        assert_eq!(info.get_starting_cash().count_money(), 0);
    }

    #[test]
    fn test_color_selection() {
        let mut menu = SkirmishGameOptionsMenu::new();
        menu.init();

        menu.handle_color_selection(0, 1, 8);
        if let Some(slot) = menu.get_game_info().get_const_slot(0) {
            assert_eq!(slot.get_color(), 1);
        }
    }

    #[test]
    fn test_team_selection() {
        let mut menu = SkirmishGameOptionsMenu::new();
        menu.init();

        menu.handle_team_selection(0, 2);
        if let Some(slot) = menu.get_game_info().get_const_slot(0) {
            assert_eq!(slot.get_team_number(), 2);
        }
    }

    #[test]
    fn test_player_template_selection() {
        let mut menu = SkirmishGameOptionsMenu::new();
        menu.init();

        menu.handle_player_template_selection(0, 3);
        if let Some(slot) = menu.get_game_info().get_const_slot(0) {
            assert_eq!(slot.get_player_template(), 3);
        }
    }

    #[test]
    fn test_start_position_assignment() {
        let mut menu = SkirmishGameOptionsMenu::new();
        menu.init();

        menu.handle_start_position_selection(0, 2, 8);
        if let Some(slot) = menu.get_game_info().get_const_slot(0) {
            assert_eq!(slot.get_start_pos(), 2);
        }
    }

    #[test]
    fn test_start_position_uniqueness() {
        let mut menu = SkirmishGameOptionsMenu::new();
        menu.init();

        // Assign position 1 to player 0
        menu.handle_start_position_selection(0, 1, 8);

        // Try to assign position 1 to player 1 (should fail)
        menu.handle_start_position_selection(1, 1, 8);

        if let Some(slot0) = menu.get_game_info().get_const_slot(0) {
            assert_eq!(slot0.get_start_pos(), 1);
        }
        if let Some(slot1) = menu.get_game_info().get_const_slot(1) {
            assert_ne!(slot1.get_start_pos(), 1); // Should not have position 1
        }
    }

    #[test]
    fn test_starting_cash() {
        let mut menu = SkirmishGameOptionsMenu::new();
        menu.init();

        menu.handle_starting_cash_selection(10000);
        assert_eq!(menu.get_game_info().get_starting_cash().count_money(), 10000);
    }

    #[test]
    fn test_superweapon_restriction() {
        let mut menu = SkirmishGameOptionsMenu::new();
        menu.init();

        menu.handle_limit_superweapons_click(true);
        assert_eq!(menu.get_game_info().get_superweapon_restriction(), 1);

        menu.handle_limit_superweapons_click(false);
        assert_eq!(menu.get_game_info().get_superweapon_restriction(), 0);
    }

    #[test]
    fn test_fps_slider() {
        let mut menu = SkirmishGameOptionsMenu::new();
        menu.init();

        let text = menu.set_fps_slider(30);
        assert_eq!(text, "30");

        let text = menu.set_fps_slider(70);
        assert_eq!(text, "--");
    }

    #[test]
    fn test_money_deposit() {
        let mut money = Money::new();
        money.deposit(5000, false);
        assert_eq!(money.count_money(), 5000);
    }

    #[test]
    fn test_map_metadata() {
        let mut mmd = MapMetaData::new();
        mmd.display_name = "Test Map".to_string();
        mmd.num_players = 4;
        mmd.is_multiplayer = true;

        assert_eq!(mmd.display_name, "Test Map");
        assert_eq!(mmd.num_players, 4);
        assert!(mmd.is_multiplayer);
    }

    #[test]
    fn test_find_draw_positions() {
        let (ul, lr) = find_draw_positions(0, 0, 256, 256, (0.0, 0.0), (1000.0, 1000.0));
        assert!(ul.x >= 0);
        assert!(ul.y >= 0);
        assert!(lr.x <= 256);
        assert!(lr.y <= 256);
    }

    #[test]
    fn test_num_players_count() {
        let mut info = SkirmishGameInfo::new();

        if let Some(slot) = info.get_slot(0) {
            slot.set_state(SlotState::Player, "Player1".to_string());
        }
        if let Some(slot) = info.get_slot(1) {
            slot.set_state(SlotState::EasyAI, "".to_string());
        }

        assert_eq!(info.get_num_players(), 2);
    }
}
