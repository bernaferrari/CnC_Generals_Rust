use crate::gui::game_window::GameWindowPort;
use crate::gui::source_catalog::GuiPortRecord;
use crate::gui::window_layout::WindowLayoutPort;
use crate::model::{GadgetWindowStyle, LegacyRect, WindowStatus};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GameWindowManager.cpp",
    "crate::gui::game_window_manager",
    "Game Window Manager",
    "Owns top-level windows, focus state, modal ordering, destruction, and layout creation.",
);

#[derive(Clone, Debug, Default)]
pub struct GameWindowManagerPort {
    pub windows: Vec<GameWindowPort>,
    pub root_order: Vec<i32>,
    pub destroy_queue: Vec<i32>,
    pub focus: Option<i32>,
    pub mouse_captor: Option<i32>,
    pub grab_window: Option<i32>,
    pub lone_window: Option<i32>,
    pub modal_stack: Vec<i32>,
    pub created_layouts: Vec<String>,
    next_generated_id: i32,
}

impl GameWindowManagerPort {
    pub fn init(&mut self) {}

    pub fn reset(&mut self) {
        self.win_destroy_all();
    }

    pub fn update(&mut self) {
        self.process_destroy_list();
    }

    pub fn window(&self, window_id: i32) -> Option<&GameWindowPort> {
        self.windows.iter().find(|window| window.id == window_id)
    }

    pub fn window_mut(&mut self, window_id: i32) -> Option<&mut GameWindowPort> {
        self.windows
            .iter_mut()
            .find(|window| window.id == window_id)
    }

    pub fn link_window(&mut self, mut window: GameWindowPort) {
        let window_id = window.id;
        window.set_parent(None);
        self.upsert_window(window);

        let insert_at = self
            .root_order
            .iter()
            .rposition(|id| self.modal_stack.contains(id) && *id != window_id)
            .map(|index| index + 1)
            .unwrap_or(0);

        self.root_order.retain(|id| *id != window_id);
        self.root_order.insert(insert_at, window_id);
        self.sync_root_links();
    }

    pub fn insert_window_ahead_of(&mut self, mut window: GameWindowPort, ahead_of: Option<i32>) {
        let Some(ahead_of_id) = ahead_of else {
            self.link_window(window);
            return;
        };

        let ahead_parent = self.window(ahead_of_id).and_then(|ahead| ahead.parent);
        let window_id = window.id;
        window.set_parent(ahead_parent);
        self.upsert_window(window);

        match ahead_parent {
            Some(parent_id) => {
                let mut children = self
                    .window(parent_id)
                    .map(|window| window.children.clone())
                    .unwrap_or_default();
                let insert_at = children
                    .iter()
                    .position(|child_id| *child_id == ahead_of_id)
                    .unwrap_or(children.len());
                children.retain(|child_id| *child_id != window_id);
                children.insert(insert_at, window_id);
                if let Some(parent) = self.window_mut(parent_id) {
                    parent.children = children;
                    parent.child = parent.children.first().copied();
                }
                self.sync_child_links(parent_id);
            }
            None => {
                let insert_at = self
                    .root_order
                    .iter()
                    .position(|id| *id == ahead_of_id)
                    .unwrap_or(self.root_order.len());
                self.root_order.retain(|id| *id != window_id);
                self.root_order.insert(insert_at, window_id);
                self.sync_root_links();
            }
        }
    }

    pub fn unlink_window(&mut self, window_id: i32) {
        self.root_order.retain(|id| *id != window_id);
        if let Some(window) = self.window_mut(window_id) {
            window.set_next(None);
            window.set_prev(None);
            window.set_parent(None);
        }
        self.sync_root_links();
    }

    pub fn unlink_child_window(&mut self, window_id: i32) {
        let parent_id = self.window(window_id).and_then(|window| window.parent);
        let Some(parent_id) = parent_id else {
            self.unlink_window(window_id);
            return;
        };

        if let Some(parent) = self.window_mut(parent_id) {
            parent.children.retain(|child_id| *child_id != window_id);
            parent.child = parent.children.first().copied();
        }
        if let Some(window) = self.window_mut(window_id) {
            window.set_parent(None);
            window.set_prev(None);
            window.set_next(None);
        }
        self.sync_child_links(parent_id);
    }

    pub fn is_enabled(&self, window_id: i32) -> bool {
        let Some(window) = self.window(window_id) else {
            return false;
        };

        window.is_enabled()
            && window
                .parent
                .map(|parent_id| self.is_enabled(parent_id))
                .unwrap_or(true)
    }

    pub fn is_hidden(&self, window_id: i32) -> bool {
        let Some(window) = self.window(window_id) else {
            return true;
        };

        window.is_hidden()
            || window
                .parent
                .map(|parent_id| self.is_hidden(parent_id))
                .unwrap_or(false)
    }

    pub fn add_window_to_parent(&mut self, mut window: GameWindowPort, parent_id: i32) {
        let window_id = window.id;
        window.set_parent(Some(parent_id));
        self.upsert_window(window);

        let mut children = self
            .window(parent_id)
            .map(|parent| parent.children.clone())
            .unwrap_or_default();
        children.retain(|child_id| *child_id != window_id);
        children.insert(0, window_id);

        if let Some(parent) = self.window_mut(parent_id) {
            parent.children = children;
            parent.child = parent.children.first().copied();
        }
        self.sync_child_links(parent_id);
    }

    pub fn add_window_to_parent_at_end(&mut self, mut window: GameWindowPort, parent_id: i32) {
        let window_id = window.id;
        window.set_parent(Some(parent_id));
        self.upsert_window(window);

        let mut children = self
            .window(parent_id)
            .map(|parent| parent.children.clone())
            .unwrap_or_default();
        children.retain(|child_id| *child_id != window_id);
        children.push(window_id);

        if let Some(parent) = self.window_mut(parent_id) {
            parent.children = children;
            parent.child = parent.children.first().copied();
        }
        self.sync_child_links(parent_id);
    }

    pub fn hide_windows_in_range(&mut self, start_id: i32, end_id: i32, hidden: bool) {
        for window_id in self.windows_in_root_range(start_id, end_id) {
            if let Some(window) = self.window_mut(window_id) {
                window.hide(hidden);
            }
        }
    }

    pub fn enable_windows_in_range(&mut self, start_id: i32, end_id: i32, enabled: bool) {
        for window_id in self.windows_in_root_range(start_id, end_id) {
            if let Some(window) = self.window_mut(window_id) {
                window.enable(enabled);
            }
        }
    }

    pub fn queue_destroy(&mut self, window_id: i32) {
        if !self.destroy_queue.contains(&window_id) {
            self.destroy_queue.push(window_id);
        }
    }

    pub fn process_destroy_list(&mut self) {
        let queued = std::mem::take(&mut self.destroy_queue);
        for window_id in queued {
            self.win_destroy(window_id);
        }
    }

    pub fn win_set_focus(&mut self, window: Option<i32>) {
        self.focus =
            window.filter(|window_id| self.is_enabled(*window_id) && !self.is_hidden(*window_id));
    }

    pub fn win_set_grab_window(&mut self, window: Option<i32>) {
        self.grab_window = window;
    }

    pub fn win_set_lone_window(&mut self, window: Option<i32>) {
        self.lone_window = window;
    }

    pub fn win_destroy(&mut self, window_id: i32) {
        let descendants = self.collect_descendants(window_id);
        let mut doomed = Vec::with_capacity(descendants.len() + 1);
        doomed.push(window_id);
        doomed.extend(descendants);

        for doomed_id in &doomed {
            self.root_order.retain(|id| id != doomed_id);
            self.modal_stack.retain(|id| id != doomed_id);
            if self.focus == Some(*doomed_id) {
                self.focus = None;
            }
            if self.mouse_captor == Some(*doomed_id) {
                self.mouse_captor = None;
            }
            if self.grab_window == Some(*doomed_id) {
                self.grab_window = None;
            }
            if self.lone_window == Some(*doomed_id) {
                self.lone_window = None;
            }
        }

        for doomed_id in &doomed {
            let parent_id = self.window(*doomed_id).and_then(|window| window.parent);
            if let Some(parent_id) = parent_id {
                if let Some(parent) = self.window_mut(parent_id) {
                    parent.children.retain(|child_id| child_id != doomed_id);
                    parent.child = parent.children.first().copied();
                }
                self.sync_child_links(parent_id);
            }
        }

        self.windows.retain(|window| !doomed.contains(&window.id));
        self.sync_root_links();
    }

    pub fn win_destroy_all(&mut self) {
        self.windows.clear();
        self.root_order.clear();
        self.destroy_queue.clear();
        self.focus = None;
        self.mouse_captor = None;
        self.grab_window = None;
        self.lone_window = None;
        self.modal_stack.clear();
    }

    pub fn win_set_modal(&mut self, window_id: i32) {
        if !self.modal_stack.contains(&window_id) {
            self.modal_stack.push(window_id);
            self.sync_root_links();
        }
    }

    pub fn win_unset_modal(&mut self, window_id: i32) {
        self.modal_stack.retain(|id| *id != window_id);
        self.sync_root_links();
    }

    pub fn win_next_tab(&self, window_id: i32) -> Option<i32> {
        let mut current = window_id;
        let first_try = self.find_last_leaf(window_id).unwrap_or(window_id);
        let mut wrapped = false;

        loop {
            current = if wrapped {
                self.find_prev_leaf(current)?
            } else {
                wrapped = true;
                first_try
            };

            if self.is_enabled(current) && !self.is_hidden(current) {
                return Some(current);
            }
            if current == window_id {
                return None;
            }
        }
    }

    pub fn win_prev_tab(&self, window_id: i32) -> Option<i32> {
        let mut current = window_id;
        let first_try = self.find_first_leaf(window_id).unwrap_or(window_id);
        let mut wrapped = false;

        loop {
            current = if wrapped {
                self.find_next_leaf(current)?
            } else {
                wrapped = true;
                first_try
            };

            if self.is_enabled(current) && !self.is_hidden(current) {
                return Some(current);
            }
            if current == window_id {
                return None;
            }
        }
    }

    pub fn create_layout(&mut self, filename: impl Into<String>) -> WindowLayoutPort {
        let filename = filename.into();
        self.created_layouts.push(filename.clone());
        let spec = layout_spec(&filename);

        let root_id = self.next_id();
        let primary_id = self.next_id();
        let panel_id = self.next_id();

        let mut root = GameWindowPort::new(
            root_id,
            spec.title,
            LegacyRect {
                x: 96,
                y: 64,
                width: 1120,
                height: 720,
            },
        );
        root.set_status(WindowStatus::ACTIVE | WindowStatus::ENABLED | WindowStatus::BORDER);
        root.style = GadgetWindowStyle::USER_WINDOW;
        root.enable(true);
        root.children = vec![primary_id, panel_id];
        root.child = Some(primary_id);

        let mut primary = GameWindowPort::new(primary_id, spec.primary_title, spec.primary_rect);
        primary.tooltip = spec.primary_tooltip.to_string();
        primary.style = spec.primary_style;
        primary.set_parent(Some(root_id));
        primary.enable(true);
        primary.set_status(spec.primary_status);
        primary.set_text(spec.primary_text);

        let mut panel = GameWindowPort::new(panel_id, spec.secondary_title, spec.secondary_rect);
        panel.tooltip = spec.secondary_tooltip.to_string();
        panel.style = spec.secondary_style;
        panel.set_parent(Some(root_id));
        panel.enable(true);
        panel.set_status(spec.secondary_status);
        panel.set_text(spec.secondary_text);

        WindowLayoutPort::new(filename, vec![root, primary, panel])
    }

    fn upsert_window(&mut self, window: GameWindowPort) {
        if let Some(index) = self
            .windows
            .iter()
            .position(|existing| existing.id == window.id)
        {
            self.windows[index] = window;
        } else {
            self.windows.push(window);
        }
    }

    fn sync_root_links(&mut self) {
        let ordered = self.root_order.clone();
        for (index, window_id) in ordered.iter().enumerate() {
            let next = index
                .checked_sub(1)
                .and_then(|prev_index| ordered.get(prev_index).copied());
            let prev = ordered.get(index + 1).copied();
            if let Some(window) = self.window_mut(*window_id) {
                window.set_parent(None);
                window.set_next(next);
                window.set_prev(prev);
            }
        }
    }

    fn sync_child_links(&mut self, parent_id: i32) {
        let children = self
            .window(parent_id)
            .map(|parent| parent.children.clone())
            .unwrap_or_default();

        if let Some(parent) = self.window_mut(parent_id) {
            parent.child = children.first().copied();
        }

        for (index, child_id) in children.iter().enumerate() {
            let next = index
                .checked_sub(1)
                .and_then(|prev_index| children.get(prev_index).copied());
            let prev = children.get(index + 1).copied();
            if let Some(child) = self.window_mut(*child_id) {
                child.set_parent(Some(parent_id));
                child.set_next(next);
                child.set_prev(prev);
            }
        }
    }

    fn windows_in_root_range(&self, start_id: i32, end_id: i32) -> Vec<i32> {
        let Some(start_index) = self.root_order.iter().position(|id| *id == start_id) else {
            return Vec::new();
        };
        let Some(end_index) = self.root_order.iter().position(|id| *id == end_id) else {
            return Vec::new();
        };

        let (start, end) = if start_index <= end_index {
            (start_index, end_index)
        } else {
            (end_index, start_index)
        };

        self.root_order[start..=end].to_vec()
    }

    fn next_id(&mut self) -> i32 {
        self.next_generated_id += 1;
        self.next_generated_id
    }

    fn collect_descendants(&self, window_id: i32) -> Vec<i32> {
        let mut descendants = Vec::new();
        let Some(window) = self.window(window_id) else {
            return descendants;
        };

        for child_id in &window.children {
            descendants.push(*child_id);
            descendants.extend(self.collect_descendants(*child_id));
        }

        descendants
    }

    fn find_first_leaf(&self, window_id: i32) -> Option<i32> {
        let mut leaf = window_id;
        while let Some(parent_id) = self.window(leaf).and_then(|window| window.parent) {
            leaf = parent_id;
        }
        while let Some(child_id) = self.window(leaf).and_then(|window| window.child) {
            leaf = child_id;
        }
        Some(leaf)
    }

    fn find_last_leaf(&self, window_id: i32) -> Option<i32> {
        let mut leaf = window_id;
        while let Some(parent_id) = self.window(leaf).and_then(|window| window.parent) {
            leaf = parent_id;
        }
        loop {
            let Some(child_id) = self.window(leaf).and_then(|window| window.child) else {
                break;
            };
            leaf = child_id;
            while let Some(next_id) = self.window(leaf).and_then(|window| window.next) {
                leaf = next_id;
            }
        }
        Some(leaf)
    }

    fn find_prev_leaf(&self, window_id: i32) -> Option<i32> {
        let mut leaf = window_id;

        if let Some(prev_id) = self.window(leaf).and_then(|window| window.prev) {
            leaf = prev_id;
            loop {
                let Some(window) = self.window(leaf) else {
                    break;
                };
                if let Some(child_id) = window
                    .child
                    .filter(|_| !window.status.contains(WindowStatus::TAB_STOP))
                {
                    leaf = child_id;
                    while let Some(next_id) = self.window(leaf).and_then(|next| next.next) {
                        leaf = next_id;
                    }
                } else {
                    break;
                }
            }
            return Some(leaf);
        }

        while let Some(parent_id) = self.window(leaf).and_then(|window| window.parent) {
            leaf = parent_id;
            if let Some(grand_parent_id) = self.window(leaf).and_then(|window| window.parent) {
                if let Some(prev_id) = self.window(leaf).and_then(|window| window.prev) {
                    leaf = prev_id;
                    loop {
                        let Some(window) = self.window(leaf) else {
                            break;
                        };
                        if let Some(child_id) = window
                            .child
                            .filter(|_| !window.status.contains(WindowStatus::TAB_STOP))
                        {
                            leaf = child_id;
                            while let Some(next_id) = self.window(leaf).and_then(|next| next.next) {
                                leaf = next_id;
                            }
                        } else {
                            break;
                        }
                    }
                    return Some(leaf);
                }
                leaf = grand_parent_id;
            }
        }

        self.find_last_leaf(window_id)
    }

    fn find_next_leaf(&self, window_id: i32) -> Option<i32> {
        let mut leaf = window_id;

        if let Some(next_id) = self.window(leaf).and_then(|window| window.next) {
            if self
                .window(next_id)
                .map(|window| window.status.contains(WindowStatus::TAB_STOP))
                .unwrap_or(false)
            {
                return Some(next_id);
            }

            let mut cursor = next_id;
            loop {
                let window = self.window(cursor)?;
                if window.child.is_none() || window.status.contains(WindowStatus::TAB_STOP) {
                    return Some(cursor);
                }
                cursor = window.child?;
            }
        }

        while let Some(parent_id) = self.window(leaf).and_then(|window| window.parent) {
            leaf = parent_id;
            if self.window(leaf).and_then(|window| window.parent).is_some() {
                if let Some(next_id) = self.window(leaf).and_then(|window| window.next) {
                    let mut cursor = next_id;
                    loop {
                        let window = self.window(cursor)?;
                        if window.child.is_none() || window.status.contains(WindowStatus::TAB_STOP)
                        {
                            return Some(cursor);
                        }
                        cursor = window.child?;
                    }
                }
            }
        }

        self.find_first_leaf(window_id)
    }
}

fn layout_title(filename: &str) -> String {
    filename
        .rsplit('/')
        .next()
        .unwrap_or(filename)
        .trim_end_matches(".wnd")
        .replace('_', " ")
}

struct LayoutSpec {
    title: &'static str,
    primary_title: &'static str,
    primary_tooltip: &'static str,
    primary_rect: LegacyRect,
    primary_style: GadgetWindowStyle,
    primary_status: WindowStatus,
    primary_text: &'static str,
    secondary_title: &'static str,
    secondary_tooltip: &'static str,
    secondary_rect: LegacyRect,
    secondary_style: GadgetWindowStyle,
    secondary_status: WindowStatus,
    secondary_text: &'static str,
}

fn layout_spec(filename: &str) -> LayoutSpec {
    match filename {
        "Menus/MainMenu.wnd" => LayoutSpec {
            title: "MainMenu",
            primary_title: "ButtonSinglePlayer",
            primary_tooltip: "Enter the single-player shell flow",
            primary_rect: LegacyRect {
                x: 128,
                y: 612,
                width: 280,
                height: 48,
            },
            primary_style: GadgetWindowStyle::PUSH_BUTTON,
            primary_status: WindowStatus::ENABLED | WindowStatus::IMAGE | WindowStatus::TAB_STOP,
            primary_text: "Single Player",
            secondary_title: "MainMenuDefaultPanel",
            secondary_tooltip: "Primary main-menu button stack",
            secondary_rect: LegacyRect {
                x: 756,
                y: 152,
                width: 300,
                height: 428,
            },
            secondary_style: GadgetWindowStyle::USER_WINDOW,
            secondary_status: WindowStatus::ENABLED | WindowStatus::SMOOTH_TEXT,
            secondary_text: "Single Player / Multiplayer / Options / Exit",
        },
        "Menus/ReplayMenu.wnd" => LayoutSpec {
            title: "ReplayMenu",
            primary_title: "ButtonLoadReplay",
            primary_tooltip: "Load the selected replay",
            primary_rect: LegacyRect {
                x: 128,
                y: 612,
                width: 220,
                height: 48,
            },
            primary_style: GadgetWindowStyle::PUSH_BUTTON,
            primary_status: WindowStatus::ENABLED | WindowStatus::IMAGE | WindowStatus::TAB_STOP,
            primary_text: "Load Replay",
            secondary_title: "ListboxReplayFiles",
            secondary_tooltip: "Replay list with metadata columns",
            secondary_rect: LegacyRect {
                x: 706,
                y: 152,
                width: 350,
                height: 428,
            },
            secondary_style: GadgetWindowStyle::SCROLL_LISTBOX,
            secondary_status: WindowStatus::ENABLED | WindowStatus::SMOOTH_TEXT,
            secondary_text: "Last Replay",
        },
        "Menus/LanLobbyMenu.wnd" => LayoutSpec {
            title: "LanLobbyMenu",
            primary_title: "TextEntryChat",
            primary_tooltip: "LAN chat entry",
            primary_rect: LegacyRect {
                x: 128,
                y: 628,
                width: 520,
                height: 32,
            },
            primary_style: GadgetWindowStyle::ENTRY_FIELD,
            primary_status: WindowStatus::ENABLED | WindowStatus::IMAGE | WindowStatus::TAB_STOP,
            primary_text: "Type message...",
            secondary_title: "ListboxPlayers",
            secondary_tooltip: "Lobby player roster",
            secondary_rect: LegacyRect {
                x: 756,
                y: 152,
                width: 300,
                height: 428,
            },
            secondary_style: GadgetWindowStyle::SCROLL_LISTBOX,
            secondary_status: WindowStatus::ENABLED | WindowStatus::SMOOTH_TEXT,
            secondary_text: "bernardo",
        },
        _ => LayoutSpec {
            title: "Layout",
            primary_title: "PrimaryControl",
            primary_tooltip: "Primary layout control",
            primary_rect: LegacyRect {
                x: 128,
                y: 612,
                width: 280,
                height: 48,
            },
            primary_style: GadgetWindowStyle::PUSH_BUTTON,
            primary_status: WindowStatus::ENABLED | WindowStatus::IMAGE | WindowStatus::TAB_STOP,
            primary_text: "Launch",
            secondary_title: "DetailPanel",
            secondary_tooltip: "Secondary layout panel",
            secondary_rect: LegacyRect {
                x: 756,
                y: 152,
                width: 300,
                height: 428,
            },
            secondary_style: GadgetWindowStyle::SCROLL_LISTBOX,
            secondary_status: WindowStatus::ENABLED | WindowStatus::SMOOTH_TEXT,
            secondary_text: "Detail",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_window_places_latest_at_head_when_no_modal_window_exists() {
        let mut manager = GameWindowManagerPort::default();
        manager.link_window(GameWindowPort::new(
            1,
            "One",
            LegacyRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        ));
        manager.link_window(GameWindowPort::new(
            2,
            "Two",
            LegacyRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        ));

        assert_eq!(manager.root_order, vec![2, 1]);
        assert_eq!(manager.window(2).and_then(|window| window.prev), Some(1));
    }

    #[test]
    fn destroy_window_removes_descendants() {
        let mut manager = GameWindowManagerPort::default();
        let mut parent = GameWindowPort::new(
            1,
            "Parent",
            LegacyRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        );
        parent.children = vec![2];
        parent.child = Some(2);

        let mut child = GameWindowPort::new(
            2,
            "Child",
            LegacyRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        );
        child.set_parent(Some(1));

        manager.link_window(parent);
        manager.windows.push(child);

        manager.win_destroy(1);

        assert!(manager.windows.is_empty());
        assert!(manager.root_order.is_empty());
    }
}
