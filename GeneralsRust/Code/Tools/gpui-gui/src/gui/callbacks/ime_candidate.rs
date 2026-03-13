use crate::gui::ime_manager::ImeManagerPort;
use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/IMECandidate.cpp",
    "crate::gui::callbacks::ime_candidate",
    "IME Candidate",
    "Ports IME candidate list display and selection callbacks.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "IME Candidate",
    "IME candidate rendering and selection callbacks.",
);

pub const IME_CANDIDATE_WINDOW_LINE_SPACING: i32 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImeCandidateRowPort {
    pub number_label: String,
    pub candidate: String,
    pub selected: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImeCandidateWindowPort {
    pub display_string_allocated: bool,
    pub line_spacing: i32,
    pub rows: Vec<ImeCandidateRowPort>,
}

impl Default for ImeCandidateWindowPort {
    fn default() -> Self {
        Self {
            display_string_allocated: false,
            line_spacing: IME_CANDIDATE_WINDOW_LINE_SPACING,
            rows: Vec::new(),
        }
    }
}

impl ImeCandidateWindowPort {
    pub fn on_create(&mut self) {
        self.display_string_allocated = true;
    }

    pub fn on_destroy(&mut self) {
        self.display_string_allocated = false;
        self.rows.clear();
    }

    pub fn sync_from_ime(&mut self, ime: &ImeManagerPort) {
        let first = ime.get_candidate_page_start();
        let total = ime.get_candidate_count();
        let page_size = ime.get_candidate_page_size();
        let count = page_size.min(total.saturating_sub(first));
        let selected = ime.get_selected_candidate_index().saturating_sub(first);

        self.rows = (0..count)
            .map(|index| ImeCandidateRowPort {
                number_label: format!("{}:", index + ime.get_index_base()),
                candidate: ime.get_candidate(first + index).to_string(),
                selected: index == selected,
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_from_ime_uses_page_slice_and_selection() {
        let mut ime = ImeManagerPort::default();
        ime.update_candidate_list(
            vec![
                "Alpha".to_string(),
                "Beta".to_string(),
                "Gamma".to_string(),
                "Delta".to_string(),
            ],
            2,
            1,
            2,
        );
        ime.index_base = 1;

        let mut window = ImeCandidateWindowPort::default();
        window.on_create();
        window.sync_from_ime(&ime);

        assert!(window.display_string_allocated);
        assert_eq!(window.rows.len(), 2);
        assert_eq!(window.rows[0].candidate, "Beta");
        assert_eq!(window.rows[1].candidate, "Gamma");
        assert!(window.rows[1].selected);
    }
}
