/*!
 * ImagePacker - Texture Atlas Generation Tool
 * 
 * Rust implementation of the C++ ImagePacker tool for generating texture atlases
 * and optimizing game assets. This tool matches the C++ functionality for
 * creating efficient texture packs for the Command & Conquer Generals engine.
 * 
 * Features:
 * - Automatic texture atlas generation
 * - Multiple packing algorithms (bin packing, max rects)
 * - Texture format optimization
 * - Asset metadata generation
 * - Command-line interface matching C++ version
 */

use anyhow::{Context, Result};
use chrono::Utc;
use eframe::egui;
use env_logger;
use log::{error, info, warn};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use texture_packer::{TexturePacker, TexturePackerConfig};
use ui_framework::{App, Framework};
use uuid::Uuid;
use walkdir::WalkDir;

mod packer;
mod atlas;
mod formats;
mod ui;

use packer::*;
use atlas::*;
use formats::*;

/// Configuration for image packing operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackerConfig {
    /// Input directory containing images to pack
    pub input_dir: PathBuf,
    /// Output directory for generated atlases
    pub output_dir: PathBuf,
    /// Maximum texture size (power of 2)
    pub max_texture_size: u32,
    /// Padding between sprites in atlas
    pub padding: u32,
    /// Whether to trim transparent pixels
    pub trim_sprites: bool,
    /// Atlas format (PNG, TGA, DDS)
    pub output_format: String,
    /// Whether to generate metadata files
    pub generate_metadata: bool,
    /// Compression settings
    pub compression: CompressionSettings,
}

impl Default for PackerConfig {
    fn default() -> Self {
        Self {
            input_dir: PathBuf::from("./input"),
            output_dir: PathBuf::from("./output"),
            max_texture_size: 2048,
            padding: 2,
            trim_sprites: true,
            output_format: "PNG".to_string(),
            generate_metadata: true,
            compression: CompressionSettings::default(),
        }
    }
}

/// Main ImagePacker application structure
pub struct ImagePackerApp {
    config: PackerConfig,
    atlas_results: Vec<AtlasResult>,
    processing_status: ProcessingStatus,
    ui_state: UiState,
}

#[derive(Debug, Clone)]
pub enum ProcessingStatus {
    Idle,
    Processing { progress: f32, current_file: String },
    Complete { total_atlases: usize, total_images: usize },
    Error(String),
}

#[derive(Debug, Clone)]
pub struct UiState {
    show_config_panel: bool,
    show_results_panel: bool,
    show_log_panel: bool,
    selected_atlas: Option<usize>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_config_panel: true,
            show_results_panel: true,
            show_log_panel: true,
            selected_atlas: None,
        }
    }
}

impl ImagePackerApp {
    pub fn new() -> Self {
        Self {
            config: PackerConfig::default(),
            atlas_results: Vec::new(),
            processing_status: ProcessingStatus::Idle,
            ui_state: UiState::default(),
        }
    }

    /// Process images from input directory and generate atlases
    pub fn process_images(&mut self) -> Result<()> {
        info!("Starting image packing process...");
        self.processing_status = ProcessingStatus::Processing {
            progress: 0.0,
            current_file: "Scanning files...".to_string(),
        };

        // Collect all image files
        let image_files = self.collect_image_files()?;
        let total_files = image_files.len();
        
        if total_files == 0 {
            warn!("No image files found in input directory: {:?}", self.config.input_dir);
            self.processing_status = ProcessingStatus::Error("No image files found".to_string());
            return Ok(());
        }

        info!("Found {} image files to process", total_files);

        // Group images by subdirectory (each subdirectory becomes an atlas)
        let mut groups = HashMap::new();
        for file in image_files {
            let relative_path = file.strip_prefix(&self.config.input_dir)?;
            let group_name = if let Some(parent) = relative_path.parent() {
                parent.to_string_lossy().to_string()
            } else {
                "default".to_string()
            };
            
            groups.entry(group_name).or_insert_with(Vec::new).push(file);
        }

        // Process each group into an atlas
        let mut results = Vec::new();
        let total_groups = groups.len();
        
        for (i, (group_name, files)) in groups.iter().enumerate() {
            self.processing_status = ProcessingStatus::Processing {
                progress: (i as f32) / (total_groups as f32),
                current_file: format!("Processing group: {}", group_name),
            };

            match self.create_atlas(group_name, files) {
                Ok(atlas_result) => {
                    info!("Successfully created atlas for group: {}", group_name);
                    results.push(atlas_result);
                }
                Err(e) => {
                    error!("Failed to create atlas for group {}: {}", group_name, e);
                    self.processing_status = ProcessingStatus::Error(format!("Atlas creation failed: {}", e));
                    return Err(e);
                }
            }
        }

        let total_images: usize = results.iter().map(|r| r.sprite_count).sum();
        self.atlas_results = results;
        self.processing_status = ProcessingStatus::Complete {
            total_atlases: total_groups,
            total_images,
        };

        info!("Image packing complete! Generated {} atlases with {} total sprites", total_groups, total_images);
        Ok(())
    }

    /// Collect all image files from input directory
    fn collect_image_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        for entry in WalkDir::new(&self.config.input_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Some(extension) = entry.path().extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "tga") {
                        files.push(entry.path().to_owned());
                    }
                }
            }
        }
        
        Ok(files)
    }

    /// Create an atlas from a group of image files
    fn create_atlas(&self, group_name: &str, files: &[PathBuf]) -> Result<AtlasResult> {
        let mut packer = TexturePacker::new_skyline(TexturePackerConfig {
            max_width: self.config.max_texture_size,
            max_height: self.config.max_texture_size,
            allow_rotation: true,
            border_padding: self.config.padding,
            texture_padding: self.config.padding,
            trim: self.config.trim_sprites,
            texture_outlines: false,
        });

        // Load and pack all images
        for file_path in files {
            let img = image::open(file_path)
                .with_context(|| format!("Failed to load image: {:?}", file_path))?;
            
            let sprite_name = file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            packer.pack_own(sprite_name, img.to_rgba8())
                .with_context(|| format!("Failed to pack image: {:?}", file_path))?;
        }

        // Generate the atlas texture
        let atlas_texture = packer.get_frame().as_image();
        
        // Save atlas image
        let atlas_filename = format!("{}.{}", group_name, self.config.output_format.to_lowercase());
        let atlas_path = self.config.output_dir.join(&atlas_filename);
        
        std::fs::create_dir_all(&self.config.output_dir)
            .context("Failed to create output directory")?;
        
        atlas_texture.save(&atlas_path)
            .with_context(|| format!("Failed to save atlas: {:?}", atlas_path))?;

        // Generate metadata if requested
        let metadata_path = if self.config.generate_metadata {
            let metadata = self.generate_metadata(&packer, group_name)?;
            let metadata_filename = format!("{}.json", group_name);
            let metadata_path = self.config.output_dir.join(&metadata_filename);
            
            let metadata_json = serde_json::to_string_pretty(&metadata)
                .context("Failed to serialize metadata")?;
            
            std::fs::write(&metadata_path, metadata_json)
                .with_context(|| format!("Failed to write metadata: {:?}", metadata_path))?;
            
            Some(metadata_path)
        } else {
            None
        };

        Ok(AtlasResult {
            id: Uuid::new_v4(),
            group_name: group_name.to_string(),
            atlas_path,
            metadata_path,
            sprite_count: files.len(),
            atlas_size: (atlas_texture.width(), atlas_texture.height()),
            created_at: Utc::now(),
        })
    }

    /// Generate metadata for packed atlas
    fn generate_metadata(&self, packer: &TexturePacker<texture_packer::SkylinePacker, image::RgbaImage>, group_name: &str) -> Result<AtlasMetadata> {
        let frame = packer.get_frame();
        let mut sprites = HashMap::new();

        for (name, frame_info) in frame.frames.iter() {
            sprites.insert(name.clone(), SpriteInfo {
                x: frame_info.frame.x,
                y: frame_info.frame.y,
                w: frame_info.frame.w,
                h: frame_info.frame.h,
                rotated: frame_info.rotated,
                trimmed: frame_info.trimmed,
                source_size: (frame_info.source_size.w, frame_info.source_size.h),
                sprite_source_size: (
                    frame_info.sprite_source_size.x,
                    frame_info.sprite_source_size.y,
                    frame_info.sprite_source_size.w,
                    frame_info.sprite_source_size.h,
                ),
            });
        }

        Ok(AtlasMetadata {
            name: group_name.to_string(),
            texture_size: (frame.width(), frame.height()),
            sprites,
            format: self.config.output_format.clone(),
            created_at: Utc::now(),
        })
    }
}

impl App for ImagePackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ui::render_ui(self, ctx);
    }
}

/// Command-line interface matching C++ version
fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        print_help();
        return Ok(());
    }

    if args.len() > 1 && args[1] == "--cli" {
        // Command-line mode matching C++ interface
        run_cli_mode(&args[2..])
    } else {
        // GUI mode
        info!("Starting ImagePacker GUI...");
        let app = ImagePackerApp::new();
        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1200.0, 800.0])
                .with_title("ImagePacker - Texture Atlas Generator"),
            ..Default::default()
        };
        
        eframe::run_native(
            "ImagePacker",
            native_options,
            Box::new(|_cc| Ok(Box::new(app))),
        ).map_err(|e| anyhow::anyhow!("Failed to run GUI: {}", e))
    }
}

/// Run in command-line mode
fn run_cli_mode(args: &[String]) -> Result<()> {
    info!("Running ImagePacker in CLI mode");
    
    let mut config = PackerConfig::default();
    
    // Parse command-line arguments
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-i" | "--input" => {
                if i + 1 < args.len() {
                    config.input_dir = PathBuf::from(&args[i + 1]);
                    i += 2;
                } else {
                    return Err(anyhow::anyhow!("Missing value for --input"));
                }
            }
            "-o" | "--output" => {
                if i + 1 < args.len() {
                    config.output_dir = PathBuf::from(&args[i + 1]);
                    i += 2;
                } else {
                    return Err(anyhow::anyhow!("Missing value for --output"));
                }
            }
            "-s" | "--size" => {
                if i + 1 < args.len() {
                    config.max_texture_size = args[i + 1].parse()
                        .context("Invalid texture size")?;
                    i += 2;
                } else {
                    return Err(anyhow::anyhow!("Missing value for --size"));
                }
            }
            "-p" | "--padding" => {
                if i + 1 < args.len() {
                    config.padding = args[i + 1].parse()
                        .context("Invalid padding value")?;
                    i += 2;
                } else {
                    return Err(anyhow::anyhow!("Missing value for --padding"));
                }
            }
            "-f" | "--format" => {
                if i + 1 < args.len() {
                    config.output_format = args[i + 1].clone();
                    i += 2;
                } else {
                    return Err(anyhow::anyhow!("Missing value for --format"));
                }
            }
            "--no-trim" => {
                config.trim_sprites = false;
                i += 1;
            }
            "--no-metadata" => {
                config.generate_metadata = false;
                i += 1;
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown argument: {}", args[i]));
            }
        }
    }

    // Validate configuration
    if !config.input_dir.exists() {
        return Err(anyhow::anyhow!("Input directory does not exist: {:?}", config.input_dir));
    }

    // Process images
    let mut app = ImagePackerApp::new();
    app.config = config;
    app.process_images()?;
    
    match app.processing_status {
        ProcessingStatus::Complete { total_atlases, total_images } => {
            info!("Successfully generated {} atlases with {} total sprites", total_atlases, total_images);
        }
        ProcessingStatus::Error(ref msg) => {
            return Err(anyhow::anyhow!("Processing failed: {}", msg));
        }
        _ => {
            return Err(anyhow::anyhow!("Unexpected processing state"));
        }
    }

    Ok(())
}

/// Print help information matching C++ version
fn print_help() {
    println!("ImagePacker - Texture Atlas Generator");
    println!("Rust implementation matching C++ ImagePacker functionality");
    println!();
    println!("USAGE:");
    println!("    image_packer [OPTIONS]");
    println!("    image_packer --cli [CLI_OPTIONS]");
    println!();
    println!("GUI MODE (default):");
    println!("    Launches the graphical user interface");
    println!();
    println!("CLI OPTIONS:");
    println!("    -i, --input <DIR>         Input directory containing images");
    println!("    -o, --output <DIR>        Output directory for atlases");
    println!("    -s, --size <SIZE>         Maximum texture size (power of 2)");
    println!("    -p, --padding <PIXELS>    Padding between sprites");
    println!("    -f, --format <FORMAT>     Output format (PNG, TGA, DDS)");
    println!("    --no-trim                 Don't trim transparent pixels");
    println!("    --no-metadata             Don't generate metadata files");
    println!("    -h, --help               Show this help message");
    println!();
    println!("EXAMPLES:");
    println!("    image_packer --cli -i ./textures -o ./atlases -s 1024");
    println!("    image_packer --cli -i ./ui -o ./packed --format PNG --padding 4");
}