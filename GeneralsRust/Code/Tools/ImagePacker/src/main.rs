/*!
 * ImagePacker - GPUI frontend with C++-faithful packing semantics.
 *
 * Corresponds to C++ tools:
 * - Tools/ImagePacker/Source/WinMain.cpp
 * - Tools/ImagePacker/Source/ImagePacker.cpp
 * - Tools/ImagePacker/Source/Window Procedures/ImagePackerProc.cpp
 */

use anyhow::{Context as AnyhowContext, Result};
use chrono::Utc;
use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context, Render, SharedString,
    Window, WindowBounds, WindowOptions,
};
use image::{DynamicImage, RgbaImage};
use image_compat::{
    DynamicImage as CompatDynamicImage, ImageBuffer as CompatImageBuffer, Rgba as CompatRgba,
};
use log::{error, info, warn};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use texture_packer::exporter::ImageExporter;
use texture_packer::{Frame as PackedFrame, TexturePacker, TexturePackerConfig};
use walkdir::WalkDir;

mod atlas;
#[path = "Window Procedures/directory_select.rs"]
mod directory_select;
mod formats;
mod image_directory;
#[path = "Window Procedures/image_error_proc.rs"]
mod image_error_proc;
mod image_info;
mod image_packer;
#[path = "Window Procedures/image_packer_proc.rs"]
mod image_packer_proc;
#[path = "Window Procedures/page_error_proc.rs"]
mod page_error_proc;
#[path = "Window Procedures/preview_proc.rs"]
mod preview_proc;
mod texture_page;
mod win_main;
mod window_proc;

use atlas::AtlasResult;
use formats::{FormatHandler, OutputFormat};
use uuid::Uuid;

const DEFAULT_TARGET_SIZE: u32 = 512;
const MAX_OUTPUT_FILE_LEN: usize = 128;
const ILLEGAL_OUTPUT_CHARS: &str = "/\\:*?<>|";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetSizeMode {
    Size128,
    Size256,
    Size512,
    Custom,
}

impl Default for TargetSizeMode {
    fn default() -> Self {
        Self::Size512
    }
}

#[derive(Clone)]
struct SourceImage {
    key: String,
    path: PathBuf,
    rgba: CompatImageBuffer<CompatRgba<u8>, Vec<u8>>,
    width: u32,
    height: u32,
    color_depth: u8,
}

struct PackedPage {
    id: u32,
    atlas_image: DynamicImage,
    frames: Vec<PackedFrame>,
    atlas_width: u32,
    atlas_height: u32,
}

struct ImagePackerGpuiApp {
    input_dirs: Vec<PathBuf>,
    selected_dir: Option<usize>,
    use_sub_folders: bool,
    output_file: String,
    output_format: OutputFormat,
    target_mode: TargetSizeMode,
    custom_target_size: u32,
    output_alpha: bool,
    create_ini: bool,
    use_texture_preview: bool,
    compress_textures: bool,
    gap_extend_rgb: bool,
    gap_gutter: bool,
    gutter_size: u32,
    processing: bool,
    status: String,
    logs: Vec<String>,
    atlas_results: Vec<AtlasResult>,
    last_output_directory: Option<PathBuf>,
    interactive_prompt: bool,
}

impl ImagePackerGpuiApp {
    fn new() -> Self {
        Self {
            input_dirs: Vec::new(),
            selected_dir: None,
            use_sub_folders: true,
            output_file: "NewImage".to_string(),
            output_format: OutputFormat::TGA,
            target_mode: TargetSizeMode::default(),
            custom_target_size: DEFAULT_TARGET_SIZE,
            output_alpha: true,
            create_ini: true,
            use_texture_preview: false,
            compress_textures: false,
            gap_extend_rgb: true,
            gap_gutter: false,
            gutter_size: 1,
            processing: false,
            status: "Select options and click Start.".to_string(),
            logs: vec!["ImagePacker initialized".to_string()],
            atlas_results: Vec::new(),
            last_output_directory: None,
            interactive_prompt: true,
        }
    }

    fn current_target_size(&self) -> u32 {
        match self.target_mode {
            TargetSizeMode::Size128 => 128,
            TargetSizeMode::Size256 => 256,
            TargetSizeMode::Size512 => 512,
            TargetSizeMode::Custom => self.custom_target_size.max(1),
        }
    }

    fn custom_size_is_power_of_two(&self) -> bool {
        self.custom_target_size.is_power_of_two() && self.custom_target_size > 0
    }

    fn push_log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
        if self.logs.len() > 80 {
            let overflow = self.logs.len() - 80;
            self.logs.drain(0..overflow);
        }
    }

    fn pick_input_directory(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            if !self.input_dirs.iter().any(|existing| existing == &path) {
                self.push_log(format!("Added folder: {}", path.display()));
                if self.output_file == "NewImage" {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        self.output_file = sanitize_output_name(name);
                    }
                }
                self.selected_dir = Some(self.input_dirs.len());
                self.input_dirs.push(path);
            }
        }
        cx.notify();
    }

    fn remove_selected_directory(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.selected_dir {
            if index < self.input_dirs.len() {
                let removed = self.input_dirs.remove(index);
                self.push_log(format!("Removed folder: {}", removed.display()));
                self.selected_dir = None;
            }
        }
        cx.notify();
    }

    fn nudge_custom_target(&mut self, delta: i32, cx: &mut Context<Self>) {
        let mut value = self.custom_target_size as i32 + delta;
        value = value.clamp(2, 16_384);
        self.custom_target_size = value as u32;
        cx.notify();
    }

    fn set_output_format(&mut self, format: OutputFormat, cx: &mut Context<Self>) {
        self.output_format = format;
        cx.notify();
    }

    fn toggle_bool(current: &mut bool, cx: &mut Context<Self>) {
        *current = !*current;
        cx.notify();
    }

    fn validate_settings(&self) -> Result<()> {
        if self.input_dirs.is_empty() {
            anyhow::bail!("at least one input folder is required");
        }
        if self.target_mode == TargetSizeMode::Custom && !self.custom_size_is_power_of_two() {
            anyhow::bail!("custom target size must be a power of two");
        }
        if self.output_file.is_empty() {
            anyhow::bail!("output filename cannot be empty");
        }
        if self.output_file.len() > MAX_OUTPUT_FILE_LEN {
            anyhow::bail!("output filename exceeds {} characters", MAX_OUTPUT_FILE_LEN);
        }
        if self
            .output_file
            .chars()
            .any(|ch| ILLEGAL_OUTPUT_CHARS.contains(ch))
        {
            anyhow::bail!(
                "output filename '{}' contains illegal characters: {}",
                self.output_file,
                ILLEGAL_OUTPUT_CHARS
            );
        }
        Ok(())
    }

    fn process(&mut self) -> Result<()> {
        self.validate_settings()?;
        self.processing = true;
        self.status = "Gathering image information...".to_string();
        self.atlas_results.clear();
        self.push_log("Starting packing process");

        let output_directory = self.prepare_output_directory()?;
        self.last_output_directory = Some(output_directory.clone());

        let image_paths = self.collect_image_paths();
        if image_paths.is_empty() {
            self.status = "No images found in selected folders.".to_string();
            self.processing = false;
            return Ok(());
        }

        let target_size = self.current_target_size();
        let source_images = self.load_and_validate_images(&image_paths, target_size)?;
        if source_images.is_empty() {
            self.status = "No valid images to pack.".to_string();
            self.processing = false;
            return Ok(());
        }

        self.status = format!("Packing {} images...", source_images.len());
        let pages = self.pack_images(source_images, target_size)?;
        self.write_pages(&pages, &output_directory)?;
        if self.create_ini {
            self.write_ini_file(&pages, &output_directory)?;
        }

        let mut summary = String::new();
        let _ = write!(
            &mut summary,
            "Image packing complete: {} page(s) from {} image(s) in {} folder(s)",
            pages.len(),
            pages.iter().map(|page| page.frames.len()).sum::<usize>(),
            self.input_dirs.len()
        );
        self.status = summary.clone();
        self.push_log(summary);
        self.processing = false;
        Ok(())
    }

    fn collect_image_paths(&mut self) -> Vec<PathBuf> {
        let mut collected = Vec::new();
        let mut unique = HashSet::new();
        let input_dirs = self.input_dirs.clone();
        for dir in &input_dirs {
            if !dir.exists() {
                self.push_log(format!("Skipping missing directory: {}", dir.display()));
                continue;
            }
            let mut walker = WalkDir::new(dir);
            if !self.use_sub_folders {
                walker = walker.max_depth(1);
            }
            for entry in walker.into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if !entry.file_type().is_file() {
                    continue;
                }
                let extension = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default();
                if !matches!(extension.as_str(), "tga") {
                    continue;
                }
                let normalized = path.to_path_buf();
                if unique.insert(normalized.clone()) {
                    collected.push(normalized);
                }
            }
        }
        self.push_log(format!("Found {} candidate image file(s)", collected.len()));
        collected
    }

    fn load_and_validate_images(
        &mut self,
        paths: &[PathBuf],
        target_size: u32,
    ) -> Result<Vec<SourceImage>> {
        let mut source_images = Vec::new();
        let mut name_counts: HashMap<String, usize> = HashMap::new();
        let mut invalid_images: Vec<String> = Vec::new();

        for path in paths {
            let image = match image::open(path) {
                Ok(img) => img,
                Err(err) => {
                    let reason = format!("Unable to read image '{}': {err}", path.display());
                    self.push_log(reason.clone());
                    invalid_images.push(reason);
                    continue;
                }
            };

            let (width, height) = (image.width(), image.height());
            let channels = image.color().channel_count();
            let color_depth = channels.saturating_mul(8);

            if width > target_size || height > target_size {
                let reason = format!(
                    "Skipping '{}' ({}x{}) larger than target {}x{}",
                    path.display(),
                    width,
                    height,
                    target_size,
                    target_size
                );
                self.push_log(reason.clone());
                invalid_images.push(reason);
                continue;
            }
            if color_depth != 24 && color_depth != 32 {
                let reason = format!(
                    "Skipping '{}' (unsupported color depth {}-bit; expected 24/32)",
                    path.display(),
                    color_depth
                );
                self.push_log(reason.clone());
                invalid_images.push(reason);
                continue;
            }

            let base_name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(sanitize_sprite_name)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "sprite".to_string());
            let counter = name_counts.entry(base_name.clone()).or_insert(0);
            let sprite_key = if *counter == 0 {
                base_name.clone()
            } else {
                format!("{base_name}_{counter}")
            };
            *counter += 1;

            source_images.push(SourceImage {
                key: sprite_key,
                path: path.clone(),
                rgba: modern_rgba_to_compat(image.to_rgba8())?,
                width,
                height,
                color_depth,
            });
        }

        source_images.sort_by(|a, b| {
            let area_a = a.width.saturating_mul(a.height);
            let area_b = b.width.saturating_mul(b.height);
            area_b.cmp(&area_a)
        });

        if !invalid_images.is_empty() && self.interactive_prompt {
            let response = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title("Image Validation Errors")
                .set_description(format!(
                    "{} image(s) cannot be processed. Continue building atlas pages with valid images only?",
                    invalid_images.len()
                ))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show();
            if response != rfd::MessageDialogResult::Yes {
                anyhow::bail!("Build Cancelled By User.");
            }
        } else if !invalid_images.is_empty() {
            self.push_log(format!(
                "Continuing in CLI mode with {} invalid image(s) skipped.",
                invalid_images.len()
            ));
        }

        self.push_log(format!(
            "Validated {} image(s) for packing",
            source_images.len()
        ));
        Ok(source_images)
    }

    fn pack_images(
        &mut self,
        images: Vec<SourceImage>,
        target_size: u32,
    ) -> Result<Vec<PackedPage>> {
        let mut open_pages = Vec::new();

        let border_padding = if self.gap_gutter { self.gutter_size } else { 0 };
        let texture_padding = if self.gap_gutter { self.gutter_size } else { 0 };

        for image in images {
            if open_pages.is_empty() {
                let mut page_packer = TexturePacker::new_skyline(TexturePackerConfig {
                    max_width: target_size,
                    max_height: target_size,
                    allow_rotation: true,
                    border_padding,
                    texture_padding,
                    trim: false,
                    texture_outlines: false,
                });
                page_packer.pack_own(image.key.clone(), image.rgba.clone());
                if page_packer.get_frames().is_empty() {
                    anyhow::bail!(
                        "unable to fit '{}' ({}, {}bpp) in a fresh page",
                        image.path.display(),
                        image.width,
                        image.color_depth
                    );
                }
                open_pages.push((1_u32, page_packer));
                continue;
            }

            let mut placed = false;
            for (_, packer) in &mut open_pages {
                let before = packer.get_frames().len();
                packer.pack_own(image.key.clone(), image.rgba.clone());
                if packer.get_frames().len() > before {
                    placed = true;
                    break;
                }
            }

            if placed {
                continue;
            }

            let page_id = open_pages.len() as u32 + 1;
            let mut page_packer = TexturePacker::new_skyline(TexturePackerConfig {
                max_width: target_size,
                max_height: target_size,
                allow_rotation: true,
                border_padding,
                texture_padding,
                trim: false,
                texture_outlines: false,
            });
            page_packer.pack_own(image.key.clone(), image.rgba.clone());
            if page_packer.get_frames().is_empty() {
                anyhow::bail!(
                    "unable to fit '{}' ({}, {}bpp) in a fresh page",
                    image.path.display(),
                    image.width,
                    image.color_depth
                );
            }
            open_pages.push((page_id, page_packer));
        }

        let mut pages = Vec::with_capacity(open_pages.len());
        for (id, packer) in open_pages {
            let atlas_compat = ImageExporter::export(&packer)
                .map_err(|err| anyhow::anyhow!("failed exporting atlas page {}: {}", id, err))?;
            let atlas_image = compat_dynamic_to_modern(atlas_compat)?;
            let mut frames: Vec<PackedFrame> = packer.get_frames().values().cloned().collect();
            frames.sort_by(|a, b| a.key.cmp(&b.key));
            pages.push(PackedPage {
                id,
                atlas_width: atlas_image.width(),
                atlas_height: atlas_image.height(),
                atlas_image,
                frames,
            });
        }

        self.push_log(format!("Packed into {} texture page(s)", pages.len()));
        Ok(pages)
    }

    fn write_pages(&mut self, pages: &[PackedPage], output_dir: &Path) -> Result<()> {
        for page in pages {
            let filename = format!(
                "{}_{:03}.{}",
                self.output_file,
                page.id,
                self.output_format.to_extension()
            );
            let atlas_path = output_dir.join(filename);
            let image_to_save = if self.output_alpha {
                page.atlas_image.clone()
            } else {
                DynamicImage::ImageRgb8(page.atlas_image.to_rgb8())
            };
            FormatHandler::save_image(&image_to_save, &atlas_path, self.output_format, None)
                .with_context(|| format!("failed writing '{}'", atlas_path.display()))?;

            self.atlas_results.push(AtlasResult {
                id: Uuid::new_v4(),
                group_name: format!("Page {:03}", page.id),
                atlas_path: atlas_path.clone(),
                metadata_path: None,
                sprite_count: page.frames.len(),
                atlas_size: (page.atlas_width, page.atlas_height),
                created_at: Utc::now(),
            });

            self.push_log(format!(
                "Wrote {} ({} sprites)",
                atlas_path.display(),
                page.frames.len()
            ));
        }
        Ok(())
    }

    fn write_ini_file(&mut self, pages: &[PackedPage], output_dir: &Path) -> Result<()> {
        let ini_path = output_dir.join(format!("{}.INI", self.output_file));
        let mut ini = String::new();
        ini.push_str("; ------------------------------------------------------------\n");
        ini.push_str("; Do NOT edit by hand, ImagePacker GPUI auto generated INI file\n");
        ini.push_str("; ------------------------------------------------------------\n\n");

        for page in pages {
            for sprite in &page.frames {
                let status = if sprite.rotated {
                    "ROTATED_90_CLOCKWISE"
                } else {
                    "NONE"
                };
                let texture_name = format!(
                    "{}_{:03}.{}",
                    self.output_file,
                    page.id,
                    self.output_format.to_extension()
                );
                let right = sprite.frame.x + sprite.frame.w;
                let bottom = sprite.frame.y + sprite.frame.h;
                let _ = writeln!(ini, "MappedImage {}", sprite.key);
                let _ = writeln!(ini, "  Texture = {}", texture_name);
                let _ = writeln!(ini, "  TextureWidth = {}", page.atlas_width);
                let _ = writeln!(ini, "  TextureHeight = {}", page.atlas_height);
                let _ = writeln!(
                    ini,
                    "  Coords = Left:{} Top:{} Right:{} Bottom:{}",
                    sprite.frame.x, sprite.frame.y, right, bottom
                );
                let _ = writeln!(ini, "  Status = {}", status);
                ini.push_str("End\n\n");
            }
        }

        fs::write(&ini_path, ini)
            .with_context(|| format!("failed writing '{}'", ini_path.display()))?;
        self.push_log(format!("Wrote {}", ini_path.display()));
        Ok(())
    }

    fn prepare_output_directory(&mut self) -> Result<PathBuf> {
        let cwd = std::env::current_dir().context("unable to read current directory")?;
        let output_dir = cwd.join("ImagePackerOutput").join(&self.output_file);
        fs::create_dir_all(&output_dir)
            .with_context(|| format!("failed creating '{}'", output_dir.display()))?;

        let existing_entries = fs::read_dir(&output_dir)
            .with_context(|| format!("failed reading '{}'", output_dir.display()))?
            .filter_map(|entry| entry.ok())
            .count();

        if existing_entries > 0 {
            if self.interactive_prompt {
                let result = rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Warning)
                    .set_title("Delete files to continue?")
                    .set_description(format!(
                        "The output directory ({}) must be empty before proceeding. Delete '{}' files and continue with build process?",
                        output_dir.display(),
                        existing_entries
                    ))
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                if result != rfd::MessageDialogResult::Yes {
                    anyhow::bail!("build process cancelled");
                }
            } else {
                self.push_log(format!(
                    "Output directory {} has {} existing file(s); cleaning for CLI run.",
                    output_dir.display(),
                    existing_entries
                ));
            }
        }

        self.clean_directory(&output_dir)?;
        Ok(output_dir)
    }

    fn clean_directory(&mut self, directory: &Path) -> Result<()> {
        for entry in fs::read_dir(directory)
            .with_context(|| format!("failed reading '{}'", directory.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                fs::remove_dir_all(&path)
                    .with_context(|| format!("failed removing '{}'", path.display()))?;
            } else {
                fs::remove_file(&path)
                    .with_context(|| format!("failed removing '{}'", path.display()))?;
            }
        }
        Ok(())
    }

    fn run_process_click(&mut self, cx: &mut Context<Self>) {
        if self.processing {
            return;
        }
        match self.process() {
            Ok(()) => info!("{}", self.status),
            Err(err) => {
                self.processing = false;
                self.status = format!("Error: {err}");
                error!("{}", self.status);
                self.push_log(self.status.clone());
            }
        }
        cx.notify();
    }

    fn open_output_directory(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.last_output_directory.clone() else {
            self.push_log("No output directory has been generated yet.");
            cx.notify();
            return;
        };

        let result = if cfg!(target_os = "macos") {
            Command::new("open").arg(&path).status()
        } else if cfg!(target_os = "windows") {
            Command::new("explorer").arg(&path).status()
        } else {
            Command::new("xdg-open").arg(&path).status()
        };

        match result {
            Ok(status) if status.success() => {
                self.push_log(format!("Opened {}", path.display()));
            }
            Ok(status) => {
                self.push_log(format!(
                    "Failed to open {} (exit code {})",
                    path.display(),
                    status
                ));
            }
            Err(err) => {
                self.push_log(format!("Failed to open {}: {err}", path.display()));
            }
        }
        cx.notify();
    }
}

impl Render for ImagePackerGpuiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let target_size = self.current_target_size();

        div()
            .size_full()
            .bg(rgb(0x0c131a))
            .text_color(rgb(0xe7edf4))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(0x1b2733))
                    .child(
                        div().flex().flex_col().child("ImagePacker (GPUI)").child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x95a7b8))
                                .child("C++-faithful texture page packing tool"),
                        ),
                    )
                    .child(div().flex().gap_2().children([
                        metric_box("Folders", self.input_dirs.len().to_string()),
                        metric_box("Target", format!("{target_size}x{target_size}")),
                        metric_box("Pages", self.atlas_results.len().to_string()),
                    ])),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .p_3()
                    .child(
                        div()
                            .w(px(460.))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(section_title("Input Folders"))
                            .child(div().flex().gap_2().children([
                                action_chip("Add Folder", false).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.pick_input_directory(cx);
                                    },
                                )),
                                action_chip("Remove Selected", false).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.remove_selected_directory(cx);
                                    },
                                )),
                                action_chip("Use Subfolders", self.use_sub_folders).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        Self::toggle_bool(&mut this.use_sub_folders, cx);
                                    }),
                                ),
                            ]))
                            .child(div().flex().flex_col().gap_1().children(
                                self.input_dirs.iter().enumerate().map(|(index, path)| {
                                    let selected = self.selected_dir == Some(index);
                                    let label = path.display().to_string();
                                    div()
                                        .id(("input-folder", index))
                                        .p_2()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(if selected {
                                            rgb(0xd1a65d)
                                        } else {
                                            rgb(0x2a3745)
                                        })
                                        .bg(if selected {
                                            rgb(0x2a2318)
                                        } else {
                                            rgb(0x101821)
                                        })
                                        .cursor_pointer()
                                        .child(label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.selected_dir = Some(index);
                                            cx.notify();
                                        }))
                                }),
                            ))
                            .child(section_title("Target Texture Size"))
                            .child(
                                div().flex().gap_2().children([
                                    action_chip(
                                        "128x128",
                                        self.target_mode == TargetSizeMode::Size128,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.target_mode = TargetSizeMode::Size128;
                                            cx.notify();
                                        },
                                    )),
                                    action_chip(
                                        "256x256",
                                        self.target_mode == TargetSizeMode::Size256,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.target_mode = TargetSizeMode::Size256;
                                            cx.notify();
                                        },
                                    )),
                                    action_chip(
                                        "512x512",
                                        self.target_mode == TargetSizeMode::Size512,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.target_mode = TargetSizeMode::Size512;
                                            cx.notify();
                                        },
                                    )),
                                    action_chip(
                                        "Custom",
                                        self.target_mode == TargetSizeMode::Custom,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.target_mode = TargetSizeMode::Custom;
                                            cx.notify();
                                        },
                                    )),
                                ]),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .items_center()
                                    .child(action_chip("-64", false).on_click(cx.listener(
                                        |this, _, _, cx| this.nudge_custom_target(-64, cx),
                                    )))
                                    .child(metric_box(
                                        "Custom Size",
                                        self.custom_target_size.to_string(),
                                    ))
                                    .child(action_chip("+64", false).on_click(cx.listener(
                                        |this, _, _, cx| this.nudge_custom_target(64, cx),
                                    )))
                                    .child(metric_box(
                                        "Power of Two",
                                        if self.custom_size_is_power_of_two() {
                                            "yes".to_string()
                                        } else {
                                            "no".to_string()
                                        },
                                    )),
                            )
                            .child(section_title("Output"))
                            .child(
                                div().flex().gap_2().children([
                                    action_chip("TGA", self.output_format == OutputFormat::TGA)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.set_output_format(OutputFormat::TGA, cx);
                                        })),
                                    action_chip("PNG", self.output_format == OutputFormat::PNG)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.set_output_format(OutputFormat::PNG, cx);
                                        })),
                                    action_chip("DDS", self.output_format == OutputFormat::DDS)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.set_output_format(OutputFormat::DDS, cx);
                                        })),
                                    action_chip("JPG", self.output_format == OutputFormat::JPG)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.set_output_format(OutputFormat::JPG, cx);
                                        })),
                                ]),
                            )
                            .child(metric_box("Output Base Name", self.output_file.clone()))
                            .child(div().flex().gap_2().children([
                                action_chip("Use First Folder Name", false).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        if let Some(first) = this.input_dirs.first() {
                                            if let Some(name) =
                                                first.file_name().and_then(|n| n.to_str())
                                            {
                                                this.output_file = sanitize_output_name(name);
                                            }
                                        }
                                        cx.notify();
                                    },
                                )),
                                action_chip("Reset Name", false).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.output_file = "NewImage".to_string();
                                        cx.notify();
                                    },
                                )),
                            ])),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(section_title("C++ Parity Options"))
                            .child(
                                div().flex().flex_wrap().gap_2().children([
                                    action_chip("Output Alpha", self.output_alpha).on_click(
                                        cx.listener(|this, _, _, cx| {
                                            Self::toggle_bool(&mut this.output_alpha, cx);
                                        }),
                                    ),
                                    action_chip("Create INI", self.create_ini).on_click(
                                        cx.listener(|this, _, _, cx| {
                                            Self::toggle_bool(&mut this.create_ini, cx);
                                        }),
                                    ),
                                    action_chip("Bitmap Preview Flag", self.use_texture_preview)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            Self::toggle_bool(&mut this.use_texture_preview, cx);
                                        })),
                                    action_chip("Compress Textures", self.compress_textures)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            Self::toggle_bool(&mut this.compress_textures, cx);
                                        })),
                                    action_chip("Gap Extend RGB", self.gap_extend_rgb).on_click(
                                        cx.listener(|this, _, _, cx| {
                                            Self::toggle_bool(&mut this.gap_extend_rgb, cx);
                                        }),
                                    ),
                                    action_chip("Gap Gutter", self.gap_gutter).on_click(
                                        cx.listener(|this, _, _, cx| {
                                            Self::toggle_bool(&mut this.gap_gutter, cx);
                                        }),
                                    ),
                                ]),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .items_center()
                                    .child(action_chip("-1 Gutter", false).on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.gutter_size = this.gutter_size.saturating_sub(1);
                                            cx.notify();
                                        },
                                    )))
                                    .child(metric_box("Gutter", self.gutter_size.to_string()))
                                    .child(action_chip("+1 Gutter", false).on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.gutter_size = (this.gutter_size + 1).min(64);
                                            cx.notify();
                                        },
                                    ))),
                            )
                            .child(section_title("Actions"))
                            .child(div().flex().gap_2().children([
                                action_chip("Start", false).on_click(
                                    cx.listener(|this, _, _, cx| this.run_process_click(cx)),
                                ),
                                action_chip("Open Output Folder", false).on_click(
                                    cx.listener(|this, _, _, cx| this.open_output_directory(cx)),
                                ),
                            ]))
                            .child(metric_box("Status", self.status.clone()))
                            .child(section_title("Generated Pages"))
                            .child(div().flex().flex_col().gap_1().children(
                                self.atlas_results.iter().map(|result| {
                                    div()
                                        .p_2()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(rgb(0x2a3745))
                                        .bg(rgb(0x111922))
                                        .child(format!(
                                            "{} | {}x{} | {} sprites | {}",
                                            result.group_name,
                                            result.atlas_size.0,
                                            result.atlas_size.1,
                                            result.sprite_count,
                                            result.atlas_path.display()
                                        ))
                                }),
                            ))
                            .child(section_title("Log"))
                            .child(div().flex().flex_col().gap_1().children(
                                self.logs.iter().rev().take(18).map(|entry| {
                                    div()
                                        .text_sm()
                                        .text_color(rgb(0x9eb0bf))
                                        .child(entry.clone())
                                }),
                            )),
                    ),
            )
    }
}

fn modern_rgba_to_compat(image: RgbaImage) -> Result<CompatImageBuffer<CompatRgba<u8>, Vec<u8>>> {
    let (width, height) = image.dimensions();
    CompatImageBuffer::from_raw(width, height, image.into_raw()).ok_or_else(|| {
        anyhow::anyhow!("failed converting RGBA image to texture_packer-compatible buffer")
    })
}

fn compat_dynamic_to_modern(image: CompatDynamicImage) -> Result<DynamicImage> {
    let rgba = image.to_rgba();
    let (width, height) = rgba.dimensions();
    let modern = RgbaImage::from_raw(width, height, rgba.into_vec()).ok_or_else(|| {
        anyhow::anyhow!("failed converting packed atlas image to modern image format")
    })?;
    Ok(DynamicImage::ImageRgba8(modern))
}

fn sanitize_output_name(input: &str) -> String {
    let mut name = input
        .chars()
        .filter(|ch| !ILLEGAL_OUTPUT_CHARS.contains(*ch) && !ch.is_control())
        .collect::<String>();
    if name.is_empty() {
        name = "NewImage".to_string();
    }
    if name.len() > MAX_OUTPUT_FILE_LEN {
        name.truncate(MAX_OUTPUT_FILE_LEN);
    }
    name
}

fn sanitize_sprite_name(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn metric_box(label: impl Into<SharedString>, value: impl Into<SharedString>) -> impl IntoElement {
    div()
        .p_2()
        .rounded_md()
        .bg(rgb(0x111922))
        .border_1()
        .border_color(rgb(0x283646))
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x94a7ba))
                        .child(label.into()),
                )
                .child(value.into()),
        )
}

fn section_title(label: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(rgb(0xd1a65d))
        .child(label.into())
}

fn action_chip(label: &'static str, active: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(if active { rgb(0xd1a65d) } else { rgb(0x2a3745) })
        .bg(if active { rgb(0x2b2418) } else { rgb(0x111922) })
        .cursor_pointer()
        .child(label)
}

fn run_gui() -> Result<()> {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1420.0), px(900.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                ..Default::default()
            },
            |_, cx| cx.new(|_| ImagePackerGpuiApp::new()),
        )
        .expect("failed to open ImagePacker GPUI window");
        cx.activate(true);
    });
    Ok(())
}

fn run_cli_mode(args: &[String]) -> Result<()> {
    let mut tool = ImagePackerGpuiApp::new();
    tool.interactive_prompt = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-i" | "--input" => {
                let value = args
                    .get(index + 1)
                    .context("missing value for --input")?
                    .to_string();
                tool.input_dirs.push(PathBuf::from(value));
                index += 2;
            }
            "-o" | "--output" => {
                let value = args
                    .get(index + 1)
                    .context("missing value for --output")?
                    .to_string();
                tool.output_file = sanitize_output_name(&value);
                index += 2;
            }
            "-s" | "--size" => {
                let value = args
                    .get(index + 1)
                    .context("missing value for --size")?
                    .parse::<u32>()
                    .context("invalid --size")?;
                tool.target_mode = TargetSizeMode::Custom;
                tool.custom_target_size = value;
                index += 2;
            }
            "-p" | "--padding" | "--gutter" => {
                let value = args
                    .get(index + 1)
                    .context("missing value for --padding/--gutter")?
                    .parse::<u32>()
                    .context("invalid --padding/--gutter")?;
                tool.gutter_size = value;
                tool.gap_gutter = true;
                index += 2;
            }
            "-f" | "--format" => {
                let value = args.get(index + 1).context("missing value for --format")?;
                tool.output_format = OutputFormat::from_string(value)?;
                index += 2;
            }
            "--no-ini" | "--no-metadata" => {
                tool.create_ini = false;
                index += 1;
            }
            "--no-subfolders" => {
                tool.use_sub_folders = false;
                index += 1;
            }
            "--compress" => {
                tool.compress_textures = true;
                index += 1;
            }
            unknown => {
                anyhow::bail!("unknown argument: {unknown}");
            }
        }
    }

    tool.process()?;
    println!("{}", tool.status);
    for line in tool.logs.iter().rev().take(12).rev() {
        println!(" - {line}");
    }
    Ok(())
}

fn print_help() {
    println!("ImagePacker - Texture Atlas Generator");
    println!("Rust GPUI port of the C++ ImagePacker workflow");
    println!();
    println!("USAGE:");
    println!("    image_packer [OPTIONS]");
    println!("    image_packer --cli [CLI_OPTIONS]");
    println!();
    println!("GUI MODE (default):");
    println!("    Launches the GPUI front-end");
    println!();
    println!("CLI OPTIONS:");
    println!("    -i, --input <DIR>         Input directory (repeatable)");
    println!("    -o, --output <NAME>       Output base filename");
    println!("    -s, --size <SIZE>         Square page size (power of two)");
    println!("    -p, --padding <PIXELS>    Gap gutter size");
    println!("    -f, --format <FORMAT>     Output format (TGA, PNG, DDS, JPG)");
    println!("    --no-ini                  Disable MappedImage INI generation");
    println!("    --no-subfolders           Disable recursive directory scan");
    println!("    --compress                Set compress texture option bit");
    println!("    -h, --help                Show this help message");
}

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
        info!("Running ImagePacker in CLI mode");
        return run_cli_mode(&args[2..]);
    }

    info!("Starting ImagePacker GPUI window");
    run_gui()
}
