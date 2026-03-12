//! GameSpy buddy thread definitions (C++ BuddyThread.cpp parity).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use crate::gamespy::peer_defs::GPProfile;

pub const MAX_BUDDY_CHAT_LEN: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuddyRequestType {
    Login,
    Relogin,
    Logout,
    Message,
    LoginNew,
    AddBuddy,
    DelBuddy,
    OkAdd,
    DenyAdd,
    SetStatus,
    DeleteAcct,
}

#[derive(Debug, Clone)]
pub struct BuddyRequest {
    pub request_type: BuddyRequestType,
    pub recipient: GPProfile,
    pub message: String,
    pub nick: String,
    pub email: String,
    pub password: String,
    pub has_firewall: bool,
    pub id: GPProfile,
    pub status: i32,
    pub status_string: String,
    pub location_string: String,
}

impl Default for BuddyRequest {
    fn default() -> Self {
        Self {
            request_type: BuddyRequestType::Login,
            recipient: 0,
            message: String::new(),
            nick: String::new(),
            email: String::new(),
            password: String::new(),
            has_firewall: false,
            id: 0,
            status: 0,
            status_string: String::new(),
            location_string: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuddyResponseType {
    Login,
    Disconnect,
    Message,
    Request,
    Status,
}

#[derive(Debug, Clone)]
pub struct BuddyResponse {
    pub response_type: BuddyResponseType,
    pub profile: GPProfile,
    pub result: i32,
    pub message_date: u32,
    pub message_nick: String,
    pub message_text: String,
    pub request_nick: String,
    pub request_email: String,
    pub request_country_code: String,
    pub request_text: String,
    pub error_code: i32,
    pub error_string: String,
    pub error_fatal: bool,
    pub status_nick: String,
    pub status_email: String,
    pub status_country_code: String,
    pub status_location: String,
    pub status_value: i32,
    pub status_string: String,
}

impl Default for BuddyResponse {
    fn default() -> Self {
        Self {
            response_type: BuddyResponseType::Login,
            profile: 0,
            result: 0,
            message_date: 0,
            message_nick: String::new(),
            message_text: String::new(),
            request_nick: String::new(),
            request_email: String::new(),
            request_country_code: String::new(),
            request_text: String::new(),
            error_code: 0,
            error_string: String::new(),
            error_fatal: false,
            status_nick: String::new(),
            status_email: String::new(),
            status_country_code: String::new(),
            status_location: String::new(),
            status_value: 0,
            status_string: String::new(),
        }
    }
}

#[derive(Default)]
pub struct GameSpyBuddyMessageQueue {
    requests: VecDeque<BuddyRequest>,
    responses: VecDeque<BuddyResponse>,
    running: bool,
    connected: bool,
    connecting: bool,
    local_profile_id: GPProfile,
}

impl GameSpyBuddyMessageQueue {
    pub fn start_thread(&mut self) {
        self.running = true;
    }

    pub fn end_thread(&mut self) {
        self.running = false;
        self.connected = false;
        self.connecting = false;
    }

    pub fn is_thread_running(&self) -> bool {
        self.running
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn is_connecting(&self) -> bool {
        self.connecting
    }

    pub fn add_request(&mut self, req: BuddyRequest) {
        self.requests.push_back(req);
    }

    pub fn get_request(&mut self) -> Option<BuddyRequest> {
        self.requests.pop_front()
    }

    pub fn add_response(&mut self, resp: BuddyResponse) {
        self.responses.push_back(resp);
    }

    pub fn get_response(&mut self) -> Option<BuddyResponse> {
        self.responses.pop_front()
    }

    pub fn set_local_profile_id(&mut self, id: GPProfile) {
        self.local_profile_id = id;
    }

    pub fn get_local_profile_id(&self) -> GPProfile {
        self.local_profile_id
    }
}

static THE_GAMESPY_BUDDY_QUEUE: OnceLock<Arc<Mutex<GameSpyBuddyMessageQueue>>> = OnceLock::new();

pub fn init_buddy_message_queue() -> Arc<Mutex<GameSpyBuddyMessageQueue>> {
    THE_GAMESPY_BUDDY_QUEUE
        .get_or_init(|| Arc::new(Mutex::new(GameSpyBuddyMessageQueue::default())))
        .clone()
}

pub fn get_buddy_message_queue() -> Option<Arc<Mutex<GameSpyBuddyMessageQueue>>> {
    THE_GAMESPY_BUDDY_QUEUE.get().cloned()
}

pub fn teardown_buddy_message_queue() {
    if let Some(queue) = THE_GAMESPY_BUDDY_QUEUE.get() {
        if let Ok(mut guard) = queue.lock() {
            guard.requests.clear();
            guard.responses.clear();
        }
    }
}
