// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

impl TerrainVisualImpl {
    pub fn oversize_terrain(&mut self, amount: i32) {
        let Some((map_width, map_height)) = self.map_sample_dimensions() else {
            return;
        };
        if map_width <= 0 || map_height <= 0 {
            return;
        }

        let mut width = NORMAL_DRAW_WIDTH;
        let mut height = NORMAL_DRAW_HEIGHT;

        if amount > 0 && amount < MAX_OVERSIZE_TILES {
            width += OVERSIZE_TILES_STEP * amount;
            height += OVERSIZE_TILES_STEP * amount;
            width = width.min(map_width).max(1);
            height = height.min(map_height).max(1);
        }

        let dx = width - self.draw_width;
        let dy = height - self.draw_height;

        self.draw_width = width;
        self.draw_height = height;

        let origin_dx = dx / 2;
        let origin_dy = dy / 2;

        self.draw_origin_x -= origin_dx;
        self.draw_origin_y -= origin_dy;

        if self.draw_origin_x < 0 {
            self.draw_origin_x = 0;
        }
        if self.draw_origin_y < 0 {
            self.draw_origin_y = 0;
        }

        // Keep draw area state consistent with map bounds.
        if self.draw_width > map_width {
            self.draw_width = map_width;
        }
        if self.draw_height > map_height {
            self.draw_height = map_height;
        }

        self.oversize_amount = amount;

        // Full rebuild behavior.
        self.chunk_meshes.clear();
        self.chunk_texture_bindings.clear();
        self.road_meshes.clear();
        self.scorch_meshes.clear();
        self.overlay_gpu_meshes_dirty = true;

        self.stats.rendered_chunks = 0;
        self.stats.triangles_rendered = 0;
        self.stats.update_time_ms = 0.0;
    }

    pub fn set_lighting(
        &mut self,
        sun_dir: Option<[f32; 3]>,
        sun_color: Option<[f32; 3]>,
        ambient: Option<[f32; 3]>,
        fog_color: Option<[f32; 3]>,
        fog_range: Option<(f32, f32)>,
    ) {
        if let Some(d) = sun_dir {
            self.sun_direction = Vec3::from_array(d);
        }
        if let Some(c) = sun_color {
            self.sun_color = c;
        }
        if let Some(a) = ambient {
            self.ambient_color = a;
        }
        if let Some(f) = fog_color {
            self.fog_color = f;
        }
        if let Some((start, end)) = fog_range {
            self.fog_start = start;
            self.fog_end = end.max(start + 1.0);
        }
    }

    #[cfg(test)]
    pub(crate) fn fog_range_span(&self) -> (f32, f32) {
        (self.fog_start, self.fog_end)
    }

    /// C++ `doTheLight` vertex bake (`BaseHeightMap.cpp:491-566`): ambient
    /// comes from light 0 only; every global light then adds
    /// `clamp(N·L, 0, 1) * diffuse`. `lights` holds (Y-up-converted light
    /// position, diffuse) pairs — see `chunk::current_global_terrain_lights`.
    pub(crate) fn terrain_static_diffuse_from_normal(
        normal: Vec3,
        lights: &[(Vec3, [f32; 3])],
        ambient_color: [f32; 3],
    ) -> [f32; 4] {
        let normal = if normal.length_squared() > 1.0e-8 && normal.is_finite() {
            normal.normalize()
        } else {
            Vec3::Y
        };
        let mut red = ambient_color[0];
        let mut green = ambient_color[1];
        let mut blue = ambient_color[2];
        for (light_pos, sun_color) in lights {
            let light_ray = if light_pos.length_squared() > 1.0e-8 && light_pos.is_finite() {
                (-*light_pos).normalize()
            } else {
                Vec3::Y
            };
            let intensity = normal.dot(light_ray).max(0.0);
            red += sun_color[0] * intensity;
            green += sun_color[1] * intensity;
            blue += sun_color[2] * intensity;
        }
        [
            red.clamp(0.0, 1.0),
            green.clamp(0.0, 1.0),
            blue.clamp(0.0, 1.0),
            1.0,
        ]
    }

    fn ensure_terrain_definitions(&mut self, reference: Option<&Path>) -> TerrainResult<()> {
        let reference = reference.or_else(|| {
            if self.filename.is_empty() {
                None
            } else {
                Some(Path::new(&self.filename))
            }
        });

        let sources = Self::collect_terrain_ini_sources(reference);
        if sources.is_empty() {
            return Ok(());
        }

        if sources == self.loaded_terrain_sources {
            return Ok(());
        }

        debug!(
            "Loading terrain definitions from: {}",
            sources
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let archive_source_count = sources
            .iter()
            .filter(|p| Self::is_archive_extracted_terrain_source(p))
            .count();
        let count = match ini_terrain::load_terrain_definitions(&sources) {
            Ok(count) => count,
            Err(err) if archive_source_count > 0 => {
                // A corrupt archive copy must not blank the whole registry:
                // retry with the loose sources only (C++ would have fallen
                // back to whatever the local Data/INI tree provides).
                warn!(
                    "Terrain definition load failed with archive sources ({}); retrying loose-only: {}",
                    archive_source_count, err
                );
                let loose_sources: Vec<PathBuf> = sources
                    .iter()
                    .filter(|p| !Self::is_archive_extracted_terrain_source(p))
                    .cloned()
                    .collect();
                ini_terrain::load_terrain_definitions(&loose_sources)?
            }
            Err(err) => return Err(err.into()),
        };
        if count == 0 {
            warn!(
                "No terrain definitions were loaded from the resolved sources: {}",
                sources
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        self.loaded_terrain_sources = sources;

        let using_fallback_defaults = !self.texture_rules.is_empty()
            && self.texture_rules.iter().all(|rule| {
                self.texture_system
                    .get_texture(rule.texture_id)
                    .map(|texture| texture.diffuse_path.starts_with("Data/Terrain/"))
                    .unwrap_or(false)
            });

        if using_fallback_defaults {
            self.texture_rules.clear();
            self.chunk_texture_bindings.clear();
            self.chunk_meshes.clear();
            self.road_meshes.clear();
            self.scorch_meshes.clear();
            self.overlay_gpu_meshes_dirty = true;

            self.active_chunk_texture_ids = None;
            self.ensure_default_textures();
        }

        Ok(())
    }

    fn collect_terrain_ini_sources(reference: Option<&Path>) -> Vec<PathBuf> {
        let mut sources = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();

        let base_relatives = [
            "Data/INI/Default/Terrain.ini",
            "Data/INI/Default/Terrain.INI",
            "Data/INI/Terrain.ini",
            "Data/INI/Terrain.INI",
        ];

        if let Some(reference_path) = reference {
            Self::collect_from_ancestors(reference_path, &base_relatives, &mut sources, &mut seen);
        }

        if let Ok(cwd) = std::env::current_dir() {
            Self::collect_from_root(&cwd, &base_relatives, &mut sources, &mut seen);
            for ancestor in cwd.ancestors() {
                Self::collect_from_root(ancestor, &base_relatives, &mut sources, &mut seen);
            }
        }

        let fallback_paths = [
            "windows_game/Command & Conquer Generals Zero Hour/Data/INI/Default/Terrain.ini",
            "windows_game/Command & Conquer Generals Zero Hour/Data/INI/Terrain.ini",
            "windows_game/Command & Conquer Generals/Data/INI/Default/Terrain.ini",
            "windows_game/Command & Conquer Generals/Data/INI/Terrain.ini",
            "../windows_game/Command & Conquer Generals Zero Hour/Data/INI/Default/Terrain.ini",
            "../windows_game/Command & Conquer Generals Zero Hour/Data/INI/Terrain.ini",
            "../windows_game/Command & Conquer Generals/Data/INI/Default/Terrain.ini",
            "../windows_game/Command & Conquer Generals/Data/INI/Terrain.ini",
        ];

        for fallback in fallback_paths {
            Self::push_if_exists(&mut sources, &mut seen, PathBuf::from(fallback));
        }


        if let Some(reference_path) = reference {
            if let Some(map_dir) = reference_path.parent() {
                Self::collect_map_specific_sources(map_dir, &mut sources, &mut seen);
            }
        }

        Self::append_archive_terrain_ini_sources(&mut sources);

        sources
    }

    /// Retail installs carry Terrain.ini only inside `INIZH.big`
    /// (`Data\INI\Default\Terrain.ini` base plus the `Data\INI\Terrain.ini`
    /// override). When no loose source was discovered, extract the archive
    /// copy through the game file system into temp files so the existing
    /// parser can consume them; loose files always keep precedence.
    fn append_archive_terrain_ini_sources(sources: &mut Vec<PathBuf>) {
        let has_loose = |relative: &str| sources.iter().any(|p| p.ends_with(relative));

        let mut extracted_default = None;
        if !has_loose("Data/INI/Default/Terrain.ini") {
            extracted_default = Self::extract_archive_ini_to_temp(
                "Data/INI/Default/Terrain.ini",
                "generals_default_terrain.ini",
            );
        }
        let mut extracted_override = None;
        if !has_loose("Data/INI/Terrain.ini") {
            extracted_override =
                Self::extract_archive_ini_to_temp("Data/INI/Terrain.ini", "generals_terrain.ini");
        }

        // Base (Default) definitions must parse before the override so the
        // retail load order is preserved even when the two halves come from
        // different places.
        if let Some(path) = extracted_default {
            sources.insert(0, path);
        }
        if let Some(path) = extracted_override {
            sources.push(path);
        }
    }

    fn extract_archive_ini_to_temp(virtual_path: &str, temp_name: &str) -> Option<PathBuf> {
        let bytes = crate::terrain::tree_buffer::read_game_fs_bytes(virtual_path)?;
        let path = std::env::temp_dir().join(temp_name);
        std::fs::write(&path, bytes).ok()?;
        Some(path)
    }

    /// Temp-file marker for the archive-extracted sources appended by
    /// [`Self::append_archive_terrain_ini_sources`].
    fn is_archive_extracted_terrain_source(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "generals_terrain.ini" || name == "generals_default_terrain.ini")
    }

    fn collect_from_ancestors(
        reference: &Path,
        relatives: &[&str],
        sources: &mut Vec<PathBuf>,
        seen: &mut HashSet<PathBuf>,
    ) {
        let mut dirs = Vec::new();

        if reference.is_dir() {
            dirs.push(reference.to_path_buf());
        } else if let Some(parent) = reference.parent() {
            dirs.push(parent.to_path_buf());
        }

        let mut current = reference.parent();
        while let Some(dir) = current {
            dirs.push(dir.to_path_buf());
            current = dir.parent();
        }

        for dir in dirs {
            Self::collect_from_root(&dir, relatives, sources, seen);
        }
    }

    fn collect_from_root(
        root: &Path,
        relatives: &[&str],
        sources: &mut Vec<PathBuf>,
        seen: &mut HashSet<PathBuf>,
    ) {
        for rel in relatives {
            Self::push_if_exists(sources, seen, root.join(rel));
            Self::push_if_exists(sources, seen, root.join("INIZH").join(rel));
        }
    }

    fn collect_map_specific_sources(
        map_dir: &Path,
        sources: &mut Vec<PathBuf>,
        seen: &mut HashSet<PathBuf>,
    ) {
        let candidates = [
            "Terrain.ini",
            "terrain.ini",
            "Terrain.INI",
            "Data/INI/Terrain.ini",
            "Data/INI/Terrain.INI",
        ];

        for candidate in &candidates {
            Self::push_if_exists(sources, seen, map_dir.join(candidate));
        }
    }

    fn push_if_exists(sources: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, candidate: PathBuf) {
        if !candidate.exists() {
            return;
        }

        let canonical = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.clone());

        if seen.insert(canonical.clone()) {
            sources.push(canonical);
        }
    }

    fn ensure_default_textures(&mut self) {
        if !self.texture_rules.is_empty() {
            return;
        }

        let mut defaults = Vec::new();

        if let Some(registry) = ini_terrain::get_terrain_types() {
            let guard = registry.read();
            let desired_surfaces = [
                TerrainSurface::Grass,
                TerrainSurface::Rock,
                TerrainSurface::Snow,
                TerrainSurface::Sand,
            ];
            let mut used_textures = HashSet::new();

            for surface in desired_surfaces {
                let terrains = guard.get_terrains_by_surface(&surface);
                for terrain in terrains {
                    let texture = terrain.texture_name.as_str().trim();
                    if texture.is_empty() {
                        continue;
                    }
                    let Some(normalized_texture) = Self::normalize_terrain_texture_path(texture)
                    else {
                        continue;
                    };

                    let texture_key = normalized_texture.to_ascii_lowercase();
                    if !used_textures.insert(texture_key) {
                        continue;
                    }

                    if !TerrainTextures::is_available_terrain_texture_path(&normalized_texture) {
                        continue;
                    }

                    let texture_id = self.texture_system.register_texture(TerrainTexture::new(
                        0,
                        terrain.name.as_str().to_string(),
                        normalized_texture,
                    ));
                    defaults.push(texture_id);
                    break;
                }
            }
        }

        // If no INI terrain types resolve, leave rules empty (mirror C++ missing-terrain behavior).

        // Startup parity/perf: decode default terrain textures before first menu frame so
        // terrain does not appear to stream tile-by-tile after shell-map load.
        for texture_id in &defaults {
            if let Err(err) = self.texture_system.load_texture(*texture_id) {
                warn!(
                    "Failed to preload startup terrain texture {}: {}",
                    texture_id, err
                );
            }
        }

        self.build_rules_from_textures(&defaults);
    }

    fn build_rules_from_textures(&mut self, texture_ids: &[TextureId]) {
        let mut rules = Vec::new();

        for &texture_id in texture_ids {
            if rules
                .iter()
                .any(|rule: &TextureRule| rule.texture_id == texture_id)
            {
                continue;
            }

            if let Some(texture) = self.texture_system.get_texture(texture_id) {
                if let Some(terrain_type) = self.find_terrain_type_for_texture(texture) {
                    rules.push(Self::rule_from_terrain_type(texture_id, &terrain_type));
                } else {
                    rules.push(Self::derive_rule_for_texture(texture_id, &texture.name));
                }
            }
        }

        if rules.is_empty() {
            return;
        }

        rules.sort_by_key(|rule| rule.priority);
        if rules.len() > 4 {
            rules.truncate(4);
        }

        self.texture_rules = rules;
        self.chunk_texture_bindings.clear();
        self.chunk_meshes.clear();
        self.road_meshes.clear();
        self.scorch_meshes.clear();
        self.overlay_gpu_meshes_dirty = true;

        self.active_chunk_texture_ids = None;
    }

    fn derive_rule_for_texture(texture_id: TextureId, name: &str) -> TextureRule {
        Self::base_rule_for(texture_id, None, name)
    }

    fn base_rule_for(
        texture_id: TextureId,
        surface: Option<&TerrainSurface>,
        name: &str,
    ) -> TextureRule {
        let profile = if let Some(surface) = surface {
            match surface {
                TerrainSurface::Sand => (-500.0, 120.0, 0.0, 0.55, 5),
                TerrainSurface::Rock | TerrainSurface::Metal => {
                    (-500.0, 500.0, 0.6, std::f32::consts::FRAC_PI_2, 25)
                }
                TerrainSurface::Snow => (150.0, 500.0, 0.0, std::f32::consts::FRAC_PI_2, 30),
                TerrainSurface::Water => (-50.0, 20.0, 0.0, 0.4, 2),
                TerrainSurface::Dirt | TerrainSurface::Pavement | TerrainSurface::Concrete => {
                    (-500.0, 220.0, 0.0, 0.75, 12)
                }
                TerrainSurface::Wood => (-500.0, 220.0, 0.0, 0.7, 12),
                TerrainSurface::Grass => (-500.0, 200.0, 0.0, 0.7, 10),
                TerrainSurface::Custom(_) => Self::heuristic_profile(name),
            }
        } else {
            Self::heuristic_profile(name)
        };

        let (preferred_gradient, gradient_tolerance) = Self::gradient_profile(surface, name);

        TextureRule {
            texture_id,
            height_min: profile.0,
            height_max: profile.1,
            slope_min: profile.2,
            slope_max: profile.3,
            priority: profile.4,
            preferred_gradient,
            gradient_tolerance,
        }
    }

    fn heuristic_profile(name: &str) -> (f32, f32, f32, f32, u8) {
        let lower = name.to_lowercase();
        if lower.contains("sand") || lower.contains("desert") {
            (-500.0, 120.0, 0.0, 0.55, 5)
        } else if lower.contains("cliff") || lower.contains("rock") {
            (-500.0, 500.0, 0.6, std::f32::consts::FRAC_PI_2, 20)
        } else if lower.contains("snow") || lower.contains("ice") {
            (150.0, 500.0, 0.0, std::f32::consts::FRAC_PI_2, 30)
        } else if lower.contains("water") || lower.contains("sea") {
            (-50.0, 20.0, 0.0, 0.4, 2)
        } else if lower.contains("mud") || lower.contains("dirt") {
            (-500.0, 180.0, 0.0, 0.75, 15)
        } else {
            (-500.0, 200.0, 0.0, 0.7, 10)
        }
    }

    fn gradient_profile(surface: Option<&TerrainSurface>, name: &str) -> (f32, f32) {
        let from_surface = surface.and_then(|surface| match surface {
            TerrainSurface::Sand => Some((0.15, 0.3)),
            TerrainSurface::Rock | TerrainSurface::Metal => Some((0.8, 0.25)),
            TerrainSurface::Snow => Some((0.35, 0.35)),
            TerrainSurface::Water => Some((0.05, 0.2)),
            TerrainSurface::Dirt => Some((0.2, 0.35)),
            TerrainSurface::Pavement | TerrainSurface::Concrete | TerrainSurface::Wood => {
                Some((0.1, 0.3))
            }
            TerrainSurface::Grass => Some((0.18, 0.35)),
            TerrainSurface::Custom(_) => None,
        });

        if let Some(profile) = from_surface {
            return profile;
        }

        let lower = name.to_lowercase();
        if lower.contains("cliff") || lower.contains("rock") || lower.contains("ridge") {
            (0.8, 0.25)
        } else if lower.contains("sand") || lower.contains("dune") {
            (0.12, 0.3)
        } else if lower.contains("snow") || lower.contains("ice") {
            (0.35, 0.35)
        } else if lower.contains("water") || lower.contains("sea") || lower.contains("river") {
            (0.05, 0.2)
        } else if lower.contains("mud") || lower.contains("dirt") || lower.contains("soil") {
            (0.2, 0.35)
        } else if lower.contains("asphalt") || lower.contains("road") || lower.contains("pave") {
            (0.1, 0.3)
        } else {
            (-1.0, 0.4)
        }
    }

    fn find_terrain_type_for_texture(&self, texture: &TerrainTexture) -> Option<TerrainType> {
        let registry = ini_terrain::get_terrain_types()?;
        let guard = registry.read();

        let mut candidates: Vec<AsciiString> = Vec::new();
        if !texture.name.is_empty() {
            candidates.push(AsciiString::from(texture.name.as_str()));
        }

        if let Some(file_name) = Path::new(&texture.diffuse_path)
            .file_name()
            .and_then(|n| n.to_str())
        {
            candidates.push(AsciiString::from(file_name));
            if let Some(stem) = Path::new(file_name).file_stem().and_then(|n| n.to_str()) {
                candidates.push(AsciiString::from(stem));
            }
        }

        for candidate in &candidates {
            if let Some(terrain) = guard.find_terrain(candidate) {
                return Some(terrain.clone());
            }
        }

        if let Some(file_name) = Path::new(&texture.diffuse_path)
            .file_name()
            .and_then(|n| n.to_str())
        {
            let file_lower = file_name.to_ascii_lowercase();
            for terrain_name in guard.get_terrain_names() {
                let key = AsciiString::from(terrain_name.as_str());
                if let Some(terrain) = guard.find_terrain(&key) {
                    let texture_path = terrain.texture_name.as_str();
                    if !texture_path.is_empty() {
                        let terrain_file = Path::new(texture_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(texture_path)
                            .to_ascii_lowercase();
                        if terrain_file == file_lower {
                            return Some(terrain.clone());
                        }
                    }
                }
            }
        }

        None
    }

    fn rule_from_terrain_type(texture_id: TextureId, terrain: &TerrainType) -> TextureRule {
        let mut rule = Self::base_rule_for(
            texture_id,
            Some(&terrain.surface_type),
            terrain.name.as_str(),
        );

        if let Some(value) =
            Self::parse_f32_property(&terrain.properties, &["HeightMin", "MinHeight"])
        {
            rule.height_min = value;
        }
        if let Some(value) =
            Self::parse_f32_property(&terrain.properties, &["HeightMax", "MaxHeight"])
        {
            rule.height_max = value;
        }
        if let Some(value) = Self::parse_f32_property(
            &terrain.properties,
            &["SlopeMin", "MinSlope", "SlopeMinDegrees"],
        ) {
            rule.slope_min = Self::normalize_slope(value);
        }
        if let Some(value) = Self::parse_f32_property(
            &terrain.properties,
            &["SlopeMax", "MaxSlope", "SlopeMaxDegrees"],
        ) {
            rule.slope_max = Self::normalize_slope(value);
        }
        if let Some(value) = Self::parse_f32_property(
            &terrain.properties,
            &["PreferredGradient", "GradientPreference"],
        ) {
            rule.preferred_gradient = value.clamp(-1.0, 1.0);
        }
        if let Some(value) =
            Self::parse_f32_property(&terrain.properties, &["GradientTolerance", "GradientRange"])
        {
            rule.gradient_tolerance = value.abs().max(0.05);
        }
        if let Some(priority) =
            Self::parse_u8_property(&terrain.properties, &["Priority", "PriorityWeight"])
        {
            rule.priority = priority;
        }

        rule
    }

    fn parse_f32_property(map: &HashMap<String, String>, keys: &[&str]) -> Option<f32> {
        for key in keys {
            if let Some(value) = map.get(*key) {
                if let Ok(parsed) = value.parse::<f32>() {
                    return Some(parsed);
                }
            }
        }
        None
    }

    fn parse_u8_property(map: &HashMap<String, String>, keys: &[&str]) -> Option<u8> {
        for key in keys {
            if let Some(value) = map.get(*key) {
                if let Ok(parsed) = value.parse::<i32>() {
                    return Some(parsed.clamp(0, 255) as u8);
                }
            }
        }
        None
    }

    fn normalize_slope(value: f32) -> f32 {
        if value > std::f32::consts::PI {
            value.to_radians()
        } else {
            value
        }
    }

    fn normalize_terrain_texture_path(path: &str) -> Option<String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return None;
        }

        let normalized = trimmed
            .replace('\\', "/")
            .chars()
            .filter(|c| *c != ' ')
            .collect::<String>();
        if normalized.contains('/') {
            Some(normalized)
        } else {
            Some(format!("{TERRAIN_TGA_DIR_PATH}{normalized}"))
        }
    }

    fn prepare_chunk_texture_binding(
        &mut self,
        chunk_id: ChunkId,
        texture_ids: &[TextureId],
        slot_map: &HashMap<TextureId, usize>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> TerrainResult<()> {
        let started = std::time::Instant::now();
        let layout = match &self.terrain_texture_bind_group_layout {
            Some(layout) => Arc::clone(layout),
            None => return Ok(()),
        };

        let sampler_mode = TerrainSamplerMode::current();
        let sampler_changed = self.terrain_sampler_mode != Some(sampler_mode);
        if self.terrain_sampler.is_none() || sampler_changed {
            self.terrain_sampler = Some(device.create_sampler(&sampler_mode.to_descriptor()));
            self.terrain_sampler_mode = Some(sampler_mode);
            if sampler_changed {
                self.chunk_texture_bindings.clear();
            }
        }

        let sampler = self
            .terrain_sampler
            .as_ref()
            .expect("terrain sampler should be initialised");

        let mut final_ids = [0; MAX_TEXTURES_PER_CHUNK];
        let fallback_texture_id = self.texture_system.first_texture_id();

        for idx in 0..MAX_TEXTURES_PER_CHUNK {
            let mut texture_id = *texture_ids.get(idx).unwrap_or(&0);
            if texture_id == 0 || self.texture_system.get_texture(texture_id).is_none() {
                texture_id = fallback_texture_id.unwrap_or(0);
            }
            final_ids[idx] = texture_id;
        }

        if let Some(existing) = self
            .chunk_texture_bindings
            .values()
            .find(|binding| binding.texture_ids == final_ids && binding.slot_map == *slot_map)
        {
            self.chunk_texture_bindings.insert(
                chunk_id,
                ChunkTextureBinding {
                    bind_group: existing.bind_group.clone(),
                    slot_map: existing.slot_map.clone(),
                    texture_ids: existing.texture_ids,
                    diffuse_views: existing.diffuse_views.clone(),
                },
            );
            let elapsed = started.elapsed();
            if elapsed >= std::time::Duration::from_millis(50) {
                warn!(
                    "Terrain chunk texture binding reuse slow: chunk={} elapsed={:?}",
                    chunk_id, elapsed
                );
            }
            return Ok(());
        }

        for texture_id in final_ids.iter() {
            if *texture_id == 0 {
                continue;
            }
            if let Err(err) = self.texture_system.load_texture(*texture_id) {
                warn!("Failed to load terrain texture {}: {}", texture_id, err);
            }
        }

        let mut diffuse_views = Vec::with_capacity(MAX_TEXTURES_PER_CHUNK);

        for (binding, texture_id) in final_ids.iter().enumerate() {
            let diffuse_fallback = DEFAULT_TERRAIN_COLORS[binding % DEFAULT_TERRAIN_COLORS.len()];
            let diffuse_view = if *texture_id == 0 {
                self.texture_system.acquire_texture_view(
                    0,
                    TextureKind::Diffuse,
                    device,
                    queue,
                    diffuse_fallback,
                )?
            } else {
                self.texture_system.acquire_texture_view(
                    *texture_id,
                    TextureKind::Diffuse,
                    device,
                    queue,
                    diffuse_fallback,
                )?
            };
            diffuse_views.push(diffuse_view);
        }

        let mut entries: Vec<wgpu::BindGroupEntry> = Vec::with_capacity(MAX_TEXTURES_PER_CHUNK + 1);

        for binding in 0..MAX_TEXTURES_PER_CHUNK {
            entries.push(wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: wgpu::BindingResource::TextureView(diffuse_views[binding].as_ref()),
            });
        }
        entries.push(wgpu::BindGroupEntry {
            binding: MAX_TEXTURES_PER_CHUNK as u32,
            resource: wgpu::BindingResource::Sampler(sampler),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("Terrain Chunk {} Texture Bind Group", chunk_id)),
            layout: layout.as_ref(),
            entries: &entries,
        });

        self.chunk_texture_bindings.insert(
            chunk_id,
            ChunkTextureBinding {
                bind_group,
                slot_map: slot_map.clone(),
                texture_ids: final_ids,
                diffuse_views,
            },
        );

        let elapsed = started.elapsed();
        if elapsed >= std::time::Duration::from_millis(50) {
            let texture_paths: Vec<String> = final_ids
                .iter()
                .map(|texture_id| {
                    self.texture_system
                        .get_texture(*texture_id)
                        .map(|texture| texture.diffuse_path.clone())
                        .unwrap_or_else(|| format!("<unknown:{}>", texture_id))
                })
                .collect();
            warn!(
                "Terrain chunk texture binding create slow: chunk={} elapsed={:?} textures={:?} paths={:?}",
                chunk_id, elapsed, final_ids, texture_paths
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod terrain_ini_archive_tests {
    use super::*;

    fn assets_root() -> Option<PathBuf> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Main/assets");
        root.join("INIZH.big")
            .is_file()
            .then_some(root)
    }

    /// (a) The terrain class registry must populate from INIZH.big when no
    /// loose Terrain.ini exists — `Data\INI\Default\Terrain.ini` plus the
    /// `Data\INI\Terrain.ini` override, extracted through the game FS.
    #[test]
    fn terrain_registry_loads_from_inizh_big() {
        let Some(assets) = assets_root() else {
            eprintln!("skipping: no INIZH.big under Main/assets");
            return;
        };
        crate::terrain::tree_buffer::add_game_fs_search_roots(&[assets], true);

        let sources = TerrainVisualImpl::collect_terrain_ini_sources(None);
        let archive_sources = sources
            .iter()
            .filter(|p| TerrainVisualImpl::is_archive_extracted_terrain_source(p))
            .count();
        assert!(
            archive_sources > 0,
            "expected archive-extracted Terrain.ini sources, got {sources:?}"
        );

        let count = ini_terrain::load_terrain_definitions(&sources)
            .expect("retail Terrain.ini must parse");
        assert!(count > 0, "terrain registry must be non-empty");

        // (b) A known retail class maps to a texture that resolves through
        // the game FS into TerrainZH.big (Art\Terrain\TMGras37a.tga).
        let registry = ini_terrain::get_terrain_types().expect("terrain registry");
        let guard = registry.read();
        let terrain = guard
            .find_terrain(&AsciiString::from("GrassMediumType37a"))
            .expect("known retail terrain class GrassMediumType37a");
        let texture = format!(
            "{TERRAIN_TGA_DIR_PATH}{}",
            terrain.texture_name.as_str().trim()
        );
        drop(guard);
        let resolved = TerrainTextures::resolve_texture_path(&texture)
            .expect("class texture must resolve through the game FS");
        assert!(
            resolved
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".tga"),
            "unexpected resolved path {resolved:?}"
        );
    }
}
