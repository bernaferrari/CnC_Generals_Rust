use crate::debug_cmd::{CommandMode, DebugCmdInterface};
use crate::debug_except::DebugExceptionhandler;
use crate::debug_getdefaultcommands::debug_get_default_commands;
use crate::debug_io::{DebugIOInterface, IOFactory, StringType};
use crate::debug_io_con::DebugIOCon;
use crate::debug_io_flat::DebugIOFlat;
use crate::debug_io_net::DebugIONet;
use crate::debug_io_ods::DebugIOOds;
use once_cell::sync::OnceCell;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrameType {
    Assert,
    Check,
    Log,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugPatternType {
    Assert,
    Check,
    Log,
}

#[derive(Clone, Debug)]
struct PatternEntry {
    frame_type: FrameType,
    active: bool,
    pattern: String,
}

#[derive(Clone, Debug)]
pub struct LogGroupEntry {
    pub name: String,
    pub description: Option<String>,
}

struct IOFactoryEntry {
    id: String,
    description: String,
    factory: IOFactory,
    io: Option<Box<dyn DebugIOInterface>>,
    input: VecDeque<u8>,
}

struct CmdGroupEntry {
    group: String,
    cmdif: Box<dyn DebugCmdInterface>,
}

#[derive(Clone, Debug)]
struct BuildInfo {
    version: String,
    internal_version: String,
    build_date: String,
}

pub struct Debug {
    io_factories: Vec<IOFactoryEntry>,
    cmd_groups: Vec<CmdGroupEntry>,
    log_groups: Vec<LogGroupEntry>,
    patterns: Vec<PatternEntry>,
    output_type: Option<StringType>,
    output_src: Option<String>,
    output_buffer: String,
    always_flush: bool,
    timestamp: bool,
    build_info: Option<BuildInfo>,
    initialized: bool,
}

impl Debug {
    fn new() -> Self {
        Self {
            io_factories: Vec::new(),
            cmd_groups: Vec::new(),
            log_groups: Vec::new(),
            patterns: Vec::new(),
            output_type: None,
            output_src: None,
            output_buffer: String::new(),
            always_flush: false,
            timestamp: false,
            build_info: None,
            initialized: false,
        }
    }

    pub fn instance() -> &'static Mutex<Debug> {
        static INSTANCE: OnceCell<Mutex<Debug>> = OnceCell::new();
        INSTANCE.get_or_init(|| Mutex::new(Debug::new()))
    }

    pub fn add_io_factory(id: &str, description: &str, factory: IOFactory) -> bool {
        let mut dbg = Debug::instance().lock().unwrap();
        dbg.add_io_factory_internal(id, description, factory)
    }

    pub fn add_commands(group: &str, cmdif: Box<dyn DebugCmdInterface>) -> bool {
        let mut dbg = Debug::instance().lock().unwrap();
        dbg.add_commands_internal(group, cmdif)
    }

    pub fn remove_commands(ptr: *const ()) {
        let mut dbg = Debug::instance().lock().unwrap();
        dbg.cmd_groups.retain(|entry| {
            let raw: *const () = (&*entry.cmdif) as *const _ as *const ();
            raw != ptr
        });
    }

    pub fn command(cmd: &str) {
        let mut dbg = Debug::instance().lock().unwrap();
        dbg.ensure_initialized();
        dbg.exec_command(cmd);
    }

    pub fn update() {
        let mut dbg = Debug::instance().lock().unwrap();
        dbg.ensure_initialized();
        dbg.poll_io();
    }

    pub fn set_build_info(version: &str, internal_version: &str, build_date: &str) {
        let mut dbg = Debug::instance().lock().unwrap();
        dbg.build_info = Some(BuildInfo {
            version: version.to_string(),
            internal_version: internal_version.to_string(),
            build_date: build_date.to_string(),
        });
    }

    pub fn write_build_info(&mut self) {
        if let Some(info) = &self.build_info {
            self.write_plain(&format!(
                "Build: {} ({}), {}\n",
                info.version, info.internal_version, info.build_date
            ));
        }
    }

    pub fn log_begin(&mut self, file_or_group: &str) -> bool {
        self.ensure_initialized();
        if !self.is_log_enabled(file_or_group) {
            return false;
        }
        self.start_output(StringType::Log, Some(file_or_group));
        true
    }

    pub fn log_done(&mut self) -> bool {
        self.ensure_initialized();
        if self.output_type == Some(StringType::Log) {
            self.flush_output();
        }
        false
    }

    pub fn assert_begin(&mut self, file: &str, line: i32, expr: &str) -> bool {
        self.ensure_initialized();
        let source = format!("{file}({line})");
        if !self.is_pattern_active(FrameType::Assert, &source) {
            return false;
        }
        self.start_output(StringType::Assert, Some(&source));
        self.write_plain(&format!("Assertion failed: {expr}\n"));
        true
    }

    pub fn assert_done(&mut self) -> bool {
        self.ensure_initialized();
        if self.output_type == Some(StringType::Assert) {
            self.flush_output();
        }
        false
    }

    pub fn check_begin(&mut self, file: &str, line: i32, expr: &str) -> bool {
        self.ensure_initialized();
        let source = format!("{file}({line})");
        if !self.is_pattern_active(FrameType::Check, &source) {
            return false;
        }
        self.start_output(StringType::Check, Some(&source));
        self.write_plain(&format!("Check failed: {expr}\n"));
        true
    }

    pub fn check_done(&mut self) -> bool {
        self.ensure_initialized();
        if self.output_type == Some(StringType::Check) {
            self.flush_output();
        }
        false
    }

    pub fn crash_begin(&mut self, file: Option<&str>, line: Option<i32>) -> bool {
        self.ensure_initialized();
        let source = match (file, line) {
            (Some(file), Some(line)) => Some(format!("{file}({line})")),
            _ => None,
        };
        self.start_output(StringType::Crash, source.as_deref());
        true
    }

    pub fn crash_done(&mut self, die: bool) -> bool {
        self.ensure_initialized();
        if self.output_type == Some(StringType::Crash) {
            self.flush_output();
        }
        if die {
            panic!("Debug crash triggered");
        }
        false
    }

    pub fn write_plain(&mut self, message: &str) {
        self.ensure_initialized();
        self.start_output(StringType::Other, None);
        self.output_buffer.push_str(message);
        self.flush_output();
    }

    fn exec_command(&mut self, cmd: &str) {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return;
        }
        let trimmed = trimmed.strip_prefix('!').unwrap_or(trimmed);
        let mut parts = trimmed.split_whitespace();
        let group_token = parts.next().unwrap_or("");
        let mut cmd_name = parts.next().unwrap_or("");
        let mut argv: Vec<&str> = parts.collect();

        let mut group = group_token;
        if let Some(dot) = group_token.find('.') {
            group = &group_token[..dot];
            let cmd_from_group = &group_token[dot + 1..];
            if !cmd_from_group.is_empty() {
                if !cmd_name.is_empty() {
                    argv.insert(0, cmd_name);
                }
                cmd_name = cmd_from_group;
            }
        }

        let mut cmd_groups = std::mem::take(&mut self.cmd_groups);
        for entry in cmd_groups.iter_mut() {
            if entry.group == group {
                let _ = entry
                    .cmdif
                    .execute(self, cmd_name, CommandMode::Normal, &argv);
            }
        }

        cmd_groups.append(&mut self.cmd_groups);
        self.cmd_groups = cmd_groups;
    }

    fn poll_io(&mut self) {
        let mut buffer = [0u8; 512];
        let mut pending_commands: Vec<String> = Vec::new();

        for entry in self.io_factories.iter_mut() {
            let Some(io) = entry.io.as_mut() else {
                continue;
            };
            let count = io.read(&mut buffer);
            if count > 0 {
                entry.input.extend(buffer[..count].iter().copied());
                while let Some(pos) = entry.input.iter().position(|b| *b == b'\n') {
                    let mut line = entry.input.drain(..=pos).collect::<Vec<_>>();
                    if let Some(last) = line.last() {
                        if *last == b'\n' {
                            line.pop();
                        }
                    }
                    if let Ok(line) = String::from_utf8(line) {
                        pending_commands.push(line);
                    }
                }
            }
        }

        for line in pending_commands {
            self.exec_command(line.trim());
        }
    }

    fn start_output(&mut self, kind: StringType, src: Option<&str>) {
        self.output_type = Some(kind);
        self.output_src = src.map(|s| s.to_string());
        self.output_buffer.clear();
        if self.timestamp {
            if let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) {
                self.output_buffer
                    .push_str(&format!("[{}] ", duration.as_secs()));
            }
        }
    }

    fn flush_output(&mut self) {
        let kind = self.output_type.unwrap_or(StringType::Other);
        let src = self.output_src.clone();
        let message = self.output_buffer.clone();

        for entry in self.io_factories.iter_mut() {
            let Some(io) = entry.io.as_mut() else {
                continue;
            };
            io.write(kind, src.as_deref(), Some(&message));
            if self.always_flush {
                io.write(kind, src.as_deref(), None);
            }
        }
        self.output_buffer.clear();
        self.output_type = None;
        self.output_src = None;
    }

    fn is_log_enabled(&self, group: &str) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        self.patterns
            .iter()
            .filter(|p| p.frame_type == FrameType::Log)
            .rev()
            .find(|p| simple_match(group, &p.pattern))
            .map(|p| p.active)
            .unwrap_or(false)
    }

    fn is_pattern_active(&self, frame_type: FrameType, name: &str) -> bool {
        self.patterns
            .iter()
            .filter(|p| p.frame_type == frame_type)
            .rev()
            .find(|p| simple_match(name, &p.pattern))
            .map(|p| p.active)
            .unwrap_or(true)
    }

    pub fn add_pattern_entry(&mut self, frame_type: FrameType, active: bool, pattern: &str) {
        self.patterns.push(PatternEntry {
            frame_type,
            active,
            pattern: pattern.to_string(),
        });
    }

    pub fn add_log_group(&mut self, name: &str, description: Option<&str>) {
        if self.log_groups.iter().any(|g| g.name == name) {
            return;
        }
        self.log_groups.push(LogGroupEntry {
            name: name.to_string(),
            description: description.map(|d| d.to_string()),
        });
    }

    pub fn list_log_groups(&self) -> Vec<LogGroupEntry> {
        self.log_groups.clone()
    }

    pub fn list_cmd_groups(&self) -> Vec<String> {
        self.cmd_groups.iter().map(|c| c.group.clone()).collect()
    }

    pub fn toggle_always_flush(&mut self, value: bool) {
        self.always_flush = value;
    }

    pub fn toggle_timestamp(&mut self, value: bool) {
        self.timestamp = value;
    }

    pub fn attach_io(&mut self, id: &str) -> bool {
        if let Some(entry) = self.io_factories.iter_mut().find(|io| io.id == id) {
            if entry.io.is_none() {
                entry.io = Some((entry.factory)());
            }
            return true;
        }
        false
    }

    pub fn detach_io(&mut self, id: &str) -> bool {
        if let Some(entry) = self.io_factories.iter_mut().find(|io| io.id == id) {
            if let Some(io) = entry.io.take() {
                io.delete();
            }
            return true;
        }
        false
    }

    pub fn list_io(&self, active_only: bool) -> Vec<(String, String)> {
        self.io_factories
            .iter()
            .filter(|io| !active_only || io.io.is_some())
            .map(|io| (io.id.clone(), io.description.clone()))
            .collect()
    }

    pub fn clear_patterns(&mut self, frame_type: DebugPatternType) {
        let ft = map_pattern_type(frame_type);
        self.patterns.retain(|p| p.frame_type != ft);
    }

    pub fn list_patterns(&self, frame_type: DebugPatternType) -> Vec<(bool, String)> {
        let ft = map_pattern_type(frame_type);
        self.patterns
            .iter()
            .filter(|p| p.frame_type == ft)
            .map(|p| (p.active, p.pattern.clone()))
            .collect()
    }

    pub fn add_pattern(&mut self, frame_type: DebugPatternType, active: bool, pattern: &str) {
        self.add_pattern_entry(map_pattern_type(frame_type), active, pattern);
    }

    pub fn io_execute(&mut self, id: &str, cmd: &str, structured: bool, argv: &[&str]) -> bool {
        let Some(index) = self.io_factories.iter().position(|io| io.id == id) else {
            return false;
        };

        if self.io_factories[index].io.is_none() {
            let factory = self.io_factories[index].factory;
            self.io_factories[index].io = Some(factory());
        }

        let Some(mut io) = self.io_factories[index].io.take() else {
            return false;
        };

        io.execute(self, cmd, structured, argv);

        if let Some(entry) = self
            .io_factories
            .iter_mut()
            .find(|entry| entry.id == id && entry.io.is_none())
        {
            entry.io = Some(io);
        } else {
            io.delete();
        }

        true
    }

    fn ensure_initialized(&mut self) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        self.register_default_factories();
        self.register_default_commands();
        DebugExceptionhandler::install_exception_handler();
        self.exec_default_commands();
    }

    fn register_default_factories(&mut self) {
        if self.io_factories.is_empty() {
            let _ = self.add_io_factory_internal("con", "Console window", DebugIOCon::create);
            let _ = self.add_io_factory_internal("flat", "Flat local file(s)", DebugIOFlat::create);
            let _ =
                self.add_io_factory_internal("net", "Network via named pipe", DebugIONet::create);
            let _ = self.add_io_factory_internal(
                "ods",
                "OutputDebugString function",
                DebugIOOds::create,
            );
        }
    }

    fn register_default_commands(&mut self) {
        if self.cmd_groups.iter().all(|c| c.group != "debug") {
            let _ = self.add_commands_internal(
                "debug",
                Box::new(crate::debug_cmd::DebugCmdInterfaceDebug::new()),
            );
        }
    }

    fn exec_default_commands(&mut self) {
        let commands = debug_get_default_commands();
        for line in commands.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            self.exec_command(trimmed);
        }
    }

    fn add_io_factory_internal(&mut self, id: &str, description: &str, factory: IOFactory) -> bool {
        if self.io_factories.iter().any(|io| io.id == id) {
            return true;
        }
        self.io_factories.push(IOFactoryEntry {
            id: id.to_string(),
            description: description.to_string(),
            factory,
            io: None,
            input: VecDeque::new(),
        });
        true
    }

    fn add_commands_internal(&mut self, group: &str, cmdif: Box<dyn DebugCmdInterface>) -> bool {
        self.cmd_groups.push(CmdGroupEntry {
            group: group.to_string(),
            cmdif,
        });
        true
    }
}

fn map_pattern_type(frame_type: DebugPatternType) -> FrameType {
    match frame_type {
        DebugPatternType::Assert => FrameType::Assert,
        DebugPatternType::Check => FrameType::Check,
        DebugPatternType::Log => FrameType::Log,
    }
}

pub(crate) fn simple_match(name: &str, pattern: &str) -> bool {
    fn inner(name: &[u8], pattern: &[u8]) -> bool {
        if pattern.is_empty() {
            return name.is_empty();
        }
        if pattern[0] == b'*' {
            if pattern.len() == 1 {
                return true;
            }
            for idx in 0..=name.len() {
                if inner(&name[idx..], &pattern[1..]) {
                    return true;
                }
            }
            return false;
        }
        if name.is_empty() {
            return false;
        }
        if pattern[0] == name[0] {
            return inner(&name[1..], &pattern[1..]);
        }
        false
    }

    inner(name.as_bytes(), pattern.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_match() {
        assert!(simple_match("debug.cpp", "debug.cpp"));
        assert!(simple_match("debug.cpp", "debug*"));
        assert!(simple_match("debug.cpp", "*"));
        assert!(!simple_match("debug.cpp", "release*"));
    }
}
