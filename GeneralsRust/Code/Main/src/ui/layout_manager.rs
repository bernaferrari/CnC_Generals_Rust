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
    pub hover_texture_id: Option<u32>,
    pub pressed_texture_id: Option<u32>,
    pub disabled_texture_id: Option<u32>,
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
            hover_texture_id: None,
            pressed_texture_id: None,
            disabled_texture_id: None,
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
            z_a.cmp(&z_b).then(a.cmp(&b))
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
        let scale_x = screen_w / 800.0;
        let scale_y = screen_h / 600.0;

        let scale_rect = |left: f32, top: f32, right: f32, bottom: f32| {
            Rect::new(
                left * scale_x,
                top * scale_y,
                (right - left) * scale_x,
                (bottom - top) * scale_y,
            )
        };
        let rgba = |r: f32, g: f32, b: f32, a: f32| Vec4::new(r / 255.0, g / 255.0, b / 255.0, a / 255.0);

        // Main menu background
        let background_id = self.create_element("MainMenuBackground");
        if let Some(bg) = self.get_element_mut(background_id) {
            bg.rect = Rect::new(0.0, 0.0, screen_w, screen_h);
            bg.background_color = Vec4::new(0.0, 0.0, 0.0, 0.25);
            bg.enabled = false;
            bg.z_order = -100;
        }
        buttons.insert("MainMenuBackground".to_string(), background_id);

        // C++ shell menu ruler overlay (MainMenu.wnd:MainMenuRuler)
        let ruler_id = self.create_element("MainMenuRuler");
        if let Some(ruler) = self.get_element_mut(ruler_id) {
            ruler.rect = scale_rect(0.0, 0.0, 800.0, 600.0);
            ruler.background_color = Vec4::new(1.0, 1.0, 1.0, 0.0);
            ruler.enabled = false;
            ruler.z_order = 0;
        }
        buttons.insert("MainMenuRuler".to_string(), ruler_id);

        // C++ shell menu logo area (MainMenu.wnd:Logo).
        let title_id = self.create_element("MainMenuTitle");
        if let Some(title) = self.get_element_mut(title_id) {
            title.rect = scale_rect(504.0, 16.0, 791.0, 110.0);
            title.text = String::new();
            title.font_name = "Generals".to_string();
            title.font_size = 26.0 * ((scale_x + scale_y) * 0.5);
            title.text_color = rgba(186.0, 255.0, 12.0, 255.0);
            title.background_color = Vec4::new(0.0, 0.0, 0.0, 0.0);
            title.enabled = false;
            title.alignment = Alignment::Center;
            title.z_order = 30;
        }
        buttons.insert("MainMenuTitle".to_string(), title_id);

        // Panel containers from MainMenu.wnd
        let panel_color = rgba(0.0, 0.0, 0.0, 126.0);
        let panel_border = rgba(47.0, 55.0, 168.0, 255.0);

        let map_border2_id = self.create_element("MapBorder2");
        if let Some(panel) = self.get_element_mut(map_border2_id) {
            panel.rect = scale_rect(532.0, 108.0, 756.0, 360.0);
            panel.background_color = panel_color;
            panel.border_color = panel_border;
            panel.enabled = false;
            panel.z_order = 10;
        }
        buttons.insert("MapBorder2".to_string(), map_border2_id);

        let map_border_id = self.create_element("MapBorder");
        if let Some(panel) = self.get_element_mut(map_border_id) {
            panel.rect = scale_rect(532.0, 108.0, 756.0, 360.0);
            panel.background_color = panel_color;
            panel.border_color = panel_border;
            panel.enabled = false;
            panel.z_order = 10;
            panel.visible = false;
        }
        buttons.insert("MapBorder".to_string(), map_border_id);

        let map_border1_id = self.create_element("MapBorder1");
        if let Some(panel) = self.get_element_mut(map_border1_id) {
            panel.rect = scale_rect(532.0, 108.0, 756.0, 240.0);
            panel.background_color = panel_color;
            panel.border_color = panel_border;
            panel.enabled = false;
            panel.z_order = 10;
            panel.visible = false;
        }
        buttons.insert("MapBorder1".to_string(), map_border1_id);

        let map_border3_id = self.create_element("MapBorder3");
        if let Some(panel) = self.get_element_mut(map_border3_id) {
            panel.rect = scale_rect(532.0, 108.0, 756.0, 240.0);
            panel.background_color = panel_color;
            panel.border_color = panel_border;
            panel.enabled = false;
            panel.z_order = 10;
            panel.visible = false;
        }
        buttons.insert("MapBorder3".to_string(), map_border3_id);

        let map_border4_id = self.create_element("MapBorder4");
        if let Some(panel) = self.get_element_mut(map_border4_id) {
            panel.rect = scale_rect(532.0, 108.0, 756.0, 320.0);
            panel.background_color = panel_color;
            panel.border_color = panel_border;
            panel.enabled = false;
            panel.z_order = 10;
            panel.visible = false;
        }
        buttons.insert("MapBorder4".to_string(), map_border4_id);

        let button_text = rgba(255.0, 255.0, 255.0, 255.0);
        let button_bg = rgba(47.0, 55.0, 168.0, 220.0);
        let button_border = rgba(40.0, 46.0, 132.0, 255.0);
        let button_font = 15.0 * ((scale_x + scale_y) * 0.5);

        // Difficulty panel static label (MapBorder4)
        let difficulty_label_id = self.create_element("StaticTextSelectDifficulty");
        if let Some(label) = self.get_element_mut(difficulty_label_id) {
            label.rect = scale_rect(540.0, 116.0, 748.0, 151.0);
            label.text = "Select Difficulty".to_string();
            label.font_name = "Generals".to_string();
            label.font_size = button_font;
            label.text_color = rgba(186.0, 255.0, 12.0, 255.0);
            label.background_color = Vec4::new(0.0, 0.0, 0.0, 0.0);
            label.enabled = false;
            label.alignment = Alignment::Center;
            label.padding = Vec4::ZERO;
            label.z_order = 20;
            label.visible = false;
        }
        buttons.insert("StaticTextSelectDifficulty".to_string(), difficulty_label_id);

        let mut add_button = |name: &str, text: &str, left: f32, top: f32, right: f32, bottom: f32, visible: bool| {
            let button_id = self.create_element(name);
            if let Some(button) = self.get_element_mut(button_id) {
                button.rect = scale_rect(left, top, right, bottom);
                button.text = text.to_string();
                button.font_name = "Generals".to_string();
                button.font_size = button_font;
                button.text_color = button_text;
                button.background_color = button_bg;
                button.border_color = button_border;
                button.alignment = Alignment::CenterLeft;
                button.padding = Vec4::new(26.0 * scale_x, 0.0, 0.0, 0.0);
                button.z_order = 20;
                button.visible = visible;
            }
            buttons.insert(name.to_string(), button_id);
        };

        // Main menu panel (MapBorder2)
        add_button(
            "ButtonSinglePlayer",
            "Single Player",
            540.0,
            116.0,
            748.0,
            152.0,
            true,
        );
        add_button(
            "ButtonMultiplayer",
            "Multiplayer",
            540.0,
            156.0,
            748.0,
            192.0,
            true,
        );
        add_button(
            "ButtonLoadReplay",
            "Replay Menu",
            540.0,
            196.0,
            748.0,
            231.0,
            true,
        );
        add_button(
            "ButtonOptions",
            "Options",
            540.0,
            236.0,
            748.0,
            272.0,
            true,
        );
        add_button(
            "ButtonCredits",
            "Credits",
            540.0,
            276.0,
            748.0,
            312.0,
            true,
        );
        add_button(
            "ButtonExit",
            "Exit",
            540.0,
            316.0,
            748.0,
            352.0,
            true,
        );

        // Single-player panel (MapBorder)
        add_button(
            "ButtonUSA",
            "USA",
            540.0,
            116.0,
            748.0,
            152.0,
            false,
        );
        add_button(
            "ButtonGLA",
            "GLA",
            540.0,
            156.0,
            748.0,
            192.0,
            false,
        );
        add_button(
            "ButtonChina",
            "China",
            540.0,
            196.0,
            748.0,
            231.0,
            false,
        );
        add_button(
            "ButtonChallenge",
            "Generals Challenge",
            540.0,
            236.0,
            748.0,
            272.0,
            false,
        );
        add_button(
            "ButtonSkirmish",
            "Skirmish",
            540.0,
            276.0,
            748.0,
            312.0,
            false,
        );
        add_button(
            "ButtonSingleBack",
            "Back",
            540.0,
            316.0,
            748.0,
            351.0,
            false,
        );

        // Multiplayer panel (MapBorder1)
        add_button(
            "ButtonOnline",
            "Online",
            540.0,
            116.0,
            748.0,
            151.0,
            false,
        );
        add_button(
            "ButtonNetwork",
            "Network",
            540.0,
            156.0,
            748.0,
            191.0,
            false,
        );
        add_button(
            "ButtonMultiBack",
            "Back",
            540.0,
            196.0,
            748.0,
            232.0,
            false,
        );

        // Load/replay panel (MapBorder3)
        add_button(
            "ButtonLoadGame",
            "Load Game",
            540.0,
            116.0,
            748.0,
            151.0,
            false,
        );
        add_button(
            "ButtonReplay",
            "Load Replay",
            540.0,
            156.0,
            748.0,
            191.0,
            false,
        );
        add_button(
            "ButtonLoadReplayBack",
            "Back",
            540.0,
            196.0,
            748.0,
            232.0,
            false,
        );

        // Difficulty panel (MapBorder4)
        add_button(
            "ButtonEasy",
            "Easy",
            540.0,
            156.0,
            748.0,
            191.0,
            false,
        );
        add_button(
            "ButtonMedium",
            "Medium",
            540.0,
            196.0,
            748.0,
            231.0,
            false,
        );
        add_button(
            "ButtonHard",
            "Hard",
            540.0,
            236.0,
            748.0,
            272.0,
            false,
        );
        add_button(
            "ButtonDiffBack",
            "Back",
            540.0,
            276.0,
            748.0,
            312.0,
            false,
        );

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
            bg.enabled = false;
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
            bg.enabled = false;
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
            bg.enabled = false;
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
            title.enabled = false;
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
            bg.background_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
            bg.enabled = false;
            bg.z_order = 180;
        }
        elements.insert("LoadingOverlay".to_string(), bg_id);

        let text_id = self.create_element("LoadingText");
        if let Some(txt) = self.get_element_mut(text_id) {
            txt.rect = Rect::new(screen_w * 0.28, screen_h * 0.78, screen_w * 0.44, 44.0);
            txt.text = "Loading assets... 0%".to_string();
            txt.font_size = 20.0;
            txt.text_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
            txt.background_color = Vec4::new(0.0, 0.0, 0.0, 0.0);
            txt.enabled = false;
            txt.alignment = Alignment::Center;
            txt.z_order = 195;
        }
        elements.insert("LoadingText".to_string(), text_id);

        let bar_track_id = self.create_element("LoadingProgressTrack");
        if let Some(track) = self.get_element_mut(bar_track_id) {
            track.rect = Rect::new(screen_w * 0.25, screen_h * 0.86, screen_w * 0.5, 18.0);
            track.background_color = Vec4::new(0.08, 0.08, 0.08, 0.88);
            track.border_color = Vec4::new(0.75, 0.75, 0.75, 1.0);
            track.enabled = false;
            track.z_order = 196;
        }
        elements.insert("LoadingProgressTrack".to_string(), bar_track_id);

        let bar_fill_id = self.create_element("LoadingProgressFill");
        if let Some(fill) = self.get_element_mut(bar_fill_id) {
            fill.rect = Rect::new(screen_w * 0.25 + 2.0, screen_h * 0.86 + 2.0, 0.0, 14.0);
            fill.background_color = Vec4::new(0.88, 0.76, 0.24, 0.98);
            fill.border_color = Vec4::new(0.95, 0.88, 0.35, 1.0);
            fill.enabled = false;
            fill.z_order = 197;
        }
        elements.insert("LoadingProgressFill".to_string(), bar_fill_id);

        elements
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_menu_non_interactive_elements_do_not_capture_hits() {
        let mut layout = UILayoutManager::new(1920.0, 1080.0);
        let buttons = layout.create_main_menu_layout();

        let background = layout
            .get_element_by_name("MainMenuBackground")
            .expect("missing main menu background");
        let title = layout
            .get_element_by_name("MainMenuTitle")
            .expect("missing game title");
        assert!(!background.enabled);
        assert!(!title.enabled);

        // Center of screen should be empty hit-test space for menu (buttons are right column).
        assert!(layout.find_element_at_position(960.0, 540.0).is_none());

        let single_player_id = *buttons
            .get("ButtonSinglePlayer")
            .expect("missing single-player button");
        let button = layout
            .get_element(single_player_id)
            .expect("single-player button not found");
        let rect = button.get_absolute_rect(&layout);
        let hit = layout.find_element_at_position(rect.x + 4.0, rect.y + 4.0);
        assert_eq!(hit, Some(single_player_id));
    }
}
