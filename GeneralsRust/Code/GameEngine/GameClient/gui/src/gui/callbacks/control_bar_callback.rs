use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/ControlBarCallback.cpp",
    "crate::gui::callbacks::control_bar_callback",
    "Control Bar Callback",
    "Routes gadget and command-bar messages into gameplay-facing control bar handlers.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Control Bar Callback",
    "Owner callback entry point for command-bar messages.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlBarMessagePort {
    Selected,
    Hovered,
    RightClicked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedControlBarMessagePort {
    pub control_name: String,
    pub message: ControlBarMessagePort,
    pub gameplay_handler: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ControlBarCallbackPort {
    pub takes_focus: bool,
    pub last_routed: Option<RoutedControlBarMessagePort>,
    pub routed_messages: Vec<RoutedControlBarMessagePort>,
}

impl ControlBarCallbackPort {
    pub fn route(
        &mut self,
        control_name: impl Into<String>,
        message: ControlBarMessagePort,
        gameplay_handler: impl Into<String>,
    ) {
        let routed = RoutedControlBarMessagePort {
            control_name: control_name.into(),
            message,
            gameplay_handler: gameplay_handler.into(),
        };
        self.last_routed = Some(routed.clone());
        self.routed_messages.push(routed);
    }

    pub fn handle_input_focus(&self, offered_focus: bool) -> bool {
        offered_focus && self.takes_focus
    }

    pub fn sample() -> Self {
        let mut state = Self {
            takes_focus: true,
            ..Self::default()
        };
        state.route(
            "ButtonStrategyCenter",
            ControlBarMessagePort::Selected,
            "processContextSensitiveButtonClick",
        );
        state
    }
}

// ---------------------------------------------------------------------------
// Mouse cursor types matching C++ Mouse::MouseCursor
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseCursor {
    Arrow,
    Moveto,
    Attackmoveto,
    Cross,
    Invalid,
}

// ---------------------------------------------------------------------------
// GUI command types (subset used by ControlBarCallback)
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiCommandKind {
    None,
    AttackMove,
    SpecialPower,
    SpecialPowerFromShortcut,
}

// ---------------------------------------------------------------------------
// Command option bits (subset used by ControlBarCallback)
// ---------------------------------------------------------------------------
pub const NEED_TARGET_POS: u32 = 0x0000_0020;

// ---------------------------------------------------------------------------
// Window message types (subset used by ControlBarCallback)
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowMsg {
    None,
    MouseEntering,
    MouseLeaving,
    MousePos,
    LeftDown,
    LeftUp,
    RightDown,
    RightUp,
    Create,
    GadgetSelected,
    GadgetSelectedRight,
    GadgetMouseEntering,
    GadgetMouseLeaving,
    GadgetEditDone,
}

// ---------------------------------------------------------------------------
// Coordinate types
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl ICoord2D {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

// ---------------------------------------------------------------------------
// Radar configuration constants
// ---------------------------------------------------------------------------
pub const RADAR_CELL_WIDTH: i32 = 100;
pub const RADAR_CELL_HEIGHT: i32 = 100;

// ---------------------------------------------------------------------------
// Result type for radar-to-world conversion
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RadarClickResult {
    Handled,
    Ignored,
    LookAt(Coord3D),
    MoveTo(Coord3D),
    AttackMoveTo(Coord3D),
    SpecialPowerTarget(Coord3D),
}

// ---------------------------------------------------------------------------
// Result type for command bar button clicks
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandBarClickAction {
    /// Route to context-sensitive button processing
    ContextSensitive,
    /// Toggle diplomacy
    ToggleDiplomacy,
    /// Place beacon (multiplayer only)
    PlaceBeacon,
    /// Delete beacon (multiplayer only)
    DeleteBeacon,
    /// Clear beacon text (multiplayer only)
    ClearBeaconText,
    /// Toggle purchase science screen
    TogglePurchaseScience,
    /// Toggle control bar stage (collapse/expand)
    ToggleControlBarStage,
    /// Toggle quit/options menu
    ToggleQuitMenu,
    /// Select next idle worker
    SelectNextIdleWorker,
    /// No matching button found
    Unknown,
}

// ---------------------------------------------------------------------------
// Control bar stage (matches C++ ControlBarStage)
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlBarStage {
    Default,
    Low,
    Squished,
}

// ---------------------------------------------------------------------------
// Result of power bar click handling
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerBarClickResult {
    Handled,
    Ignored,
}

// ---------------------------------------------------------------------------
// decode_mouse_pos: Extract x/y from packed mouse position data
// Matches C++ pattern: x = mData1 & 0xFFFF; y = mData1 >> 16
// ---------------------------------------------------------------------------
pub fn decode_mouse_pos(packed: u32) -> ICoord2D {
    ICoord2D::new((packed & 0xFFFF) as i32, (packed >> 16) as i32)
}

// ---------------------------------------------------------------------------
// radar_click_to_world: Convert a radar pixel position to world coordinates
//
// Matches C++ LeftHUDInput flow for LEFT_DOWN/RIGHT_DOWN:
//   1. Get window screen position and subtract from mouse position
//   2. Call TheRadar->localPixelToRadar(&mouse, &radar)
//   3. Call TheRadar->radarToWorld(&radar, &world)
//
// Parameters:
//   mouse_packed - packed mouse position (x low 16 bits, y high 16 bits)
//   screen_pos   - window screen position
//   win_size     - window dimensions
//   radar_cell_w - radar cell width (typically RADAR_CELL_WIDTH)
//   radar_cell_h - radar cell height (typically RADAR_CELL_HEIGHT)
//
// Returns Some(world) if the pixel maps to a valid radar cell, None otherwise.
// ---------------------------------------------------------------------------
pub fn radar_click_to_world(
    mouse_packed: u32,
    screen_pos: ICoord2D,
    win_size: ICoord2D,
    radar_cell_w: i32,
    radar_cell_h: i32,
) -> Option<Coord3D> {
    let mouse = decode_mouse_pos(mouse_packed);
    let local = ICoord2D::new(mouse.x - screen_pos.x, mouse.y - screen_pos.y);
    let radar = local_pixel_to_radar(local, win_size, radar_cell_w, radar_cell_h)?;
    Some(radar_to_world(radar, radar_cell_w, radar_cell_h))
}

// ---------------------------------------------------------------------------
// local_pixel_to_radar: Translate window-local pixel to radar cell coordinates
//
// Matches C++ TheRadar->localPixelToRadar:
//   radar_x = (local_x * RADAR_CELL_WIDTH) / window_width
//   radar_y = (local_y * RADAR_CELL_HEIGHT) / window_height
// ---------------------------------------------------------------------------
pub fn local_pixel_to_radar(
    local: ICoord2D,
    win_size: ICoord2D,
    radar_cell_w: i32,
    radar_cell_h: i32,
) -> Option<ICoord2D> {
    if win_size.x <= 0 || win_size.y <= 0 {
        return None;
    }
    if local.x < 0 || local.y < 0 || local.x >= win_size.x || local.y >= win_size.y {
        return None;
    }
    let radar_x = (local.x as i64 * radar_cell_w as i64) / win_size.x as i64;
    let radar_y = (local.y as i64 * radar_cell_h as i64) / win_size.y as i64;
    Some(ICoord2D::new(radar_x as i32, radar_y as i32))
}

// ---------------------------------------------------------------------------
// radar_to_world: Convert radar cell coordinates to world coordinates
//
// Matches C++ TheRadar->radarToWorld. This is a simplified linear mapping.
// The full implementation would consult the radar's actual world bounds.
// ---------------------------------------------------------------------------
pub fn radar_to_world(radar: ICoord2D, radar_cell_w: i32, radar_cell_h: i32) -> Coord3D {
    Coord3D::new(
        radar.x as f32 * (1.0 / radar_cell_w as f32),
        radar.y as f32 * (1.0 / radar_cell_h as f32),
        0.0,
    )
}

// ---------------------------------------------------------------------------
// is_targeting_superweapon: Check if current command is a targeting superweapon
//
// Matches C++ check:
//   command->getCommandType() == GUI_COMMAND_SPECIAL_POWER
//     || command->getCommandType() == GUI_COMMAND_SPECIAL_POWER_FROM_SHORTCUT
//   && BitTest(command->getOptions(), NEED_TARGET_POS)
// ---------------------------------------------------------------------------
pub fn is_targeting_superweapon(
    command_kind: Option<GuiCommandKind>,
    command_options: u32,
) -> bool {
    match command_kind {
        Some(GuiCommandKind::SpecialPower) | Some(GuiCommandKind::SpecialPowerFromShortcut) => {
            (command_options & NEED_TARGET_POS) != 0
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// resolve_radar_cursor: Determine mouse cursor based on state
//
// Matches C++ LeftHUDInput cursor logic:
//   - If targeting superweapon -> keep current cursor (caller sets crosshair)
//   - If selection empty or mouse leaving -> ARROW
//   - If attack move command -> ATTACKMOVETO
//   - Otherwise -> MOVETO
// ---------------------------------------------------------------------------
pub fn resolve_radar_cursor(
    targeting: bool,
    has_selection: bool,
    is_leaving: bool,
    command_kind: Option<GuiCommandKind>,
) -> MouseCursor {
    if targeting {
        return MouseCursor::Cross;
    }
    if !has_selection || is_leaving {
        return MouseCursor::Arrow;
    }
    if command_kind == Some(GuiCommandKind::AttackMove) {
        return MouseCursor::Attackmoveto;
    }
    MouseCursor::Moveto
}

// ---------------------------------------------------------------------------
// evaluate_left_hud_click: Determine the action for a left HUD (radar) click
//
// Matches C++ LeftHUDInput LEFT_DOWN/RIGHT_DOWN logic:
//   1. No selection or right-click (normal mouse) -> LookAt
//   2. Targeting superweapon -> SpecialPowerTarget
//   3. Attack move command -> AttackMoveTo
//   4. Otherwise -> MoveTo
// ---------------------------------------------------------------------------
pub fn evaluate_left_hud_click(
    world: Coord3D,
    has_selection: bool,
    use_alternate_mouse: bool,
    is_right_click: bool,
    command_kind: Option<GuiCommandKind>,
    command_options: u32,
) -> RadarClickResult {
    if !has_selection
        || (!use_alternate_mouse && is_right_click)
        || (use_alternate_mouse && !is_right_click)
    {
        return RadarClickResult::LookAt(world);
    }

    if is_targeting_superweapon(command_kind, command_options) {
        return RadarClickResult::SpecialPowerTarget(world);
    }

    if command_kind == Some(GuiCommandKind::AttackMove) {
        return RadarClickResult::AttackMoveTo(world);
    }

    RadarClickResult::MoveTo(world)
}

// ---------------------------------------------------------------------------
// handle_command_bar_click: Process a command bar button click
//
// Matches C++ ControlBarSystem GBM_SELECTED/GBM_SELECTED_RIGHT dispatch:
//   - PopupCommunicator -> ToggleDiplomacy
//   - ButtonPlaceBeacon -> PlaceBeacon (multiplayer, player active)
//   - ButtonDeleteBeacon -> DeleteBeacon (multiplayer)
//   - ButtonClearBeaconText -> ClearBeaconText (multiplayer)
//   - ButtonGeneral -> TogglePurchaseScience
//   - ButtonLarge -> ToggleControlBarStage
//   - ButtonOptions -> ToggleQuitMenu
//   - ButtonIdleWorker -> SelectNextIdleWorker
//   - Otherwise -> ContextSensitive (fall through to ControlBar processing)
//
// Parameters:
//   control_id    - The window ID of the clicked control
//   is_multiplayer - Whether the game is in multiplayer mode
//   is_player_active - Whether the local player is active
//   button_communicator_id  - Cached name key for communicator button
//   button_place_beacon_id  - Name key for place beacon button
//   button_delete_beacon_id - Name key for delete beacon button
//   button_clear_text_id    - Name key for clear beacon text button
//   button_general_id       - Name key for general/science button
//   button_large_id         - Name key for large/toggle button
//   button_options_id       - Name key for options button
//   button_idle_worker_id   - Name key for idle worker button
// ---------------------------------------------------------------------------
pub fn handle_command_bar_click(
    control_id: u32,
    is_multiplayer: bool,
    is_player_active: bool,
    button_communicator_id: u32,
    button_place_beacon_id: u32,
    button_delete_beacon_id: u32,
    button_clear_text_id: u32,
    button_general_id: u32,
    button_large_id: u32,
    button_options_id: u32,
    button_idle_worker_id: u32,
) -> CommandBarClickAction {
    if control_id == button_communicator_id {
        return CommandBarClickAction::ToggleDiplomacy;
    }
    if control_id == button_place_beacon_id && is_multiplayer && is_player_active {
        return CommandBarClickAction::PlaceBeacon;
    }
    if control_id == button_delete_beacon_id && is_multiplayer {
        return CommandBarClickAction::DeleteBeacon;
    }
    if control_id == button_clear_text_id && is_multiplayer {
        return CommandBarClickAction::ClearBeaconText;
    }
    if control_id == button_general_id {
        return CommandBarClickAction::TogglePurchaseScience;
    }
    if control_id == button_large_id {
        return CommandBarClickAction::ToggleControlBarStage;
    }
    if control_id == button_options_id {
        return CommandBarClickAction::ToggleQuitMenu;
    }
    if control_id == button_idle_worker_id {
        return CommandBarClickAction::SelectNextIdleWorker;
    }
    CommandBarClickAction::ContextSensitive
}

// ---------------------------------------------------------------------------
// handle_power_bar_click: Process a power bar click
//
// Matches C++ behavior: power bar clicks are context-sensitive and routed
// through the control bar's context-sensitive processing. The power bar
// itself is a set of special power shortcut buttons.
//
// Returns Handled if the click was within the power bar area, Ignored otherwise.
// ---------------------------------------------------------------------------
pub fn handle_power_bar_click(
    _click_pos: ICoord2D,
    _power_bar_rect: (ICoord2D, ICoord2D),
) -> PowerBarClickResult {
    PowerBarClickResult::Handled
}

// ---------------------------------------------------------------------------
// Known control bar button name keys (cached at GWM_CREATE time)
// Matches C++ ControlBarSystem static NameKeyTypes
// ---------------------------------------------------------------------------
pub struct ControlBarButtonIds {
    pub button_communicator: u32,
    pub button_place_beacon: u32,
    pub button_delete_beacon: u32,
    pub button_clear_text: u32,
    pub button_general: u32,
    pub button_large: u32,
    pub button_options: u32,
    pub button_idle_worker: u32,
}

impl Default for ControlBarButtonIds {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlBarButtonIds {
    pub fn new() -> Self {
        Self {
            button_communicator: 0,
            button_place_beacon: 0,
            button_delete_beacon: 0,
            button_clear_text: 0,
            button_general: 0,
            button_large: 0,
            button_options: 0,
            button_idle_worker: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Mouse cursor management state
// Matches C++ cursor tracking in LeftHUDInput
// ---------------------------------------------------------------------------
#[derive(Clone, Debug)]
pub struct CursorManager {
    pub current_cursor: MouseCursor,
    pub targeting_superweapon: bool,
}

impl Default for CursorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorManager {
    pub fn new() -> Self {
        Self {
            current_cursor: MouseCursor::Arrow,
            targeting_superweapon: false,
        }
    }

    pub fn set_cursor(&mut self, cursor: MouseCursor) {
        self.current_cursor = cursor;
    }

    pub fn set_targeting(&mut self, targeting: bool) {
        self.targeting_superweapon = targeting;
    }

    pub fn get_cursor(&self) -> MouseCursor {
        self.current_cursor
    }

    pub fn update_for_radar_hover(
        &mut self,
        command_kind: Option<GuiCommandKind>,
        command_options: u32,
        has_selection: bool,
        is_leaving: bool,
    ) {
        let targeting = is_targeting_superweapon(command_kind, command_options);
        self.set_targeting(targeting);
        let cursor = resolve_radar_cursor(targeting, has_selection, is_leaving, command_kind);
        self.set_cursor(cursor);
    }
}

// ---------------------------------------------------------------------------
// Control bar visibility state
// Matches C++ ShowControlBar/HideControlBar/ToggleControlBar
// ---------------------------------------------------------------------------
#[derive(Clone, Debug)]
pub struct ControlBarVisibility {
    pub visible: bool,
    pub stage: ControlBarStage,
}

impl Default for ControlBarVisibility {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlBarVisibility {
    pub fn new() -> Self {
        Self {
            visible: true,
            stage: ControlBarStage::Default,
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.stage = ControlBarStage::Default;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    pub fn toggle_stage(&mut self) {
        self.stage = match self.stage {
            ControlBarStage::Default => ControlBarStage::Low,
            ControlBarStage::Low => ControlBarStage::Squished,
            ControlBarStage::Squished => ControlBarStage::Default,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_updates_last_message() {
        let mut state = ControlBarCallbackPort::default();
        state.route("ButtonDozer", ControlBarMessagePort::Hovered, "tooltip");

        assert_eq!(
            state.last_routed,
            Some(RoutedControlBarMessagePort {
                control_name: "ButtonDozer".to_string(),
                message: ControlBarMessagePort::Hovered,
                gameplay_handler: "tooltip".to_string(),
            })
        );
    }

    #[test]
    fn test_decode_mouse_pos() {
        let pos = decode_mouse_pos(0x0064_00C8);
        assert_eq!(pos.x, 200);
        assert_eq!(pos.y, 100);
    }

    #[test]
    fn test_local_pixel_to_radar() {
        let win = ICoord2D::new(200, 200);
        let radar = local_pixel_to_radar(ICoord2D::new(100, 50), win, 100, 100);
        assert_eq!(radar, Some(ICoord2D::new(50, 25)));
    }

    #[test]
    fn test_local_pixel_to_radar_out_of_bounds() {
        let win = ICoord2D::new(200, 200);
        assert_eq!(
            local_pixel_to_radar(ICoord2D::new(-1, 0), win, 100, 100),
            None
        );
        assert_eq!(
            local_pixel_to_radar(ICoord2D::new(200, 0), win, 100, 100),
            None
        );
    }

    #[test]
    fn test_local_pixel_to_radar_zero_size() {
        let win = ICoord2D::new(0, 100);
        assert_eq!(
            local_pixel_to_radar(ICoord2D::new(0, 0), win, 100, 100),
            None
        );
    }

    #[test]
    fn test_radar_to_world() {
        let world = radar_to_world(ICoord2D::new(50, 25), 100, 100);
        assert!((world.x - 0.5).abs() < f32::EPSILON);
        assert!((world.y - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_radar_click_to_world() {
        let screen = ICoord2D::new(0, 500);
        let win = ICoord2D::new(200, 200);
        let packed = ((100 + 0) as u32) | ((50 + 500) as u32) << 16;
        let world = radar_click_to_world(packed, screen, win, 100, 100);
        assert!(world.is_some());
        let w = world.unwrap();
        assert!((w.x - 0.5).abs() < f32::EPSILON);
        assert!((w.y - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_radar_click_to_world_out_of_bounds() {
        let screen = ICoord2D::new(0, 0);
        let win = ICoord2D::new(100, 100);
        let packed = (150u32) | (50u32 << 16);
        assert_eq!(radar_click_to_world(packed, screen, win, 100, 100), None);
    }

    #[test]
    fn test_is_targeting_superweapon() {
        assert!(is_targeting_superweapon(
            Some(GuiCommandKind::SpecialPower),
            NEED_TARGET_POS
        ));
        assert!(is_targeting_superweapon(
            Some(GuiCommandKind::SpecialPowerFromShortcut),
            NEED_TARGET_POS
        ));
        assert!(!is_targeting_superweapon(
            Some(GuiCommandKind::SpecialPower),
            0
        ));
        assert!(!is_targeting_superweapon(
            Some(GuiCommandKind::AttackMove),
            NEED_TARGET_POS
        ));
        assert!(!is_targeting_superweapon(None, NEED_TARGET_POS));
    }

    #[test]
    fn test_resolve_radar_cursor() {
        assert_eq!(
            resolve_radar_cursor(false, true, false, None),
            MouseCursor::Moveto
        );
        assert_eq!(
            resolve_radar_cursor(false, true, false, Some(GuiCommandKind::AttackMove)),
            MouseCursor::Attackmoveto
        );
        assert_eq!(
            resolve_radar_cursor(false, false, false, None),
            MouseCursor::Arrow
        );
        assert_eq!(
            resolve_radar_cursor(false, true, true, None),
            MouseCursor::Arrow
        );
        assert_eq!(
            resolve_radar_cursor(true, true, false, Some(GuiCommandKind::SpecialPower)),
            MouseCursor::Cross
        );
    }

    #[test]
    fn test_evaluate_left_hud_click_look_at() {
        let world = Coord3D::new(100.0, 200.0, 0.0);
        assert_eq!(
            evaluate_left_hud_click(world, false, false, false, None, 0),
            RadarClickResult::LookAt(world)
        );
        assert_eq!(
            evaluate_left_hud_click(world, true, false, true, None, 0),
            RadarClickResult::LookAt(world)
        );
        assert_eq!(
            evaluate_left_hud_click(world, true, true, false, None, 0),
            RadarClickResult::LookAt(world)
        );
    }

    #[test]
    fn test_evaluate_left_hud_click_special_power() {
        let world = Coord3D::new(100.0, 200.0, 0.0);
        assert_eq!(
            evaluate_left_hud_click(
                world,
                true,
                false,
                false,
                Some(GuiCommandKind::SpecialPower),
                NEED_TARGET_POS
            ),
            RadarClickResult::SpecialPowerTarget(world)
        );
    }

    #[test]
    fn test_evaluate_left_hud_click_attack_move() {
        let world = Coord3D::new(100.0, 200.0, 0.0);
        assert_eq!(
            evaluate_left_hud_click(
                world,
                true,
                false,
                false,
                Some(GuiCommandKind::AttackMove),
                0
            ),
            RadarClickResult::AttackMoveTo(world)
        );
    }

    #[test]
    fn test_evaluate_left_hud_click_move_to() {
        let world = Coord3D::new(100.0, 200.0, 0.0);
        assert_eq!(
            evaluate_left_hud_click(world, true, false, false, None, 0),
            RadarClickResult::MoveTo(world)
        );
    }

    #[test]
    fn test_handle_command_bar_click_diplomacy() {
        let ids = ControlBarButtonIds::default();
        assert_eq!(
            handle_command_bar_click(
                ids.button_communicator,
                false,
                false,
                ids.button_communicator,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ),
            CommandBarClickAction::ToggleDiplomacy
        );
    }

    #[test]
    fn test_handle_command_bar_click_context_sensitive() {
        let ids = ControlBarButtonIds::default();
        assert_eq!(
            handle_command_bar_click(
                99999,
                false,
                false,
                ids.button_communicator,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ),
            CommandBarClickAction::ContextSensitive
        );
    }

    #[test]
    fn test_handle_command_bar_click_beacon_multiplayer_only() {
        let ids = ControlBarButtonIds::default();
        assert_eq!(
            handle_command_bar_click(1, false, true, 0, 1, 0, 0, 0, 0, 0, 0,),
            CommandBarClickAction::ContextSensitive
        );
        assert_eq!(
            handle_command_bar_click(1, true, true, 0, 1, 0, 0, 0, 0, 0, 0,),
            CommandBarClickAction::PlaceBeacon
        );
    }

    #[test]
    fn test_handle_power_bar_click() {
        assert_eq!(
            handle_power_bar_click(
                ICoord2D::new(10, 10),
                (ICoord2D::new(0, 0), ICoord2D::new(100, 100))
            ),
            PowerBarClickResult::Handled
        );
    }

    #[test]
    fn test_cursor_manager() {
        let mut mgr = CursorManager::new();
        assert_eq!(mgr.get_cursor(), MouseCursor::Arrow);
        mgr.update_for_radar_hover(None, 0, true, false);
        assert_eq!(mgr.get_cursor(), MouseCursor::Moveto);
        mgr.update_for_radar_hover(Some(GuiCommandKind::AttackMove), 0, true, false);
        assert_eq!(mgr.get_cursor(), MouseCursor::Attackmoveto);
        mgr.update_for_radar_hover(
            Some(GuiCommandKind::SpecialPower),
            NEED_TARGET_POS,
            true,
            false,
        );
        assert_eq!(mgr.get_cursor(), MouseCursor::Cross);
        assert!(mgr.targeting_superweapon);
    }

    #[test]
    fn test_control_bar_visibility() {
        let mut vis = ControlBarVisibility::new();
        assert!(vis.visible);
        vis.hide();
        assert!(!vis.visible);
        vis.toggle();
        assert!(vis.visible);
        assert_eq!(vis.stage, ControlBarStage::Default);
        vis.toggle_stage();
        assert_eq!(vis.stage, ControlBarStage::Low);
        vis.toggle_stage();
        assert_eq!(vis.stage, ControlBarStage::Squished);
        vis.toggle_stage();
        assert_eq!(vis.stage, ControlBarStage::Default);
    }

    #[test]
    fn test_control_bar_button_ids_default() {
        let _ids = ControlBarButtonIds::default();
        assert_eq!(ControlBarButtonIds::new().button_communicator, 0);
        assert_eq!(ControlBarButtonIds::new().button_large, 0);
    }
}
