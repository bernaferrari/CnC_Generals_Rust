//! Object management system for World Builder

use crate::map::{Map, MapObject};
use anyhow::Result;
use glam::{Quat, Vec3};
use std::collections::HashMap;
use uuid::Uuid;

/// Manages objects in the map
pub struct ObjectManager {
    object_templates: HashMap<String, ObjectTemplate>,
    selected_objects: Vec<Uuid>,
    clipboard: Vec<MapObject>,
    gizmo_mode: GizmoMode,
    snap_to_grid: bool,
    grid_size: f32,
    dirty: bool,
}

impl ObjectManager {
    pub fn new() -> Self {
        Self {
            object_templates: HashMap::new(),
            selected_objects: Vec::new(),
            clipboard: Vec::new(),
            gizmo_mode: GizmoMode::Translate,
            snap_to_grid: true,
            grid_size: 1.0,
            dirty: false,
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.load_object_templates()?;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        // Update any animated objects or systems
        Ok(())
    }

    /// Load object definitions from templates
    fn load_object_templates(&mut self) -> Result<()> {
        // Load default object templates
        self.object_templates.insert(
            "infantry_barracks".to_string(),
            ObjectTemplate {
                id: "infantry_barracks".to_string(),
                name: "Infantry Barracks".to_string(),
                category: "Buildings".to_string(),
                model_path: "models/buildings/barracks.w3d".into(),
                icon_path: Some("icons/barracks.png".into()),
                default_scale: Vec3::ONE,
                snap_points: vec![
                    Vec3::new(0.0, 0.0, 10.0), // Front door
                ],
                properties: vec![
                    ObjectProperty {
                        name: "Player".to_string(),
                        property_type: PropertyType::Integer {
                            min: 0,
                            max: 7,
                            default: 0,
                        },
                        description: "Owner player".to_string(),
                    },
                    ObjectProperty {
                        name: "Initial Health".to_string(),
                        property_type: PropertyType::Float {
                            min: 0.1,
                            max: 1.0,
                            default: 1.0,
                        },
                        description: "Initial health percentage".to_string(),
                    },
                ],
            },
        );

        // Add more templates...
        Ok(())
    }

    /// Place a new object at the specified position
    pub fn place_object(
        &mut self,
        template_id: &str,
        position: Vec3,
        map: &mut Map,
    ) -> Result<Uuid> {
        if let Some(template) = self.object_templates.get(template_id) {
            let adjusted_position = if self.snap_to_grid {
                self.snap_position_to_grid(position)
            } else {
                position
            };

            let object = MapObject::new(
                template.name.clone(),
                template.id.clone(),
                adjusted_position,
            );

            let object_id = object.id;
            map.add_object(object);
            self.dirty = true;

            log::info!(
                "Placed object '{}' at {:?}",
                template.name,
                adjusted_position
            );
            Ok(object_id)
        } else {
            Err(anyhow::anyhow!("Unknown object template: {}", template_id))
        }
    }

    /// Delete selected objects
    pub fn delete_selected(&mut self, map: &mut Map) -> Result<()> {
        for object_id in &self.selected_objects {
            map.remove_object(*object_id);
        }

        let count = self.selected_objects.len();
        self.selected_objects.clear();
        self.dirty = true;

        log::info!("Deleted {} objects", count);
        Ok(())
    }

    /// Copy selected objects to clipboard
    pub fn copy_selected(&mut self, map: &Map) -> Result<()> {
        self.clipboard.clear();

        for object_id in &self.selected_objects {
            if let Some(object) = map.find_object(*object_id) {
                self.clipboard.push(object.clone());
            }
        }

        log::info!("Copied {} objects to clipboard", self.clipboard.len());
        Ok(())
    }

    /// Paste objects from clipboard
    pub fn paste(&mut self, position: Vec3, map: &mut Map) -> Result<()> {
        if self.clipboard.is_empty() {
            return Ok(());
        }

        // Calculate offset from first object
        let first_pos = self.clipboard[0].position;
        let offset = position - first_pos;

        self.selected_objects.clear();

        for clipboard_object in &self.clipboard {
            let mut new_object = clipboard_object.clone();
            new_object.id = Uuid::new_v4();
            new_object.position += offset;

            if self.snap_to_grid {
                new_object.position = self.snap_position_to_grid(new_object.position);
            }

            self.selected_objects.push(new_object.id);
            map.add_object(new_object);
        }

        self.dirty = true;
        log::info!("Pasted {} objects", self.clipboard.len());
        Ok(())
    }

    /// Select object at position
    pub fn select_object_at(
        &mut self,
        position: Vec3,
        map: &Map,
        add_to_selection: bool,
    ) -> Option<Uuid> {
        // Find closest object to position
        let mut closest_object = None;
        let mut closest_distance = f32::INFINITY;

        for object in map.objects() {
            let distance = object.position.distance(position);
            if distance < closest_distance && distance < 5.0 {
                // 5 unit selection threshold
                closest_distance = distance;
                closest_object = Some(object.id);
            }
        }

        if let Some(object_id) = closest_object {
            if add_to_selection {
                if !self.selected_objects.contains(&object_id) {
                    self.selected_objects.push(object_id);
                }
            } else {
                self.selected_objects.clear();
                self.selected_objects.push(object_id);
            }

            Some(object_id)
        } else {
            if !add_to_selection {
                self.selected_objects.clear();
            }
            None
        }
    }

    /// Move selected objects
    pub fn move_selected(&mut self, delta: Vec3, map: &mut Map) -> Result<()> {
        for object_id in &self.selected_objects {
            if let Some(object) = map.find_object_mut(*object_id) {
                object.position += delta;

                if self.snap_to_grid {
                    object.position = self.snap_position_to_grid(object.position);
                }
            }
        }

        if !self.selected_objects.is_empty() {
            self.dirty = true;
        }

        Ok(())
    }

    /// Rotate selected objects
    pub fn rotate_selected(&mut self, rotation: Quat, map: &mut Map) -> Result<()> {
        for object_id in &self.selected_objects {
            if let Some(object) = map.find_object_mut(*object_id) {
                object.rotation = rotation * object.rotation;
            }
        }

        if !self.selected_objects.is_empty() {
            self.dirty = true;
        }

        Ok(())
    }

    /// Scale selected objects
    pub fn scale_selected(&mut self, scale: Vec3, map: &mut Map) -> Result<()> {
        for object_id in &self.selected_objects {
            if let Some(object) = map.find_object_mut(*object_id) {
                object.scale = object.scale * scale;
            }
        }

        if !self.selected_objects.is_empty() {
            self.dirty = true;
        }

        Ok(())
    }

    /// Snap position to grid
    fn snap_position_to_grid(&self, position: Vec3) -> Vec3 {
        Vec3::new(
            (position.x / self.grid_size).round() * self.grid_size,
            position.y, // Don't snap Y to allow free height placement
            (position.z / self.grid_size).round() * self.grid_size,
        )
    }

    /// Load objects from map
    pub fn load_objects(&mut self, map: &Map) -> Result<()> {
        self.selected_objects.clear();
        self.dirty = false;
        Ok(())
    }

    /// Save object changes to map
    pub fn save_to_map(&self, map: &mut Map) -> Result<()> {
        // Object changes are applied directly to the map
        Ok(())
    }

    /// Check if there are unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    /// Clear all selections and clipboard
    pub fn clear(&mut self) {
        self.selected_objects.clear();
        self.clipboard.clear();
        self.dirty = false;
    }

    /// Get object templates by category
    pub fn get_templates_by_category(&self, category: &str) -> Vec<&ObjectTemplate> {
        self.object_templates
            .values()
            .filter(|template| template.category == category)
            .collect()
    }

    /// Get all categories
    pub fn get_categories(&self) -> Vec<String> {
        let mut categories: Vec<String> = self
            .object_templates
            .values()
            .map(|template| template.category.clone())
            .collect();
        categories.sort();
        categories.dedup();
        categories
    }

    // Getters and setters
    pub fn selected_objects(&self) -> &[Uuid] {
        &self.selected_objects
    }
    pub fn gizmo_mode(&self) -> GizmoMode {
        self.gizmo_mode
    }
    pub fn gizmo_mode_mut(&mut self) -> &mut GizmoMode {
        &mut self.gizmo_mode
    }
    pub fn set_gizmo_mode(&mut self, mode: GizmoMode) {
        self.gizmo_mode = mode;
    }
    pub fn snap_to_grid(&self) -> bool {
        self.snap_to_grid
    }
    pub fn snap_to_grid_mut(&mut self) -> &mut bool {
        &mut self.snap_to_grid
    }
    pub fn set_snap_to_grid(&mut self, enabled: bool) {
        self.snap_to_grid = enabled;
    }
    pub fn grid_size(&self) -> f32 {
        self.grid_size
    }
    pub fn grid_size_mut(&mut self) -> &mut f32 {
        &mut self.grid_size
    }
    pub fn set_grid_size(&mut self, size: f32) {
        self.grid_size = size;
    }
}

/// Object template definition
#[derive(Debug, Clone)]
pub struct ObjectTemplate {
    pub id: String,
    pub name: String,
    pub category: String,
    pub model_path: std::path::PathBuf,
    pub icon_path: Option<std::path::PathBuf>,
    pub default_scale: Vec3,
    pub snap_points: Vec<Vec3>,
    pub properties: Vec<ObjectProperty>,
}

/// Object property definition
#[derive(Debug, Clone)]
pub struct ObjectProperty {
    pub name: String,
    pub property_type: PropertyType,
    pub description: String,
}

/// Property types for objects
#[derive(Debug, Clone)]
pub enum PropertyType {
    String {
        default: String,
    },
    Integer {
        min: i32,
        max: i32,
        default: i32,
    },
    Float {
        min: f32,
        max: f32,
        default: f32,
    },
    Boolean {
        default: bool,
    },
    Enum {
        options: Vec<String>,
        default: usize,
    },
    Vector3 {
        default: Vec3,
    },
    Color {
        default: [f32; 4],
    },
}

/// Gizmo modes for object manipulation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

impl GizmoMode {
    pub fn name(&self) -> &'static str {
        match self {
            GizmoMode::Translate => "Move",
            GizmoMode::Rotate => "Rotate",
            GizmoMode::Scale => "Scale",
        }
    }
}

/// Object selection utilities
pub struct SelectionUtils;

impl SelectionUtils {
    /// Select objects within a rectangular area
    pub fn select_in_rect(manager: &mut ObjectManager, map: &Map, min: Vec3, max: Vec3) {
        manager.selected_objects.clear();

        for object in map.objects() {
            if object.position.x >= min.x
                && object.position.x <= max.x
                && object.position.z >= min.z
                && object.position.z <= max.z
            {
                manager.selected_objects.push(object.id);
            }
        }
    }

    /// Calculate bounding box of selected objects
    pub fn get_selection_bounds(manager: &ObjectManager, map: &Map) -> Option<(Vec3, Vec3)> {
        if manager.selected_objects.is_empty() {
            return None;
        }

        let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

        for object_id in &manager.selected_objects {
            if let Some(object) = map.find_object(*object_id) {
                min = min.min(object.position);
                max = max.max(object.position);
            }
        }

        Some((min, max))
    }

    /// Get center point of selected objects
    pub fn get_selection_center(manager: &ObjectManager, map: &Map) -> Option<Vec3> {
        if manager.selected_objects.is_empty() {
            return None;
        }

        let mut center = Vec3::ZERO;
        let mut count = 0;

        for object_id in &manager.selected_objects {
            if let Some(object) = map.find_object(*object_id) {
                center += object.position;
                count += 1;
            }
        }

        if count > 0 {
            Some(center / count as f32)
        } else {
            None
        }
    }
}
