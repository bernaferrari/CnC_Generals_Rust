//! ParticleEditor tool chrome (egui + ui_framework).
//!
//! Layout mirrors the C++ DebugWindowDialog workflow:
//! - File / Edit / View menus
//! - Left: list of particle systems (`m_listOfParticleSystems` + selection)
//! - Center: preview placeholder + key params bound to `ParticleSystemInfo`
//! - Right: property fields written into the same struct `_writeSingleParticleSystem`
//!   / [`crate::export::ParticleExporter::generate_ini_content`] ships
//! - Status: system count, selected name, dirty flag

use crate::export::ParticleExporter;
use crate::particles::{
    DistributionType, GameClientRandomVariable, ParticlePriorityType, ParticleShaderType,
    ParticleSystem, ParticleType,
};
use crate::preview::ParticlePreview;
use eframe::egui;
use std::collections::HashMap;

/// Menu definitions used by both the live bar and unit tests.
#[derive(Debug, Clone, Copy)]
pub struct ChromeMenu {
    pub label: &'static str,
    pub items: &'static [ChromeMenuItem],
}

#[derive(Debug, Clone, Copy)]
pub struct ChromeMenuItem {
    pub label: &'static str,
    pub action: Option<ChromeAction>,
}

/// Actions raised by File / Edit menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeAction {
    NewSystem,
    Open,
    Save,
    ExportIni,
    Exit,
    ResetSystem,
    DuplicateSystem,
    DeleteSystem,
}

/// View-panel visibility + exit request.
#[derive(Debug, Clone)]
pub struct ChromeViewState {
    pub show_preview: bool,
    pub show_timeline: bool,
    pub show_properties: bool,
    pub exit_requested: bool,
}

impl Default for ChromeViewState {
    fn default() -> Self {
        Self {
            show_preview: true,
            show_timeline: true,
            show_properties: true,
            exit_requested: false,
        }
    }
}

impl ChromeViewState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Field edits that write the exact `ParticleSystemInfo` members export.rs serializes.
#[derive(Debug, Clone, PartialEq)]
pub enum ChromeField {
    Name(String),
    Lifetime { low: f32, high: f32 },
    BurstCount { low: f32, high: f32 },
    BurstDelay { low: f32, high: f32 },
    ParticleType(ParticleType),
    ParticleTypeName(String),
    ShaderType(ParticleShaderType),
    Priority(ParticlePriorityType),
    Gravity(f32),
    SystemLifetime(u32),
    StartSize { low: f32, high: f32 },
    IsOneShot(bool),
    IsHollow(bool),
    IsGroundAligned(bool),
    IsEmitAboveGroundOnly(bool),
    IsParticleUpTowardsEmitter(bool),
}

pub const CHROME_MENUS: &[ChromeMenu] = &[
    ChromeMenu {
        label: "File",
        items: &[
            ChromeMenuItem {
                label: "New System",
                action: Some(ChromeAction::NewSystem),
            },
            ChromeMenuItem {
                label: "Open...",
                action: Some(ChromeAction::Open),
            },
            ChromeMenuItem {
                label: "Save",
                action: Some(ChromeAction::Save),
            },
            ChromeMenuItem {
                label: "Export INI...",
                action: Some(ChromeAction::ExportIni),
            },
            ChromeMenuItem {
                label: "Exit",
                action: Some(ChromeAction::Exit),
            },
        ],
    },
    ChromeMenu {
        label: "Edit",
        items: &[
            ChromeMenuItem {
                label: "Reset System",
                action: Some(ChromeAction::ResetSystem),
            },
            ChromeMenuItem {
                label: "Duplicate System",
                action: Some(ChromeAction::DuplicateSystem),
            },
            ChromeMenuItem {
                label: "Delete System",
                action: Some(ChromeAction::DeleteSystem),
            },
        ],
    },
    ChromeMenu {
        label: "View",
        items: &[
            ChromeMenuItem {
                label: "Preview",
                action: None,
            },
            ChromeMenuItem {
                label: "Timeline",
                action: None,
            },
            ChromeMenuItem {
                label: "Properties",
                action: None,
            },
        ],
    },
];

/// Testable menu table (File / Edit / View).
pub fn chrome_menus() -> &'static [ChromeMenu] {
    CHROME_MENUS
}

/// Status-bar text: system count, selected name, dirty flag.
pub fn status_bar_text(system_count: usize, selected_name: &str, dirty: bool) -> String {
    format!(
        "Systems: {} | Selected: {} | {}",
        system_count,
        selected_name,
        if dirty { "Dirty" } else { "Saved" }
    )
}

/// Apply a chrome field edit onto the system struct used by `_writeSingleParticleSystem`.
pub fn apply_chrome_field(system: &mut ParticleSystem, field: ChromeField) {
    match field {
        ChromeField::Name(name) => system.info.name = name,
        ChromeField::Lifetime { low, high } => system.info.lifetime.set_range(low, high),
        ChromeField::BurstCount { low, high } => system.info.burst_count.set_range(low, high),
        ChromeField::BurstDelay { low, high } => system.info.burst_delay.set_range(low, high),
        ChromeField::ParticleType(pt) => system.info.particle_type = pt,
        ChromeField::ParticleTypeName(name) => system.info.particle_type_name = name,
        ChromeField::ShaderType(shader) => system.info.shader_type = shader,
        ChromeField::Priority(priority) => system.info.priority = priority,
        ChromeField::Gravity(g) => system.info.gravity = g,
        ChromeField::SystemLifetime(frames) => system.info.system_lifetime = frames,
        ChromeField::StartSize { low, high } => system.info.start_size.set_range(low, high),
        ChromeField::IsOneShot(v) => system.info.is_one_shot = v,
        ChromeField::IsHollow(v) => system.info.is_emission_volume_hollow = v,
        ChromeField::IsGroundAligned(v) => system.info.is_ground_aligned = v,
        ChromeField::IsEmitAboveGroundOnly(v) => system.info.is_emit_above_ground_only = v,
        ChromeField::IsParticleUpTowardsEmitter(v) => {
            system.info.is_particle_up_towards_emitter = v
        }
    }
}

/// Generate shipped INI for a system (same path as File > Export INI).
pub fn export_system_ini(system: &ParticleSystem) -> String {
    ParticleExporter::new().generate_ini_content(system)
}

/// Draw File / Edit / View into the ui_framework tool menu bar.
pub fn show_menu_bar(ui: &mut egui::Ui, view: &mut ChromeViewState) -> Option<ChromeAction> {
    let mut action = None;

    for menu in CHROME_MENUS {
        ui.menu_button(menu.label, |ui| {
            if menu.label == "View" {
                ui.checkbox(&mut view.show_preview, "Preview");
                ui.checkbox(&mut view.show_timeline, "Timeline");
                ui.checkbox(&mut view.show_properties, "Properties");
                return;
            }
            for item in menu.items {
                if ui.button(item.label).clicked() {
                    if let Some(a) = item.action {
                        action = Some(a);
                    }
                    ui.close();
                }
            }
        });
    }

    action
}

/// Left-panel system list. Returns a newly selected index, if any.
pub fn show_system_list(
    ui: &mut egui::Ui,
    systems: &[ParticleSystem],
    selected: Option<usize>,
    templates: &HashMap<String, crate::particles::ParticleSystemTemplate>,
) -> SystemListCommand {
    let mut cmd = SystemListCommand::None;

    ui.heading("Particle Systems");
    ui.label(format!("{} system(s)", systems.len()));
    ui.separator();

    if ui.button("New System").clicked() {
        cmd = SystemListCommand::NewBlank;
    }

    egui::ScrollArea::vertical()
        .id_salt("particle_system_list")
        .max_height(ui.available_height() - 120.0)
        .show(ui, |ui| {
            if systems.is_empty() {
                ui.weak("No systems — File > New System");
            }
            for (i, system) in systems.iter().enumerate() {
                let selected_here = selected == Some(i);
                if ui
                    .selectable_label(selected_here, &system.info.name)
                    .clicked()
                {
                    cmd = SystemListCommand::Select(i);
                }
            }
        });

    ui.separator();
    ui.label("New from template:");
    for name in sorted_template_names(templates) {
        if ui.small_button(&name).clicked() {
            cmd = SystemListCommand::NewFromTemplate(name);
        }
    }

    cmd
}

fn sorted_template_names(
    templates: &HashMap<String, crate::particles::ParticleSystemTemplate>,
) -> Vec<String> {
    let mut names: Vec<String> = templates.keys().cloned().collect();
    names.sort();
    names
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemListCommand {
    None,
    Select(usize),
    NewBlank,
    NewFromTemplate(String),
}

/// Center: preview placeholder + key params bound to the live system.
/// Returns true if a bound field changed.
pub fn show_center_panel(
    ui: &mut egui::Ui,
    system: Option<&mut ParticleSystem>,
    preview: &mut ParticlePreview,
    current_time: f32,
    show_preview: bool,
) -> bool {
    let mut changed = false;

    ui.heading("Preview");
    if let Some(system) = system {
        ui.group(|ui| {
            ui.label("Key parameters (export-bound)");
            changed |= show_key_params(ui, system);
        });
        ui.separator();
        if show_preview {
            preview.show(ui, Some(system), current_time);
        } else {
            ui.weak("Preview hidden (View > Preview)");
        }
    } else {
        ui.weak("No particle system selected");
        ui.allocate_space(egui::vec2(ui.available_width(), 240.0));
    }

    changed
}

fn show_key_params(ui: &mut egui::Ui, system: &mut ParticleSystem) -> bool {
    let mut changed = false;

    // Emission rate == C++ BurstCount (particles per burst).
    changed |= edit_random_var(
        ui,
        "Emission Rate (Burst Count)",
        &mut system.info.burst_count,
    );
    changed |= edit_random_var(ui, "Burst Delay", &mut system.info.burst_delay);
    changed |= edit_random_var(ui, "Lifetime", &mut system.info.lifetime);
    changed |= edit_particle_type(ui, &mut system.info.particle_type);

    changed
}

/// Right: property fields that write into the export system struct.
pub fn show_properties_panel(ui: &mut egui::Ui, system: Option<&mut ParticleSystem>) -> bool {
    let mut changed = false;
    ui.heading("Properties");
    ui.label("Writes ParticleSystemInfo used by _writeSingleParticleSystem");
    ui.separator();

    if let Some(system) = system {
        ui.horizontal(|ui| {
            ui.label("Name:");
            if ui.text_edit_singleline(&mut system.info.name).changed() {
                changed = true;
            }
        });

        changed |= edit_particle_type(ui, &mut system.info.particle_type);

        ui.horizontal(|ui| {
            ui.label("Particle Name:");
            if ui
                .text_edit_singleline(&mut system.info.particle_type_name)
                .changed()
            {
                changed = true;
            }
        });

        changed |= edit_shader_type(ui, &mut system.info.shader_type);
        changed |= edit_priority(ui, &mut system.info.priority);

        if ui
            .checkbox(&mut system.info.is_one_shot, "Is One Shot")
            .changed()
        {
            changed = true;
        }

        changed |= edit_random_var(ui, "Lifetime", &mut system.info.lifetime);
        ui.horizontal(|ui| {
            ui.label("System Lifetime (frames):");
            if ui
                .add(egui::DragValue::new(&mut system.info.system_lifetime).speed(1.0))
                .changed()
            {
                changed = true;
            }
        });
        changed |= edit_random_var(ui, "Size", &mut system.info.start_size);
        changed |= edit_random_var(
            ui,
            "Emission Rate (Burst Count)",
            &mut system.info.burst_count,
        );
        changed |= edit_random_var(ui, "Burst Delay", &mut system.info.burst_delay);
        changed |= edit_random_var(ui, "Initial Delay", &mut system.info.initial_delay);

        ui.horizontal(|ui| {
            ui.label("Gravity:");
            if ui
                .add(egui::DragValue::new(&mut system.info.gravity).speed(0.01))
                .changed()
            {
                changed = true;
            }
        });

        ui.separator();
        ui.label("Switches");
        if ui
            .checkbox(&mut system.info.is_emission_volume_hollow, "Hollow")
            .changed()
        {
            changed = true;
        }
        if ui
            .checkbox(&mut system.info.is_ground_aligned, "Ground Aligned")
            .changed()
        {
            changed = true;
        }
        if ui
            .checkbox(
                &mut system.info.is_emit_above_ground_only,
                "Emit Above Ground Only",
            )
            .changed()
        {
            changed = true;
        }
        if ui
            .checkbox(
                &mut system.info.is_particle_up_towards_emitter,
                "Particle Up Towards Emitter",
            )
            .changed()
        {
            changed = true;
        }
    } else {
        ui.weak("No particle system selected");
    }

    changed
}

/// Bottom status bar: count, selected name, dirty, fps.
pub fn show_status_bar(
    ui: &mut egui::Ui,
    system_count: usize,
    selected_name: &str,
    dirty: bool,
    fps: f64,
) {
    ui.horizontal(|ui| {
        ui.label(status_bar_text(system_count, selected_name, dirty));
        if dirty {
            ui.colored_label(egui::Color32::YELLOW, "Unsaved Changes");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("FPS: {:.0}", fps));
        });
    });
}

fn edit_random_var(ui: &mut egui::Ui, label: &str, var: &mut GameClientRandomVariable) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        let low = ui.add(egui::DragValue::new(&mut var.low).speed(0.1));
        ui.label("-");
        let high = ui.add(egui::DragValue::new(&mut var.high).speed(0.1));
        if low.changed() || high.changed() {
            changed = true;
            if var.low > var.high {
                if low.changed() {
                    var.high = var.low;
                } else {
                    var.low = var.high;
                }
            }
            var.distribution = if (var.low - var.high).abs() <= f32::EPSILON {
                DistributionType::Constant
            } else {
                DistributionType::Uniform
            };
        }
    });
    changed
}

fn edit_particle_type(ui: &mut egui::Ui, value: &mut ParticleType) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Particle Type:");
        egui::ComboBox::from_id_salt("chrome_particle_type")
            .selected_text(particle_type_label(*value))
            .show_ui(ui, |ui| {
                for pt in [
                    ParticleType::Particle,
                    ParticleType::Drawable,
                    ParticleType::Streak,
                    ParticleType::VolumeParticle,
                    ParticleType::Smudge,
                ] {
                    if ui
                        .selectable_label(*value == pt, particle_type_label(pt))
                        .clicked()
                    {
                        *value = pt;
                        changed = true;
                    }
                }
            });
    });
    changed
}

fn edit_shader_type(ui: &mut egui::Ui, value: &mut ParticleShaderType) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Shader:");
        egui::ComboBox::from_id_salt("chrome_shader_type")
            .selected_text(shader_label(*value))
            .show_ui(ui, |ui| {
                for shader in [
                    ParticleShaderType::Additive,
                    ParticleShaderType::Alpha,
                    ParticleShaderType::AlphaTest,
                    ParticleShaderType::Multiply,
                ] {
                    if ui
                        .selectable_label(*value == shader, shader_label(shader))
                        .clicked()
                    {
                        *value = shader;
                        changed = true;
                    }
                }
            });
    });
    changed
}

fn edit_priority(ui: &mut egui::Ui, value: &mut ParticlePriorityType) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Priority:");
        egui::ComboBox::from_id_salt("chrome_priority")
            .selected_text(priority_label(*value))
            .show_ui(ui, |ui| {
                for prio in [
                    ParticlePriorityType::WeaponExplosion,
                    ParticlePriorityType::ScorchMark,
                    ParticlePriorityType::DustTrail,
                    ParticlePriorityType::Buildup,
                    ParticlePriorityType::DebrisTrail,
                    ParticlePriorityType::UnitDamageFx,
                    ParticlePriorityType::DeathExplosion,
                    ParticlePriorityType::SemiConstant,
                    ParticlePriorityType::Constant,
                    ParticlePriorityType::WeaponTrail,
                    ParticlePriorityType::AreaEffect,
                    ParticlePriorityType::Critical,
                    ParticlePriorityType::AlwaysRender,
                ] {
                    if ui
                        .selectable_label(*value == prio, priority_label(prio))
                        .clicked()
                    {
                        *value = prio;
                        changed = true;
                    }
                }
            });
    });
    changed
}

fn particle_type_label(pt: ParticleType) -> &'static str {
    match pt {
        ParticleType::Invalid => "INVALID",
        ParticleType::Particle => "PARTICLE",
        ParticleType::Drawable => "DRAWABLE",
        ParticleType::Streak => "STREAK",
        ParticleType::VolumeParticle => "VOLUME_PARTICLE",
        ParticleType::Smudge => "SMUDGE",
    }
}

fn shader_label(shader: ParticleShaderType) -> &'static str {
    match shader {
        ParticleShaderType::Invalid => "INVALID",
        ParticleShaderType::Additive => "ADDITIVE",
        ParticleShaderType::Alpha => "ALPHA",
        ParticleShaderType::AlphaTest => "ALPHA_TEST",
        ParticleShaderType::Multiply => "MULTIPLY",
    }
}

fn priority_label(priority: ParticlePriorityType) -> &'static str {
    match priority {
        ParticlePriorityType::Invalid => "INVALID",
        ParticlePriorityType::WeaponExplosion => "WEAPON_EXPLOSION",
        ParticlePriorityType::ScorchMark => "SCORCHMARK",
        ParticlePriorityType::DustTrail => "DUST_TRAIL",
        ParticlePriorityType::Buildup => "BUILDUP",
        ParticlePriorityType::DebrisTrail => "DEBRIS_TRAIL",
        ParticlePriorityType::UnitDamageFx => "UNIT_DAMAGE_FX",
        ParticlePriorityType::DeathExplosion => "DEATH_EXPLOSION",
        ParticlePriorityType::SemiConstant => "SEMI_CONSTANT",
        ParticlePriorityType::Constant => "CONSTANT",
        ParticlePriorityType::WeaponTrail => "WEAPON_TRAIL",
        ParticlePriorityType::AreaEffect => "AREA_EFFECT",
        ParticlePriorityType::Critical => "CRITICAL",
        ParticlePriorityType::AlwaysRender => "ALWAYS_RENDER",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::ParticleEditorTool;

    #[test]
    fn chrome_menus_exist() {
        let menus = chrome_menus();
        let labels: Vec<_> = menus.iter().map(|m| m.label).collect();
        assert_eq!(labels, ["File", "Edit", "View"]);

        let file = menus.iter().find(|m| m.label == "File").expect("File");
        for item in ["New System", "Open...", "Save", "Export INI...", "Exit"] {
            assert!(
                file.items.iter().any(|i| i.label == item),
                "File menu missing {item}"
            );
        }
        assert!(
            file.items
                .iter()
                .any(|i| i.label == "New System" && i.action == Some(ChromeAction::NewSystem))
        );
        assert!(
            file.items
                .iter()
                .any(|i| i.label == "Export INI..." && i.action == Some(ChromeAction::ExportIni))
        );

        let edit = menus.iter().find(|m| m.label == "Edit").expect("Edit");
        assert!(!edit.items.is_empty());

        let view = menus.iter().find(|m| m.label == "View").expect("View");
        for item in ["Preview", "Timeline", "Properties"] {
            assert!(
                view.items.iter().any(|i| i.label == item),
                "View menu missing {item}"
            );
        }
    }

    #[test]
    fn create_and_select_system() {
        let mut tool = ParticleEditorTool::new().expect("tool");
        assert_eq!(tool.system_count(), 0);
        assert!(tool.selected_system().is_none());

        let a = tool.create_system("Alpha").expect("alpha");
        let b = tool.create_system("Beta").expect("beta");
        assert_eq!(a, 0);
        assert_eq!(b, 1);
        assert_eq!(tool.system_count(), 2);
        assert_eq!(tool.selected_name().as_deref(), Some("Beta"));

        tool.select_system(0).expect("select alpha");
        assert_eq!(tool.selected_name().as_deref(), Some("Alpha"));
        assert_eq!(tool.system_names(), ["Alpha", "Beta"]);

        tool.apply_chrome_action(ChromeAction::NewSystem)
            .expect("new via chrome");
        assert_eq!(tool.system_count(), 3);
        assert!(tool.selected_index().is_some());
    }

    #[test]
    fn chrome_field_edit_visible_on_export_and_struct() {
        let mut tool = ParticleEditorTool::new().expect("tool");
        tool.create_system("ChromeFx").expect("create");

        {
            let system = tool.selected_system_mut().expect("selected");
            apply_chrome_field(
                system,
                ChromeField::Lifetime {
                    low: 12.0,
                    high: 24.0,
                },
            );
            apply_chrome_field(
                system,
                ChromeField::BurstCount {
                    low: 5.0,
                    high: 5.0,
                },
            );
            apply_chrome_field(system, ChromeField::ParticleType(ParticleType::Streak));
            apply_chrome_field(
                system,
                ChromeField::ParticleTypeName("EXPtracer".to_string()),
            );
            apply_chrome_field(system, ChromeField::Gravity(-2.5));
            apply_chrome_field(system, ChromeField::IsOneShot(true));
        }

        let selected = tool.selected_system().expect("selected");
        assert_eq!(selected.info.particle_type, ParticleType::Streak);
        assert_eq!(selected.info.particle_type_name, "EXPtracer");
        assert!((selected.info.lifetime.low - 12.0).abs() < 1e-5);
        assert!((selected.info.lifetime.high - 24.0).abs() < 1e-5);
        assert!((selected.info.burst_count.low - 5.0).abs() < 1e-5);
        assert!(selected.info.is_one_shot);

        let ini = tool.export_selected_ini().expect("ini");
        assert!(ini.contains("ParticleSystem ChromeFx"), "{ini}");
        assert!(
            ini.contains("Lifetime = 12 24") || ini.contains("Lifetime = 12.0 24.0"),
            "{ini}"
        );
        assert!(ini.contains("BurstCount = 5"), "{ini}");
        assert!(ini.contains("Type = STREAK"), "{ini}");
        assert!(ini.contains("ParticleName = EXPtracer"), "{ini}");
        assert!(
            ini.contains("Gravity = -2.5") || ini.contains("Gravity = -2.50"),
            "{ini}"
        );
        assert!(ini.contains("IsOneShot = Yes"), "{ini}");

        // Same bytes as File > Export INI / `_writeSingleParticleSystem`.
        assert_eq!(ini, export_system_ini(selected));
    }

    #[test]
    fn status_bar_reports_count_name_and_dirty() {
        let mut tool = ParticleEditorTool::new().expect("tool");
        assert!(!tool.has_unsaved_changes());
        tool.create_system("Dust").expect("create");
        assert!(tool.has_unsaved_changes());
        let text = status_bar_text(
            tool.system_count(),
            tool.selected_name().as_deref().unwrap_or("(none)"),
            tool.has_unsaved_changes(),
        );
        assert!(text.contains("Systems: 1"), "{text}");
        assert!(text.contains("Dust"), "{text}");
        assert!(text.contains("Dirty"), "{text}");
    }

    #[test]
    fn duplicate_and_delete_via_chrome_actions() {
        let mut tool = ParticleEditorTool::new().expect("tool");
        tool.create_system("Spark").expect("create");
        tool.apply_chrome_action(ChromeAction::DuplicateSystem)
            .expect("dup");
        assert_eq!(tool.system_count(), 2);
        assert_eq!(tool.selected_name().as_deref(), Some("Spark Copy"));
        tool.apply_chrome_action(ChromeAction::DeleteSystem)
            .expect("del");
        assert_eq!(tool.system_count(), 1);
        assert_eq!(tool.selected_name().as_deref(), Some("Spark"));
    }
}
