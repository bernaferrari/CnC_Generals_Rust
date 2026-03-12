//! UI Module
//!
//! Handles the user interface for the particle editor.

use crate::particles::*;
use anyhow::Result;
use eframe::egui;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum UiAction {
    NewParticleSystem,
    LoadParticleSystem,
    SaveParticleSystem,
}

/// Particle editor UI
#[derive(Debug, Clone)]
pub struct ParticleEditorUI {
    pub show_preview: bool,
    pub show_timeline: bool,
    pub show_properties: bool,
    pub show_emission_panel: bool,
    pub show_velocity_panel: bool,
    pub show_particle_panel: bool,
    pub show_shader_panel: bool,
    pub selected_property_tab: PropertyTab,
    pub parameter_changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PropertyTab {
    Emission,
    Velocity,
    Particle,
    Shader,
    Physics,
    Keyframes,
}

impl ParticleEditorUI {
    pub fn new() -> Self {
        Self {
            show_preview: true,
            show_timeline: true,
            show_properties: true,
            show_emission_panel: true,
            show_velocity_panel: true,
            show_particle_panel: true,
            show_shader_panel: true,
            selected_property_tab: PropertyTab::Emission,
            parameter_changed: false,
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        log::info!("Initializing particle editor UI");
        Ok(())
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        system: &mut Option<ParticleSystem>,
    ) -> Option<UiAction> {
        let mut action = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(menu_action) = self.show_menu_bar(ui) {
                action = Some(menu_action);
            }

            ui.horizontal(|ui| {
                // Left panel - Properties
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    self.show_properties_panel(ui, system);
                });

                ui.separator();

                // Right panel - Preview and Timeline
                ui.vertical(|ui| {
                    if self.show_preview {
                        self.show_preview_panel(ui, system);
                    }

                    if self.show_timeline {
                        ui.separator();
                        self.show_timeline_panel(ui, system);
                    }
                });
            });
        });
        action
    }

    fn show_menu_bar(&mut self, ui: &mut egui::Ui) -> Option<UiAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Particle System").clicked() {
                    action = Some(UiAction::NewParticleSystem);
                }
                if ui.button("Load Particle System").clicked() {
                    action = Some(UiAction::LoadParticleSystem);
                }
                if ui.button("Save Particle System").clicked() {
                    action = Some(UiAction::SaveParticleSystem);
                }
            });

            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.show_preview, "Preview");
                ui.checkbox(&mut self.show_timeline, "Timeline");
                ui.checkbox(&mut self.show_properties, "Properties");
                ui.separator();
                ui.checkbox(&mut self.show_emission_panel, "Emission Panel");
                ui.checkbox(&mut self.show_velocity_panel, "Velocity Panel");
                ui.checkbox(&mut self.show_particle_panel, "Particle Panel");
                ui.checkbox(&mut self.show_shader_panel, "Shader Panel");
            });

            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    // TODO: Show about dialog
                }
            });
        });
        action
    }

    pub fn show_templates_panel<F>(
        &mut self,
        ui: &mut egui::Ui,
        templates: &HashMap<String, ParticleSystemTemplate>,
        mut on_select: F,
    ) where
        F: FnMut(&str),
    {
        ui.group(|ui| {
            ui.heading("Templates");

            ui.label("Select a template to create a new particle system:");
            ui.separator();

            for (name, _template) in templates {
                if ui.button(name).clicked() {
                    on_select(name);
                }
            }
        });
    }

    pub fn show_properties_panel(
        &mut self,
        ui: &mut egui::Ui,
        system: &mut Option<ParticleSystem>,
    ) {
        ui.group(|ui| {
            ui.heading("Properties");

            if let Some(system) = system {
                ui.horizontal(|ui| {
                    ui.label("System Name:");
                    ui.text_edit_singleline(&mut system.info.name);
                });

                ui.separator();

                // Tab selection
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            self.selected_property_tab == PropertyTab::Emission,
                            "Emission",
                        )
                        .clicked()
                    {
                        self.selected_property_tab = PropertyTab::Emission;
                    }
                    if ui
                        .selectable_label(
                            self.selected_property_tab == PropertyTab::Velocity,
                            "Velocity",
                        )
                        .clicked()
                    {
                        self.selected_property_tab = PropertyTab::Velocity;
                    }
                    if ui
                        .selectable_label(
                            self.selected_property_tab == PropertyTab::Particle,
                            "Particle",
                        )
                        .clicked()
                    {
                        self.selected_property_tab = PropertyTab::Particle;
                    }
                    if ui
                        .selectable_label(
                            self.selected_property_tab == PropertyTab::Shader,
                            "Shader",
                        )
                        .clicked()
                    {
                        self.selected_property_tab = PropertyTab::Shader;
                    }
                    if ui
                        .selectable_label(
                            self.selected_property_tab == PropertyTab::Physics,
                            "Physics",
                        )
                        .clicked()
                    {
                        self.selected_property_tab = PropertyTab::Physics;
                    }
                    if ui
                        .selectable_label(
                            self.selected_property_tab == PropertyTab::Keyframes,
                            "Keyframes",
                        )
                        .clicked()
                    {
                        self.selected_property_tab = PropertyTab::Keyframes;
                    }
                });

                ui.separator();

                match self.selected_property_tab {
                    PropertyTab::Emission => self.show_emission_tab(ui, system),
                    PropertyTab::Velocity => self.show_velocity_tab(ui, system),
                    PropertyTab::Particle => self.show_particle_tab(ui, system),
                    PropertyTab::Shader => self.show_shader_tab(ui, system),
                    PropertyTab::Physics => self.show_physics_tab(ui, system),
                    PropertyTab::Keyframes => self.show_keyframes_tab(ui, system),
                }
            } else {
                ui.label("No particle system loaded");
                if ui.button("Create New System").clicked() {
                    if let Ok(new_system) = ParticleSystem::new("NewParticleSystem".to_string()) {
                        *system = Some(new_system);
                    }
                }
            }
        });
    }

    fn show_emission_tab(&mut self, ui: &mut egui::Ui, system: &mut ParticleSystem) {
        ui.label("Emission Settings");

        // Emission volume type
        ui.horizontal(|ui| {
            ui.label("Volume Type:");
            egui::ComboBox::from_label("")
                .selected_text(format!("{:?}", system.info.emission_volume_type))
                .show_ui(ui, |ui| {
                    for volume_type in [
                        EmissionVolumeType::Point,
                        EmissionVolumeType::Line,
                        EmissionVolumeType::Box,
                        EmissionVolumeType::Sphere,
                        EmissionVolumeType::Cylinder,
                        EmissionVolumeType::Invalid,
                    ] {
                        if ui
                            .selectable_label(
                                system.info.emission_volume_type == volume_type,
                                format!("{:?}", volume_type),
                            )
                            .clicked()
                        {
                            system.info.emission_volume_type = volume_type;
                            // Reset emission volume data based on type
                            system.info.emission_volume = match volume_type {
                                EmissionVolumeType::Point => EmissionVolumeData::Point,
                                EmissionVolumeType::Line => EmissionVolumeData::Line {
                                    start: Coord3D::default(),
                                    end: Coord3D {
                                        x: 1.0,
                                        y: 0.0,
                                        z: 0.0,
                                    },
                                },
                                EmissionVolumeType::Box => EmissionVolumeData::Box {
                                    half_size: Coord3D {
                                        x: 0.5,
                                        y: 0.5,
                                        z: 0.5,
                                    },
                                },
                                EmissionVolumeType::Sphere => {
                                    EmissionVolumeData::Sphere { radius: 1.0 }
                                }
                                EmissionVolumeType::Cylinder => EmissionVolumeData::Cylinder {
                                    radius: 1.0,
                                    length: 2.0,
                                },
                                EmissionVolumeType::Invalid => EmissionVolumeData::Point, // Default to Point for Invalid
                            };
                        }
                    }
                });
        });

        // Emission volume parameters
        match &mut system.info.emission_volume {
            EmissionVolumeData::Point => {
                ui.label("Point emission - no parameters needed");
            }
            EmissionVolumeData::Line { start, end } => {
                ui.label("Line Start:");
                ui.horizontal(|ui| {
                    ui.label("X:");
                    ui.add(egui::DragValue::new(&mut start.x).speed(0.1));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut start.y).speed(0.1));
                    ui.label("Z:");
                    ui.add(egui::DragValue::new(&mut start.z).speed(0.1));
                });
                ui.label("Line End:");
                ui.horizontal(|ui| {
                    ui.label("X:");
                    ui.add(egui::DragValue::new(&mut end.x).speed(0.1));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut end.y).speed(0.1));
                    ui.label("Z:");
                    ui.add(egui::DragValue::new(&mut end.z).speed(0.1));
                });
            }
            EmissionVolumeData::Box { half_size } => {
                ui.label("Box Half Size:");
                ui.horizontal(|ui| {
                    ui.label("X:");
                    ui.add(egui::DragValue::new(&mut half_size.x).speed(0.1));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut half_size.y).speed(0.1));
                    ui.label("Z:");
                    ui.add(egui::DragValue::new(&mut half_size.z).speed(0.1));
                });
            }
            EmissionVolumeData::Sphere { radius } => {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    ui.add(egui::DragValue::new(radius).speed(0.1));
                });
            }
            EmissionVolumeData::Cylinder { radius, length } => {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    ui.add(egui::DragValue::new(radius).speed(0.1));
                    ui.label("Length:");
                    ui.add(egui::DragValue::new(length).speed(0.1));
                });
            }
        }

        ui.checkbox(&mut system.info.is_emission_volume_hollow, "Hollow");
        ui.checkbox(
            &mut system.info.is_emit_above_ground_only,
            "Emit Above Ground Only",
        );
    }

    fn show_velocity_tab(&mut self, ui: &mut egui::Ui, system: &mut ParticleSystem) {
        ui.label("Velocity Settings");

        // Emission velocity type
        ui.horizontal(|ui| {
            ui.label("Velocity Type:");
            egui::ComboBox::from_label("")
                .selected_text(format!("{:?}", system.info.emission_velocity_type))
                .show_ui(ui, |ui| {
                    for velocity_type in [
                        EmissionVelocityType::Ortho,
                        EmissionVelocityType::Spherical,
                        EmissionVelocityType::Hemispherical,
                        EmissionVelocityType::Cylindrical,
                        EmissionVelocityType::Outward,
                        EmissionVelocityType::Invalid,
                    ] {
                        if ui
                            .selectable_label(
                                system.info.emission_velocity_type == velocity_type,
                                format!("{:?}", velocity_type),
                            )
                            .clicked()
                        {
                            system.info.emission_velocity_type = velocity_type;
                            // Reset emission velocity data based on type
                            system.info.emission_velocity = match velocity_type {
                                EmissionVelocityType::Ortho => EmissionVelocityData::Ortho {
                                    x: GameClientRandomVariable::constant(0.0),
                                    y: GameClientRandomVariable::constant(0.0),
                                    z: GameClientRandomVariable::constant(1.0),
                                },
                                EmissionVelocityType::Spherical => {
                                    EmissionVelocityData::Spherical {
                                        speed: GameClientRandomVariable::constant(1.0),
                                    }
                                }
                                EmissionVelocityType::Hemispherical => {
                                    EmissionVelocityData::Hemispherical {
                                        speed: GameClientRandomVariable::constant(1.0),
                                    }
                                }
                                EmissionVelocityType::Cylindrical => {
                                    EmissionVelocityData::Cylindrical {
                                        radial: GameClientRandomVariable::constant(0.0),
                                        normal: GameClientRandomVariable::constant(1.0),
                                    }
                                }
                                EmissionVelocityType::Outward => EmissionVelocityData::Outward {
                                    speed: GameClientRandomVariable::constant(1.0),
                                    other_speed: GameClientRandomVariable::constant(0.0),
                                },
                                EmissionVelocityType::Invalid => EmissionVelocityData::Ortho {
                                    x: GameClientRandomVariable::constant(0.0),
                                    y: GameClientRandomVariable::constant(0.0),
                                    z: GameClientRandomVariable::constant(1.0),
                                },
                            };
                        }
                    }
                });
        });

        // Velocity parameters
        match &mut system.info.emission_velocity {
            EmissionVelocityData::Ortho { x, y, z } => {
                self.show_random_variable(ui, "X Velocity", x);
                self.show_random_variable(ui, "Y Velocity", y);
                self.show_random_variable(ui, "Z Velocity", z);
            }
            EmissionVelocityData::Spherical { speed } => {
                self.show_random_variable(ui, "Speed", speed);
            }
            EmissionVelocityData::Hemispherical { speed } => {
                self.show_random_variable(ui, "Speed", speed);
            }
            EmissionVelocityData::Cylindrical { radial, normal } => {
                self.show_random_variable(ui, "Radial", radial);
                self.show_random_variable(ui, "Normal", normal);
            }
            EmissionVelocityData::Outward { speed, other_speed } => {
                self.show_random_variable(ui, "Speed", speed);
                self.show_random_variable(ui, "Other Speed", other_speed);
            }
        }
    }

    fn show_particle_tab(&mut self, ui: &mut egui::Ui, system: &mut ParticleSystem) {
        ui.label("Particle Settings");

        // Particle type
        ui.horizontal(|ui| {
            ui.label("Particle Type:");
            egui::ComboBox::from_label("")
                .selected_text(format!("{:?}", system.info.particle_type))
                .show_ui(ui, |ui| {
                    for particle_type in [
                        ParticleType::Particle,
                        ParticleType::Drawable,
                        ParticleType::Streak,
                    ] {
                        if ui
                            .selectable_label(
                                system.info.particle_type == particle_type,
                                format!("{:?}", particle_type),
                            )
                            .clicked()
                        {
                            system.info.particle_type = particle_type;
                        }
                    }
                });
        });

        // Basic properties
        self.show_random_variable(ui, "Lifetime", &mut system.info.lifetime);
        self.show_random_variable(ui, "Start Size", &mut system.info.start_size);
        self.show_random_variable(ui, "Size Rate", &mut system.info.size_rate);
        self.show_random_variable(ui, "Angular Rate Z", &mut system.info.angular_rate_z);

        ui.checkbox(&mut system.info.is_one_shot, "One Shot");
        ui.checkbox(&mut system.info.is_ground_aligned, "Ground Aligned");
        ui.checkbox(
            &mut system.info.is_particle_up_towards_emitter,
            "Particle Up Towards Emitter",
        );
    }

    fn show_shader_tab(&mut self, ui: &mut egui::Ui, system: &mut ParticleSystem) {
        ui.label("Shader Settings");

        // Shader type
        ui.horizontal(|ui| {
            ui.label("Shader Type:");
            egui::ComboBox::from_label("")
                .selected_text(format!("{:?}", system.info.shader_type))
                .show_ui(ui, |ui| {
                    for shader_type in [
                        ParticleShaderType::Additive,
                        ParticleShaderType::Alpha,
                        ParticleShaderType::AlphaTest,
                    ] {
                        if ui
                            .selectable_label(
                                system.info.shader_type == shader_type,
                                format!("{:?}", shader_type),
                            )
                            .clicked()
                        {
                            system.info.shader_type = shader_type;
                        }
                    }
                });
        });

        // Priority
        ui.horizontal(|ui| {
            ui.label("Priority:");
            egui::ComboBox::from_label("")
                .selected_text(format!("{:?}", system.info.priority))
                .show_ui(ui, |ui| {
                    for priority in [
                        ParticlePriorityType::Invalid,
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
                            .selectable_label(
                                system.info.priority == priority,
                                format!("{:?}", priority),
                            )
                            .clicked()
                        {
                            system.info.priority = priority;
                        }
                    }
                });
        });
    }

    fn show_physics_tab(&mut self, ui: &mut egui::Ui, system: &mut ParticleSystem) {
        ui.label("Physics Settings");

        self.show_validated_drag_value(
            ui,
            "Gravity",
            &mut system.info.gravity,
            -100.0,
            100.0,
            0.01,
        );

        ui.label("Drift Velocity:");
        ui.horizontal(|ui| {
            ui.label("X:");
            ui.add(egui::DragValue::new(&mut system.info.drift_velocity.x).speed(0.1));
            ui.label("Y:");
            ui.add(egui::DragValue::new(&mut system.info.drift_velocity.y).speed(0.1));
            ui.label("Z:");
            ui.add(egui::DragValue::new(&mut system.info.drift_velocity.z).speed(0.1));
        });

        self.show_random_variable(ui, "Velocity Damping", &mut system.info.vel_damping);
        self.show_random_variable(ui, "Angular Damping", &mut system.info.angular_damping);

        // Parameter validation feedback
        if system.info.gravity < -50.0 || system.info.gravity > 50.0 {
            ui.colored_label(
                egui::Color32::YELLOW,
                "⚠ High gravity values may cause instability",
            );
        }
    }

    fn show_keyframes_tab(&mut self, ui: &mut egui::Ui, system: &mut ParticleSystem) {
        ui.label("Keyframe Settings");

        ui.collapsing("Alpha Keyframes", |ui| {
            for i in 0..MAX_KEYFRAMES {
                ui.horizontal(|ui| {
                    ui.label(format!("Frame {}:", i));
                    ui.add(egui::DragValue::new(&mut system.info.alpha_key[i].frame).speed(1));
                    ui.label("Value:");
                    ui.add(egui::DragValue::new(&mut system.info.alpha_key[i].var.low).speed(0.01));
                    ui.label("-");
                    ui.add(
                        egui::DragValue::new(&mut system.info.alpha_key[i].var.high).speed(0.01),
                    );
                });
            }
        });

        ui.collapsing("Color Keyframes", |ui| {
            for i in 0..MAX_KEYFRAMES {
                ui.horizontal(|ui| {
                    ui.label(format!("Frame {}:", i));
                    ui.add(egui::DragValue::new(&mut system.info.color_key[i].frame).speed(1));
                    ui.label("R:");
                    ui.add(
                        egui::DragValue::new(&mut system.info.color_key[i].color.red).speed(0.01),
                    );
                    ui.label("G:");
                    ui.add(
                        egui::DragValue::new(&mut system.info.color_key[i].color.green).speed(0.01),
                    );
                    ui.label("B:");
                    ui.add(
                        egui::DragValue::new(&mut system.info.color_key[i].color.blue).speed(0.01),
                    );
                });
            }
        });
    }

    fn show_random_variable(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        var: &mut GameClientRandomVariable,
    ) {
        ui.horizontal(|ui| {
            ui.label(label);
            let low_response = ui.add(egui::DragValue::new(&mut var.low).speed(0.1));
            ui.label("-");
            let high_response = ui.add(egui::DragValue::new(&mut var.high).speed(0.1));

            // Ensure low <= high
            if var.low > var.high {
                if low_response.changed() {
                    var.high = var.low;
                } else if high_response.changed() {
                    var.low = var.high;
                }
            }
        });
    }

    fn show_validated_drag_value(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        value: &mut f32,
        min: f32,
        max: f32,
        speed: f32,
    ) {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.add(
                egui::DragValue::new(value)
                    .speed(speed)
                    .clamp_range(min..=max),
            );
        });
    }

    fn show_parameter_group(
        &self,
        ui: &mut egui::Ui,
        title: &str,
        add_contents: impl FnOnce(&mut egui::Ui),
    ) {
        ui.collapsing(title, add_contents);
    }

    fn show_preview_panel(&mut self, ui: &mut egui::Ui, system: &Option<ParticleSystem>) {
        ui.group(|ui| {
            ui.heading("Particle Preview");

            if let Some(system) = system {
                ui.label(format!("System: {}", system.info.name));
                ui.label(format!("Particles: {}", system.particles.len()));
                ui.label(format!("Active: {}", system.is_active));

                // Placeholder for actual 3D preview
                ui.allocate_space(egui::vec2(400.0, 300.0));
                ui.label("(3D Preview would go here)");
            } else {
                ui.label("No system to preview");
            }
        });
    }

    fn show_timeline_panel(&mut self, ui: &mut egui::Ui, system: &Option<ParticleSystem>) {
        ui.group(|ui| {
            ui.heading("Timeline");

            if let Some(_system) = system {
                ui.horizontal(|ui| {
                    if ui.button("⏮").clicked() {
                        // Rewind
                    }
                    if ui.button("▶").clicked() {
                        // Play/Pause
                    }
                    if ui.button("⏸").clicked() {
                        // Stop
                    }
                    if ui.button("⏭").clicked() {
                        // Fast forward
                    }
                });

                // Placeholder timeline
                ui.allocate_space(egui::vec2(400.0, 100.0));
                ui.label("(Timeline controls would go here)");
            } else {
                ui.label("No system loaded");
            }
        });
    }
}

impl Default for ParticleEditorUI {
    fn default() -> Self {
        Self {
            show_preview: true,
            show_timeline: true,
            show_properties: true,
            show_emission_panel: true,
            show_velocity_panel: true,
            show_particle_panel: true,
            show_shader_panel: true,
            selected_property_tab: PropertyTab::Emission,
            parameter_changed: false,
        }
    }
}
