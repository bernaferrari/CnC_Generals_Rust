//! Typed single-player campaign-launch delivery for AuthorityOnly Main.
//!
//! C++ uses one GlobalData/GameInfo path from the shell into GameLogic.  Rust's
//! retail GameClient shell and Main authority are deliberately separate, so a
//! campaign or Challenge launch must carry its exact selection across that
//! boundary instead of letting Main guess from a stale HUD faction.

/// Immutable shell selection that belongs to exactly one queued `MSG_NEW_GAME`.
///
/// `player_template_*` is deliberately opaque to this bridge: Main validates
/// the template only to derive a base faction today.  The active GameLogic
/// PlayerTemplate binding is a separate authority seam, rather than silently
/// treating a General template as an ordinary Team.
pub use game_engine::common::message_stream::ClientCampaignLaunch as HostCampaignLaunchDescriptor;

/// Immutable facts borrowed from the driving native world for one UI operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuWorldFacts {
    pub in_game: bool,
    pub in_shell_game: bool,
}

/// A source lifecycle action, delivered after the current UI loans end.
#[derive(Clone, Copy, Debug)]
pub enum MenuLaunchSource {
    MainMenu,
    ChallengeMenu,
    ScoreScreen,
}

#[derive(Debug)]
pub enum MenuHostRequest {
    StartCampaign {
        descriptor: HostCampaignLaunchDescriptor,
        clear_previous: bool,
        source: MenuLaunchSource,
        main_menu_layout: Option<std::rc::Weak<std::cell::RefCell<crate::gui::WindowLayout>>>,
    },
    Quit,
    SignalUiInteract(&'static str),
    SetDifficulty(i32),
    ResetPlacement,
    SetCursorVisibility(bool),
}

/// Receiving-client transport only. Campaign runtime state stays in CampaignManager.
#[derive(Default)]
pub struct MenuHostMailbox {
    requests: Vec<MenuHostRequest>,
}
impl MenuHostMailbox {
    pub fn request(&mut self, request: MenuHostRequest) {
        self.requests.push(request);
    }
    pub fn take_requests(&mut self) -> Vec<MenuHostRequest> {
        std::mem::take(&mut self.requests)
    }
    pub fn has_requests(&self) -> bool {
        !self.requests.is_empty()
    }
    pub fn has_lifecycle_request(&self) -> bool {
        self.requests.iter().any(|request| {
            matches!(
                request,
                MenuHostRequest::StartCampaign { .. } | MenuHostRequest::Quit
            )
        })
    }
    pub fn has_campaign_start(&self) -> bool {
        self.requests.iter().any(|request| {
            matches!(
                request,
                MenuHostRequest::StartCampaign {
                    source: MenuLaunchSource::MainMenu,
                    ..
                }
            )
        })
    }
    pub fn clear_pending(&mut self) {
        self.requests.clear();
    }
    pub fn campaign_message(
        descriptor: HostCampaignLaunchDescriptor,
    ) -> crate::message_stream::GameMessage {
        let mut message = crate::message_stream::GameMessage::new(
            crate::message_stream::GameMessageType::NewGame,
        );
        message.append_integer_argument(descriptor.game_mode_code);
        message.append_integer_argument(descriptor.difficulty_code);
        message.append_integer_argument(descriptor.rank_points);
        if let Some(fps) = descriptor.max_fps {
            message.append_integer_argument(fps);
        }
        message.set_campaign_launch(descriptor);
        message
    }
}
