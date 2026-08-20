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
        self.sync_shadow_render_flags();
        if self.terrain_decal != TerrainDecalType::None {
            self.apply_terrain_decal(self.terrain_decal);
        }
    }

    fn apply_shadows_enabled(&mut self, enable: bool) {
        // C++ setShadowsEnabled: enableShadowRender(enable); m_shadowEnabled = enable.
        self.shadow_enabled = enable;
        self.sync_shadow_render_flags();
        if self.terrain_decal != TerrainDecalType::None {
            self.apply_terrain_decal(self.terrain_decal);
        }
    }

    fn allocate_template_shadow(&mut self) {
        // C++ allocateShadows: TheW3DShadowManager->addShadow from ThingTemplate ShadowType.
        // GameLogic talks to the projected-shadow manager through TerrainDecalClient.
        if self.shadow_allocated {
            return;
        }
        let Some(owner_id) = self.owner_id else {
            return;
        };
        if self.terrain_decal != TerrainDecalType::None {
            // A live horde/crate decal already occupies the client handle.
            self.sync_shadow_render_flags();
            self.shadow_allocated = true;
            return;
        }
        self.apply_terrain_decal(TerrainDecalType::ShadowTexture);
        if terrain_decal_client().is_some() {
            self.shadow_allocated = true;
        }
        let _ = owner_id;
        self.sync_shadow_render_flags();
    }

    fn release_template_shadow(&mut self) {
        // C++ releaseShadows: m_shadow->release(); m_shadow = NULL.
        if !self.shadow_allocated {
            self.sync_shadow_render_flags();
            return;
        }
        if self.terrain_decal == TerrainDecalType::None
            || self.terrain_decal == TerrainDecalType::ShadowTexture
        {
            if let Some(owner_id) = self.owner_id {
                if let Some(client) = terrain_decal_client() {
                    client.release(owner_id);
                }
            }
            self.terrain_decal = TerrainDecalType::None;
        } else {
            self.sync_shadow_render_flags();
        }
        self.shadow_allocated = false;
    }
}
