/// C++ `enableShadowRender(!hidden)` combined with Options `m_shadowEnabled`.
fn shadow_should_render(hidden: bool, shadow_enabled: bool) -> bool {
    !hidden && shadow_enabled
}

impl W3DModelDraw {
    fn sync_shadow_render_flags(&self) {
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(client) = terrain_decal_client() else {
            return;
        };
        client.set_shadow_enabled(
            owner_id,
            shadow_should_render(self.hidden, self.shadow_enabled),
        );
    }

    fn apply_hidden_shadow_and_decal(&mut self, hidden: bool) {
        // C++ setHidden: enableShadowRender(!hidden) on m_shadow and m_terrainDecal,
        // cap tracks, stop particles. Keep Options `shadow_enabled` independent of hide.
        if self.hidden != hidden {
            self.hidden = hidden;
            self.do_start_or_stop_particle_sys();
            if hidden {
                self.cap_terrain_track();
            }
        }
        if self.terrain_decal != TerrainDecalType::None {
            self.apply_terrain_decal(self.terrain_decal);
        }
        // Decal render is !hidden. Blob render is !hidden && options, and shroud stays.
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(client) = terrain_decal_client() else {
            return;
        };
        client.set_shadow_enabled(owner_id, !self.hidden);
        client.set_blob_render(owner_id, !self.hidden && self.shadow_enabled);
    }

    fn apply_shadows_enabled(&mut self, enable: bool) {
        // C++ setShadowsEnabled only toggles m_shadow, including while a ring exists.
        self.shadow_enabled = enable;
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(client) = terrain_decal_client() else {
            return;
        };
        client.set_blob_render(owner_id, enable && !self.hidden);
    }

    fn allocate_template_shadow(&mut self) {
        // C++ allocateShadows addShadow even when m_terrainDecal is a ring.
        if self.shadow_allocated {
            return;
        }
        let Some(owner_id) = self.owner_id else {
            return;
        };
        if !game_engine::common::game_lod::use_shadow_decals() {
            return;
        }
        let Some(client) = terrain_decal_client() else {
            return;
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return;
        };
        let Ok(obj) = object.read() else {
            return;
        };
        let tmpl = obj.get_template();
        let shadow_type = tmpl.as_ref().get_shadow_type_bits();
        if shadow_type == 0 {
            return;
        }
        let position = *obj.get_position();
        let mut texture_name = tmpl.as_ref().get_shadow_texture_name().to_string();
        // C++ `addShadow`: an empty `m_ShadowName` on `SHADOW_PROJECTION` uses
        // `robj->Get_Name()`. Decal shadows stay empty so the client can
        // substitute `shadow.tga`.
        const SHADOW_PROJECTION: u32 = 0x0000_0004;
        if shadow_type == SHADOW_PROJECTION && texture_name.is_empty() {
            if let Some(state) = self.current_state() {
                texture_name = state.model_name.as_str().to_string();
            }
        }
        client.add_unit_shadow(&TerrainDecalDesc {
            object_id: owner_id,
            texture_name,
            size_x: tmpl.as_ref().get_shadow_size_x(),
            size_y: tmpl.as_ref().get_shadow_size_y(),
            opacity: 1.0,
            offset_x: tmpl.as_ref().get_shadow_offset_x(),
            offset_y: tmpl.as_ref().get_shadow_offset_y(),
            position,
            angle: obj.get_orientation(),
            hidden: self.hidden,
            shrouded: self.fully_obscured_by_shroud,
            shadow_enabled: self.shadow_enabled,
            is_unit_blob: true,
            shadow_type,
        });
        self.shadow_allocated = true;
    }

    fn release_template_shadow(&mut self) {
        if let Some(owner_id) = self.owner_id {
            if let Some(client) = terrain_decal_client() {
                client.release_unit_shadow(owner_id);
            }
        }
        self.shadow_allocated = false;
    }
}
