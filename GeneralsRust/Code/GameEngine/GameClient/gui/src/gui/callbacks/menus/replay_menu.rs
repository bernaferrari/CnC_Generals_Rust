use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/ReplayMenu.cpp",
    "crate::gui::callbacks::menus::replay_menu",
    "Replay Menu",
    "Replay-browser callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "ReplayMenu",
    "Replay Menu",
    "Browse and launch saved replays.",
    "Shell",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayKindPort {
    SinglePlayer,
    Multiplayer,
}

impl ReplayKindPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::SinglePlayer => "SP",
            Self::Multiplayer => "MP",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayEntryPort {
    pub replay_name: String,
    pub replay_filename: String,
    pub display_time: String,
    pub version: String,
    pub map_name: String,
    pub replay_kind: ReplayKindPort,
    pub version_is_compatible: bool,
    pub requires_version_confirmation: bool,
    pub is_last_replay: bool,
}

impl ReplayEntryPort {
    pub fn display_label(&self) -> String {
        let color_hint = if self.version_is_compatible {
            "OK"
        } else {
            "CRC mismatch"
        };
        format!(
            "{} [{}] {} · {} · {}",
            self.replay_name,
            self.replay_kind.label(),
            self.display_time,
            self.map_name,
            color_hint
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayPromptPort {
    NoSelection {
        title: String,
        body: String,
    },
    OlderVersion {
        title: String,
        body: String,
        filename: String,
    },
    DeleteConfirm {
        title: String,
        body: String,
        filename: String,
    },
    CopyConfirm {
        title: String,
        body: String,
        filename: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplayFileError {
    NoSelection,
    FileNotFound(PathBuf),
    DeleteFailed(PathBuf, String),
    CopyFailed(PathBuf, String),
    DesktopNotFound,
}

impl std::fmt::Display for ReplayFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSelection => write!(f, "No replay selected"),
            Self::FileNotFound(p) => write!(f, "File not found: {}", p.display()),
            Self::DeleteFailed(p, e) => write!(f, "Error deleting {}: {}", p.display(), e),
            Self::CopyFailed(p, e) => write!(f, "Error copying {}: {}", p.display(), e),
            Self::DesktopNotFound => write!(f, "Desktop directory not found"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayMenuPort {
    pub shell_map_visible: bool,
    pub visible: bool,
    pub is_shutting_down: bool,
    pub initial_gadget_delay: u16,
    pub just_entered: bool,
    pub gadget_parent_hidden: bool,
    pub wants_input_focus: bool,
    pub entries: Vec<ReplayEntryPort>,
    pub selected_index: Option<usize>,
    pub pending_prompt: Option<ReplayPromptPort>,
    pub call_copy: bool,
    pub call_delete: bool,
    pub loaded_replay: Option<String>,
    pub copied_replay: Option<String>,
    pub deleted_replay: Option<String>,
    pub back_requested: bool,
    pub active_transition_group: Option<String>,
    pub reverse_transition_group: Option<String>,
}

impl Default for ReplayMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ReplayMenuPort {
    pub fn init(entries: Vec<ReplayEntryPort>) -> Self {
        let selected_index = (!entries.is_empty()).then_some(0);
        Self {
            shell_map_visible: true,
            visible: true,
            is_shutting_down: false,
            initial_gadget_delay: 2,
            just_entered: true,
            gadget_parent_hidden: true,
            wants_input_focus: true,
            entries,
            selected_index,
            pending_prompt: None,
            call_copy: false,
            call_delete: false,
            loaded_replay: None,
            copied_replay: None,
            deleted_replay: None,
            back_requested: false,
            active_transition_group: None,
            reverse_transition_group: None,
        }
    }

    pub fn update(
        &mut self,
        shell_anim_finished: bool,
        transition_finished: bool,
        replay_dir: &Path,
    ) -> bool {
        if self.just_entered {
            if self.initial_gadget_delay == 1 {
                self.active_transition_group = Some("ReplayMenuFade".to_string());
                self.initial_gadget_delay = 2;
                self.just_entered = false;
            } else {
                self.initial_gadget_delay = self.initial_gadget_delay.saturating_sub(1);
            }
        }

        if self.call_copy {
            let _ = self.copy_selected(replay_dir);
        }
        if self.call_delete {
            let _ = self.delete_selected(replay_dir);
        }

        if self.is_shutting_down && shell_anim_finished && transition_finished {
            self.is_shutting_down = false;
            self.visible = false;
            return true;
        }

        false
    }

    pub fn shutdown(&mut self, pop_immediate: bool) -> bool {
        if pop_immediate {
            self.visible = false;
            return true;
        }

        self.reverse_transition_group = Some("ReplayMenuFade".to_string());
        self.is_shutting_down = true;
        false
    }

    pub fn handle_escape(&mut self, key_up: bool) -> bool {
        if !key_up {
            return false;
        }
        self.back_requested = true;
        true
    }

    pub fn take_input_focus(&self, offered_focus: bool) -> bool {
        offered_focus && self.wants_input_focus
    }

    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.selected_index = Some(index);
        true
    }

    pub fn double_click_selected(&mut self) -> bool {
        let Some(entry) = self.selected_entry() else {
            self.pending_prompt = Some(ReplayPromptPort::NoSelection {
                title: "No replay selected".to_string(),
                body: "Please select a replay file.".to_string(),
            });
            return false;
        };

        self.loaded_replay = Some(entry.replay_filename.clone());
        self.visible = false;
        true
    }

    pub fn load_selected(&mut self) -> bool {
        let Some(entry) = self.selected_entry() else {
            self.pending_prompt = Some(ReplayPromptPort::NoSelection {
                title: "No replay selected".to_string(),
                body: "Please select a replay file.".to_string(),
            });
            return false;
        };

        if entry.requires_version_confirmation {
            self.pending_prompt = Some(ReplayPromptPort::OlderVersion {
                title: "Older replay version".to_string(),
                body: "This replay was recorded with a different build. Continue anyway?"
                    .to_string(),
                filename: entry.replay_filename.clone(),
            });
            return false;
        }

        self.loaded_replay = Some(entry.replay_filename.clone());
        self.visible = false;
        true
    }

    pub fn confirm_version_load(&mut self) -> bool {
        let Some(ReplayPromptPort::OlderVersion { filename, .. }) = self.pending_prompt.take()
        else {
            return false;
        };
        self.loaded_replay = Some(filename);
        self.visible = false;
        true
    }

    pub fn request_delete(&mut self) -> bool {
        let Some(entry) = self.selected_entry() else {
            self.pending_prompt = Some(ReplayPromptPort::NoSelection {
                title: "No replay selected".to_string(),
                body: "Please select a replay file.".to_string(),
            });
            return false;
        };

        self.pending_prompt = Some(ReplayPromptPort::DeleteConfirm {
            title: "Delete file".to_string(),
            body: "Are you sure you want to delete this replay?".to_string(),
            filename: entry.replay_filename.clone(),
        });
        true
    }

    pub fn confirm_delete(&mut self, replay_dir: &Path) -> Result<(), ReplayFileError> {
        let Some(ReplayPromptPort::DeleteConfirm { .. }) = self.pending_prompt else {
            return Ok(());
        };
        self.call_delete = true;
        self.delete_selected(replay_dir)
    }

    pub fn request_copy(&mut self) -> bool {
        let Some(entry) = self.selected_entry() else {
            self.pending_prompt = Some(ReplayPromptPort::NoSelection {
                title: "No replay selected".to_string(),
                body: "Please select a replay file.".to_string(),
            });
            return false;
        };

        self.pending_prompt = Some(ReplayPromptPort::CopyConfirm {
            title: "Copy replay".to_string(),
            body: "Copy this replay to the desktop?".to_string(),
            filename: entry.replay_filename.clone(),
        });
        true
    }

    pub fn confirm_copy(&mut self, replay_dir: &Path) -> Result<(), ReplayFileError> {
        let Some(ReplayPromptPort::CopyConfirm { .. }) = self.pending_prompt else {
            return Ok(());
        };
        self.call_copy = true;
        self.copy_selected(replay_dir)
    }

    pub fn populate_replay_file_list(
        &mut self,
        replay_dir: &Path,
        replay_ext: &str,
        last_replay_filename: &str,
        version_string: &str,
        version_number: u32,
        exe_crc: u32,
        ini_crc: u32,
    ) {
        self.entries.clear();

        let ext_without_dot = replay_ext.strip_prefix('.').unwrap_or(replay_ext);
        let mut replay_files: Vec<PathBuf> = Vec::new();

        if let Ok(entries) = fs::read_dir(replay_dir) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_file() {
                        let path = entry.path();
                        if let Some(ext) = path.extension() {
                            if ext == ext_without_dot {
                                replay_files.push(path);
                            }
                        }
                    }
                }
            }
        }

        for filepath in &replay_files {
            let filename = filepath
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let header = match read_replay_header_from_file(filepath) {
                Some(h) => h,
                None => continue,
            };

            let mut info = ReplayGameInfoPort::default();
            if !parse_ascii_string_to_game_info_port(&mut info, &header.game_options) {
                continue;
            }

            let mut replay_name = header.replay_name.clone();
            for _ in 0..replay_ext.len() {
                replay_name.pop();
            }

            let last_replay_full = format!("{}{}", last_replay_filename, replay_ext);
            let is_last_replay = filename == last_replay_full;
            if is_last_replay {
                replay_name = "Last Replay".to_string();
            }

            let display_time = header.time_val.to_display_string();

            let map_name = info.map_name.clone();

            let version_is_compatible = header.version_string == version_string
                && header.version_number == version_number
                && header.exe_crc == exe_crc
                && header.ini_crc == ini_crc;

            let requires_version_confirmation = !version_is_compatible;

            let replay_kind = if header.local_player_index >= 0 {
                ReplayKindPort::Multiplayer
            } else {
                ReplayKindPort::SinglePlayer
            };

            self.entries.push(ReplayEntryPort {
                replay_name,
                replay_filename: filename,
                display_time,
                version: header.version_string.clone(),
                map_name,
                replay_kind,
                version_is_compatible,
                requires_version_confirmation,
                is_last_replay,
            });
        }

        self.selected_index = (!self.entries.is_empty()).then_some(0);
    }

    pub fn sample() -> Self {
        Self::init(vec![
            ReplayEntryPort {
                replay_name: "Last Replay".to_string(),
                replay_filename: "LastReplay.rep".to_string(),
                display_time: "2026-03-11 21:15".to_string(),
                version: "1.04".to_string(),
                map_name: "Tournament Desert".to_string(),
                replay_kind: ReplayKindPort::SinglePlayer,
                version_is_compatible: true,
                requires_version_confirmation: false,
                is_last_replay: true,
            },
            ReplayEntryPort {
                replay_name: "Ladder Finals".to_string(),
                replay_filename: "LadderFinals.rep".to_string(),
                display_time: "2026-03-09 18:42".to_string(),
                version: "1.04".to_string(),
                map_name: "Defcon 6".to_string(),
                replay_kind: ReplayKindPort::Multiplayer,
                version_is_compatible: true,
                requires_version_confirmation: false,
                is_last_replay: false,
            },
            ReplayEntryPort {
                replay_name: "Old Patch Run".to_string(),
                replay_filename: "OldPatchRun.rep".to_string(),
                display_time: "2025-11-28 09:05".to_string(),
                version: "1.03".to_string(),
                map_name: "Forgotten Forest".to_string(),
                replay_kind: ReplayKindPort::SinglePlayer,
                version_is_compatible: false,
                requires_version_confirmation: true,
                is_last_replay: false,
            },
        ])
    }

    fn selected_entry(&self) -> Option<&ReplayEntryPort> {
        self.selected_index
            .and_then(|index| self.entries.get(index))
    }

    fn copy_selected(&mut self, replay_dir: &Path) -> Result<(), ReplayFileError> {
        self.call_copy = false;
        let Some(index) = self.selected_index else {
            return Err(ReplayFileError::NoSelection);
        };
        let entry = self
            .entries
            .get(index)
            .ok_or(ReplayFileError::NoSelection)?;

        let source = replay_dir.join(&entry.replay_filename);
        if !source.exists() {
            return Err(ReplayFileError::FileNotFound(source));
        }

        let desktop = get_desktop_path().ok_or(ReplayFileError::DesktopNotFound)?;
        let dest = desktop.join(&entry.replay_filename);

        fs::copy(&source, &dest)
            .map_err(|e| ReplayFileError::CopyFailed(source.clone(), e.to_string()))?;

        self.copied_replay = Some(dest.display().to_string());
        self.pending_prompt = None;
        Ok(())
    }

    fn delete_selected(&mut self, replay_dir: &Path) -> Result<(), ReplayFileError> {
        self.call_delete = false;
        let Some(index) = self.selected_index else {
            return Err(ReplayFileError::NoSelection);
        };
        if index >= self.entries.len() {
            return Err(ReplayFileError::NoSelection);
        }
        let entry = &self.entries[index];
        let filepath = replay_dir.join(&entry.replay_filename);
        let filename = entry.replay_filename.clone();
        if !filepath.exists() {
            self.entries.remove(index);
            self.deleted_replay = Some(filename);
            self.selected_index = if self.entries.is_empty() {
                None
            } else {
                Some(0)
            };
            self.pending_prompt = None;
            return Ok(());
        }

        fs::remove_file(&filepath)
            .map_err(|e| ReplayFileError::DeleteFailed(filepath.clone(), e.to_string()))?;

        let removed = self.entries.remove(index);
        self.deleted_replay = Some(removed.replay_filename);
        self.selected_index = if self.entries.is_empty() {
            None
        } else {
            Some(0)
        };
        self.pending_prompt = None;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
struct ReplayHeaderPort {
    filename: String,
    for_playback: bool,
    replay_name: String,
    time_val: TimeValuePort,
    version_string: String,
    version_time_string: String,
    version_number: u32,
    exe_crc: u32,
    ini_crc: u32,
    start_time: u64,
    end_time: u64,
    frame_duration: u32,
    quit_early: bool,
    desync_game: bool,
    game_options: String,
    local_player_index: i32,
}

#[derive(Clone, Debug)]
struct TimeValuePort {
    year: u16,
    month: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
}

impl Default for TimeValuePort {
    fn default() -> Self {
        Self {
            year: 0,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
        }
    }
}

impl TimeValuePort {
    fn to_display_string(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute
        )
    }
}

#[derive(Clone, Debug, Default)]
struct ReplayGameInfoPort {
    map_name: String,
}

fn parse_ascii_string_to_game_info_port(info: &mut ReplayGameInfoPort, options: &str) -> bool {
    if options.is_empty() {
        return false;
    }

    let map_key = "Map=";
    if let Some(pos) = options.find(map_key) {
        let rest = &options[pos + map_key.len()..];
        let end = rest
            .find(|c: char| c == '\n' || c == '\r' || c == ' ')
            .unwrap_or(rest.len());
        info.map_name = rest[..end].to_string();
    } else {
        return false;
    }

    true
}

fn read_replay_header_from_file(filepath: &Path) -> Option<ReplayHeaderPort> {
    let file = fs::File::open(filepath).ok()?;
    let mut reader = std::io::BufReader::new(file);
    read_replay_header_port(&mut reader).ok()
}

fn read_replay_header_port(
    reader: &mut std::io::BufReader<std::fs::File>,
) -> Result<ReplayHeaderPort, std::io::Error> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if &magic != b"ZHRY" {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid replay magic",
        ));
    }

    let mut header = ReplayHeaderPort::default();
    header.filename = filepath_from_reader(reader)?;

    let mut buf_4 = [0u8; 4];
    reader.read_exact(&mut buf_4)?;
    header.for_playback = buf_4[0] != 0;

    let mut buf_name = [0u8; 256];
    reader.read_exact(&mut buf_name)?;
    let name_end = buf_name
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(buf_name.len());
    header.replay_name = String::from_utf8_lossy(&buf_name[..name_end]).to_string();

    reader.read_exact(&mut buf_4)?;
    let year = u16::from_le_bytes([buf_4[0], buf_4[1]]);
    let month = u16::from_le_bytes([buf_4[2], buf_4[3]]);
    reader.read_exact(&mut buf_4)?;
    let day = u16::from_le_bytes([buf_4[0], buf_4[1]]);
    let hour = u16::from_le_bytes([buf_4[2], buf_4[3]]);
    reader.read_exact(&mut buf_4)?;
    let minute = u16::from_le_bytes([buf_4[0], buf_4[1]]);
    let second = u16::from_le_bytes([buf_4[2], buf_4[3]]);
    header.time_val = TimeValuePort {
        year,
        month,
        day,
        hour,
        minute,
        second,
    };

    let mut buf_version = [0u8; 64];
    reader.read_exact(&mut buf_version)?;
    let ver_end = buf_version
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(buf_version.len());
    header.version_string = String::from_utf8_lossy(&buf_version[..ver_end]).to_string();

    let mut buf_vtime = [0u8; 64];
    reader.read_exact(&mut buf_vtime)?;
    let vt_end = buf_vtime
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(buf_vtime.len());
    header.version_time_string = String::from_utf8_lossy(&buf_vtime[..vt_end]).to_string();

    reader.read_exact(&mut buf_4)?;
    header.version_number = u32::from_le_bytes(buf_4);

    reader.read_exact(&mut buf_4)?;
    header.exe_crc = u32::from_le_bytes(buf_4);

    reader.read_exact(&mut buf_4)?;
    header.ini_crc = u32::from_le_bytes(buf_4);

    reader.read_exact(&mut buf_4)?;
    header.start_time = u64::from(u32::from_le_bytes(buf_4));
    reader.read_exact(&mut buf_4)?;
    let end_hi = u32::from_le_bytes(buf_4);
    reader.read_exact(&mut buf_4)?;
    let end_lo = u32::from_le_bytes(buf_4);
    header.end_time = (u64::from(end_hi) << 32) | u64::from(end_lo);

    reader.read_exact(&mut buf_4)?;
    header.frame_duration = u32::from_le_bytes(buf_4);

    let mut buf_bool = [0u8; 4];
    reader.read_exact(&mut buf_bool)?;
    header.quit_early = buf_bool[0] != 0;
    reader.read_exact(&mut buf_bool)?;
    header.desync_game = buf_bool[0] != 0;

    let mut buf_i32 = [0u8; 4];
    reader.read_exact(&mut buf_i32)?;
    header.local_player_index = i32::from_le_bytes(buf_i32);

    let mut buf_options_len = [0u8; 4];
    reader.read_exact(&mut buf_options_len)?;
    let options_len = u32::from_le_bytes(buf_options_len) as usize;
    let mut options_buf = vec![0u8; options_len];
    if options_len > 0 {
        reader.read_exact(&mut options_buf)?;
    }
    header.game_options = String::from_utf8_lossy(&options_buf).to_string();

    Ok(header)
}

fn filepath_from_reader(
    reader: &mut std::io::BufReader<std::fs::File>,
) -> Result<String, std::io::Error> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut path_buf = vec![0u8; len];
    if len > 0 {
        reader.read_exact(&mut path_buf)?;
    }
    let s = String::from_utf8_lossy(&path_buf).to_string();
    if let Some(backslash) = s.rfind('\\') {
        Ok(s[backslash + 1..].to_string())
    } else if let Some(slash) = s.rfind('/') {
        Ok(s[slash + 1..].to_string())
    } else {
        Ok(s)
    }
}

fn get_desktop_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let desktop = PathBuf::from(userprofile).join("Desktop");
            if desktop.is_dir() {
                return Some(desktop);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let desktop = PathBuf::from(home).join("Desktop");
            if desktop.is_dir() {
                return Some(desktop);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let desktop = PathBuf::from(home).join("Desktop");
            if desktop.is_dir() {
                return Some(desktop);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incompatible_replay_requires_confirmation_before_load() {
        let mut menu = ReplayMenuPort::sample();
        assert!(menu.select_index(2));

        assert!(!menu.load_selected());
        assert!(matches!(
            menu.pending_prompt,
            Some(ReplayPromptPort::OlderVersion { .. })
        ));
        assert!(menu.confirm_version_load());
        assert_eq!(menu.loaded_replay.as_deref(), Some("OldPatchRun.rep"));
    }

    #[test]
    fn delete_replay_removes_selected_entry_after_confirmation() {
        let tmp = tempfile::tempdir().unwrap();
        let replay_path = tmp.path().join("LadderFinals.rep");
        fs::write(&replay_path, b"test").unwrap();

        let mut menu = ReplayMenuPort::sample();
        assert!(menu.select_index(1));

        assert!(menu.request_delete());
        assert!(menu.confirm_delete(tmp.path()).is_ok());
        assert!(!menu.update(false, false, tmp.path()));

        assert_eq!(menu.entries.len(), 2);
        assert_eq!(menu.deleted_replay.as_deref(), Some("LadderFinals.rep"));
        assert!(!replay_path.exists());
    }

    #[test]
    fn delete_nonexistent_file_removes_entry_gracefully() {
        let tmp = tempfile::tempdir().unwrap();

        let mut menu = ReplayMenuPort::sample();
        assert!(menu.select_index(1));

        assert!(menu.request_delete());
        assert!(menu.confirm_delete(tmp.path()).is_ok());
        assert_eq!(menu.entries.len(), 2);
    }

    #[test]
    fn update_sets_fade_group_after_entry_delay() {
        let tmp = tempfile::tempdir().unwrap();
        let mut menu = ReplayMenuPort::sample();

        assert!(!menu.update(false, false, tmp.path()));
        assert!(!menu.update(false, false, tmp.path()));
        assert_eq!(
            menu.active_transition_group.as_deref(),
            Some("ReplayMenuFade")
        );
        assert!(!menu.just_entered);
    }

    #[test]
    fn copy_replay_to_desktop() {
        let tmp = tempfile::tempdir().unwrap();
        let desktop = tmp.path().join("Desktop");
        fs::create_dir_all(&desktop).unwrap();
        let replay_path = tmp.path().join("LastReplay.rep");
        fs::write(&replay_path, b"replay_data").unwrap();

        let mut menu = ReplayMenuPort::init(vec![ReplayEntryPort {
            replay_name: "Last Replay".to_string(),
            replay_filename: "LastReplay.rep".to_string(),
            display_time: "2026-03-11 21:15".to_string(),
            version: "1.04".to_string(),
            map_name: "TestMap".to_string(),
            replay_kind: ReplayKindPort::SinglePlayer,
            version_is_compatible: true,
            requires_version_confirmation: false,
            is_last_replay: true,
        }]);

        menu.request_copy();
        let result = menu.confirm_copy(tmp.path());
        assert!(result.is_ok());
        assert!(menu.copied_replay.is_some());
        assert!(desktop.join("LastReplay.rep").exists());
    }

    #[test]
    fn copy_replay_no_desktop_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let replay_path = tmp.path().join("LastReplay.rep");
        fs::write(&replay_path, b"data").unwrap();

        let mut menu = ReplayMenuPort::init(vec![ReplayEntryPort {
            replay_name: "Last Replay".to_string(),
            replay_filename: "LastReplay.rep".to_string(),
            display_time: "2026-03-11 21:15".to_string(),
            version: "1.04".to_string(),
            map_name: "TestMap".to_string(),
            replay_kind: ReplayKindPort::SinglePlayer,
            version_is_compatible: true,
            requires_version_confirmation: false,
            is_last_replay: true,
        }]);

        menu.request_copy();
        let result = menu.confirm_copy(tmp.path());
        assert!(matches!(result, Err(ReplayFileError::DesktopNotFound)));
    }

    #[test]
    fn populate_scans_directory_and_parses_headers() {
        let tmp = tempfile::tempdir().unwrap();

        let header =
            build_test_replay_header("TestReplay.rep", "1.04", 100, 0, 0, -1, "TestMap=MyMap\n");
        fs::write(tmp.path().join("TestReplay.rep"), &header).unwrap();

        let header2 =
            build_test_replay_header("MPReplay.rep", "1.04", 100, 0, 0, 0, "Map=OtherMap\n");
        fs::write(tmp.path().join("MPReplay.rep"), &header2).unwrap();

        fs::write(tmp.path().join("readme.txt"), b"not a replay").unwrap();

        let mut menu = ReplayMenuPort::init(vec![]);
        menu.populate_replay_file_list(tmp.path(), ".rep", "LastReplay", "1.04", 100, 0, 0);

        assert_eq!(menu.entries.len(), 2);
        assert_eq!(menu.entries[0].replay_name, "TestReplay");
        assert!(menu.entries[0].version_is_compatible);
        assert_eq!(menu.entries[0].replay_kind, ReplayKindPort::SinglePlayer);
        assert_eq!(menu.entries[1].replay_name, "MPReplay");
        assert_eq!(menu.entries[1].replay_kind, ReplayKindPort::Multiplayer);
    }

    fn build_test_replay_header(
        filename: &str,
        version: &str,
        version_num: u32,
        exe_crc: u32,
        ini_crc: u32,
        local_player: i32,
        game_options: &str,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"ZHRY");

        let fname_bytes = filename.as_bytes();
        buf.extend_from_slice(&(fname_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(fname_bytes);

        buf.push(0);
        buf.extend_from_slice(&[0u8; 3]);

        let mut name_buf = [0u8; 256];
        let name = filename.strip_suffix(".rep").unwrap_or(filename);
        name_buf[..name.len()].copy_from_slice(name.as_bytes());
        buf.extend_from_slice(&name_buf);

        buf.extend_from_slice(&2026u16.to_le_bytes());
        buf.extend_from_slice(&3u16.to_le_bytes());
        buf.extend_from_slice(&11u16.to_le_bytes());
        buf.extend_from_slice(&21u16.to_le_bytes());
        buf.extend_from_slice(&15u16.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());

        let mut ver_buf = [0u8; 64];
        ver_buf[..version.len().min(63)]
            .copy_from_slice(&version.as_bytes()[..version.len().min(63)]);
        buf.extend_from_slice(&ver_buf);

        let vt_buf = [0u8; 64];
        buf.extend_from_slice(&vt_buf);

        buf.extend_from_slice(&version_num.to_le_bytes());
        buf.extend_from_slice(&exe_crc.to_le_bytes());
        buf.extend_from_slice(&ini_crc.to_le_bytes());

        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());

        buf.extend_from_slice(&0u32.to_le_bytes());

        buf.extend_from_slice(&[0u8; 4]);
        buf.extend_from_slice(&[0u8; 4]);

        buf.extend_from_slice(&local_player.to_le_bytes());

        let options_bytes = game_options.as_bytes();
        buf.extend_from_slice(&(options_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(options_bytes);

        buf
    }
}
