use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/DownloadMenu.cpp",
    "crate::gui::callbacks::menus::download_menu",
    "Download Menu",
    "Patch/download screen callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "DownloadMenu",
    "Download Menu",
    "Patch and download workflow.",
    "Shell",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadEntryPort {
    pub label: String,
    pub status: String,
    pub progress_pct: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadMenuPort {
    pub patch_server: String,
    pub queue: Vec<DownloadEntryPort>,
    pub selected_download: usize,
    pub total_progress_pct: u8,
    pub can_cancel: bool,
    pub notes: Vec<String>,
}

impl Default for DownloadMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl DownloadMenuPort {
    pub fn active_download(&self) -> Option<&DownloadEntryPort> {
        self.queue.get(self.selected_download)
    }

    pub fn sample() -> Self {
        Self {
            patch_server: "patch.generals.example".to_string(),
            queue: vec![
                DownloadEntryPort {
                    label: "Official Map Pack".to_string(),
                    status: "Verifying archive".to_string(),
                    progress_pct: 82,
                },
                DownloadEntryPort {
                    label: "Balance Hotfix Notes".to_string(),
                    status: "Queued".to_string(),
                    progress_pct: 0,
                },
            ],
            selected_download: 0,
            total_progress_pct: 61,
            can_cancel: true,
            notes: vec![
                "Downloads reuse the legacy patch/update shell flow.".to_string(),
                "Checksum verification finishes before the next archive starts.".to_string(),
            ],
        }
    }
}
