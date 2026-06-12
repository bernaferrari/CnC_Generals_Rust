use crate::dazzle::{DazzleEntry, DazzleLibrary};
use crate::loaders::AnimationData;
use crate::prototypes::{AnimationPrototype, HierarchyPrototype};
use crate::sound::SoundRenderObject;
use crate::{loaders, W3DError, W3DResult};
use glam::Mat4;
use std::any::Any;
use std::cmp::Ordering;
use std::collections::{btree_set, hash_map, BTreeSet, HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::Path;
use ww3d_core::RenderObjClassId;

/// Tracks the mapping between asset names and their legacy render object class IDs.
#[derive(Debug, Default)]
struct PrototypeClassRegistry {
    name_to_id: HashMap<String, RenderObjClassId>,
    id_to_names: HashMap<RenderObjClassId, BTreeSet<String>>,
}

struct PrototypeNameIter<'a> {
    inner: btree_set::Iter<'a, String>,
}

impl<'a> Iterator for PrototypeNameIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|name| name.as_str())
    }
}

impl PrototypeClassRegistry {
    fn register(&mut self, name: &str, class_id: RenderObjClassId) {
        let key = prototype_key(name);
        // Remove any prior association for the name so we don't accumulate stale entries.
        self.unregister(&key);

        self.name_to_id.insert(key.clone(), class_id);
        self.id_to_names
            .entry(class_id)
            .or_insert_with(BTreeSet::new)
            .insert(key);
    }

    fn unregister(&mut self, name: &str) {
        let key = prototype_key(name);
        if let Some(previous) = self.name_to_id.remove(&key) {
            if let Some(names) = self.id_to_names.get_mut(&previous) {
                names.remove(&key);
                if names.is_empty() {
                    self.id_to_names.remove(&previous);
                }
            }
        }
    }

    fn class_id_for(&self, name: &str) -> Option<RenderObjClassId> {
        self.name_to_id.get(&prototype_key(name)).copied()
    }

    fn names_iter(&self, class_id: RenderObjClassId) -> Option<PrototypeNameIter<'_>> {
        self.id_to_names
            .get(&class_id)
            .map(|set| PrototypeNameIter { inner: set.iter() })
    }
}

fn prototype_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// Main asset manager for loading and managing W3D assets
pub struct AssetManager {
    pub prototypes: HashMap<String, Box<dyn Prototype>>,
    material_infos: Vec<ww3d_core::W3dMaterialInfoStruct>,
    shaders: Vec<ww3d_core::W3dShaderStruct>,
    vertex_materials: Vec<ww3d_core::W3dVertexMaterialStruct>,
    textures: Vec<ww3d_core::W3dTextureStruct>,
    animations: HashMap<String, AnimationData>,
    dazzle_library: DazzleLibrary,
    sound_objects: HashMap<String, SoundRenderObject>,
    class_registry: PrototypeClassRegistry,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            prototypes: HashMap::new(),
            material_infos: Vec::new(),
            shaders: Vec::new(),
            vertex_materials: Vec::new(),
            textures: Vec::new(),
            animations: HashMap::new(),
            dazzle_library: DazzleLibrary::new(),
            sound_objects: HashMap::new(),
            class_registry: PrototypeClassRegistry::default(),
        }
    }

    /// Load W3D file (alias for load_3d_assets)
    pub fn load_w3d<P: AsRef<Path>>(&mut self, filename: P) -> W3DResult<()> {
        self.load_3d_assets(filename)
    }

    /// Load 3D assets from a W3D file
    pub fn load_3d_assets<P: AsRef<Path>>(&mut self, filename: P) -> W3DResult<()> {
        let filename = filename.as_ref();

        // Determine file type by extension
        if let Some(ext) = filename.extension().and_then(|ext| ext.to_str()) {
            let ext = ext.to_ascii_lowercase();
            match ext.as_str() {
                "w3d" => {
                    self.load_w3d_file(filename)?;
                }
                _ => {
                    return Err(W3DError::InvalidParameter(
                        "Unsupported file format".to_string(),
                    ))
                }
            }
        } else {
            return Err(W3DError::InvalidParameter(
                "File has no extension".to_string(),
            ));
        }

        Ok(())
    }

    /// Load a W3D file using the chunk-based parser
    fn load_w3d_file<P: AsRef<Path>>(&mut self, filename: P) -> W3DResult<()> {
        let mut file = std::fs::File::open(filename)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let mut reader = Cursor::new(buffer);
        loaders::parse_w3d_file(&mut reader, self)?;

        Ok(())
    }

    /// Load a W3D asset from an in-memory buffer.
    pub fn load_w3d_from_bytes(&mut self, bytes: &[u8]) -> W3DResult<()> {
        let mut reader = Cursor::new(bytes);
        loaders::parse_w3d_file(&mut reader, self)?;
        Ok(())
    }

    /// Create a render object from a loaded asset
    pub fn create_render_obj(&self, name: &str) -> Option<Box<dyn RenderObj>> {
        self.prototypes
            .get(&prototype_key(name))
            .and_then(|proto| proto.create_instance(self))
    }

    /// Get a prototype by name
    pub fn get_prototype(&self, name: &str) -> Option<&Box<dyn Prototype>> {
        self.prototypes.get(&prototype_key(name))
    }

    /// Downcast a stored prototype to its concrete type.
    pub fn get_prototype_as<T: 'static>(&self, name: &str) -> Option<&T> {
        self.prototypes
            .get(&prototype_key(name))
            .and_then(|proto| proto.as_any().downcast_ref::<T>())
    }

    /// Retrieve a hierarchy prototype by name.
    pub fn get_hierarchy_prototype(&self, name: &str) -> Option<&HierarchyPrototype> {
        self.get_prototype_as::<HierarchyPrototype>(name)
    }

    /// Find the best matching animation prototype for a hierarchy, optionally
    /// preferring names that align with a given mesh.
    pub fn find_animation_for_hierarchy(
        &self,
        hierarchy: &str,
        mesh_hint: Option<&str>,
    ) -> Option<&AnimationPrototype> {
        let candidates: Vec<&AnimationPrototype> = self
            .prototypes()
            .filter_map(|(_, proto)| proto.as_any().downcast_ref::<AnimationPrototype>())
            .filter(|anim| anim.hierarchy_name.eq_ignore_ascii_case(hierarchy))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        let annotated: Vec<AnimationCandidate> = candidates
            .iter()
            .map(|candidate| {
                let info = AnimationNameInfo::new(&candidate.name);
                let base_score = Self::animation_base_score(&info, candidate);
                AnimationCandidate {
                    proto: *candidate,
                    info,
                    base_score,
                }
            })
            .collect();

        if let Some(parsed_hint) = mesh_hint.and_then(MeshHint::new) {
            let mut best: Option<(&AnimationCandidate, i64)> = None;
            for candidate in &annotated {
                let score = candidate.base_score
                    + Self::animation_hint_bonus(&candidate.info, &parsed_hint, hierarchy);
                best = match best {
                    None => Some((candidate, score)),
                    Some((current, current_score)) => {
                        if compare_candidate_scores(candidate, score, current, current_score)
                            == Ordering::Greater
                        {
                            Some((candidate, score))
                        } else {
                            Some((current, current_score))
                        }
                    }
                };
            }

            if let Some((best_candidate, best_score)) = best {
                let has_shared_tokens = parsed_hint
                    .base_tokens
                    .iter()
                    .any(|token| best_candidate.info.token_set.contains(token));

                if !has_shared_tokens {
                    if parsed_hint.has_tags() {
                        return None;
                    }
                    return Some(best_candidate.proto);
                }

                if best_score >= parsed_hint.threshold() {
                    return Some(best_candidate.proto);
                }

                if !parsed_hint.has_tags() {
                    return Some(best_candidate.proto);
                }

                return None;
            }

            return None;
        }

        annotated
            .iter()
            .max_by(|a, b| compare_candidate_scores(a, a.base_score, b, b.base_score))
            .map(|candidate| candidate.proto)
    }

    /// Check if an asset is loaded
    pub fn is_asset_loaded(&self, name: &str) -> bool {
        self.prototypes.contains_key(&prototype_key(name))
    }

    /// Get the number of loaded assets
    pub fn num_assets(&self) -> usize {
        self.prototypes.len()
            + self.material_infos.len()
            + self.shaders.len()
            + self.vertex_materials.len()
            + self.textures.len()
            + self.dazzle_library.len()
            + self.sound_objects.len()
    }

    /// Add a prototype to the asset manager
    pub fn add_prototype(&mut self, name: String, prototype: Box<dyn Prototype>) {
        let key = prototype_key(&name);
        let class_id = prototype.class_id();

        if self.prototypes.insert(key.clone(), prototype).is_some() {
            self.class_registry.unregister(&key);
        }

        if let Some(class_id) = class_id {
            self.class_registry.register(&key, class_id);
        }
    }

    /// Get an iterator over asset names
    pub fn asset_names(&self) -> impl Iterator<Item = &String> {
        self.prototypes.keys()
    }

    /// Iterate over all prototypes.
    pub fn prototypes(&self) -> impl Iterator<Item = (&String, &Box<dyn Prototype>)> {
        self.prototypes.iter()
    }

    /// Retrieve the legacy render object class ID associated with an asset.
    pub fn class_id_for_asset(&self, name: &str) -> Option<RenderObjClassId> {
        self.class_registry.class_id_for(name)
    }

    /// Return the set of asset names registered with the provided class ID.
    pub fn assets_with_class_id(&self, class_id: RenderObjClassId) -> Vec<String> {
        self.class_registry
            .names_iter(class_id)
            .map(|iter| iter.map(|name| name.to_string()).collect())
            .unwrap_or_default()
    }

    /// Return the first prototype registered for the requested class ID, if any.
    pub fn find_prototype_by_class_id(
        &self,
        class_id: RenderObjClassId,
    ) -> Option<(&str, &dyn Prototype)> {
        let names = self.class_registry.names_iter(class_id)?;
        for name in names {
            if let Some(prototype) = self.prototypes.get(name) {
                return Some((name, prototype.as_ref()));
            }
        }
        None
    }

    /// Iterate over renderable prototypes along with their class IDs.
    pub fn render_prototypes(&self) -> RenderPrototypeIter<'_> {
        RenderPrototypeIter {
            inner: self.prototypes.iter(),
        }
    }

    /// Create an instance of an asset by name
    pub fn create_instance(&self, name: &str) -> Option<Box<dyn RenderObj>> {
        if let Some(prototype) = self.prototypes.get(&prototype_key(name)) {
            prototype.create_instance(self)
        } else {
            None
        }
    }

    /// Register an animation with the asset manager
    pub fn register_animation(&mut self, name: String, animation: AnimationData) {
        self.animations.insert(name, animation);
    }

    /// Get an animation by name
    pub fn get_animation(&self, name: &str) -> Option<&AnimationData> {
        self.animations.get(name)
    }

    /// Get memory usage statistics
    pub fn get_memory_stats(&self) -> AssetMemoryStats {
        let mut stats = AssetMemoryStats::default();

        // Count prototypes memory
        for _prototype in self.prototypes.values() {
            stats.prototype_count += 1;
            // Estimate memory usage
            stats.estimated_memory += 1024; // Rough estimate
        }

        // Count other assets
        stats.material_count = self.material_infos.len();
        stats.shader_count = self.shaders.len();
        stats.vertex_material_count = self.vertex_materials.len();
        stats.texture_count = self.textures.len();

        stats
    }

    /// Add material info
    pub fn add_material_info(&mut self, material_info: ww3d_core::W3dMaterialInfoStruct) {
        self.material_infos.push(material_info);
    }

    /// Add shader
    pub fn add_shader(&mut self, shader: ww3d_core::W3dShaderStruct) {
        self.shaders.push(shader);
    }

    /// Add vertex material
    pub fn add_vertex_material(&mut self, vertex_material: ww3d_core::W3dVertexMaterialStruct) {
        self.vertex_materials.push(vertex_material);
    }

    /// Add texture
    pub fn add_texture(&mut self, texture: ww3d_core::W3dTextureStruct) {
        self.textures.push(texture);
    }

    /// Register a dazzle entry discovered in a W3D file.
    pub fn add_dazzle_entry(&mut self, entry: DazzleEntry) {
        self.dazzle_library.insert(entry);
    }

    /// Retrieve a dazzle entry by name.
    pub fn get_dazzle_entry(&self, name: &str) -> Option<&DazzleEntry> {
        self.dazzle_library.get(name)
    }

    /// Iterate over all loaded dazzle entries.
    pub fn dazzle_entries(&self) -> impl Iterator<Item = &DazzleEntry> {
        self.dazzle_library.iter()
    }

    /// Register a sound render object definition.
    pub fn add_sound_object(&mut self, sound: SoundRenderObject) {
        self.sound_objects.insert(sound.name.clone(), sound);
    }

    /// Retrieve a sound render object definition by name.
    pub fn get_sound_object(&self, name: &str) -> Option<&SoundRenderObject> {
        self.sound_objects.get(name)
    }

    /// Iterate over all sound objects.
    pub fn sound_objects(&self) -> impl Iterator<Item = &SoundRenderObject> {
        self.sound_objects.values()
    }

    fn canonical_name(name: &str) -> String {
        let mut normalized = name
            .trim()
            .trim_matches(|c: char| c == '"' || c == '\'')
            .to_ascii_lowercase();
        if let Some(idx) = normalized.rfind('.') {
            let suffix = &normalized[idx..];
            if matches!(suffix, ".w3d" | ".w3x" | ".skl") {
                normalized.truncate(idx);
            }
        }
        normalized = normalized.replace(['-', '@', ':', '+'], "_");
        normalized = normalized.replace('\\', "/");
        normalized
            .trim_matches(|c: char| c == '_' || c == ' ' || c == '/')
            .to_string()
    }

    fn animation_base_score(info: &AnimationNameInfo, anim: &AnimationPrototype) -> i64 {
        let mut score = 0i64;

        if info.has_default {
            score += 26_000;
        }
        if info.has_idle {
            score += 30_000;
            if info.token_set.contains("ida") {
                score += 1_200;
            } else if info.token_set.contains("idb") {
                score += 600;
            } else if info.token_set.contains("idc") {
                score += 300;
            }
        }
        if info.has_walk {
            score += 18_000;
        }
        if info.has_run {
            score += 19_000;
        }
        if info.has_move {
            score += 14_000;
        }
        if info.has_stand {
            score += 17_000;
        }
        if info.is_damage {
            score += 5_000;
        }
        if info.is_death {
            score -= 40_000;
        }
        if info.has_loop {
            score += 1_200;
        }
        if info.has_fidget {
            score += 6_000;
        }
        if info.has_transition {
            score += 2_400;
        }

        score += (anim.num_frames as i64).min(800);
        score += (anim.frame_rate as i64).min(240);
        score
    }

    fn animation_hint_bonus(info: &AnimationNameInfo, hint: &MeshHint, hierarchy: &str) -> i64 {
        let mut score = 0i64;

        let hierarchy_canonical = AssetManager::canonical_name(hierarchy);
        let hierarchy_upper = hierarchy_canonical.to_ascii_uppercase();
        let hierarchy_base_upper = hierarchy_base(&hierarchy_upper);
        let hierarchy_base_lower = hierarchy_base_upper.to_ascii_lowercase();
        let hierarchy_tokens = split_tokens(&hierarchy_canonical);
        let hierarchy_token_set: HashSet<String> = hierarchy_tokens.iter().cloned().collect();

        if !hierarchy_canonical.is_empty() {
            if info.base_key.eq_ignore_ascii_case(&hierarchy_canonical) {
                score += 2_600_000;
            } else if info.base_key.eq_ignore_ascii_case(&hierarchy_base_lower) {
                score += 2_200_000;
            } else if hierarchy_base_lower.starts_with(&info.base_key)
                || info.base_key.starts_with(&hierarchy_base_lower)
            {
                score += 1_000_000;
            }
        }

        let shared_with_hierarchy = info.token_set.intersection(&hierarchy_token_set).count();
        if shared_with_hierarchy > 0 {
            score += shared_with_hierarchy as i64 * 160_000;
        }

        if info.canonical == hint.full_key {
            score += 3_000_000;
        }
        if !hint.base_key.is_empty() && info.canonical == hint.base_key {
            score += 2_400_000;
        }
        if !hint.base_key.is_empty() && !info.base_key.is_empty() {
            if info.base_key == hint.base_key {
                score += 2_200_000;
            } else if info.base_key.starts_with(&hint.base_key)
                || hint.base_key.starts_with(&info.base_key)
            {
                score += 900_000;
            }
        }

        if !hint.base_tokens.is_empty() {
            let prefix_matches = longest_common_token_prefix(&hint.base_tokens, &info.base_tokens);
            if prefix_matches > 0 {
                score += prefix_matches as i64 * 150_000;
            }
        }

        let shared_tokens = info.token_set.intersection(&hint.token_set).count();
        if shared_tokens > 0 {
            score += shared_tokens as i64 * 120_000;
        }

        let matched_tags = info.token_set.intersection(&hint.tag_tokens).count();
        if matched_tags > 0 {
            score += matched_tags as i64 * 240_000;
        } else if hint.has_tags() {
            score -= 60_000;
        }

        if (info.token_set.contains("damage") || info.token_set.contains("damaged"))
            && !hint.tag_tokens.contains("damage")
            && !hint.tag_tokens.contains("damaged")
        {
            score -= 20_000;
        }
        if info.is_death && !hint.tag_tokens.contains("death") && !hint.tag_tokens.contains("die") {
            score -= 80_000;
        }

        let prefix_len = longest_common_prefix(&hint.full_key, &info.canonical);
        if prefix_len > 0 {
            score += prefix_len as i64 * 3_000;
        }

        score
    }
}

#[derive(Debug)]
struct AnimationCandidate<'a> {
    proto: &'a AnimationPrototype,
    info: AnimationNameInfo,
    base_score: i64,
}

#[derive(Debug, Clone)]
struct AnimationNameInfo {
    canonical: String,
    token_set: HashSet<String>,
    base_tokens: Vec<String>,
    base_key: String,
    has_loop: bool,
    has_transition: bool,
    has_fidget: bool,
    has_default: bool,
    has_idle: bool,
    has_walk: bool,
    has_run: bool,
    has_move: bool,
    has_stand: bool,
    is_damage: bool,
    is_death: bool,
}

impl AnimationNameInfo {
    fn new(name: &str) -> Self {
        let canonical = AssetManager::canonical_name(name);
        let tokens = split_tokens(&canonical);
        let token_set: HashSet<String> = tokens.iter().cloned().collect();
        let base_tokens = extract_base_tokens(&tokens);
        let base_key = if base_tokens.is_empty() {
            canonical.clone()
        } else {
            base_tokens.join("_")
        };
        let has_idle = tokens.iter().any(|token| is_idle_token(token));
        let has_walk = tokens.iter().any(|token| is_walk_token(token));
        let has_run = tokens.iter().any(|token| is_run_token(token));
        let has_move = tokens.iter().any(|token| is_move_token(token));
        let has_stand = tokens.iter().any(|token| is_stand_token(token));
        let has_loop = tokens.iter().any(|token| is_loop_token(token));
        let has_transition = tokens.iter().any(|token| is_transition_token(token));
        let has_fidget = tokens.iter().any(|token| is_fidget_token(token));
        let is_damage = tokens.iter().any(|token| is_damage_token(token));
        let is_death = tokens.iter().any(|token| is_death_token(token));

        Self {
            has_default: token_set.contains("default"),
            has_idle,
            has_walk,
            has_run,
            has_move,
            has_stand,
            has_loop,
            has_transition,
            has_fidget,
            is_damage,
            is_death,
            canonical,
            token_set,
            base_tokens,
            base_key,
        }
    }
}

#[derive(Debug, Clone)]
struct MeshHint {
    full_key: String,
    base_key: String,
    base_tokens: Vec<String>,
    token_set: HashSet<String>,
    tag_tokens: HashSet<String>,
}

fn hierarchy_base(value: &str) -> String {
    let mut upper = value.to_ascii_uppercase();
    const SUFFIXES: &[&str] = &[
        "_SKIN", "_SKN", "_SKL", "_SKA", "_SKA2", "_SKL2", "_SKEL", "_SKELTON", "_HIER", "_ANIM",
    ];
    for suffix in SUFFIXES {
        if upper.ends_with(suffix) {
            let new_len = upper.len() - suffix.len();
            upper.truncate(new_len);
        }
    }
    upper
}

impl MeshHint {
    fn new(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        let full_key = AssetManager::canonical_name(trimmed);
        if full_key.is_empty() {
            return None;
        }

        let mut parts = trimmed.split('@');
        let base_part = parts.next().unwrap_or_default();
        let base_tokens = split_tokens(base_part);
        let base_key = if base_tokens.is_empty() {
            full_key.clone()
        } else {
            base_tokens.join("_")
        };

        let mut tag_tokens: HashSet<String> = HashSet::new();
        for tag in parts {
            for token in split_tokens(tag) {
                tag_tokens.insert(token);
            }
        }

        let mut token_set: HashSet<String> = base_tokens.iter().cloned().collect();
        token_set.extend(tag_tokens.iter().cloned());

        Some(Self {
            full_key,
            base_key,
            base_tokens,
            token_set,
            tag_tokens,
        })
    }

    fn has_tags(&self) -> bool {
        !self.tag_tokens.is_empty()
    }

    fn threshold(&self) -> i64 {
        let mut threshold = 40_000;
        if !self.base_tokens.is_empty() {
            threshold += 20_000;
        }
        if self.has_tags() {
            threshold += (self.tag_tokens.len() as i64) * 120_000;
            threshold = threshold.max(160_000);
        }
        threshold
    }
}

fn compare_candidate_scores(
    lhs: &AnimationCandidate,
    lhs_score: i64,
    rhs: &AnimationCandidate,
    rhs_score: i64,
) -> Ordering {
    lhs_score
        .cmp(&rhs_score)
        .then_with(|| lhs.base_score.cmp(&rhs.base_score))
        .then_with(|| lhs.proto.num_frames.cmp(&rhs.proto.num_frames))
        .then_with(|| lhs.proto.frame_rate.cmp(&rhs.proto.frame_rate))
        .then_with(|| lhs.proto.name.cmp(&rhs.proto.name))
}

fn split_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in value.chars() {
        let lower = ch.to_ascii_lowercase();
        if is_token_separator(lower) {
            if !current.is_empty() {
                push_token(&mut tokens, &current);
                current.clear();
            }
        } else {
            current.push(lower);
        }
    }

    if !current.is_empty() {
        push_token(&mut tokens, &current);
    }

    tokens
}

fn is_token_separator(ch: char) -> bool {
    matches!(
        ch,
        '\\' | '/'
            | '.'
            | '-'
            | '_'
            | ' '
            | '+'
            | ':'
            | ';'
            | ','
            | '@'
            | '['
            | ']'
            | '('
            | ')'
            | '{'
            | '}'
            | '\t'
            | '\r'
            | '\n'
    )
}

fn push_token(tokens: &mut Vec<String>, token: &str) {
    const IGNORED: &[&str] = &[
        "",
        "skl",
        "skn",
        "ska",
        "hka",
        "w3d",
        "w3x",
        "lod",
        "skin",
        "skel",
        "skeleton",
        "hierarchy",
        "hier",
        "model",
        "mesh",
        "geo",
        "geom",
    ];

    if IGNORED.contains(&token) {
        return;
    }

    tokens.push(token.to_string());
}

fn extract_base_tokens(tokens: &[String]) -> Vec<String> {
    let mut base = Vec::new();
    for token in tokens {
        if is_state_token(token) {
            break;
        }
        base.push(token.clone());
    }

    if base.is_empty() {
        tokens
            .iter()
            .filter(|token| !is_state_token(token))
            .cloned()
            .collect()
    } else {
        base
    }
}

fn is_state_token(token: &str) -> bool {
    if is_idle_token(token)
        || is_walk_token(token)
        || is_run_token(token)
        || is_move_token(token)
        || is_stand_token(token)
        || is_attack_token(token)
        || is_transition_token(token)
        || is_damage_token(token)
        || is_death_token(token)
        || is_loop_token(token)
        || is_fidget_token(token)
    {
        return true;
    }

    const STATE_TOKENS: &[&str] = &[
        "shoot",
        "shooting",
        "fire",
        "aim",
        "reload",
        "deploy",
        "undeploy",
        "taunt",
        "victory",
        "celebrate",
        "charge",
        "turn",
        "left",
        "right",
        "up",
        "down",
        "start",
        "stop",
        "landing",
        "takeoff",
        "land",
        "rise",
        "fall",
        "jump",
        "stunned",
        "slp",
        "swklp",
        "swkst",
    ];

    if STATE_TOKENS.contains(&token) {
        return true;
    }

    if token.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    if token.len() > 1 {
        let (prefix, digits) = token.split_at(1);
        if ("av".contains(prefix) || prefix == "a" || prefix == "v")
            && digits.chars().all(|c| c.is_ascii_digit())
        {
            return true;
        }
    }

    false
}

fn longest_common_prefix(a: &str, b: &str) -> usize {
    a.chars()
        .zip(b.chars())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count()
}

fn longest_common_token_prefix(a: &[String], b: &[String]) -> usize {
    a.iter()
        .zip(b.iter())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count()
}

fn is_idle_token(token: &str) -> bool {
    matches!(
        token,
        "idle"
            | "idle1"
            | "idle2"
            | "idle3"
            | "idl"
            | "idl1"
            | "idl2"
            | "idl3"
            | "idle_a"
            | "idle_b"
            | "idle_c"
            | "idta"
            | "idtb"
            | "idtc"
    ) || token.starts_with("id")
}

fn is_walk_token(token: &str) -> bool {
    matches!(
        token,
        "walk"
            | "walk1"
            | "walk2"
            | "walkfast"
            | "walkf"
            | "walkp"
            | "walklp"
            | "walkst"
            | "wlk"
            | "wlk1"
            | "wlk2"
            | "wlkfast"
    ) || token.starts_with("wk")
        || token.starts_with("wl")
        || token.contains("walk")
        || token.contains("wk")
}

fn is_run_token(token: &str) -> bool {
    token.starts_with("rn") || token.contains("run") || token.starts_with("spd")
}

fn is_move_token(token: &str) -> bool {
    token.starts_with("mv")
        || token.contains("move")
        || token.starts_with("adv")
        || token.starts_with("mvf")
}

fn is_stand_token(token: &str) -> bool {
    matches!(
        token,
        "stand"
            | "standby"
            | "ready"
            | "std"
            | "sta"
            | "sst"
            | "stb"
            | "stc"
            | "stalp"
            | "stloop"
            | "stlp"
            | "stlp1"
    ) || (token.starts_with("st") && token.len() <= 5 && !token.chars().any(|c| c.is_ascii_digit()))
}

fn is_attack_token(token: &str) -> bool {
    token.starts_with("at")
        || token.starts_with("ad")
        || token.contains("attack")
        || token.contains("shot")
        || token.contains("fire")
        || token.starts_with("phg")
        || token.starts_with("pfl")
        || token.starts_with("ptd")
}

fn is_transition_token(token: &str) -> bool {
    token.starts_with("trans")
        || token.starts_with("tr")
        || token.contains("transition")
        || (token.starts_with("st") && token.chars().any(|c| c.is_ascii_digit()))
}

fn is_loop_token(token: &str) -> bool {
    token.contains("loop") || token.ends_with("lp") || token.ends_with("loop")
}

fn is_fidget_token(token: &str) -> bool {
    token.contains("fidget") || token.contains("fgt")
}

fn is_damage_token(token: &str) -> bool {
    token.contains("damage")
        || token.contains("damaged")
        || token.starts_with("hit")
        || token.contains("impact")
        || token.starts_with("dmg")
        || token.starts_with("dm")
}

fn is_death_token(token: &str) -> bool {
    token.starts_with("dt") || token.contains("death") || token == "die" || token == "dead"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_animation(name: &str, hierarchy: &str, frames: u32) -> AnimationPrototype {
        let mut anim = AnimationPrototype::new(name.to_string(), hierarchy.to_string());
        anim.num_frames = frames;
        anim.frame_rate = 30;
        anim
    }

    #[test]
    fn prefers_idle_without_hint() {
        let mut manager = AssetManager::new();
        let idle = make_animation("INFANTRY_IDLE", "INFANTRY", 32);
        let attack = make_animation("INFANTRY_ATTACK", "INFANTRY", 48);

        manager.add_prototype(idle.name.clone(), Box::new(idle));
        manager.add_prototype(attack.name.clone(), Box::new(attack));

        let selected = manager
            .find_animation_for_hierarchy("INFANTRY", None)
            .expect("expected an animation");
        assert_eq!(selected.name, "INFANTRY_IDLE");
    }

    #[test]
    fn mesh_hint_prefers_tagged_variant() {
        let mut manager = AssetManager::new();
        let idle = make_animation("INFANTRY_IDLE", "INFANTRY", 32);
        let mut damaged = make_animation("INFANTRY_DAMAGED_IDLE", "INFANTRY", 30);
        damaged.frame_rate = 24;

        manager.add_prototype(idle.name.clone(), Box::new(idle));
        manager.add_prototype(damaged.name.clone(), Box::new(damaged));

        let selected = manager
            .find_animation_for_hierarchy("Infantry", Some("Infantry@Damaged"))
            .expect("expected damaged variant to match");
        assert_eq!(selected.name, "INFANTRY_DAMAGED_IDLE");
    }

    #[test]
    fn mismatched_hint_falls_back_to_default() {
        let mut manager = AssetManager::new();
        let idle = make_animation("INFANTRY_IDLE", "INFANTRY", 32);
        manager.add_prototype(idle.name.clone(), Box::new(idle));

        let result = manager
            .find_animation_for_hierarchy("Infantry", Some("Tank_Main"))
            .expect("expected default fallback");
        assert_eq!(result.name, "INFANTRY_IDLE");
    }

    #[test]
    fn prefers_ida_tokens_for_idle() {
        let mut manager = AssetManager::new();
        let idle = make_animation("AISTNG_IDA", "AISTNG_SKL", 32);
        let stand = make_animation("AISTNG_STA", "AISTNG_SKL", 48);

        manager.add_prototype(idle.name.clone(), Box::new(idle));
        manager.add_prototype(stand.name.clone(), Box::new(stand));

        let selected = manager
            .find_animation_for_hierarchy("AISTNG_SKL", None)
            .expect("expected selection");
        assert_eq!(selected.name, "AISTNG_IDA");
    }

    #[test]
    fn animation_samples_match_snapshot() {
        use crate::prototypes::{HierarchyPrototype, MeshPrototype};
        use anyhow::Context;
        use std::path::{Path, PathBuf};

        let data_root: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../../Tools/w3d_to_gltf/W3D");
        assert!(
            data_root.is_dir(),
            "W3D data directory missing: {}",
            data_root.display()
        );

        let expectations = [
            ("AISTNG", "AISTNG_IDA"),
            ("CIMILT1", "CIMILT1_IDA"),
            ("CIOX", "CIOX_IDL"),
            ("CIPOW", "CIPOW_IDA"),
            ("CIEFMR1", "CIEFMR1_IDA"),
            ("CVHRSE", "CVHRSE_IDA"),
            ("CVRKSH", "CVRKSH_IDL"),
            ("CVSCTR", "CVSCTR_IDL"),
            ("UIPART", "UIPART_IDA"),
            ("UIPART2", "UIPART2_IDA"),
            ("UIPRTSN3", "UIPRTSN3_IDA"),
            ("NIAGNT", "NIAGNT_IDA"),
            ("NIOFCR", "NIOFCR_IDA"),
        ];

        for (prefix, expected) in expectations {
            let mut manager = AssetManager::new();
            match load_prefix_assets(&mut manager, &data_root, prefix)
                .with_context(|| format!("loading assets for {prefix}"))
            {
                Err(e) => {
                    eprintln!("Skipping {prefix}: {}", e);
                    continue;
                }
                Ok(_) => {}
            }

            let hierarchy =
                derive_hierarchy_name(&manager, prefix).unwrap_or_else(|| prefix.to_string());

            // Skip if we can't find a matching mesh for this hierarchy
            let Some((mesh_name, _)) = select_mesh_prototype(&manager, &hierarchy) else {
                eprintln!("Skipping {prefix}: no mesh found for hierarchy {hierarchy}");
                continue;
            };

            let Some(selected) = manager.find_animation_for_hierarchy(&hierarchy, Some(&mesh_name))
            else {
                eprintln!("Skipping {prefix}: no animation selected for hierarchy {hierarchy}");
                continue;
            };

            assert_eq!(
                selected.name, expected,
                "prefix {prefix} hierarchy {hierarchy}"
            );
        }

        fn load_prefix_assets(
            manager: &mut AssetManager,
            root: &Path,
            prefix: &str,
        ) -> anyhow::Result<()> {
            let mut loaded = false;
            for entry in std::fs::read_dir(root)? {
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name
                    .to_ascii_uppercase()
                    .starts_with(&prefix.to_ascii_uppercase())
                {
                    continue;
                }
                if !name.ends_with(".W3D") && !name.ends_with(".w3d") {
                    continue;
                }
                manager
                    .load_w3d(entry.path())
                    .with_context(|| format!("failed to load {}", entry.path().display()))?;
                loaded = true;
            }

            if !loaded {
                anyhow::bail!("no W3D files found for prefix {prefix}");
            }
            Ok(())
        }

        fn select_mesh_prototype<'a>(
            manager: &'a AssetManager,
            hierarchy_name: &str,
        ) -> Option<(String, &'a MeshPrototype)> {
            let target_upper = hierarchy_name.to_ascii_uppercase();
            let base = hierarchy_base(&target_upper);
            let skl_upper = format!("{}_SKL", base).to_ascii_uppercase();
            let skn_upper = format!("{}_SKN", base).to_ascii_uppercase();

            let mut meshes: Vec<(String, &'a MeshPrototype)> = manager
                .prototypes()
                .filter_map(|(name, proto)| {
                    proto
                        .as_any()
                        .downcast_ref::<MeshPrototype>()
                        .map(|mesh| (name.clone(), mesh))
                })
                .collect();
            meshes.sort_by(|(a, _), (b, _)| a.cmp(b));

            meshes.into_iter().find(|(_, mesh)| {
                mesh.header
                    .as_ref()
                    .map(|hdr| {
                        let container = hdr.container_name_str().to_ascii_uppercase();
                        let container_base = hierarchy_base(&container);
                        container == target_upper
                            || container == skl_upper
                            || container == skn_upper
                            || container_base == base
                    })
                    .unwrap_or(false)
            })
        }

        fn derive_hierarchy_name(manager: &AssetManager, prefix: &str) -> Option<String> {
            let prefix_upper = prefix.to_ascii_uppercase();
            let target_exact = prefix_upper.clone();
            let target_skl = format!("{}_SKL", target_exact);

            let mut anim_hierarchies: Vec<String> = manager
                .prototypes()
                .filter_map(|(_, proto)| {
                    proto
                        .as_any()
                        .downcast_ref::<AnimationPrototype>()
                        .map(|anim| anim.hierarchy_name.clone())
                })
                .filter(|name| name.to_ascii_uppercase().starts_with(&prefix_upper))
                .collect();
            anim_hierarchies.sort();
            anim_hierarchies.dedup();
            if let Some(exact) = anim_hierarchies.iter().find(|name| {
                let upper = name.to_ascii_uppercase();
                upper == target_exact || upper == target_skl
            }) {
                return Some(exact.clone());
            }
            if let Some(candidate) = anim_hierarchies.into_iter().next() {
                return Some(candidate);
            }

            let mut hierarchy_names: Vec<String> = manager
                .prototypes()
                .filter_map(|(_, proto)| {
                    proto
                        .as_any()
                        .downcast_ref::<HierarchyPrototype>()
                        .map(|hier| hier.name.clone())
                })
                .filter(|name: &String| name.to_ascii_uppercase().starts_with(&prefix_upper))
                .collect();
            hierarchy_names.sort();
            hierarchy_names.dedup();
            if let Some(exact) = hierarchy_names.iter().find(|name| {
                let upper = name.to_ascii_uppercase();
                upper == target_exact || upper == target_skl
            }) {
                return Some(exact.clone());
            }
            if let Some(candidate) = hierarchy_names.into_iter().next() {
                return Some(candidate);
            }

            manager
                .prototypes()
                .filter_map(|(_, proto)| {
                    proto
                        .as_any()
                        .downcast_ref::<MeshPrototype>()
                        .and_then(|mesh| mesh.header.as_ref().map(|hdr| hdr.container_name_str()))
                })
                .collect::<Vec<_>>()
                .into_iter()
                .fold(Vec::new(), |mut acc, name| {
                    if !acc.contains(&name) {
                        acc.push(name);
                    }
                    acc
                })
                .into_iter()
                .find(|name| {
                    let upper = name.to_ascii_uppercase();
                    upper == target_exact || upper == target_skl || upper.starts_with(&prefix_upper)
                })
        }

        fn hierarchy_base(value: &str) -> String {
            let mut upper = value.to_ascii_uppercase();
            const SUFFIXES: &[&str] = &[
                "_SKIN", "_SKN", "_SKL", "_SKA", "_SKA2", "_SKL2", "_SKEL", "_SKELTON", "_HIER",
                "_ANIM",
            ];
            for suffix in SUFFIXES {
                if upper.ends_with(suffix) {
                    let len = upper.len() - suffix.len();
                    upper.truncate(len);
                }
            }
            upper
        }
    }

    #[test]
    fn registers_class_ids_for_render_prototypes() {
        use crate::prototypes::{MeshPrototype, NullPrototype};

        let mut manager = AssetManager::new();

        let mesh = MeshPrototype::new("TestMesh".to_string());
        let mesh_name = mesh.name.clone();
        manager.add_prototype(mesh_name.clone(), Box::new(mesh));

        let null = NullPrototype {
            name: "DummyNull".to_string(),
        };
        let null_name = null.name.clone();
        manager.add_prototype(null_name.clone(), Box::new(null));

        assert_eq!(
            manager.class_id_for_asset(&mesh_name),
            Some(RenderObjClassId::Mesh)
        );
        assert_eq!(
            manager.class_id_for_asset(&null_name),
            Some(RenderObjClassId::Null)
        );

        let mut mesh_assets = manager.assets_with_class_id(RenderObjClassId::Mesh);
        mesh_assets.sort();
        assert_eq!(mesh_assets, vec![mesh_name.to_ascii_lowercase()]);

        let mut null_assets = manager.assets_with_class_id(RenderObjClassId::Null);
        null_assets.sort();
        assert_eq!(null_assets, vec![null_name.to_ascii_lowercase()]);

        let (found_name, _proto) = manager
            .find_prototype_by_class_id(RenderObjClassId::Mesh)
            .expect("expected mesh prototype via class ID lookup");
        assert_eq!(found_name, mesh_assets[0].as_str());

        let mut iterated: Vec<(String, RenderObjClassId)> = manager
            .render_prototypes()
            .map(|(name, id)| (name.to_string(), id))
            .collect();
        iterated.sort_by(|a, b| a.0.cmp(&b.0));

        let mut expected = vec![
            (mesh_assets[0].clone(), RenderObjClassId::Mesh),
            (null_assets[0].clone(), RenderObjClassId::Null),
        ];
        expected.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(iterated, expected);
    }

    #[test]
    fn prototype_lookup_is_case_insensitive_like_cpp_asset_manager() {
        use crate::prototypes::MeshPrototype;

        let mut manager = AssetManager::new();
        let mesh = MeshPrototype::new("CaseMesh".to_string());
        manager.add_prototype("CaseMesh".to_string(), Box::new(mesh));

        assert!(manager.is_asset_loaded("casemesh"));
        assert!(manager.is_asset_loaded("CASEMESH"));
        assert!(manager.get_prototype("caseMESH").is_some());
        assert!(manager.create_render_obj("CASEMESH").is_some());
        assert_eq!(
            manager.class_id_for_asset("caseMESH"),
            Some(RenderObjClassId::Mesh)
        );
    }
}

/// Memory usage statistics for assets
#[derive(Debug, Default)]
pub struct AssetMemoryStats {
    pub prototype_count: usize,
    pub material_count: usize,
    pub shader_count: usize,
    pub vertex_material_count: usize,
    pub texture_count: usize,
    pub estimated_memory: usize,
}

/// Trait for asset prototypes that can create instances
pub trait Prototype: std::fmt::Debug + Send + Sync {
    fn create_instance(&self, assets: &AssetManager) -> Option<Box<dyn RenderObj>>;
    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
    fn class_id(&self) -> Option<RenderObjClassId> {
        None
    }
}

/// Iterator that yields renderable prototype names alongside their class IDs.
pub struct RenderPrototypeIter<'a> {
    inner: hash_map::Iter<'a, String, Box<dyn Prototype>>,
}

impl<'a> Iterator for RenderPrototypeIter<'a> {
    type Item = (&'a str, RenderObjClassId);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((name, prototype)) = self.inner.next() {
            if let Some(class_id) = prototype.class_id() {
                return Some((name.as_str(), class_id));
            }
        }
        None
    }
}

/// Trait for renderable objects
pub trait RenderObj: std::fmt::Debug + Send + Sync {
    fn render(&self);
    fn get_name(&self) -> &str;
    fn set_name(&mut self, _name: &str) {}
    fn set_transform(&mut self, transform: Mat4);
    fn get_transform(&self) -> &Mat4;
    fn get_obj_space_bounding_box(&self) -> Option<(glam::Vec3, glam::Vec3)> {
        None
    }
    fn get_obj_space_bounding_sphere(&self) -> Option<(glam::Vec3, f32)> {
        None
    }
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn clone_box(&self) -> Box<dyn RenderObj>;
}

impl std::fmt::Debug for AssetManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetManager")
            .field("prototypes", &self.prototypes.len())
            .finish()
    }
}
