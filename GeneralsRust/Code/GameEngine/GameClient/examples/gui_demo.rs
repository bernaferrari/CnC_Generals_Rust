//! # GUI System Demo
//!
//! Demonstrates the complete GUI system for Command & Conquer Generals Zero Hour.
//! Shows window management, font rendering, shell menus, and gadget controls.
//!
//! Run with: `cargo run --example gui_demo`

use std::collections::HashMap;
use std::time::Instant;

const WINDOW_TITLE: &str = "C&C Generals Zero Hour - GUI Demo";
const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;

/// Window status flags matching C++ GameWindow
#[derive(Debug, Clone, Copy, Default)]
struct WindowStatus {
    enabled: bool,
    visible: bool,
    selected: bool,
    hidden: bool,
    pressed: bool,
}

/// Simulated window ID
type WindowId = u32;

/// Window data for the demo
#[derive(Debug, Clone)]
struct Window {
    id: WindowId,
    name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    status: WindowStatus,
    text: String,
    children: Vec<WindowId>,
    parent: Option<WindowId>,
}

impl Window {
    fn new(id: WindowId, name: &str, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            name: name.to_string(),
            x,
            y,
            width,
            height,
            status: WindowStatus {
                enabled: true,
                visible: true,
                ..Default::default()
            },
            text: String::new(),
            children: Vec::new(),
            parent: None,
        }
    }

    fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.status.enabled = enabled;
    }

    fn set_visible(&mut self, visible: bool) {
        self.status.visible = visible;
    }

    fn contains_point(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x + self.width as i32
            && py >= self.y
            && py < self.y + self.height as i32
    }
}

/// Font description matching C++ FontDesc
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FontDesc {
    name: String,
    size: u32,
    bold: bool,
    italic: bool,
}

impl FontDesc {
    fn new(name: &str, size: u32) -> Self {
        Self {
            name: name.to_string(),
            size,
            bold: false,
            italic: false,
        }
    }

    fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
}

/// Font metrics for text layout
#[derive(Debug, Clone)]
struct FontMetrics {
    ascent: i32,
    descent: i32,
    line_height: i32,
    average_char_width: i32,
}

/// Font library for managing loaded fonts
struct FontLibrary {
    fonts: HashMap<FontDesc, FontMetrics>,
    default_font: Option<FontDesc>,
}

impl FontLibrary {
    fn new() -> Self {
        Self {
            fonts: HashMap::new(),
            default_font: None,
        }
    }

    fn load_font(&mut self, desc: FontDesc, metrics: FontMetrics) {
        println!(
            "  Loaded font: {} ({}pt, bold={}, italic={})",
            desc.name, desc.size, desc.bold, desc.italic
        );
        self.fonts.insert(desc, metrics);
    }

    fn get_font(&self, desc: &FontDesc) -> Option<&FontMetrics> {
        self.fonts.get(desc)
    }

    fn set_default_font(&mut self, desc: FontDesc) {
        println!("  Default font set to: {} ({}pt)", desc.name, desc.size);
        self.default_font = Some(desc);
    }

    fn get_default_metrics(&self) -> Option<&FontMetrics> {
        self.default_font.as_ref().and_then(|d| self.fonts.get(d))
    }
}

/// Window manager for the demo
struct WindowManager {
    windows: HashMap<WindowId, Window>,
    next_id: WindowId,
    focused: Option<WindowId>,
    mouse_x: i32,
    mouse_y: i32,
}

impl WindowManager {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            next_id: 1,
            focused: None,
            mouse_x: 0,
            mouse_y: 0,
        }
    }

    fn create_window(
        &mut self,
        parent: Option<WindowId>,
        name: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> WindowId {
        let id = self.next_id;
        self.next_id += 1;

        let mut window = Window::new(id, name, x, y, width, height);
        window.parent = parent;

        if let Some(parent_id) = parent {
            if let Some(parent_win) = self.windows.get_mut(&parent_id) {
                parent_win.children.push(id);
            }
        }

        self.windows.insert(id, window);
        println!(
            "  Created window '{}' (id={}, {}x{} at {},{})",
            name, id, width, height, x, y
        );
        id
    }

    fn get_window(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(&id)
    }

    fn get_window_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.get_mut(&id)
    }

    fn hit_test(&self, x: i32, y: i32) -> Option<WindowId> {
        // Test children first (top-most)
        for window in self.windows.values().rev() {
            if window.status.visible && window.contains_point(x, y) {
                return Some(window.id);
            }
        }
        None
    }

    fn set_focus(&mut self, id: Option<WindowId>) {
        self.focused = id;
        if let Some(win_id) = id {
            if let Some(win) = self.windows.get(&win_id) {
                println!("  Focus set to window '{}' (id={})", win.name, win_id);
            }
        }
    }

    fn window_count(&self) -> usize {
        self.windows.len()
    }
}

/// Shell menu state for stack-based navigation
#[derive(Debug)]
struct ShellMenu {
    name: String,
    window_id: Option<WindowId>,
    is_popup: bool,
}

/// Shell system for menu management
struct Shell {
    menu_stack: Vec<ShellMenu>,
    current_screen: String,
}

impl Shell {
    fn new() -> Self {
        Self {
            menu_stack: Vec::new(),
            current_screen: "None".to_string(),
        }
    }

    fn push_menu(&mut self, name: &str, is_popup: bool) {
        let menu = ShellMenu {
            name: name.to_string(),
            window_id: None,
            is_popup,
        };
        println!("  Shell: Pushing menu '{}' (popup={})", name, is_popup);
        self.current_screen = name.to_string();
        self.menu_stack.push(menu);
    }

    fn pop_menu(&mut self) -> Option<ShellMenu> {
        let menu = self.menu_stack.pop();
        if let Some(ref m) = menu {
            println!("  Shell: Popped menu '{}'", m.name);
        }
        self.current_screen = self
            .menu_stack
            .last()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "None".to_string());
        menu
    }

    fn stack_depth(&self) -> usize {
        self.menu_stack.len()
    }

    fn current_screen(&self) -> &str {
        &self.current_screen
    }
}

/// Main GUI demo application
struct GUIDemo {
    window_manager: WindowManager,
    font_library: FontLibrary,
    shell: Shell,
}

impl GUIDemo {
    fn new() -> Self {
        Self {
            window_manager: WindowManager::new(),
            font_library: FontLibrary::new(),
            shell: Shell::new(),
        }
    }

    fn run(&mut self) {
        println!("=== Command & Conquer Generals Zero Hour - GUI System Demo ===\n");

        self.demo_font_system();
        self.demo_window_management();
        self.demo_shell_system();
        self.demo_input_handling();

        println!("\n=== GUI Demo Complete ===");
    }

    fn demo_font_system(&mut self) {
        println!("=== Font System Demo ===\n");

        // Load fonts matching the original game
        self.font_library.load_font(
            FontDesc::new("Arial", 10),
            FontMetrics {
                ascent: 9,
                descent: 2,
                line_height: 12,
                average_char_width: 6,
            },
        );

        self.font_library.load_font(
            FontDesc::new("Arial", 12).bold(),
            FontMetrics {
                ascent: 11,
                descent: 3,
                line_height: 14,
                average_char_width: 7,
            },
        );

        self.font_library.load_font(
            FontDesc::new("Generals", 16),
            FontMetrics {
                ascent: 14,
                descent: 4,
                line_height: 18,
                average_char_width: 10,
            },
        );

        self.font_library.load_font(
            FontDesc::new("Generals", 24).bold(),
            FontMetrics {
                ascent: 22,
                descent: 6,
                line_height: 28,
                average_char_width: 15,
            },
        );

        self.font_library
            .set_default_font(FontDesc::new("Arial", 12));

        // Demonstrate text measurement
        if let Some(metrics) = self.font_library.get_default_metrics() {
            let sample_text = "Welcome, Commander!";
            let text_width = sample_text.len() as i32 * metrics.average_char_width;
            println!(
                "\n  Text measurement for '{}': {}px wide, {}px tall",
                sample_text, text_width, metrics.line_height
            );
        }
    }

    fn demo_window_management(&mut self) {
        println!("\n=== Window Management Demo ===\n");

        // Create main window hierarchy matching the game UI
        let root = self.window_manager.create_window(
            None,
            "RootWindow",
            0,
            0,
            DEFAULT_WIDTH,
            DEFAULT_HEIGHT,
        );

        // Create main menu window
        let main_menu = self.window_manager.create_window(
            Some(root),
            "MainMenu.wnd",
            0,
            0,
            DEFAULT_WIDTH,
            DEFAULT_HEIGHT,
        );

        // Create control bar at bottom
        let control_bar = self.window_manager.create_window(
            Some(root),
            "ControlBar.wnd",
            0,
            DEFAULT_HEIGHT as i32 - 120,
            DEFAULT_WIDTH,
            120,
        );

        // Create buttons on main menu
        let btn_single = self.window_manager.create_window(
            Some(main_menu),
            "ButtonSinglePlayer",
            100,
            200,
            200,
            40,
        );

        let btn_multi = self.window_manager.create_window(
            Some(main_menu),
            "ButtonMultiplayer",
            100,
            250,
            200,
            40,
        );

        let btn_options =
            self.window_manager
                .create_window(Some(main_menu), "ButtonOptions", 100, 300, 200, 40);

        let btn_quit =
            self.window_manager
                .create_window(Some(main_menu), "ButtonQuit", 100, 350, 200, 40);

        // Set button text
        if let Some(win) = self.window_manager.get_window_mut(btn_single) {
            win.set_text("Single Player");
        }
        if let Some(win) = self.window_manager.get_window_mut(btn_multi) {
            win.set_text("Multiplayer");
        }
        if let Some(win) = self.window_manager.get_window_mut(btn_options) {
            win.set_text("Options");
        }
        if let Some(win) = self.window_manager.get_window_mut(btn_quit) {
            win.set_text("Quit Game");
        }

        // Demo focus
        self.window_manager.set_focus(Some(btn_single));

        println!(
            "\n  Total windows created: {}",
            self.window_manager.window_count()
        );
    }

    fn demo_shell_system(&mut self) {
        println!("\n=== Shell Menu System Demo ===\n");

        // Simulate menu navigation
        self.shell.push_menu("Menus/MainMenu.wnd", false);
        self.shell.push_menu("Menus/SinglePlayerMenu.wnd", false);
        self.shell.push_menu("Menus/SkirmishMenu.wnd", false);

        println!("\n  Current screen: {}", self.shell.current_screen());
        println!("  Stack depth: {}", self.shell.stack_depth());

        // Navigate back
        println!("\n  Navigating back...");
        self.shell.pop_menu();
        println!("  Current screen: {}", self.shell.current_screen());

        self.shell.pop_menu();
        println!("  Current screen: {}", self.shell.current_screen());
    }

    fn demo_input_handling(&mut self) {
        println!("\n=== Input Handling Demo ===\n");

        // Simulate mouse events
        let test_points = [(150, 220), (50, 50), (150, 370), (640, 360)];
        let descriptions = [
            "Single Player button",
            "Background area",
            "Quit Game button",
            "Control bar area",
        ];

        for ((x, y), desc) in test_points.iter().zip(descriptions.iter()) {
            match self.window_manager.hit_test(*x, *y) {
                Some(win_id) => {
                    if let Some(win) = self.window_manager.get_window(win_id) {
                        println!(
                            "  Hit test at ({}, {}): Window '{}' ({})",
                            x, y, win.name, desc
                        );
                    }
                }
                None => {
                    println!("  Hit test at ({}, {}): No window ({})", x, y, desc);
                }
            }
        }
    }
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("C&C Generals Zero Hour - GUI System Demo");
    println!("=========================================\n");

    let mut demo = GUIDemo::new();
    demo.run();

    println!("\nGUI system demonstration complete.");
    println!("This demo shows the window management, font system,");
    println!("shell navigation, and input handling subsystems.");
}
