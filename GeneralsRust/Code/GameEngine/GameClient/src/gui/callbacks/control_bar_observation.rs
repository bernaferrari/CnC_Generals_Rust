//! Read-only native observations admitted at one receiving callback boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlBarPlayerObservation {
    pub id: u32,
    pub active: bool,
    pub selection_count: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlBarMouseObservation {
    pub middle_held: bool,
    pub alternate_mouse: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlBarCallbackObservation {
    pub game_ending: bool,
    pub multiplayer: bool,
    pub local_player: Option<ControlBarPlayerObservation>,
    /// Exact side token from the receiving player's admitted PlayerTemplate.
    pub local_player_side: Option<String>,
    pub mouse: Option<ControlBarMouseObservation>,
}
