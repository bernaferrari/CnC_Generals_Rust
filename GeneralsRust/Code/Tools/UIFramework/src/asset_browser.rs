//! Asset browser for game development tools

use crate::{Panel, UIError};
use anyhow::Result;
use eframe::egui;
use image::DynamicImage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Asset browser panel for browsing and selecting game assets
pub struct AssetBrowserPanel {
    id: String,
    title: String,
    visible: bool,
    current_path: PathBuf,
    asset_entries: Vec<AssetEntry>,
    selected_asset: Option<String>,
    preview: Option<AssetPreview>,
    search_filter: String,
    asset_type_filter: AssetTypeFilter,
    thumbnail_cache: HashMap<PathBuf, egui::TextureHandle>,
    loading_thumbnails: Vec<PathBuf>,
}

impl AssetBrowserPanel {
    pub fn new() -> Self {
        Self {
            id: "asset_browser".to_string(),
            title: "Asset Browser".to_string(),
            visible: true,
            current_path: PathBuf::from("assets"),
            asset_entries: Vec::new(),
            selected_asset: None,
            preview: None,
            search_filter: String::new(),
            asset_type_filter: AssetTypeFilter::All,
            thumbnail_cache: HashMap::new(),
            loading_thumbnails: Vec::new(),
        }
    }

    pub fn set_root_path(&mut self, path: PathBuf) {
        self.current_path = path;
        self.refresh_assets();
    }

    pub fn selected_asset(&self) -> Option<&str> {
        self.selected_asset.as_deref()
    }

    fn refresh_assets(&mut self) {
        self.asset_entries.clear();

        if let Ok(entries) = std::fs::read_dir(&self.current_path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    let asset_type = if metadata.is_dir() {
                        AssetType::Directory
                    } else {
                        AssetType::from_extension(&path)
                    };

                    // Apply filters
                    if self.should_show_asset(&asset_type, &name) {
                        self.asset_entries.push(AssetEntry {
                            name,
                            path: path.clone(),
                            asset_type,
                            size: metadata.len(),
                            modified: metadata.modified().ok(),
                        });

                        // Queue thumbnail generation for images
                        if matches!(asset_type, AssetType::Texture) {
                            self.loading_thumbnails.push(path);
                        }
                    }
                }
            }
        }

        // Sort entries: directories first, then by name
        self.asset_entries
            .sort_by(|a, b| match (&a.asset_type, &b.asset_type) {
                (AssetType::Directory, AssetType::Directory) => a.name.cmp(&b.name),
                (AssetType::Directory, _) => std::cmp::Ordering::Less,
                (_, AssetType::Directory) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            });
    }

    fn should_show_asset(&self, asset_type: &AssetType, name: &str) -> bool {
        // Apply search filter
        if !self.search_filter.is_empty() {
            if !name
                .to_lowercase()
                .contains(&self.search_filter.to_lowercase())
            {
                return false;
            }
        }

        // Apply type filter
        match self.asset_type_filter {
            AssetTypeFilter::All => true,
            AssetTypeFilter::Textures => matches!(asset_type, AssetType::Texture),
            AssetTypeFilter::Models => matches!(asset_type, AssetType::Model),
            AssetTypeFilter::Audio => matches!(asset_type, AssetType::Audio),
            AssetTypeFilter::Scripts => matches!(asset_type, AssetType::Script),
            AssetTypeFilter::Maps => matches!(asset_type, AssetType::Map),
        }
    }

    fn show_asset_grid(&mut self, ui: &mut egui::Ui) {
        let item_size = egui::Vec2::new(100.0, 120.0);
        let available_width = ui.available_width();
        let columns = (available_width / item_size.x).floor() as usize;
        let columns = columns.max(1);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::new(8.0, 8.0);

            // Collect entries first to avoid double borrow
            let entries_clone = self.asset_entries.clone();
            let chunks: Vec<Vec<AssetEntry>> = entries_clone
                .chunks(columns)
                .map(|chunk| chunk.to_vec())
                .collect();

            for chunk in chunks {
                ui.horizontal(|ui| {
                    for entry in &chunk {
                        self.show_asset_item(ui, entry, item_size);
                    }
                });
            }
        });
    }

    fn show_asset_item(&mut self, ui: &mut egui::Ui, entry: &AssetEntry, size: egui::Vec2) {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        // Background
        let is_selected = self.selected_asset.as_ref() == Some(&entry.name);
        let bg_color = if is_selected {
            ui.visuals().selection.bg_fill
        } else if response.hovered() {
            ui.visuals().widgets.hovered.bg_fill
        } else {
            ui.visuals().widgets.inactive.bg_fill
        };

        ui.painter()
            .rect_filled(rect, egui::Rounding::same(4), bg_color);

        // Icon/Thumbnail
        let icon_rect = egui::Rect::from_center_size(
            rect.center() - egui::Vec2::new(0.0, 20.0),
            egui::Vec2::new(64.0, 64.0),
        );

        match &entry.asset_type {
            AssetType::Directory => {
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "📁",
                    egui::FontId::proportional(32.0),
                    ui.visuals().text_color(),
                );
            }
            AssetType::Texture => {
                if let Some(thumbnail) = self.thumbnail_cache.get(&entry.path) {
                    ui.painter().image(
                        thumbnail.id(),
                        icon_rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    ui.painter().text(
                        icon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "🖼️",
                        egui::FontId::proportional(32.0),
                        ui.visuals().text_color(),
                    );
                }
            }
            AssetType::Model => {
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "🎯",
                    egui::FontId::proportional(32.0),
                    ui.visuals().text_color(),
                );
            }
            AssetType::Audio => {
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "🎵",
                    egui::FontId::proportional(32.0),
                    ui.visuals().text_color(),
                );
            }
            AssetType::Script => {
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "📝",
                    egui::FontId::proportional(32.0),
                    ui.visuals().text_color(),
                );
            }
            AssetType::Map => {
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "🗺️",
                    egui::FontId::proportional(32.0),
                    ui.visuals().text_color(),
                );
            }
            AssetType::Unknown => {
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "❓",
                    egui::FontId::proportional(32.0),
                    ui.visuals().text_color(),
                );
            }
        }

        // Name
        let text_rect = egui::Rect::from_min_size(
            egui::Pos2::new(rect.min.x + 4.0, rect.max.y - 20.0),
            egui::Vec2::new(rect.width() - 8.0, 16.0),
        );

        ui.painter().text(
            text_rect.center(),
            egui::Align2::CENTER_CENTER,
            &entry.name,
            egui::FontId::proportional(10.0),
            ui.visuals().text_color(),
        );

        // Handle clicks
        if response.clicked() {
            if matches!(entry.asset_type, AssetType::Directory) {
                // Navigate to directory
                self.current_path = entry.path.clone();
                self.refresh_assets();
            } else {
                // Select asset
                self.selected_asset = Some(entry.name.clone());
                self.preview = Some(AssetPreview::new(&entry.path, &entry.asset_type));
            }
        }
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Navigation
            if ui.button("⬅️").clicked() {
                if let Some(parent) = self.current_path.parent() {
                    self.current_path = parent.to_path_buf();
                    self.refresh_assets();
                }
            }

            ui.separator();

            // Path
            ui.label("Path:");
            ui.label(self.current_path.display().to_string());

            ui.separator();

            // Search
            ui.label("Search:");
            let search_response = ui.text_edit_singleline(&mut self.search_filter);
            if search_response.changed() {
                self.refresh_assets();
            }

            ui.separator();

            // Filter
            ui.label("Filter:");
            egui::ComboBox::from_id_source("asset_type_filter")
                .selected_text(format!("{:?}", self.asset_type_filter))
                .show_ui(ui, |ui| {
                    for filter_type in [
                        AssetTypeFilter::All,
                        AssetTypeFilter::Textures,
                        AssetTypeFilter::Models,
                        AssetTypeFilter::Audio,
                        AssetTypeFilter::Scripts,
                        AssetTypeFilter::Maps,
                    ] {
                        if ui
                            .selectable_value(
                                &mut self.asset_type_filter,
                                filter_type,
                                format!("{:?}", filter_type),
                            )
                            .clicked()
                        {
                            self.refresh_assets();
                        }
                    }
                });
        });
    }
}

impl Panel for AssetBrowserPanel {
    fn title(&self) -> &str {
        &self.title
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn show_content(&mut self, ui: &mut egui::Ui) -> Result<()> {
        // Toolbar
        self.show_toolbar(ui);
        ui.separator();

        // Main content area
        ui.horizontal(|ui| {
            // Asset grid (left side)
            ui.vertical(|ui| {
                ui.set_min_width(300.0);
                self.show_asset_grid(ui);
            });

            ui.separator();

            // Preview pane (right side)
            ui.vertical(|ui| {
                ui.set_min_width(200.0);

                if let Some(ref preview) = self.preview {
                    ui.heading("Preview");
                    ui.separator();
                    preview.show(ui);
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("No asset selected");
                    });
                }
            });
        });

        Ok(())
    }
}

/// Asset entry in the browser
#[derive(Debug, Clone)]
struct AssetEntry {
    name: String,
    path: PathBuf,
    asset_type: AssetType,
    size: u64,
    modified: Option<std::time::SystemTime>,
}

/// Type of asset
#[derive(Debug, Clone, Copy, PartialEq)]
enum AssetType {
    Directory,
    Texture,
    Model,
    Audio,
    Script,
    Map,
    Unknown,
}

impl AssetType {
    fn from_extension(path: &Path) -> Self {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "png" | "jpg" | "jpeg" | "bmp" | "tga" | "dds" => AssetType::Texture,
                "w3d" | "obj" | "fbx" | "3ds" | "max" => AssetType::Model,
                "wav" | "mp3" | "ogg" | "flac" => AssetType::Audio,
                "lua" | "py" | "js" | "cs" => AssetType::Script,
                "map" | "wnd" => AssetType::Map,
                _ => AssetType::Unknown,
            }
        } else {
            AssetType::Unknown
        }
    }
}

/// Asset type filter
#[derive(Debug, Clone, Copy, PartialEq)]
enum AssetTypeFilter {
    All,
    Textures,
    Models,
    Audio,
    Scripts,
    Maps,
}

/// Asset preview display
struct AssetPreview {
    path: PathBuf,
    asset_type: AssetType,
    metadata: Option<AssetMetadata>,
}

impl AssetPreview {
    fn new(path: &Path, asset_type: &AssetType) -> Self {
        // AssetMetadata::load is async, but we're in a sync context.
        // We cannot block on it here without a runtime context.
        // Option 1: Don't load metadata synchronously (set to None)
        // Option 2: Make new() async and propagate the change
        // Choosing Option 1 to avoid cascading async changes
        let metadata = None;

        Self {
            path: path.to_path_buf(),
            asset_type: asset_type.clone(),
            metadata,
        }
    }

    fn show(&self, ui: &mut egui::Ui) {
        ui.label(format!(
            "Name: {}",
            self.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
        ));

        ui.label(format!("Type: {:?}", self.asset_type));

        if let Some(ref metadata) = self.metadata {
            ui.separator();

            match &metadata.details {
                AssetDetails::Image {
                    width,
                    height,
                    format,
                } => {
                    ui.label(format!("Dimensions: {}x{}", width, height));
                    ui.label(format!("Format: {}", format));
                }
                AssetDetails::Audio {
                    duration,
                    sample_rate,
                    channels,
                } => {
                    ui.label(format!("Duration: {:.2}s", duration));
                    ui.label(format!("Sample Rate: {}Hz", sample_rate));
                    ui.label(format!("Channels: {}", channels));
                }
                AssetDetails::Model {
                    vertices,
                    triangles,
                    materials,
                } => {
                    ui.label(format!("Vertices: {}", vertices));
                    ui.label(format!("Triangles: {}", triangles));
                    ui.label(format!("Materials: {}", materials));
                }
                AssetDetails::Generic => {}
            }

            ui.separator();
            ui.label(format!("Size: {} bytes", metadata.file_size));

            if let Some(modified) = metadata.modified {
                ui.label(format!("Modified: {:?}", modified));
            }
        }
    }
}

/// Asset metadata
struct AssetMetadata {
    file_size: u64,
    modified: Option<std::time::SystemTime>,
    details: AssetDetails,
}

impl AssetMetadata {
    async fn load(path: &Path) -> Result<Self, UIError> {
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|e| UIError::AssetLoadingError(e.to_string()))?;

        let details = match AssetType::from_extension(path) {
            AssetType::Texture => {
                if let Ok(img) = image::open(path) {
                    AssetDetails::Image {
                        width: img.width(),
                        height: img.height(),
                        format: format!("{:?}", img.color()),
                    }
                } else {
                    AssetDetails::Generic
                }
            }
            _ => AssetDetails::Generic,
        };

        Ok(Self {
            file_size: metadata.len(),
            modified: metadata.modified().ok(),
            details,
        })
    }
}

/// Asset-specific details
enum AssetDetails {
    Generic,
    Image {
        width: u32,
        height: u32,
        format: String,
    },
    Audio {
        duration: f32,
        sample_rate: u32,
        channels: u8,
    },
    Model {
        vertices: u32,
        triangles: u32,
        materials: u32,
    },
}
