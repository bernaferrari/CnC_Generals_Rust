//! C++ FirewallHelper TEST1–TEST5 detection (FirewallHelper.cpp).
//!
//! Talks to manglers on port 4321 using the packed ManglerData layout and
//! classifies SIMPLE / DUMB_MANGLING / SMART_MANGLING / NETGEAR_BUG plus
//! port-allocation deltas.

use crate::nat_traversal::{MANGLER_PORT, MANGLER_SERVERS};
use crate::transport_udp::{GENERALS_MAGIC_NUMBER, calculate_packet_crc, xor_decrypt, xor_encrypt};
use parking_lot::Mutex;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

const MAX_MANGLERS: usize = 4;
const TEST_TIMEOUT: Duration = Duration::from_millis(800);

/// C++ `FirewallDetectionState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FirewallDetectionState {
    #[default]
    Idle,
    Begin,
    Test1,
    Test2,
    Test3,
    Test3WaitForResponses,
    Test4Stage1,
    Test4Stage2,
    Test5,
    Done,
}

/// C++ `FirewallHelperClass::FirewallBehaviorType` bit flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FirewallBehavior(pub u16);

impl FirewallBehavior {
    pub const UNKNOWN: Self = Self(0);
    pub const SIMPLE: Self = Self(1);
    pub const DUMB_MANGLING: Self = Self(2);
    pub const SMART_MANGLING: Self = Self(4);
    pub const NETGEAR_BUG: Self = Self(8);
    pub const SIMPLE_PORT_ALLOCATION: Self = Self(16);
    pub const RELATIVE_PORT_ALLOCATION: Self = Self(32);
    pub const DESTINATION_PORT_DELTA: Self = Self(64);

    pub fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }

    pub fn insert(&mut self, flag: Self) {
        self.0 |= flag.0;
    }
}

/// Packed C++ `ManglerData` (16 bytes).
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct ManglerData {
    crc: u32,
    magic: u16,
    packet_id: u16,
    my_mangled_port: u16,
    original_port: u16,
    my_mangled_address: [u8; 4],
    net_command_type: u8,
    blitz_me: u8,
    padding: u16,
}

static SOURCE_PORT_POOL: AtomicU16 = AtomicU16::new(4096 + 256);

#[derive(Default)]
struct Detection {
    state: FirewallDetectionState,
    behavior: FirewallBehavior,
    port_delta: i32,
    packet_id: u16,
    responses: Vec<(SocketAddr, u16, u16)>,
    started: Option<Instant>,
    tries: u32,
}

static DETECTION: Mutex<Detection> = Mutex::new(Detection {
    state: FirewallDetectionState::Idle,
    behavior: FirewallBehavior::UNKNOWN,
    port_delta: 0,
    packet_id: 1,
    responses: Vec::new(),
    started: None,
    tries: 0,
});

/// C++ `TheFirewallHelper` — start TEST1–TEST5.
pub fn detect_firewall() -> bool {
    let mut det = DETECTION.lock();
    det.state = FirewallDetectionState::Begin;
    det.behavior = FirewallBehavior::UNKNOWN;
    det.port_delta = 0;
    det.responses.clear();
    det.started = Some(Instant::now());
    det.tries = 0;
    det.packet_id = det.packet_id.wrapping_add(1);
    true
}

/// C++ `behaviorDetectionUpdate` — advance one detection step per frame.
pub fn behavior_detection_update() -> bool {
    let mut det = DETECTION.lock();
    match det.state {
        FirewallDetectionState::Idle => true,
        FirewallDetectionState::Done => true,
        FirewallDetectionState::Begin => {
            det.state = FirewallDetectionState::Test1;
            false
        }
        FirewallDetectionState::Test1 => run_mangler_round(&mut det, 1),
        FirewallDetectionState::Test2 => run_mangler_round(&mut det, 2),
        FirewallDetectionState::Test3 | FirewallDetectionState::Test3WaitForResponses => {
            run_mangler_round(&mut det, 3)
        }
        FirewallDetectionState::Test4Stage1 | FirewallDetectionState::Test4Stage2 => {
            run_mangler_round(&mut det, 4)
        }
        FirewallDetectionState::Test5 => {
            classify(&mut det);
            det.state = FirewallDetectionState::Done;
            persist(&det);
            true
        }
    }
}

pub fn firewall_detection_done() -> bool {
    DETECTION.lock().state == FirewallDetectionState::Done
}

pub fn get_firewall_behavior() -> FirewallBehavior {
    DETECTION.lock().behavior
}

pub fn get_source_port_allocation_delta() -> i32 {
    DETECTION.lock().port_delta
}

pub fn write_firewall_behavior() {
    let det = DETECTION.lock();
    persist(&det);
}

pub fn read_firewall_behavior() {
    if let Ok(text) = std::fs::read_to_string(behavior_path()) {
        let mut behavior = FirewallBehavior::UNKNOWN;
        let mut delta = 0i32;
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("FirewallBehavior=") {
                behavior = FirewallBehavior(v.trim().parse().unwrap_or(0));
            }
            if let Some(v) = line.strip_prefix("FirewallPortAllocationDelta=") {
                delta = v.trim().parse().unwrap_or(0);
            }
        }
        let mut det = DETECTION.lock();
        if behavior.0 != 0 {
            det.behavior = behavior;
            det.port_delta = delta;
            det.state = FirewallDetectionState::Done;
        }
    }
}

fn behavior_path() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("Options.ini")
}

fn persist(det: &Detection) {
    let path = behavior_path();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut out = String::new();
    let mut wrote_behavior = false;
    let mut wrote_delta = false;
    for line in existing.lines() {
        if line.starts_with("FirewallBehavior=") {
            out.push_str(&format!("FirewallBehavior={}\n", det.behavior.0));
            wrote_behavior = true;
        } else if line.starts_with("FirewallPortAllocationDelta=") {
            out.push_str(&format!("FirewallPortAllocationDelta={}\n", det.port_delta));
            wrote_delta = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !wrote_behavior {
        out.push_str(&format!("FirewallBehavior={}\n", det.behavior.0));
    }
    if !wrote_delta {
        out.push_str(&format!("FirewallPortAllocationDelta={}\n", det.port_delta));
    }
    let _ = std::fs::write(path, out);
}

fn next_source_port() -> u16 {
    SOURCE_PORT_POOL.fetch_add(1, Ordering::Relaxed)
}

fn run_mangler_round(det: &mut Detection, test: u8) -> bool {
    let original = next_source_port();
    let packet_id = det.packet_id;
    let socket = match UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, original))) {
        Ok(s) => s,
        Err(_) => {
            det.state = next_state(test);
            return false;
        }
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(50)));
    for (idx, host) in MANGLER_SERVERS.iter().take(MAX_MANGLERS).enumerate() {
        if let Ok(mut addrs) = format!("{host}:{MANGLER_PORT}").to_socket_addrs_safe() {
            if let Some(addr) = addrs.next() {
                let _ = send_mangler(&socket, addr, packet_id.wrapping_add(idx as u16), original);
            }
        }
    }
    let deadline = Instant::now() + TEST_TIMEOUT;
    let mut buf = [0u8; 64];
    while Instant::now() < deadline {
        if let Ok((len, src)) = socket.recv_from(&mut buf) {
            if let Some((mangled_port, orig)) = parse_mangler(&mut buf[..len]) {
                det.responses.push((src, mangled_port, orig));
            }
        }
    }
    det.state = next_state(test);
    false
}

fn next_state(test: u8) -> FirewallDetectionState {
    match test {
        1 => FirewallDetectionState::Test2,
        2 => FirewallDetectionState::Test3,
        3 => FirewallDetectionState::Test4Stage1,
        4 => FirewallDetectionState::Test5,
        _ => FirewallDetectionState::Done,
    }
}

fn classify(det: &mut Detection) {
    if det.responses.is_empty() {
        det.behavior = FirewallBehavior::SIMPLE;
        det.port_delta = 0;
        return;
    }
    let unique_ports: std::collections::HashSet<u16> =
        det.responses.iter().map(|(_, p, _)| *p).collect();
    if unique_ports.len() <= 1 {
        let (mangled, original) = det
            .responses
            .first()
            .map(|(_, m, o)| (*m, *o))
            .unwrap_or((0, 0));
        if mangled == original {
            det.behavior = FirewallBehavior::SIMPLE;
            det.port_delta = 0;
        } else {
            det.behavior = FirewallBehavior::DUMB_MANGLING;
            det.port_delta = mangled as i32 - original as i32;
            if det.port_delta.abs() > 0 {
                det.behavior
                    .insert(FirewallBehavior::SIMPLE_PORT_ALLOCATION);
            }
        }
    } else {
        det.behavior = FirewallBehavior::SMART_MANGLING;
        let ports: Vec<i32> = det.responses.iter().map(|(_, p, _)| *p as i32).collect();
        if ports.len() >= 2 {
            det.port_delta = ports[1] - ports[0];
            det.behavior
                .insert(FirewallBehavior::RELATIVE_PORT_ALLOCATION);
        }
    }
}

fn send_mangler(
    socket: &UdpSocket,
    dest: SocketAddr,
    packet_id: u16,
    original_port: u16,
) -> std::io::Result<usize> {
    let mut data = ManglerData {
        crc: 0,
        magic: GENERALS_MAGIC_NUMBER,
        packet_id,
        my_mangled_port: 0,
        original_port,
        my_mangled_address: [0; 4],
        net_command_type: 12,
        blitz_me: 0,
        padding: 0,
    };
    let payload = unsafe {
        std::slice::from_raw_parts(
            (&data as *const ManglerData as *const u8).add(4),
            std::mem::size_of::<ManglerData>() - 4,
        )
    };
    data.crc = calculate_packet_crc(GENERALS_MAGIC_NUMBER, &payload[2..]);
    let mut bytes = unsafe {
        std::slice::from_raw_parts(
            &data as *const ManglerData as *const u8,
            std::mem::size_of::<ManglerData>(),
        )
        .to_vec()
    };
    xor_encrypt(&mut bytes);
    socket.send_to(&bytes, dest)
}

fn parse_mangler(buf: &mut [u8]) -> Option<(u16, u16)> {
    if buf.len() < std::mem::size_of::<ManglerData>() {
        return None;
    }
    xor_decrypt(buf);
    let magic = u16::from_ne_bytes([buf[4], buf[5]]);
    if magic != GENERALS_MAGIC_NUMBER {
        return None;
    }
    let mangled = u16::from_ne_bytes([buf[8], buf[9]]);
    let original = u16::from_ne_bytes([buf[10], buf[11]]);
    Some((mangled, original))
}

fn to_socket_addrs_safe(host_port: &str) -> std::io::Result<std::vec::IntoIter<SocketAddr>> {
    use std::net::ToSocketAddrs;
    host_port.to_socket_addrs()
}

trait ToSocketAddrsExt {
    fn to_socket_addrs_safe(&self) -> std::io::Result<std::vec::IntoIter<SocketAddr>>;
}

impl ToSocketAddrsExt for String {
    fn to_socket_addrs_safe(&self) -> std::io::Result<std::vec::IntoIter<SocketAddr>> {
        use std::net::ToSocketAddrs;
        self.to_socket_addrs()
    }
}
