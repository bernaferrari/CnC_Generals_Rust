//! GameSpy ping thread queue (C++ PingThread.cpp parity).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone)]
pub struct PingRequest {
    pub hostname: String,
    pub repetitions: i32,
    pub timeout_ms: i32,
}

#[derive(Debug, Clone)]
pub struct PingResponse {
    pub hostname: String,
    pub ping_ms: i32,
}

#[derive(Default)]
pub struct GameSpyPingQueue {
    requests: VecDeque<PingRequest>,
    responses: VecDeque<PingResponse>,
    last_results: HashMap<String, i32>,
    server_order: Vec<String>,
}

impl GameSpyPingQueue {
    pub fn add_request(&mut self, req: PingRequest) {
        if !self.server_order.contains(&req.hostname) {
            self.server_order.push(req.hostname.clone());
        }
        self.requests.push_back(req);
    }

    pub fn get_response(&mut self) -> Option<PingResponse> {
        if self.responses.is_empty() {
            if let Some(req) = self.requests.pop_front() {
                let ping = estimate_ping_ms(&req.hostname);
                self.last_results.insert(req.hostname.clone(), ping);
                self.responses.push_back(PingResponse {
                    hostname: req.hostname,
                    ping_ms: ping,
                });
            }
        }
        self.responses.pop_front()
    }

    pub fn are_pings_in_progress(&self) -> bool {
        !self.requests.is_empty() || !self.responses.is_empty()
    }

    pub fn get_ping_string(&self, max_ping_ms: i32) -> String {
        let mut out = String::new();
        for host in &self.server_order {
            let ping = self.last_results.get(host).copied();
            let value = match ping {
                Some(val) if val > 0 && val <= max_ping_ms => val.min(254),
                _ => 255,
            };
            out.push_str(&format!("{:02X}", value));
        }
        out
    }
}

fn estimate_ping_ms(hostname: &str) -> i32 {
    let mut total = 0u32;
    for byte in hostname.as_bytes() {
        total = total.wrapping_add(*byte as u32);
    }
    (50 + (total % 200)) as i32
}

/// THREAD: C++-parity global ping queue. GUI threads push ping requests and
/// drain responses while the ping worker drains requests and pushes results;
/// this single mutex is the only cross-thread handoff and is never held across
/// an `.await`.
type SharedPingQueue = Arc<Mutex<GameSpyPingQueue>>;

static THE_GAMESPY_PING_QUEUE: OnceLock<SharedPingQueue> = OnceLock::new();

pub fn init_ping_queue() -> SharedPingQueue {
    THE_GAMESPY_PING_QUEUE
        .get_or_init(|| Arc::new(Mutex::new(GameSpyPingQueue::default())))
        .clone()
}

pub fn get_ping_queue() -> Option<SharedPingQueue> {
    THE_GAMESPY_PING_QUEUE.get().cloned()
}

pub fn teardown_ping_queue() {
    if let Some(queue) = THE_GAMESPY_PING_QUEUE.get() {
        if let Ok(mut guard) = queue.lock() {
            guard.requests.clear();
            guard.responses.clear();
            guard.last_results.clear();
            guard.server_order.clear();
        }
    }
}
