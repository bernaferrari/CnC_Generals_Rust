use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLLoginMenu.cpp",
    "crate::gui::callbacks::menus::wol_login_menu",
    "WOL Login Menu",
    "WOL login callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLLoginMenu",
    "WOL Login",
    "Online account sign-in flow.",
    "WOL",
);

pub const LOGIN_TIMEOUT_MS: u32 = 10_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredLoginPort {
    pub email: String,
    pub nicks: Vec<String>,
    pub has_password: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolLoginMenuPort {
    pub web_browser_active: bool,
    pub use_web_browser_for_tos: bool,
    pub is_shutting_down: bool,
    pub button_pushed: bool,
    pub next_screen: Option<String>,
    pub login_attempt_time_ms: Option<u32>,
    pub remember_account: bool,
    pub email: String,
    pub nick: String,
    pub password_present: bool,
    pub stored_logins: Vec<StoredLoginPort>,
    pub status_message: String,
}

impl Default for WolLoginMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolLoginMenuPort {
    pub fn init(stored_logins: Vec<StoredLoginPort>) -> Self {
        let first = stored_logins.first().cloned();
        Self {
            web_browser_active: false,
            use_web_browser_for_tos: false,
            is_shutting_down: false,
            button_pushed: false,
            next_screen: None,
            login_attempt_time_ms: None,
            remember_account: true,
            email: first
                .as_ref()
                .map(|login| login.email.clone())
                .unwrap_or_default(),
            nick: first
                .as_ref()
                .and_then(|login| login.nicks.first().cloned())
                .unwrap_or_default(),
            password_present: first
                .as_ref()
                .map(|login| login.has_password)
                .unwrap_or(false),
            stored_logins,
            status_message: "Waiting for credentials.".to_string(),
        }
    }

    pub fn choose_email(&mut self, email: &str) -> bool {
        let Some(login) = self.stored_logins.iter().find(|login| login.email == email) else {
            return false;
        };
        self.email = login.email.clone();
        self.nick = login.nicks.first().cloned().unwrap_or_default();
        self.password_present = login.has_password;
        true
    }

    pub fn attempt_login(&mut self, now_ms: u32) -> bool {
        if self.email.trim().is_empty() || self.nick.trim().is_empty() {
            self.status_message = "Email and nickname are required.".to_string();
            return false;
        }
        self.button_pushed = true;
        self.login_attempt_time_ms = Some(now_ms);
        self.status_message = "Connecting to GameSpy services...".to_string();
        true
    }

    pub fn check_timeout(&mut self, now_ms: u32) -> bool {
        let Some(start_ms) = self.login_attempt_time_ms else {
            return false;
        };
        if now_ms.saturating_sub(start_ms) < LOGIN_TIMEOUT_MS {
            return false;
        }

        self.login_attempt_time_ms = None;
        self.button_pushed = false;
        self.status_message = "Login timed out.".to_string();
        true
    }

    pub fn complete_login(&mut self, next_screen: impl Into<String>) {
        self.is_shutting_down = true;
        self.next_screen = Some(next_screen.into());
        self.status_message = "Login accepted.".to_string();
    }

    pub fn forget_selected_login(&mut self) -> bool {
        let Some(index) = self
            .stored_logins
            .iter()
            .position(|login| login.email == self.email)
        else {
            return false;
        };
        self.stored_logins.remove(index);
        self.email.clear();
        self.nick.clear();
        self.password_present = false;
        true
    }

    pub fn sample() -> Self {
        Self::init(vec![
            StoredLoginPort {
                email: "player@example.com".to_string(),
                nicks: vec!["ZeroHourAce".to_string(), "ZHAlt".to_string()],
                has_password: true,
            },
            StoredLoginPort {
                email: "old@example.com".to_string(),
                nicks: vec!["LegacyGeneral".to_string()],
                has_password: false,
            },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempt_login_requires_credentials() {
        let mut login = WolLoginMenuPort::init(vec![]);
        assert!(!login.attempt_login(0));
        assert_eq!(login.status_message, "Email and nickname are required.");
    }

    #[test]
    fn timeout_resets_pending_login() {
        let mut login = WolLoginMenuPort::sample();
        assert!(login.attempt_login(100));
        assert!(login.check_timeout(10_200));
        assert_eq!(login.status_message, "Login timed out.");
        assert!(!login.button_pushed);
    }
}
