//! Common dialog system for game development tools

use eframe::egui;
use std::collections::HashMap;

/// Dialog manager for handling modal dialogs
pub struct DialogManager {
    open_dialogs: HashMap<String, Box<dyn Dialog>>,
}

impl DialogManager {
    pub fn new() -> Self {
        Self {
            open_dialogs: HashMap::new(),
        }
    }

    /// Open a dialog
    pub fn open_dialog(&mut self, id: String, dialog: Box<dyn Dialog>) {
        self.open_dialogs.insert(id, dialog);
    }

    /// Close a dialog
    pub fn close_dialog(&mut self, id: &str) {
        self.open_dialogs.remove(id);
    }

    /// Get a mutable reference to a dialog
    pub fn get_panel_mut(&mut self, id: &str) -> Option<&mut Box<dyn Dialog>> {
        self.open_dialogs.get_mut(id)
    }

    /// Update all open dialogs
    pub fn update(&mut self, ctx: &egui::Context) {
        let mut to_close = Vec::new();

        for (id, dialog) in &mut self.open_dialogs {
            if !dialog.show(ctx) {
                to_close.push(id.clone());
            }
        }

        for id in to_close {
            self.close_dialog(&id);
        }
    }
}

/// Trait for modal dialogs
pub trait Dialog: Send + Sync {
    /// Show the dialog, returns false if dialog should be closed
    fn show(&mut self, ctx: &egui::Context) -> bool;

    /// Get dialog title
    fn title(&self) -> &str;

    /// Get dialog size
    fn size(&self) -> [f32; 2] {
        [400.0, 300.0]
    }

    /// Cast to Any for downcasting
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// About dialog
pub struct AboutDialog {
    title: String,
    version: String,
    description: String,
    open: bool,
}

impl AboutDialog {
    pub fn new(title: String, version: String, description: String) -> Self {
        Self {
            title,
            version,
            description,
            open: true,
        }
    }
}

impl Dialog for AboutDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut should_close = false;

        egui::Window::new("About")
            .collapsible(false)
            .resizable(false)
            .default_size(self.size())
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading(&self.title);
                    ui.label(format!("Version: {}", self.version));
                    ui.separator();
                    ui.label(&self.description);

                    ui.add_space(20.0);

                    if ui.button("Close").clicked() {
                        should_close = true;
                    }
                });
            });

        if should_close {
            self.open = false;
        }

        self.open
    }

    fn title(&self) -> &str {
        "About"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// File operations dialog (New, Open, Save)
pub struct FileDialog {
    dialog_type: FileDialogType,
    current_path: String,
    file_name: String,
    file_filter: String,
    result: Option<FileDialogResult>,
    open: bool,
}

#[derive(Debug, Clone)]
pub enum FileDialogType {
    Open,
    Save,
    SaveAs,
}

#[derive(Debug, Clone)]
pub struct FileDialogResult {
    pub path: String,
    pub action: FileDialogAction,
}

#[derive(Debug, Clone)]
pub enum FileDialogAction {
    Open,
    Save,
    Cancel,
}

impl FileDialog {
    pub fn new(dialog_type: FileDialogType, filter: &str) -> Self {
        Self {
            dialog_type,
            current_path: std::env::current_dir()
                .unwrap_or_default()
                .display()
                .to_string(),
            file_name: String::new(),
            file_filter: filter.to_string(),
            result: None,
            open: true,
        }
    }

    pub fn get_result(&mut self) -> Option<FileDialogResult> {
        self.result.take()
    }
}

impl Dialog for FileDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut should_close = false;

        let title = match self.dialog_type {
            FileDialogType::Open => "Open File",
            FileDialogType::Save => "Save File",
            FileDialogType::SaveAs => "Save File As",
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .default_size([600.0, 400.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Path:");
                    ui.text_edit_singleline(&mut self.current_path);
                    if ui.button("Browse...").clicked() {
                        // Use native file dialog
                        match self.dialog_type {
                            FileDialogType::Open => {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Supported Files", &[&self.file_filter])
                                    .pick_file()
                                {
                                    self.result = Some(FileDialogResult {
                                        path: path.display().to_string(),
                                        action: FileDialogAction::Open,
                                    });
                                    should_close = true;
                                }
                            }
                            FileDialogType::Save | FileDialogType::SaveAs => {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Supported Files", &[&self.file_filter])
                                    .save_file()
                                {
                                    self.result = Some(FileDialogResult {
                                        path: path.display().to_string(),
                                        action: FileDialogAction::Save,
                                    });
                                    should_close = true;
                                }
                            }
                        }
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("File name:");
                    ui.text_edit_singleline(&mut self.file_name);
                });

                ui.separator();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::BOTTOM), |ui| {
                    if ui.button("Cancel").clicked() {
                        self.result = Some(FileDialogResult {
                            path: String::new(),
                            action: FileDialogAction::Cancel,
                        });
                        should_close = true;
                    }

                    let action_text = match self.dialog_type {
                        FileDialogType::Open => "Open",
                        FileDialogType::Save | FileDialogType::SaveAs => "Save",
                    };

                    if ui.button(action_text).clicked() {
                        let full_path = if self.file_name.is_empty() {
                            self.current_path.clone()
                        } else {
                            format!("{}/{}", self.current_path, self.file_name)
                        };

                        self.result = Some(FileDialogResult {
                            path: full_path,
                            action: match self.dialog_type {
                                FileDialogType::Open => FileDialogAction::Open,
                                FileDialogType::Save | FileDialogType::SaveAs => {
                                    FileDialogAction::Save
                                }
                            },
                        });
                        should_close = true;
                    }
                });
            });

        if should_close {
            self.open = false;
        }

        self.open
    }

    fn title(&self) -> &str {
        match self.dialog_type {
            FileDialogType::Open => "Open File",
            FileDialogType::Save => "Save File",
            FileDialogType::SaveAs => "Save File As",
        }
    }

    fn size(&self) -> [f32; 2] {
        [600.0, 400.0]
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Confirmation dialog
pub struct ConfirmDialog {
    title: String,
    message: String,
    result: Option<bool>,
    open: bool,
}

impl ConfirmDialog {
    pub fn new(title: String, message: String) -> Self {
        Self {
            title,
            message,
            result: None,
            open: true,
        }
    }

    pub fn get_result(&mut self) -> Option<bool> {
        self.result.take()
    }
}

impl Dialog for ConfirmDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut should_close = false;

        egui::Window::new(&self.title)
            .collapsible(false)
            .resizable(false)
            .default_size(self.size())
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(&self.message);

                    ui.add_space(20.0);

                    ui.horizontal(|ui| {
                        if ui.button("Yes").clicked() {
                            self.result = Some(true);
                            should_close = true;
                        }

                        if ui.button("No").clicked() {
                            self.result = Some(false);
                            should_close = true;
                        }
                    });
                });
            });

        if should_close {
            self.open = false;
        }

        self.open
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Progress dialog for long operations
pub struct ProgressDialog {
    title: String,
    message: String,
    progress: f32,
    can_cancel: bool,
    cancelled: bool,
    open: bool,
}

impl ProgressDialog {
    pub fn new(title: String, message: String, can_cancel: bool) -> Self {
        Self {
            title,
            message,
            progress: 0.0,
            can_cancel,
            cancelled: false,
            open: true,
        }
    }

    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

impl Dialog for ProgressDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut should_close = false;

        egui::Window::new(&self.title)
            .collapsible(false)
            .resizable(false)
            .default_size(self.size())
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(&self.message);

                    ui.add_space(10.0);

                    // Progress bar
                    crate::widgets::ProgressBarWidget::show(ui, self.progress, None);

                    ui.add_space(10.0);

                    if self.can_cancel {
                        if ui.button("Cancel").clicked() {
                            self.cancelled = true;
                            should_close = true;
                        }
                    }
                });
            });

        if should_close {
            self.open = false;
        }

        self.open
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn size(&self) -> [f32; 2] {
        [400.0, 150.0]
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
