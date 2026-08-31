// Split from `gui/shell/base.rs` dump. Included by `base/mod.rs`.
// C++ ShellMenuScheme.cpp: menu scheme theming and ShellMenuScheme.ini discovery.
/// Shell menu scheme line for decorative elements
#[derive(Debug, Clone)]
pub struct ShellMenuSchemeLine {
    pub start_pos: Coord2D,
    pub end_pos: Coord2D,
    pub width: i32,
    pub color: Color,
}

impl ShellMenuSchemeLine {
    pub fn new(start: Coord2D, end: Coord2D, width: i32, color: Color) -> Self {
        Self {
            start_pos: start,
            end_pos: end,
            width,
            color,
        }
    }
}

/// Shell menu scheme image for decorative elements
#[derive(Debug, Clone)]
pub struct ShellMenuSchemeImage {
    pub name: String,
    pub position: Coord2D,
    pub size: Coord2D,
    // In a real implementation, this would hold an image handle
    pub image_data: Option<Vec<u8>>,
}

impl ShellMenuSchemeImage {
    pub fn new(name: String, position: Coord2D, size: Coord2D) -> Self {
        Self {
            name,
            position,
            size,
            image_data: None,
        }
    }
}

/// Shell menu scheme for theming and decoration
#[derive(Debug)]
pub struct ShellMenuScheme {
    pub name: String,
    pub images: Vec<ShellMenuSchemeImage>,
    pub lines: Vec<ShellMenuSchemeLine>,
}

impl ShellMenuScheme {
    pub fn new(name: String) -> Self {
        Self {
            name,
            images: Vec::new(),
            lines: Vec::new(),
        }
    }

    pub fn add_image(&mut self, image: ShellMenuSchemeImage) {
        self.images.push(image);
    }

    pub fn add_line(&mut self, line: ShellMenuSchemeLine) {
        self.lines.push(line);
    }

    pub fn draw(&self) {
        with_window_manager_ref(|manager| {
            for image in &self.images {
                if let Some(mapped) = manager.win_find_image(&image.name) {
                    manager.win_draw_image(
                        &mapped,
                        image.position.x,
                        image.position.y,
                        image.position.x + image.size.x,
                        image.position.y + image.size.y,
                        WIN_COLOR_UNDEFINED,
                    );
                }
            }

            for line in &self.lines {
                let color = ((line.color.a as u32) << 24)
                    | ((line.color.r as u32) << 16)
                    | ((line.color.g as u32) << 8)
                    | line.color.b as u32;
                manager.win_draw_line(
                    color,
                    line.width as f32,
                    line.start_pos.x,
                    line.start_pos.y,
                    line.end_pos.x,
                    line.end_pos.y,
                );
            }
        });
    }
}

/// Manager for shell menu schemes
#[derive(Debug)]
pub struct ShellMenuSchemeManager {
    schemes: HashMap<String, ShellMenuScheme>,
    scheme_order: Vec<String>,
    current_scheme: Option<String>,
}

impl ShellMenuSchemeManager {
    pub fn new() -> Self {
        Self {
            schemes: HashMap::new(),
            scheme_order: Vec::new(),
            current_scheme: None,
        }
    }

    pub fn init(&mut self) -> Result<(), ShellError> {
        log::info!("Initializing shell menu scheme manager");
        self.load_default_scheme_files();
        Ok(())
    }

    pub fn update(&mut self) -> Result<(), ShellError> {
        // Schemes don't need regular updates
        Ok(())
    }

    pub fn set_shell_menu_scheme(&mut self, name: &str) {
        if name.is_empty() {
            self.current_scheme = None;
            return;
        }
        let key = name.to_ascii_lowercase();
        if self.schemes.contains_key(&key) {
            self.current_scheme = Some(key);
            log::debug!("Set shell menu scheme to: {}", name);
        } else {
            // The C++ shell path does not require every menu to have a separate decorative
            // scheme object. Missing placeholder schemes should not spam startup warnings.
            log::debug!("Shell menu scheme not found: {}", name);
        }
    }

    pub fn draw(&self) {
        if let Some(scheme_name) = &self.current_scheme {
            if let Some(scheme) = self.schemes.get(scheme_name) {
                scheme.draw();
            }
        }
    }

    pub fn new_shell_menu_scheme(&mut self, name: String) -> &mut ShellMenuScheme {
        let key = name.trim().to_ascii_lowercase();
        self.schemes.remove(&key);
        self.scheme_order.retain(|existing| existing != &key);
        self.schemes
            .insert(key.clone(), ShellMenuScheme::new(key.clone()));
        self.scheme_order.push(key.clone());
        self.schemes.get_mut(&key).unwrap()
    }

    fn get_shell_menu_scheme_mut(&mut self, name: &str) -> Option<&mut ShellMenuScheme> {
        self.schemes.get_mut(&name.trim().to_ascii_lowercase())
    }

    fn load_default_scheme_files(&mut self) {
        let files = discover_shell_menu_scheme_ini_files();
        for path in files {
            if let Ok(contents) = fs::read_to_string(&path) {
                self.parse_shell_menu_schemes(&contents);
            }
        }
    }

    fn parse_shell_menu_schemes(&mut self, contents: &str) {
        let mut current_scheme: Option<String> = None;
        let mut current_image: Option<ShellMenuSchemeImage> = None;
        let mut current_line: Option<ShellMenuSchemeLine> = None;

        let flush_image = |manager: &mut ShellMenuSchemeManager,
                           scheme_name: &Option<String>,
                           image: &mut Option<ShellMenuSchemeImage>| {
            if let (Some(name), Some(image)) = (scheme_name.as_ref(), image.take()) {
                if let Some(scheme) = manager.get_shell_menu_scheme_mut(name) {
                    scheme.add_image(image);
                }
            }
        };
        let flush_line = |manager: &mut ShellMenuSchemeManager,
                          scheme_name: &Option<String>,
                          line: &mut Option<ShellMenuSchemeLine>| {
            if let (Some(name), Some(line)) = (scheme_name.as_ref(), line.take()) {
                if let Some(scheme) = manager.get_shell_menu_scheme_mut(name) {
                    scheme.add_line(line);
                }
            }
        };

        for raw_line in contents.lines() {
            let line = raw_line
                .split_once(';')
                .map(|(head, _)| head)
                .unwrap_or(raw_line)
                .trim();
            if line.is_empty() {
                continue;
            }
            if line.eq_ignore_ascii_case("End") {
                flush_image(self, &current_scheme, &mut current_image);
                flush_line(self, &current_scheme, &mut current_line);
                current_scheme = None;
                continue;
            }
            if line.eq_ignore_ascii_case("EndImagePart") {
                flush_image(self, &current_scheme, &mut current_image);
                continue;
            }
            if line.eq_ignore_ascii_case("EndLinePart") {
                flush_line(self, &current_scheme, &mut current_line);
                continue;
            }
            if let Some(name) = line.strip_prefix("ShellMenuScheme ") {
                flush_image(self, &current_scheme, &mut current_image);
                flush_line(self, &current_scheme, &mut current_line);
                let name = name.trim().to_string();
                self.new_shell_menu_scheme(name.clone());
                current_scheme = Some(name);
                continue;
            }
            if line.eq_ignore_ascii_case("ImagePart") {
                flush_image(self, &current_scheme, &mut current_image);
                current_image = Some(ShellMenuSchemeImage::new(
                    String::new(),
                    Coord2D::zero(),
                    Coord2D::zero(),
                ));
                continue;
            }
            if line.eq_ignore_ascii_case("LinePart") {
                flush_line(self, &current_scheme, &mut current_line);
                current_line = Some(ShellMenuSchemeLine::new(
                    Coord2D::zero(),
                    Coord2D::zero(),
                    1,
                    Color::transparent(),
                ));
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();

            if let Some(image) = current_image.as_mut() {
                match key {
                    "Position" => image.position = parse_coord2d(value),
                    "Size" => image.size = parse_coord2d(value),
                    "ImageName" => image.name = value.to_string(),
                    _ => {}
                }
                continue;
            }

            if let Some(line_part) = current_line.as_mut() {
                match key {
                    "StartPosition" => line_part.start_pos = parse_coord2d(value),
                    "EndPosition" => line_part.end_pos = parse_coord2d(value),
                    "Color" => line_part.color = parse_color_int(value),
                    "Width" => line_part.width = value.parse().unwrap_or(1),
                    _ => {}
                }
            }
        }

        flush_image(self, &current_scheme, &mut current_image);
        flush_line(self, &current_scheme, &mut current_line);
    }
}

fn parse_coord2d(value: &str) -> Coord2D {
    let mut parts = value.split_whitespace();
    let x = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let y = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    Coord2D::new(x, y)
}

fn parse_color_int(value: &str) -> Color {
    let parsed = value.parse::<u32>().unwrap_or(WIN_COLOR_UNDEFINED);
    Color::new(
        ((parsed >> 16) & 0xFF) as u8,
        ((parsed >> 8) & 0xFF) as u8,
        (parsed & 0xFF) as u8,
        ((parsed >> 24) & 0xFF) as u8,
    )
}

fn push_shell_menu_scheme_ini_file(
    files: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    path: PathBuf,
) {
    if !path.exists() {
        return;
    }
    if let Ok(canonical) = fs::canonicalize(&path) {
        if seen.insert(canonical.clone()) {
            files.push(canonical);
        }
    } else if seen.insert(path.clone()) {
        files.push(path);
    }
}

fn push_unique_root(roots: &mut Vec<PathBuf>, root: PathBuf) {
    if !roots.iter().any(|existing| existing == &root) {
        roots.push(root);
    }
}

fn ordered_shell_menu_scheme_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let mut ancestors: Vec<PathBuf> = parent.ancestors().map(Path::to_path_buf).collect();
            ancestors.reverse();
            for ancestor in ancestors {
                push_unique_root(&mut roots, ancestor);
            }
        }
    }

    if let Ok(current) = std::env::current_dir() {
        let mut ancestors: Vec<PathBuf> = current.ancestors().map(Path::to_path_buf).collect();
        ancestors.reverse();
        for ancestor in ancestors {
            push_unique_root(&mut roots, ancestor);
        }
    }

    if let Some(global) = get_global_data() {
        let mod_dir = global.read().mod_dir.clone();
        if !mod_dir.trim().is_empty() {
            push_unique_root(&mut roots, PathBuf::from(mod_dir.trim()));
        }
    }

    roots
}

fn discover_shell_menu_scheme_ini_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    for root in ordered_shell_menu_scheme_roots() {
        push_shell_menu_scheme_ini_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/Default/ShellMenuScheme.ini"),
        );
        push_shell_menu_scheme_ini_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/ShellMenuScheme.ini"),
        );
        for extracted in [
            root.join("windows_game/extracted_big_files/INIZH"),
            root.join("windows_game/extracted_big_files_v2/INIZH"),
        ] {
            push_shell_menu_scheme_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/Default/ShellMenuScheme.ini"),
            );
            push_shell_menu_scheme_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/ShellMenuScheme.ini"),
            );
        }
    }
    files
}

impl Default for ShellMenuSchemeManager {
    fn default() -> Self {
        Self::new()
    }
}
