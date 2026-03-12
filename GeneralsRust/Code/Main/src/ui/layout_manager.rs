use glam::{Vec2, Vec4};
use std::collections::HashMap;

/// Window status matching C++ WIN_STATUS
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowStatus {
    None,
    Enabled,
    Disabled,
    Hidden,
    Image,
}

/// UI element alignment
#[derive(Debug, Clone, Copy)]
pub enum Alignment {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Rectangle definition for UI elements
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.width * 0.5, self.y + self.height * 0.5)
    }
}

/// UI Element - matches C++ GameWindow structure
#[derive(Debug, Clone)]
pub struct UIElement {
    pub id: u32,
    pub name: String,
    pub rect: Rect,
    pub status: WindowStatus,
    pub parent: Option<u32>,
    pub children: Vec<u32>,
    pub z_order: i32,
    pub visible: bool,
    pub enabled: bool,
    pub text: String,
    pub font_name: String,
    pub font_size: f32,
    pub text_color: Vec4,
    pub background_color: Vec4,
    pub border_color: Vec4,
    pub texture_id: Option<u32>,
    pub alignment: Alignment,
    pub margin: Vec4,  // left, top, right, bottom
    pub padding: Vec4, // left, top, right, bottom
}

impl UIElement {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            rect: Rect::new(0.0, 0.0, 100.0, 30.0),
            status: WindowStatus::Enabled,
            parent: None,
            children: Vec::new(),
            z_order: 0,
            visible: true,
            enabled: true,
            text: String::new(),
            font_name: "Arial".to_string(),
            font_size: 12.0,
            text_color: Vec4::new(1.0, 1.0, 1.0, 1.0), // White
            background_color: Vec4::new(0.3, 0.3, 0.3, 1.0), // Dark gray
            border_color: Vec4::new(0.5, 0.5, 0.5, 1.0), // Gray
            texture_id: None,
            alignment: Alignment::TopLeft,
            margin: Vec4::ZERO,
            padding: Vec4::new(4.0, 4.0, 4.0, 4.0),
        }
    }

    pub fn with_rect(mut self, rect: Rect) -> Self {
        self.rect = rect;
        self
    }

    pub fn with_text(mut self, text: &str) -> Self {
        self.text = text.to_string();
        self
    }

    pub fn with_font(mut self, font_name: &str, size: f32) -> Self {
        self.font_name = font_name.to_string();
        self.font_size = size;
        self
    }

    pub fn with_colors(mut self, text: Vec4, background: Vec4, border: Vec4) -> Self {
        self.text_color = text;
        self.background_color = background;
        self.border_color = border;
        self
    }

    pub fn with_texture(mut self, texture_id: u32) -> Self {
        self.texture_id = Some(texture_id);
        self
    }

    pub fn get_absolute_rect(&self, layout_manager: &UILayoutManager) -> Rect {
        if let Some(parent_id) = self.parent {
            if let Some(parent) = layout_manager.get_element(parent_id) {
                let parent_rect = parent.get_absolute_rect(layout_manager);
                return Rect::new(
                    parent_rect.x + self.rect.x,
                    parent_rect.y + self.rect.y,
                    self.rect.width,
                    self.rect.height,
                );
            }
        }
        self.rect
    }
}

/// Main UI Layout Manager - matches C++ GameWindowManager
pub struct UILayoutManager {
    elements: HashMap<u32, UIElement>,
    root_elements: Vec<u32>,
    next_id: u32,
    screen_width: f32,
    screen_height: f32,
    element_by_name: HashMap<String, u32>,
}

impl UILayoutManager {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            elements: HashMap::new(),
            root_elements: Vec::new(),
            next_id: 1,
            screen_width,
            screen_height,
            element_by_name: HashMap::new(),
        }
    }

    pub fn add_element(&mut self, element: UIElement) -> u32 {
        let id = element.id;
        self.element_by_name.insert(element.name.clone(), id);

        if element.parent.is_none() {
            self.root_elements.push(id);
        } else if let Some(parent_id) = element.parent {
            if let Some(parent) = self.elements.get_mut(&parent_id) {
                parent.children.push(id);
            }
        }

        self.elements.insert(id, element);
        id
    }

    pub fn create_element(&mut self, name: &str) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let element = UIElement::new(id, name);
        self.add_element(element)
    }

    pub fn get_element(&self, id: u32) -> Option<&UIElement> {
        self.elements.get(&id)
    }

    pub fn get_element_mut(&mut self, id: u32) -> Option<&mut UIElement> {
        self.elements.get_mut(&id)
    }

    pub fn get_element_by_name(&self, name: &str) -> Option<&UIElement> {
        self.element_by_name
            .get(name)
            .and_then(|&id| self.elements.get(&id))
    }

    pub fn remove_element(&mut self, id: u32) -> bool {
        if let Some(element) = self.elements.remove(&id) {
            // Remove from parent's children
            if let Some(parent_id) = element.parent {
                if let Some(parent) = self.elements.get_mut(&parent_id) {
                    parent.children.retain(|&child_id| child_id != id);
                }
            } else {
                // Remove from root elements
                self.root_elements.retain(|&root_id| root_id != id);
            }

            // Remove from name lookup
            self.element_by_name.remove(&element.name);

            // Remove all children recursively
            let children = element.children.clone();
            for child_id in children {
                self.remove_element(child_id);
            }

            true
        } else {
            false
        }
    }

    pub fn set_element_rect(&mut self, id: u32, rect: Rect) -> bool {
        if let Some(element) = self.elements.get_mut(&id) {
            element.rect = rect;
            true
        } else {
            false
        }
    }

    pub fn set_element_visible(&mut self, id: u32, visible: bool) -> bool {
        if let Some(element) = self.elements.get_mut(&id) {
            element.visible = visible;
            true
        } else {
            false
        }
    }

    pub fn set_element_enabled(&mut self, id: u32, enabled: bool) -> bool {
        if let Some(element) = self.elements.get_mut(&id) {
            element.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn find_element_at_position(&self, x: f32, y: f32) -> Option<u32> {
        // Find topmost element at position (highest z-order)
        let mut found_element = None;
        let mut highest_z = i32::MIN;

        self.find_element_at_position_recursive(
            &self.root_elements,
            x,
            y,
            &mut found_element,
            &mut highest_z,
        );
        found_element
    }

    fn find_element_at_position_recursive(
        &self,
        elements: &[u32],
        x: f32,
        y: f32,
        found: &mut Option<u32>,
        highest_z: &mut i32,
    ) {
        for &element_id in elements {
            if let Some(element) = self.elements.get(&element_id) {
                if element.visible && element.enabled {
                    let abs_rect = element.get_absolute_rect(self);
                    if abs_rect.contains_point(x, y) && element.z_order > *highest_z {
                        *found = Some(element_id);
                        *highest_z = element.z_order;
                    }

                    // Check children
                    self.find_element_at_position_recursive(
                        &element.children,
                        x,
                        y,
                        found,
                        highest_z,
                    );
                }
            }
        }
    }

    pub fn get_all_visible_elements(&self) -> Vec<u32> {
        let mut visible_elements = Vec::new();
        self.collect_visible_elements(&self.root_elements, &mut visible_elements);

        // Sort by z-order for proper rendering
        visible_elements.sort_by(|&a, &b| {
            let z_a = self.elements.get(&a).map(|e| e.z_order).unwrap_or(0);
            let z_b = self.elements.get(&b).map(|e| e.z_order).unwrap_or(0);
            z_a.cmp(&z_b)
        });

        visible_elements
    }

    fn collect_visible_elements(&self, elements: &[u32], visible: &mut Vec<u32>) {
        for &element_id in elements {
            if let Some(element) = self.elements.get(&element_id) {
                if element.visible {
                    visible.push(element_id);
                    self.collect_visible_elements(&element.children, visible);
                }
            }
        }
    }

    pub fn resize(&mut self, new_width: f32, new_height: f32) {
        let scale_x = new_width / self.screen_width;
        let scale_y = new_height / self.screen_height;

        self.screen_width = new_width;
        self.screen_height = new_height;

        // Scale root elements
        let root_elements = self.root_elements.clone();
        for element_id in root_elements {
            self.scale_element_recursive(element_id, scale_x, scale_y);
        }
    }

    fn scale_element_recursive(&mut self, element_id: u32, scale_x: f32, scale_y: f32) {
        if let Some(element) = self.elements.get_mut(&element_id) {
            element.rect.x *= scale_x;
            element.rect.y *= scale_y;
            element.rect.width *= scale_x;
            element.rect.height *= scale_y;
            element.font_size *= (scale_x + scale_y) * 0.5; // Average scaling for font

            let children = element.children.clone();
            for child_id in children {
                self.scale_element_recursive(child_id, scale_x, scale_y);
            }
        }
    }

    pub fn get_screen_size(&self) -> (f32, f32) {
        (self.screen_width, self.screen_height)
    }
}

/// Main Menu Layout - matches C++ MainMenu button layout
impl UILayoutManager {
    pub fn create_main_menu_layout(&mut self) -> HashMap<String, u32> {
        let mut buttons = HashMap::new();
        let (screen_w, screen_h) = self.get_screen_size();

        // Main menu background
        let background_id = self.create_element("MainMenuBackground");
        if let Some(bg) = self.get_element_mut(background_id) {
            bg.rect = Rect::new(0.0, 0.0, screen_w, screen_h);
            bg.background_color = Vec4::new(0.0, 0.0, 0.0, 0.8);
            bg.z_order = -100;
        }

        // Calculate button positioning (matching C++ layout)
        let button_width = 200.0;
        let button_height = 50.0;
        let button_spacing = 20.0;
        let start_x = screen_w * 0.1; // 10% from left
        let start_y = screen_h * 0.3; // 30% from top

        let button_names = [
            "SinglePlayer",
            "Skirmish",
            "Network",
            "Options",
            "LoadReplay",
            "Credits",
            "Exit",
        ];

        for (i, &button_name) in button_names.iter().enumerate() {
            let button_id = self.create_element(button_name);
            if let Some(button) = self.get_element_mut(button_id) {
                button.rect = Rect::new(
                    start_x,
                    start_y + i as f32 * (button_height + button_spacing),
                    button_width,
                    button_height,
                );
                button.text = button_name.to_string();
                button.font_size = 16.0;
                button.background_color = Vec4::new(0.3, 0.3, 0.3, 0.9);
                button.text_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
                button.border_color = Vec4::new(0.6, 0.6, 0.6, 1.0);
                button.z_order = 10;
            }
            buttons.insert(button_name.to_string(), button_id);
        }

        // Logo/Title area
        let title_id = self.create_element("GameTitle");
        if let Some(title) = self.get_element_mut(title_id) {
            title.rect = Rect::new(screen_w * 0.1, screen_h * 0.1, screen_w * 0.8, 100.0);
            title.text = "COMMAND & CONQUER GENERALS".to_string();
            title.font_size = 32.0;
            title.text_color = Vec4::new(1.0, 0.8, 0.0, 1.0); // Gold
            title.background_color = Vec4::new(0.0, 0.0, 0.0, 0.0); // Transparent
            title.alignment = Alignment::Center;
            title.z_order = 20;
        }

        buttons
    }

    pub fn create_control_bar_layout(&mut self) -> HashMap<String, u32> {
        let mut elements = HashMap::new();
        let (screen_w, screen_h) = self.get_screen_size();

        // Control bar background (bottom of screen)
        let control_bar_height = 150.0;
        let control_bar_y = screen_h - control_bar_height;

        let background_id = self.create_element("ControlBarBackground");
        if let Some(bg) = self.get_element_mut(background_id) {
            bg.rect = Rect::new(0.0, control_bar_y, screen_w, control_bar_height);
            bg.background_color = Vec4::new(0.1, 0.1, 0.1, 0.9);
            bg.z_order = 100;
        }

        // Resource display (top-left)
        let resource_panel_id = self.create_element("ResourcePanel");
        if let Some(panel) = self.get_element_mut(resource_panel_id) {
            panel.rect = Rect::new(10.0, 10.0, 300.0, 40.0);
            panel.background_color = Vec4::new(0.0, 0.0, 0.0, 0.7);
            panel.z_order = 200;
        }

        // Command buttons (center-right)
        let command_panel_width = 300.0;
        let command_panel_x = screen_w - command_panel_width - 10.0;
        let command_panel_id = self.create_element("CommandPanel");
        if let Some(panel) = self.get_element_mut(command_panel_id) {
            panel.rect = Rect::new(
                command_panel_x,
                control_bar_y + 10.0,
                command_panel_width,
                control_bar_height - 20.0,
            );
            panel.background_color = Vec4::new(0.2, 0.2, 0.2, 0.8);
            panel.z_order = 110;
        }

        // Create command buttons grid (3x4 matching C++ layout)
        let button_size = 60.0;
        let button_gap = 5.0;
        for row in 0..3 {
            for col in 0..4 {
                let button_name = format!("CommandButton{}_{}", row, col);
                let button_id = self.create_element(&button_name);
                if let Some(button) = self.get_element_mut(button_id) {
                    button.rect = Rect::new(
                        command_panel_x + 10.0 + col as f32 * (button_size + button_gap),
                        control_bar_y + 20.0 + row as f32 * (button_size + button_gap),
                        button_size,
                        button_size,
                    );
                    button.background_color = Vec4::new(0.4, 0.4, 0.4, 1.0);
                    button.border_color = Vec4::new(0.7, 0.7, 0.7, 1.0);
                    button.z_order = 120;
                }
                elements.insert(button_name, button_id);
            }
        }

        // Minimap (bottom-left)
        let minimap_size = 120.0;
        let minimap_id = self.create_element("Minimap");
        if let Some(minimap) = self.get_element_mut(minimap_id) {
            minimap.rect = Rect::new(10.0, control_bar_y + 20.0, minimap_size, minimap_size);
            minimap.background_color = Vec4::new(0.0, 0.3, 0.0, 1.0); // Dark green
            minimap.border_color = Vec4::new(0.5, 0.5, 0.5, 1.0);
            minimap.z_order = 110;
        }

        elements.insert("ControlBar".to_string(), background_id);
        elements.insert("ResourcePanel".to_string(), resource_panel_id);
        elements.insert("CommandPanel".to_string(), command_panel_id);
        elements.insert("Minimap".to_string(), minimap_id);
        elements
    }
}

impl UILayoutManager {
    /// Simple faction selection layout with faction buttons.
    pub fn create_faction_selection_layout(&mut self) -> HashMap<String, u32> {
        let mut elements = HashMap::new();
        let (screen_w, screen_h) = self.get_screen_size();

        let background_id = self.create_element("FactionBackground");
        if let Some(bg) = self.get_element_mut(background_id) {
            bg.rect = Rect::new(
                screen_w * 0.2,
                screen_h * 0.2,
                screen_w * 0.6,
                screen_h * 0.6,
            );
            bg.background_color = Vec4::new(0.0, 0.0, 0.0, 0.85);
            bg.z_order = 150;
        }

        let factions = ["USA", "China", "GLA"];
        for (i, name) in factions.iter().enumerate() {
            let btn_id = self.create_element(&format!("Faction{}", name));
            if let Some(btn) = self.get_element_mut(btn_id) {
                btn.rect = Rect::new(
                    screen_w * 0.25,
                    screen_h * 0.3 + i as f32 * 80.0,
                    screen_w * 0.5,
                    60.0,
                );
                btn.text = name.to_string();
                btn.font_size = 20.0;
                btn.background_color = Vec4::new(0.15, 0.15, 0.2, 0.95);
                btn.text_color = Vec4::new(0.9, 0.9, 0.9, 1.0);
                btn.border_color = Vec4::new(0.6, 0.6, 0.6, 1.0);
                btn.z_order = 160;
            }
            elements.insert(format!("Faction{}", name), btn_id);
        }

        let start_id = self.create_element("FactionStart");
        if let Some(start) = self.get_element_mut(start_id) {
            start.rect = Rect::new(screen_w * 0.25, screen_h * 0.55, screen_w * 0.5, 60.0);
            start.text = "Start Game".to_string();
            start.font_size = 22.0;
            start.background_color = Vec4::new(0.25, 0.4, 0.25, 0.95);
            start.text_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
            start.border_color = Vec4::new(0.6, 0.8, 0.6, 1.0);
            start.z_order = 170;
        }
        elements.insert("FactionStart".to_string(), start_id);

        elements.insert("FactionBackground".to_string(), background_id);
        elements
    }

    /// Pause menu overlay: resume, options, quit.
    pub fn create_pause_menu_layout(&mut self) -> HashMap<String, u32> {
        let mut elements = HashMap::new();
        let (screen_w, screen_h) = self.get_screen_size();

        let bg_id = self.create_element("PauseOverlay");
        if let Some(bg) = self.get_element_mut(bg_id) {
            bg.rect = Rect::new(
                screen_w * 0.3,
                screen_h * 0.25,
                screen_w * 0.4,
                screen_h * 0.5,
            );
            bg.background_color = Vec4::new(0.05, 0.05, 0.05, 0.85);
            bg.z_order = 150;
        }

        let items = ["Resume", "Options", "QuitToMenu"];
        for (i, name) in items.iter().enumerate() {
            let btn_id = self.create_element(&format!("Pause{}", name));
            if let Some(btn) = self.get_element_mut(btn_id) {
                btn.rect = Rect::new(
                    screen_w * 0.32,
                    screen_h * 0.35 + i as f32 * 70.0,
                    screen_w * 0.36,
                    50.0,
                );
                btn.text = name.replace("QuitToMenu", "Quit to Menu");
                btn.font_size = 18.0;
                btn.background_color = Vec4::new(0.2, 0.2, 0.2, 0.95);
                btn.text_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
                btn.border_color = Vec4::new(0.6, 0.6, 0.6, 1.0);
                btn.z_order = 160;
            }
            elements.insert(format!("Pause{}", name), btn_id);
        }

        elements.insert("PauseOverlay".to_string(), bg_id);
        elements
    }

    /// Victory layout with summary and exit button.
    pub fn create_victory_layout(&mut self) -> HashMap<String, u32> {
        let mut elements = HashMap::new();
        let (screen_w, screen_h) = self.get_screen_size();

        let bg_id = self.create_element("VictoryOverlay");
        if let Some(bg) = self.get_element_mut(bg_id) {
            bg.rect = Rect::new(
                screen_w * 0.25,
                screen_h * 0.2,
                screen_w * 0.5,
                screen_h * 0.6,
            );
            bg.background_color = Vec4::new(0.0, 0.0, 0.0, 0.9);
            bg.z_order = 190;
        }
        elements.insert("VictoryOverlay".to_string(), bg_id);

        let title_id = self.create_element("VictoryTitle");
        if let Some(title) = self.get_element_mut(title_id) {
            title.rect = Rect::new(screen_w * 0.25, screen_h * 0.22, screen_w * 0.5, 80.0);
            title.text = "Victory!".to_string();
            title.font_size = 28.0;
            title.text_color = Vec4::new(0.0, 1.0, 0.0, 1.0);
            title.background_color = Vec4::new(0.0, 0.0, 0.0, 0.0);
            title.alignment = Alignment::Center;
            title.z_order = 200;
        }
        elements.insert("VictoryTitle".to_string(), title_id);

        let exit_id = self.create_element("VictoryExit");
        if let Some(exit) = self.get_element_mut(exit_id) {
            exit.rect = Rect::new(screen_w * 0.35, screen_h * 0.7, screen_w * 0.3, 50.0);
            exit.text = "Exit to Menu".to_string();
            exit.font_size = 20.0;
            exit.background_color = Vec4::new(0.3, 0.3, 0.3, 0.95);
            exit.text_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
            exit.border_color = Vec4::new(0.6, 0.6, 0.6, 1.0);
            exit.z_order = 200;
        }
        elements.insert("VictoryExit".to_string(), exit_id);

        elements
    }

    /// Loading screen layout with progress placeholder.
    pub fn create_loading_layout(&mut self) -> HashMap<String, u32> {
        let mut elements = HashMap::new();
        let (screen_w, screen_h) = self.get_screen_size();

        let bg_id = self.create_element("LoadingOverlay");
        if let Some(bg) = self.get_element_mut(bg_id) {
            bg.rect = Rect::new(0.0, 0.0, screen_w, screen_h);
            bg.background_color = Vec4::new(0.0, 0.0, 0.0, 0.85);
            bg.z_order = 180;
        }
        elements.insert("LoadingOverlay".to_string(), bg_id);

        let text_id = self.create_element("LoadingText");
        if let Some(txt) = self.get_element_mut(text_id) {
            txt.rect = Rect::new(screen_w * 0.3, screen_h * 0.45, screen_w * 0.4, 60.0);
            txt.text = "Loading...".to_string();
            txt.font_size = 24.0;
            txt.text_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
            txt.background_color = Vec4::new(0.0, 0.0, 0.0, 0.0);
            txt.alignment = Alignment::Center;
            txt.z_order = 190;
        }
        elements.insert("LoadingText".to_string(), text_id);

        elements
    }
}
