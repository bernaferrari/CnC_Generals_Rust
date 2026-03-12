////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// CRCDebug.rs ///////////////////////////////////////////////////////////////
// Macros/functions/etc to help logging values for tracking down sync errors
// Author: Matthew D. Campbell, June 2002

#[cfg(feature = "debug_crc")]
use once_cell::sync::OnceCell;
#[cfg(feature = "debug_crc")]
use parking_lot::{Mutex, RwLock};
#[cfg(feature = "debug_crc")]
use std::env;
#[cfg(feature = "debug_crc")]
use std::fs::File;
#[cfg(feature = "debug_crc")]
use std::io::Write;
#[cfg(not(feature = "debug_crc"))]
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
#[cfg(feature = "debug_crc")]
use std::sync::Arc;
#[cfg(feature = "debug_crc")]
use tracing::field::{Field, Visit};
#[cfg(feature = "debug_crc")]
use tracing::Subscriber;
#[cfg(feature = "debug_crc")]
use tracing::{event, Level};
#[cfg(feature = "debug_crc")]
use tracing_subscriber::layer::{Context, Layer};
#[cfg(feature = "debug_crc")]
use tracing_subscriber::registry::LookupSpan;

#[cfg(feature = "debug_crc")]
use crate::common::command_line::CrcDebugSettings;
use crate::common::time;

/// Maximum number of debug strings to store
const MAX_STRINGS: usize = 64000;

#[cfg(feature = "debug_crc")]
#[derive(Debug, Clone)]
pub struct CrcDebugConfig {
    pub enabled: bool,
    pub debug_ignore_sync_errors: bool,
    pub first_frame_to_log: i32,
    pub last_frame_to_log: u32,
    pub keep_crc_saves: bool,
    pub crc_module_data_from_logic: bool,
    pub crc_module_data_from_client: bool,
    pub verify_client_crc: bool,
    pub client_deep_crc: bool,
    pub log_object_crcs: bool,
    pub net_crc_interval: i32,
    pub replay_crc_interval: i32,
}

#[cfg(feature = "debug_crc")]
impl Default for CrcDebugConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            debug_ignore_sync_errors: false,
            first_frame_to_log: -1,
            last_frame_to_log: 0xffffffff,
            keep_crc_saves: false,
            crc_module_data_from_logic: false,
            crc_module_data_from_client: false,
            verify_client_crc: false,
            client_deep_crc: false,
            log_object_crcs: false,
            net_crc_interval: 1,
            replay_crc_interval: 1,
        }
    }
}

#[cfg(feature = "debug_crc")]
#[derive(Debug)]
struct CrcDebugBuffer {
    debug_strings: Vec<String>,
    next_debug_string: usize,
    num_debug_strings: usize,
    dumped: bool,
    last_crc_debug_frame: i32,
    last_crc_debug_index: i32,
}

#[cfg(feature = "debug_crc")]
impl Default for CrcDebugBuffer {
    fn default() -> Self {
        Self {
            debug_strings: vec![String::new(); MAX_STRINGS],
            next_debug_string: 0,
            num_debug_strings: 0,
            dumped: false,
            last_crc_debug_frame: 0,
            last_crc_debug_index: 0,
        }
    }
}

#[cfg(feature = "debug_crc")]
#[derive(Debug)]
struct CrcDebugState {
    config: Arc<RwLock<CrcDebugConfig>>,
    buffer: Mutex<CrcDebugBuffer>,
}

#[cfg(feature = "debug_crc")]
impl CrcDebugState {
    fn new() -> Self {
        let mut config = CrcDebugConfig::default();
        config.apply_env_overrides();
        Self {
            config: Arc::new(RwLock::new(config)),
            buffer: Mutex::new(CrcDebugBuffer::default()),
        }
    }

    fn config(&self) -> &Arc<RwLock<CrcDebugConfig>> {
        &self.config
    }

    fn buffer(&self) -> &Mutex<CrcDebugBuffer> {
        &self.buffer
    }

    fn update_config<F>(&self, mut update: F)
    where
        F: FnMut(&mut CrcDebugConfig),
    {
        let mut guard = self.config.write();
        let before = guard.clone();
        update(&mut guard);
        let after = guard.clone();
        drop(guard);
        self.after_config_change(&before, &after);
    }

    fn set_config(&self, new_config: CrcDebugConfig) {
        let mut guard = self.config.write();
        let before = guard.clone();
        *guard = new_config.clone();
        drop(guard);
        self.after_config_change(&before, &new_config);
    }

    fn after_config_change(&self, before: &CrcDebugConfig, after: &CrcDebugConfig) {
        if before.enabled != after.enabled {
            let mut buffer = self.buffer.lock();
            buffer.reset_for_enable(after.enabled);
        }

        event!(
            target: "crc_debug",
            Level::INFO,
            enabled = after.enabled,
            first_frame_to_log = after.first_frame_to_log,
            last_frame_to_log = after.last_frame_to_log,
            keep_crc_saves = after.keep_crc_saves,
            crc_module_data_from_logic = after.crc_module_data_from_logic,
            crc_module_data_from_client = after.crc_module_data_from_client,
            verify_client_crc = after.verify_client_crc,
            client_deep_crc = after.client_deep_crc,
            log_object_crcs = after.log_object_crcs,
            net_crc_interval = after.net_crc_interval,
            replay_crc_interval = after.replay_crc_interval,
            "crc_debug_config_updated"
        );
    }
}

#[cfg(feature = "debug_crc")]
impl CrcDebugConfig {
    fn apply_env_overrides(&mut self) {
        if let Ok(spec) = env::var("GENRUST_CRC_DEBUG") {
            for token in spec.split(',') {
                let entry = token.trim();
                if entry.is_empty() {
                    continue;
                }
                let mut parts = entry.splitn(2, '=');
                let key = parts
                    .next()
                    .map(|k| k.trim().to_ascii_lowercase())
                    .unwrap_or_default();
                let value = parts.next().map(|v| v.trim()).unwrap_or("true");

                match key.as_str() {
                    "enabled" => self.enabled = parse_bool(value).unwrap_or(self.enabled),
                    "ignore_sync_errors" => {
                        self.debug_ignore_sync_errors =
                            parse_bool(value).unwrap_or(self.debug_ignore_sync_errors)
                    }
                    "first_frame" => {
                        if let Ok(v) = value.parse::<i32>() {
                            self.first_frame_to_log = v;
                        }
                    }
                    "last_frame" => {
                        if let Ok(v) = value.parse::<u32>() {
                            self.last_frame_to_log = v;
                        }
                    }
                    "keep_crc_saves" => {
                        self.keep_crc_saves = parse_bool(value).unwrap_or(self.keep_crc_saves)
                    }
                    "module_logic" => {
                        self.crc_module_data_from_logic =
                            parse_bool(value).unwrap_or(self.crc_module_data_from_logic)
                    }
                    "module_client" => {
                        self.crc_module_data_from_client =
                            parse_bool(value).unwrap_or(self.crc_module_data_from_client)
                    }
                    "verify_client" => {
                        self.verify_client_crc = parse_bool(value).unwrap_or(self.verify_client_crc)
                    }
                    "deep_crc" => {
                        self.client_deep_crc = parse_bool(value).unwrap_or(self.client_deep_crc)
                    }
                    "log_object_crcs" => {
                        self.log_object_crcs = parse_bool(value).unwrap_or(self.log_object_crcs)
                    }
                    "net_interval" => {
                        if let Ok(v) = value.parse::<i32>() {
                            self.net_crc_interval = v;
                        }
                    }
                    "replay_interval" => {
                        if let Ok(v) = value.parse::<i32>() {
                            self.replay_crc_interval = v;
                        }
                    }
                    _ => {
                        event!(
                            target: "crc_debug",
                            Level::WARN,
                            entry = entry,
                            "unknown GENRUST_CRC_DEBUG override"
                        );
                    }
                }
            }

            if self.first_frame_to_log >= 0 {
                self.enabled = true;
            }
        }
    }
}

#[cfg(feature = "debug_crc")]
fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(feature = "debug_crc")]
impl CrcDebugBuffer {
    fn reset_for_enable(&mut self, enabled: bool) {
        if enabled {
            self.clear_history();
        } else {
            self.dumped = false;
        }
    }

    fn clear_history(&mut self) {
        for entry in &mut self.debug_strings {
            entry.clear();
        }
        self.next_debug_string = 0;
        self.num_debug_strings = 0;
        self.dumped = false;
        self.last_crc_debug_frame = 0;
        self.last_crc_debug_index = 0;
    }
}

#[cfg(feature = "debug_crc")]
fn crc_state() -> &'static Arc<CrcDebugState> {
    static CRC_DEBUG_STATE: OnceCell<Arc<CrcDebugState>> = OnceCell::new();
    CRC_DEBUG_STATE.get_or_init(|| Arc::new(CrcDebugState::new()))
}

#[cfg(feature = "debug_crc")]
pub fn current_crc_debug_config() -> CrcDebugConfig {
    crc_state().config().read().clone()
}

#[cfg(feature = "debug_crc")]
pub fn set_crc_debug_config(config: CrcDebugConfig) {
    crc_state().set_config(config);
}

#[cfg(feature = "debug_crc")]
pub fn apply_command_line_settings(settings: &CrcDebugSettings) {
    crc_state().update_config(|config| {
        config.first_frame_to_log = settings.first_frame_to_log;
        config.last_frame_to_log = settings.last_frame_to_log;
        config.keep_crc_saves = settings.keep_crc_saves;
        config.crc_module_data_from_logic = settings.crc_module_data_from_logic;
        config.crc_module_data_from_client = settings.crc_module_data_from_client;
        config.verify_client_crc = settings.verify_client_crc;
        config.client_deep_crc = settings.client_deep_crc;
        config.log_object_crcs = settings.log_object_crcs;
        config.net_crc_interval = settings.net_crc_interval;
        config.replay_crc_interval = settings.replay_crc_interval;
        config.enabled = settings.first_frame_to_log >= 0;
    });
}

#[cfg(feature = "debug_crc")]
#[derive(Default, Clone)]
struct CrcDebugConfigUpdate {
    enabled: Option<bool>,
    debug_ignore_sync_errors: Option<bool>,
    first_frame_to_log: Option<i32>,
    last_frame_to_log: Option<u32>,
    keep_crc_saves: Option<bool>,
    crc_module_data_from_logic: Option<bool>,
    crc_module_data_from_client: Option<bool>,
    verify_client_crc: Option<bool>,
    client_deep_crc: Option<bool>,
    log_object_crcs: Option<bool>,
    net_crc_interval: Option<i32>,
    replay_crc_interval: Option<i32>,
}

#[cfg(feature = "debug_crc")]
impl CrcDebugConfigUpdate {
    fn apply(&self, config: &mut CrcDebugConfig) {
        if let Some(value) = self.enabled {
            config.enabled = value;
        }
        if let Some(value) = self.debug_ignore_sync_errors {
            config.debug_ignore_sync_errors = value;
        }
        if let Some(value) = self.first_frame_to_log {
            config.first_frame_to_log = value;
        }
        if let Some(value) = self.last_frame_to_log {
            config.last_frame_to_log = value;
        }
        if let Some(value) = self.keep_crc_saves {
            config.keep_crc_saves = value;
        }
        if let Some(value) = self.crc_module_data_from_logic {
            config.crc_module_data_from_logic = value;
        }
        if let Some(value) = self.crc_module_data_from_client {
            config.crc_module_data_from_client = value;
        }
        if let Some(value) = self.verify_client_crc {
            config.verify_client_crc = value;
        }
        if let Some(value) = self.client_deep_crc {
            config.client_deep_crc = value;
        }
        if let Some(value) = self.log_object_crcs {
            config.log_object_crcs = value;
        }
        if let Some(value) = self.net_crc_interval {
            config.net_crc_interval = value;
        }
        if let Some(value) = self.replay_crc_interval {
            config.replay_crc_interval = value;
        }
    }

    fn is_empty(&self) -> bool {
        self.enabled.is_none()
            && self.debug_ignore_sync_errors.is_none()
            && self.first_frame_to_log.is_none()
            && self.last_frame_to_log.is_none()
            && self.keep_crc_saves.is_none()
            && self.crc_module_data_from_logic.is_none()
            && self.crc_module_data_from_client.is_none()
            && self.verify_client_crc.is_none()
            && self.client_deep_crc.is_none()
            && self.log_object_crcs.is_none()
            && self.net_crc_interval.is_none()
            && self.replay_crc_interval.is_none()
    }
}

#[cfg(feature = "debug_crc")]
#[derive(Default)]
struct ConfigVisitor {
    update: CrcDebugConfigUpdate,
}

#[cfg(feature = "debug_crc")]
impl Visit for ConfigVisitor {
    fn record_bool(&mut self, field: &Field, value: bool) {
        match field.name() {
            "enabled" => self.update.enabled = Some(value),
            "ignore_sync_errors" => self.update.debug_ignore_sync_errors = Some(value),
            "keep_crc_saves" => self.update.keep_crc_saves = Some(value),
            "crc_module_data_from_logic" => self.update.crc_module_data_from_logic = Some(value),
            "crc_module_data_from_client" => self.update.crc_module_data_from_client = Some(value),
            "verify_client_crc" => self.update.verify_client_crc = Some(value),
            "client_deep_crc" => self.update.client_deep_crc = Some(value),
            "log_object_crcs" => self.update.log_object_crcs = Some(value),
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        match field.name() {
            "first_frame_to_log" | "first_frame" => {
                self.update.first_frame_to_log = Some(value as i32)
            }
            "last_frame_to_log" | "last_frame" => {
                self.update.last_frame_to_log = Some(value as u32)
            }
            "net_crc_interval" | "net_interval" => {
                self.update.net_crc_interval = Some(value as i32)
            }
            "replay_crc_interval" | "replay_interval" => {
                self.update.replay_crc_interval = Some(value as i32)
            }
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "last_frame_to_log" | "last_frame" => {
                self.update.last_frame_to_log = Some(value as u32)
            }
            _ => {}
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if let Some(bool_value) = parse_bool(value) {
            self.record_bool(field, bool_value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let rendered = format!("{value:?}");
        self.record_str(field, &rendered);
    }
}

#[cfg(feature = "debug_crc")]
#[derive(Default)]
pub struct CrcDebugLayer;

#[cfg(feature = "debug_crc")]
impl<S> Layer<S> for CrcDebugLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() != "crc_debug::config" {
            return;
        }

        let mut visitor = ConfigVisitor::default();
        event.record(&mut visitor);
        let update = visitor.update;
        if update.is_empty() {
            return;
        }

        let update_to_apply = update.clone();
        crc_state().update_config(move |config| {
            update_to_apply.apply(config);
            if config.first_frame_to_log >= 0 {
                config.enabled = true;
            }
        });
    }
}

#[cfg(feature = "debug_crc")]
pub fn crc_debug_layer() -> CrcDebugLayer {
    CrcDebugLayer::default()
}

/// 3D Vector structure
#[derive(Debug, Clone, Copy)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// 3D Coordinate structure
#[derive(Debug, Clone, Copy)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// 3D Matrix structure
#[derive(Debug, Clone, Copy)]
pub struct Matrix3D {
    pub data: [[f32; 4]; 3],
}

/// CRC Verification guard - equivalent to the C++ RAII class
#[cfg(feature = "debug_crc")]
pub struct CrcVerification {
    start_crc: u32,
}

#[cfg(feature = "debug_crc")]
impl CrcVerification {
    pub fn new() -> Self {
        let start_crc = if is_frame_ok_to_log() {
            // Would call TheGameLogic->getCRC() here
            get_game_logic_crc(true)
        } else {
            0
        };

        Self { start_crc }
    }
}

#[cfg(feature = "debug_crc")]
impl Drop for CrcVerification {
    fn drop(&mut self) {
        if is_frame_ok_to_log() {
            let end_crc = get_game_logic_crc(true);

            if self.start_crc != end_crc {
                if is_in_multiplayer_game() {
                    // Would show UI message here
                    println!("GameLogic changed outside of GameLogic::update() - call Matt!");
                }
                add_crc_debug_line(&format!(
                    "GameLogic changed outside of GameLogic::update()!!!"
                ));
            }
        }
    }
}

/// Non-debug version of CRC verification (does nothing)
#[cfg(not(feature = "debug_crc"))]
pub struct CrcVerification;

#[cfg(not(feature = "debug_crc"))]
impl CrcVerification {
    pub fn new() -> Self {
        Self
    }
}

/// Check if current frame is ok to log
#[cfg(feature = "debug_crc")]
fn is_frame_ok_to_log() -> bool {
    let state = crc_state();
    let config = state.config().read();

    if !config.enabled {
        return false;
    }

    let current_frame = get_current_frame();

    is_in_game()
        && !is_in_shell_game()
        && !config.debug_ignore_sync_errors
        && config.first_frame_to_log >= 0
        && config.first_frame_to_log <= current_frame
        && current_frame <= config.last_frame_to_log as i32
}

/// Placeholder functions that would be implemented elsewhere
fn is_in_game() -> bool {
    // Would check TheGameLogic->isInGame()
    true
}

fn is_in_shell_game() -> bool {
    // Would check TheGameLogic->isInShellGame()
    false
}

fn is_in_multiplayer_game() -> bool {
    // Would check TheGameLogic->isInMultiplayerGame()
    false
}

fn get_current_frame() -> i32 {
    time::frame() as i32
}

fn get_game_logic_crc(_deep: bool) -> u32 {
    // Would call TheGameLogic->getCRC()
    0
}

fn get_machine_name() -> String {
    // Would get machine name from IP enumeration
    "localhost".to_string()
}

/// Output CRC debug lines to file
#[cfg(feature = "debug_crc")]
pub fn output_crc_debug_lines() {
    let state = crc_state();
    let mut buffer = state.buffer().lock();

    if buffer.dumped {
        return;
    }
    buffer.dumped = true;

    let machine_name = get_machine_name();
    let filename = format!("crcDebug{}.txt", machine_name);

    if let Ok(mut file) = File::create(&filename) {
        let start = if buffer.num_debug_strings >= MAX_STRINGS {
            buffer.next_debug_string.wrapping_sub(MAX_STRINGS)
        } else {
            0
        };
        let end = buffer.next_debug_string;

        for i in start..end {
            let index = (i + MAX_STRINGS) % MAX_STRINGS;
            let line = &buffer.debug_strings[index];
            println!("{}", line);
            writeln!(file, "{}", line).ok();
        }
    }
}

/// Output CRC dump lines (placeholder)
#[cfg(feature = "debug_crc")]
pub fn output_crc_dump_lines() {
    // This was commented out in the original C++ code
}

/// Get filename from full path
fn get_fname(path: &str) -> &str {
    if let Some(pos) = path.rfind('\\') {
        &path[pos + 1..]
    } else {
        path
    }
}

/// Add CRC debug line
#[cfg(feature = "debug_crc")]
pub fn add_crc_debug_line(message: &str) {
    let state = crc_state();

    if !is_frame_ok_to_log() {
        return;
    }

    let mut buffer = state.buffer().lock();

    if buffer.dumped {
        return;
    }

    let current_frame = get_current_frame();
    if buffer.last_crc_debug_frame != current_frame {
        buffer.last_crc_debug_frame = current_frame;
        buffer.last_crc_debug_index = 0;
    }

    let formatted_message = format!(
        "{}:{} {}",
        current_frame, buffer.last_crc_debug_index, message
    );
    buffer.last_crc_debug_index += 1;

    // Clean up newlines and carriage returns
    let cleaned_message = formatted_message.replace('\r', " ").replace('\n', " ");

    let index = buffer.next_debug_string;
    buffer.debug_strings[index] = cleaned_message;
    buffer.next_debug_string = (index + 1) % MAX_STRINGS;
    buffer.num_debug_strings = buffer.num_debug_strings.saturating_add(1);
}

/// Add CRC generation line
#[cfg(feature = "debug_crc")]
pub fn add_crc_gen_line(message: &str) {
    if !is_frame_ok_to_log() {
        return;
    }

    add_crc_debug_line(message);
}

/// Add CRC dump line (placeholder)
#[cfg(feature = "debug_crc")]
pub fn add_crc_dump_line(_message: &str) {
    // This was commented out in the original C++ code
}

/// Dump Vector3 for CRC debugging
#[cfg(feature = "debug_crc")]
pub fn dump_vector3(v: &Vector3, name: &str, fname: &str, line: i32) {
    if !is_frame_ok_to_log() {
        return;
    }

    let fname_lower = get_fname(fname).to_lowercase();
    let message = format!(
        "dumpVector3() {}:{} {} {:08X} {:08X} {:08X}",
        fname_lower,
        line,
        name,
        v.x.to_bits(),
        v.y.to_bits(),
        v.z.to_bits()
    );
    add_crc_debug_line(&message);
}

/// Dump Coord3D for CRC debugging
#[cfg(feature = "debug_crc")]
pub fn dump_coord3d(c: &Coord3D, name: &str, fname: &str, line: i32) {
    if !is_frame_ok_to_log() {
        return;
    }

    let fname_lower = get_fname(fname).to_lowercase();
    let message = format!(
        "dumpCoord3D() {}:{} {} {:08X} {:08X} {:08X}",
        fname_lower,
        line,
        name,
        c.x.to_bits(),
        c.y.to_bits(),
        c.z.to_bits()
    );
    add_crc_debug_line(&message);
}

/// Dump Matrix3D for CRC debugging
#[cfg(feature = "debug_crc")]
pub fn dump_matrix3d(m: &Matrix3D, name: &str, fname: &str, line: i32) {
    if !is_frame_ok_to_log() {
        return;
    }

    let fname_lower = get_fname(fname).to_lowercase();
    let message = format!("dumpMatrix3D() {}:{} {}", fname_lower, line, name);
    add_crc_debug_line(&message);

    for i in 0..3 {
        let row_message = format!(
            "      0x{:08X} 0x{:08X} 0x{:08X} 0x{:08X}",
            m.data[i][0].to_bits(),
            m.data[i][1].to_bits(),
            m.data[i][2].to_bits(),
            m.data[i][3].to_bits()
        );
        add_crc_debug_line(&row_message);
    }
}

/// Dump Real (float) for CRC debugging
#[cfg(feature = "debug_crc")]
pub fn dump_real(r: f32, name: &str, fname: &str, line: i32) {
    if !is_frame_ok_to_log() {
        return;
    }

    let fname_lower = get_fname(fname).to_lowercase();
    let message = format!(
        "dumpReal() {}:{} {} {:08X} ({})",
        fname_lower,
        line,
        name,
        r.to_bits(),
        r
    );
    add_crc_debug_line(&message);
}

/// No-op versions for when debug_crc feature is disabled
#[cfg(not(feature = "debug_crc"))]
pub fn output_crc_debug_lines() {}

#[cfg(not(feature = "debug_crc"))]
pub fn output_crc_dump_lines() {}

#[cfg(not(feature = "debug_crc"))]
pub fn add_crc_debug_line(_message: &str) {}

#[cfg(not(feature = "debug_crc"))]
pub fn add_crc_gen_line(_message: &str) {}

#[cfg(not(feature = "debug_crc"))]
pub fn add_crc_dump_line(_message: &str) {}

#[cfg(not(feature = "debug_crc"))]
pub fn dump_vector3(_v: &Vector3, _name: &str, _fname: &str, _line: i32) {}

#[cfg(not(feature = "debug_crc"))]
pub fn dump_coord3d(_c: &Coord3D, _name: &str, _fname: &str, _line: i32) {}

#[cfg(not(feature = "debug_crc"))]
pub fn dump_matrix3d(_m: &Matrix3D, _name: &str, _fname: &str, _line: i32) {}

#[cfg(not(feature = "debug_crc"))]
pub fn dump_real(_r: f32, _name: &str, _fname: &str, _line: i32) {}

/// Macros for debug logging (equivalent to C++ macros)
#[macro_export]
macro_rules! crcdebug_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "debug_crc")]
        {
            let message = format!($($arg)*);
            crate::common::crc_debug::add_crc_debug_line(&message);
        }
    };
}

#[macro_export]
macro_rules! crcgen_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "debug_crc")]
        {
            let message = format!($($arg)*);
            crate::common::crc_debug::add_crc_gen_line(&message);
        }
    };
}

#[macro_export]
macro_rules! crcdump_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "debug_crc")]
        {
            let message = format!($($arg)*);
            crate::common::crc_debug::add_crc_dump_line(&message);
        }
    };
}

/// Macro for CRC verification guard
#[macro_export]
macro_rules! verify_crc {
    () => {
        let _crc_verification = crate::common::crc_debug::CrcVerification::new();
    };
}

/// Dump macros for vectors, coordinates, etc.
#[macro_export]
macro_rules! dump_vector3 {
    ($v:expr) => {
        #[cfg(feature = "debug_crc")]
        crate::common::crc_debug::dump_vector3($v, stringify!($v), file!(), line!() as i32);
    };
}

#[macro_export]
macro_rules! dump_vector3_named {
    ($v:expr, $name:expr) => {
        #[cfg(feature = "debug_crc")]
        crate::common::crc_debug::dump_vector3($v, $name, file!(), line!() as i32);
    };
}

#[macro_export]
macro_rules! dump_coord3d {
    ($c:expr) => {
        #[cfg(feature = "debug_crc")]
        crate::common::crc_debug::dump_coord3d($c, stringify!($c), file!(), line!() as i32);
    };
}

#[macro_export]
macro_rules! dump_coord3d_named {
    ($c:expr, $name:expr) => {
        #[cfg(feature = "debug_crc")]
        crate::common::crc_debug::dump_coord3d($c, $name, file!(), line!() as i32);
    };
}

#[macro_export]
macro_rules! dump_matrix3d {
    ($m:expr) => {
        #[cfg(feature = "debug_crc")]
        crate::common::crc_debug::dump_matrix3d($m, stringify!($m), file!(), line!() as i32);
    };
}

#[macro_export]
macro_rules! dump_matrix3d_named {
    ($m:expr, $name:expr) => {
        #[cfg(feature = "debug_crc")]
        crate::common::crc_debug::dump_matrix3d($m, $name, file!(), line!() as i32);
    };
}

#[macro_export]
macro_rules! dump_real {
    ($r:expr) => {
        #[cfg(feature = "debug_crc")]
        crate::common::crc_debug::dump_real($r, stringify!($r), file!(), line!() as i32);
    };
}

#[macro_export]
macro_rules! dump_real_named {
    ($r:expr, $name:expr) => {
        #[cfg(feature = "debug_crc")]
        crate::common::crc_debug::dump_real($r, $name, file!(), line!() as i32);
    };
}

/// Global debug flags for builds without the debug_crc feature
#[cfg(not(feature = "debug_crc"))]
#[allow(dead_code)]
pub static DEBUG_IGNORE_SYNC_ERRORS: AtomicBool = AtomicBool::new(false);

/// Network and replay CRC intervals used when debug CRC support is disabled
#[cfg(not(feature = "debug_crc"))]
#[allow(dead_code)]
pub static NET_CRC_INTERVAL: AtomicI32 = AtomicI32::new(1);
#[cfg(not(feature = "debug_crc"))]
#[allow(dead_code)]
pub static REPLAY_CRC_INTERVAL: AtomicI32 = AtomicI32::new(1);

#[cfg(feature = "debug_crc")]
pub fn set_debug_ignore_sync_errors(enabled: bool) {
    crc_state().update_config(|config| {
        config.debug_ignore_sync_errors = enabled;
    });
}

#[cfg(not(feature = "debug_crc"))]
#[allow(dead_code)]
pub fn set_debug_ignore_sync_errors(enabled: bool) {
    DEBUG_IGNORE_SYNC_ERRORS.store(enabled, Ordering::Relaxed);
}

#[cfg(feature = "debug_crc")]
pub fn debug_ignore_sync_errors() -> bool {
    crc_state().config().read().debug_ignore_sync_errors
}

#[cfg(not(feature = "debug_crc"))]
#[allow(dead_code)]
pub fn debug_ignore_sync_errors() -> bool {
    DEBUG_IGNORE_SYNC_ERRORS.load(Ordering::Relaxed)
}

#[cfg(feature = "debug_crc")]
pub fn set_net_crc_interval(interval: i32) {
    crc_state().update_config(|config| {
        config.net_crc_interval = interval;
    });
}

#[cfg(not(feature = "debug_crc"))]
#[allow(dead_code)]
pub fn set_net_crc_interval(interval: i32) {
    NET_CRC_INTERVAL.store(interval, Ordering::Relaxed);
}

#[cfg(feature = "debug_crc")]
pub fn net_crc_interval() -> i32 {
    crc_state().config().read().net_crc_interval
}

#[cfg(not(feature = "debug_crc"))]
#[allow(dead_code)]
pub fn net_crc_interval() -> i32 {
    NET_CRC_INTERVAL.load(Ordering::Relaxed)
}

#[cfg(feature = "debug_crc")]
pub fn set_replay_crc_interval(interval: i32) {
    crc_state().update_config(|config| {
        config.replay_crc_interval = interval;
    });
}

#[cfg(not(feature = "debug_crc"))]
#[allow(dead_code)]
pub fn set_replay_crc_interval(interval: i32) {
    REPLAY_CRC_INTERVAL.store(interval, Ordering::Relaxed);
}

#[cfg(feature = "debug_crc")]
pub fn replay_crc_interval() -> i32 {
    crc_state().config().read().replay_crc_interval
}

#[cfg(not(feature = "debug_crc"))]
#[allow(dead_code)]
pub fn replay_crc_interval() -> i32 {
    REPLAY_CRC_INTERVAL.load(Ordering::Relaxed)
}
