// C++ InGameUI::placeBuildAvailable / handleBuildPlacements / destroyPlacementIcons.
// Included by ingame_ui/mod.rs.

use crate::drawable::drawable::{DrawableStatus, DrawableType, Vector3};
use crate::drawable::drawable_manager::with_drawable_manager;
use crate::drawable::DrawableId;
use crate::message_stream::place_event_confirm::{
    is_line_build_template_name, placement_angle_from_world_drag,
};

const MAX_LINE_BUILD_PLACE_ICONS: usize = 50;


impl InGameUI {
    /// C++ `InGameUI::placeBuildAvailable` — spawn a translucent 3D place icon.
    pub fn place_build_available_icons(
        &mut self,
        template_name: Option<String>,
        source_object_id: Option<u32>,
    ) {
        if template_name.is_some() && self.pending_place_icon_template.is_some() {
            self.destroy_placement_icons();
        }

        self.destroy_placement_icons();
        self.pending_place_icon_template = template_name.clone();
        self.pending_place_icon_source = source_object_id.unwrap_or(0);

        let Some(name) = template_name else {
            self.mouse_mode = MouseMode::Default;
            return;
        };

        self.set_radius_cursor(
            RadiusCursorType::None,
            Coord3D::new(0.0, 0.0, 0.0),
            0.0,
        );
        self.mouse_mode = MouseMode::BuildPlace;

        let angle = TheThingFactory::find_template(&name)
            .map(|t| t.get_placement_view_angle())
            .unwrap_or(0.0);
        if let Some(preview) = self.placement_preview.as_mut() {
            preview.rotation = angle;
        }
        if let Some(icon) = self.spawn_place_icon(&name, Coord3D::new(0.0, 0.0, 0.0), angle) {
            self.place_icons.push(icon);
        }
        TheInGameUI::set_placement_angle(angle);
    }

    pub fn destroy_placement_icons(&mut self) {
        // C++ InGameUI.cpp:2933-2948: removeFactionBibDrawable then destroyDrawable,
        // then removeAllBibs.
        if let Ok(mut guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(visual) = guard.as_mut() {
                for icon in &self.place_icons {
                    visual.remove_faction_bib(
                        icon.drawable_id,
                        crate::terrain::terrain_visual::TerrainBibOwnerKind::Drawable,
                    );
                }
                visual.remove_all_bibs();
            }
        }
        for icon in self.place_icons.drain(..) {
            with_drawable_manager(|manager| {
                manager.destroy_drawable(DrawableId(icon.drawable_id));
            });
        }
        self.pending_place_icon_template = None;
        self.pending_place_icon_source = 0;
    }

    fn spawn_place_icon(
        &self,
        template_name: &str,
        pos: Coord3D,
        angle: f32,
    ) -> Option<PlaceIconGhost> {
        let id = with_drawable_manager(|manager| {
            let drawable_id = manager.create_drawable(DrawableType::Model {
                model_name: template_name.to_string(),
                position: Vector3::new(pos.x, pos.y, pos.z),
                scale: 1.0,
                animation_state: String::new(),
            });
            if let Some(drawable) = manager.get_drawable_mut(drawable_id) {
                let mut status = drawable.get_status();
                status.set(DrawableStatus::NO_STATE_PARTICLES);
                drawable.set_status(status);
                drawable.set_opacity(PLACEMENT_OPACITY);
                drawable.set_position(Vector3::new(pos.x, pos.y, pos.z));
            }
            drawable_id.0
        });
        Some(PlaceIconGhost {
            drawable_id: id,
            position: pos,
            angle,
            illegal: false,
        })
    }

    fn update_place_icon_drawable(icon: &PlaceIconGhost) {
        with_drawable_manager(|manager| {
            if let Some(drawable) = manager.get_drawable_mut(DrawableId(icon.drawable_id)) {
                drawable.set_position(Vector3::new(
                    icon.position.x,
                    icon.position.y,
                    icon.position.z,
                ));
                drawable.set_opacity(PLACEMENT_OPACITY);
                if icon.illegal {
                    drawable.set_tint_color(Vector3::new(
                        ILLEGAL_BUILD_COLOR[0],
                        ILLEGAL_BUILD_COLOR[1],
                        ILLEGAL_BUILD_COLOR[2],
                    ));
                } else {
                    drawable.set_tint_color(Vector3::new(1.0, 1.0, 1.0));
                }
            }
        });
    }

    /// C++ `InGameUI::handleBuildPlacements` — follow mouse, drag-aim, tile LINEBUILD.
    pub fn handle_build_placements(&mut self) {
        let Some(template_name) = self.pending_place_icon_template.clone().or_else(|| {
            self.placement_preview
                .as_ref()
                .map(|p| p.template_name.clone())
        }) else {
            return;
        };

        let mouse = with_mouse(|mouse| mouse.state().position());
        let loc = if TheInGameUI::is_placement_anchored() {
            TheInGameUI::get_placement_points()
                .map(|(start, _)| IPoint2::new(start.x, start.y))
                .unwrap_or(IPoint2::new(mouse.0 as i32, mouse.1 as i32))
        } else {
            IPoint2::new(mouse.0 as i32, mouse.1 as i32)
        };

        let Some(world) = with_tactical_view_ref(|view| {
            view.screen_to_terrain(&loc)
                .ok()
                .map(|p| Coord3D::new(p.x, p.y, p.z))
        }) else {
            return;
        };

        let mut angle = self
            .place_icons
            .first()
            .map(|icon| icon.angle)
            .or_else(|| self.placement_preview.as_ref().map(|p| p.rotation))
            .unwrap_or(TheInGameUI::get_placement_angle());

        if TheInGameUI::is_placement_anchored() {
            if let Some((start, end)) = TheInGameUI::get_placement_points() {
                if start.x != end.x || start.y != end.y {
                    let start_world = with_tactical_view_ref(|view| {
                        view.screen_to_terrain(&IPoint2::new(start.x, start.y))
                            .ok()
                            .map(|p| Coord3D::new(p.x, p.y, p.z))
                    });
                    let end_world = with_tactical_view_ref(|view| {
                        view.screen_to_terrain(&IPoint2::new(end.x, end.y))
                            .ok()
                            .map(|p| Coord3D::new(p.x, p.y, p.z))
                    });
                    if let (Some(s), Some(e)) = (start_world, end_world) {
                        if let Some(drag_angle) = placement_angle_from_world_drag(
                            &MsgCoord3D::new(s.x, s.y, s.z),
                            &MsgCoord3D::new(e.x, e.y, e.z),
                        ) {
                            angle = drag_angle;
                            TheInGameUI::set_placement_angle(angle);
                        }
                    }
                }
            }
        }

        let legal = self.can_place_at(&world);
        if let Some(preview) = self.placement_preview.as_mut() {
            preview.position = glam::Vec3::new(world.x, world.y, world.z);
            preview.rotation = angle;
            preview.is_legal = legal;
        }

        if self.place_icons.is_empty() {
            if let Some(icon) = self.spawn_place_icon(&template_name, world.clone(), angle) {
                self.place_icons.push(icon);
            }
        }
        if let Some(icon) = self.place_icons.first_mut() {
            icon.position = world.clone();
            icon.angle = angle;
            icon.illegal = self
                .placement_preview
                .as_ref()
                .map(|p| !p.is_legal)
                .unwrap_or(false);
        }
        if let Some(icon) = self.place_icons.first().cloned() {
            Self::update_place_icon_drawable(&icon);
            self.sync_place_icon_faction_bibs(&icon);
        }

        if TheInGameUI::is_placement_anchored() && is_line_build_template_name(&template_name) {
            self.tile_line_build_icons(&template_name, angle);
        }
    }

    fn tile_line_build_icons(&mut self, template_name: &str, angle: f32) {
        let Some((start, end)) = TheInGameUI::get_placement_points() else {
            return;
        };
        let Some(world_start) = with_tactical_view_ref(|view| {
            view.screen_to_terrain(&IPoint2::new(start.x, start.y))
                .ok()
                .map(|p| Coord3D::new(p.x, p.y, p.z))
        }) else {
            return;
        };
        let Some(world_end) = with_tactical_view_ref(|view| {
            view.screen_to_terrain(&IPoint2::new(end.x, end.y))
                .ok()
                .map(|p| Coord3D::new(p.x, p.y, p.z))
        }) else {
            return;
        };

        // C++ BuildAssistant::buildTiledLocations — majorRadius * 2.0, max 50.
        let object_size = TheThingFactory::find_template(template_name)
            .map(|t| t.get_template_geometry_info().get_major_radius() * 2.0)
            .filter(|s| *s > 1.0)
            .unwrap_or(20.0);
        let dx = world_end.x - world_start.x;
        let dy = world_end.y - world_start.y;
        let length = (dx * dx + dy * dy).sqrt();
        let tiles_needed = if length < 1.0 {
            1usize
        } else {
            ((length / object_size).floor() as usize + 1).clamp(1, MAX_LINE_BUILD_PLACE_ICONS)
        };
        let (dir_x, dir_y) = if length < 1.0 {
            (0.0, 0.0)
        } else {
            (dx / length, dy / length)
        };

        // First tile is always the start; later tiles stop at the first illegal.
        let mut tiles = 1usize;
        let mut positions = vec![world_start.clone()];
        for i in 1..tiles_needed {
            let pos = Coord3D::new(
                world_start.x + dir_x * object_size * i as f32,
                world_start.y + dir_y * object_size * i as f32,
                world_start.z,
            );
            if !self.can_place_at(&pos) {
                break;
            }
            positions.push(pos);
            tiles += 1;
        }

        while self.place_icons.len() < tiles {
            if let Some(icon) = self.spawn_place_icon(template_name, world_start.clone(), angle) {
                self.place_icons.push(icon);
            } else {
                break;
            }
        }
        while self.place_icons.len() > tiles {
            if let Some(icon) = self.place_icons.pop() {
                with_drawable_manager(|manager| {
                    manager.destroy_drawable(DrawableId(icon.drawable_id));
                });
            }
        }

        for (i, icon) in self.place_icons.iter_mut().enumerate() {
            let pos = positions.get(i).cloned().unwrap_or_else(|| world_start.clone());
            icon.position = pos;
            icon.angle = angle;
            // Extra tiles past the first inherit legality from can_place_at.
            icon.illegal = if i == 0 {
                self.placement_preview
                    .as_ref()
                    .map(|p| !p.is_legal)
                    .unwrap_or(false)
            } else {
                false
            };
        }
        for icon in &self.place_icons {
            Self::update_place_icon_drawable(icon);
        }
    }

    /// C++ InGameUI.cpp:1473-1479 addFactionBibDrawable when LBC != OK.
    fn sync_place_icon_faction_bibs(&self, icon: &PlaceIconGhost) {
        let Ok(mut guard) = crate::terrain::terrain_visual::get_terrain_visual() else {
            return;
        };
        let Some(visual) = guard.as_mut() else {
            return;
        };
        if icon.illegal {
            let transform = glam::Mat4::from_translation(glam::Vec3::new(
                icon.position.x,
                icon.position.y,
                icon.position.z,
            )) * glam::Mat4::from_rotation_z(icon.angle);
            let (major, minor) = self
                .placement_preview
                .as_ref()
                .map(|p| (p.footprint.x.max(1.0), p.footprint.y.max(1.0)))
                .unwrap_or((30.0, 30.0));
            visual.add_faction_bib(
                icon.drawable_id,
                crate::terrain::terrain_visual::TerrainBibOwnerKind::Drawable,
                transform,
                major,
                minor,
                true,
                0.0,
                0.0,
                true,
                0.0,
            );
        } else {
            visual.remove_faction_bib(
                icon.drawable_id,
                crate::terrain::terrain_visual::TerrainBibOwnerKind::Drawable,
            );
        }
    }

}
