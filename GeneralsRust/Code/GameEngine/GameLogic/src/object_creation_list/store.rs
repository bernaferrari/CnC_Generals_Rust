// FILE: store.rs - Object Creation List Store
// Author: Steven Johnson, December 2001 (C++)
// Rust Port: 2025
// Desc: ObjectCreationListStore - manages all OCL definitions
//
// Ported from GeneralsMD/Code/GameEngine/Include/GameLogic/ObjectCreationList.h
//
// The store holds all ObjectCreationList definitions loaded from INI files.
// ObjectCreationLists are immutable and shared between multiple users.

use super::advanced_nuggets::{
    ApplyRandomForceNugget, AttackNugget, DeliverPayloadNugget, FireWeaponNugget, Payload,
};
use super::nuggets::{DebrisDisposition, GenericObjectCreationNugget, ObjectCreationNugget};
use super::{CreationContext, CreationResult};
use crate::common::NameKeyGenerator;
use crate::common::*;
use crate::object::Object;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

/// Object Creation List ID type (index into store)
pub type ObjectCreationListId = usize;

/// ObjectCreationList - a collection of nuggets that create objects together
/// Matches C++ class ObjectCreationList (ObjectCreationList.h:102-157)
///
/// ObjectCreationLists are:
/// - Specified solely by name
/// - Shared between multiple units (immutable after creation)
/// - No inheritance or overriding
/// - Can't be modified by subsequent INI loads
#[derive(Clone)]
pub struct ObjectCreationList {
    /// Nuggets to execute (in order)
    /// Note: nuggets are owned by the store, this just holds references
    nuggets: Vec<Arc<dyn ObjectCreationNugget>>,
}

impl std::fmt::Debug for ObjectCreationList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectCreationList")
            .field("nugget_count", &self.nuggets.len())
            .finish()
    }
}

impl ObjectCreationList {
    /// Create empty OCL
    pub fn new() -> Self {
        Self {
            nuggets: Vec::new(),
        }
    }

    /// Add a nugget to this OCL
    /// Matches C++ ObjectCreationList::addObjectCreationNugget (ObjectCreationList.cpp:1502-1506)
    pub fn add_nugget(&mut self, nugget: Arc<dyn ObjectCreationNugget>) {
        self.nuggets.push(nugget);
    }

    /// Clear all nuggets
    /// Matches C++ ObjectCreationList::clear (ObjectCreationList.cpp:1495-1499)
    pub fn clear(&mut self) {
        self.nuggets.clear();
    }

    /// Create objects with position and angle
    /// Matches C++ static ObjectCreationList::create with angle parameter (ObjectCreationList.h:126-131)
    ///
    /// Returns the first object created (or None if no objects created)
    pub fn create_with_angle(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Real,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        self.create_internal_with_angle(
            ctx,
            primary_obj,
            primary,
            secondary,
            angle,
            lifetime_frames,
        )
    }

    /// Create objects with object parameters
    /// Matches C++ static ObjectCreationList::create with objects (ObjectCreationList.h:136-141)
    ///
    /// Returns the first object created (or None if no objects created)
    pub fn create_with_objects(
        &self,
        ctx: &CreationContext<'_>,
        primary: &Object,
        secondary: Option<&Object>,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        self.create_internal_with_objects(ctx, primary, secondary, lifetime_frames)
    }

    /// Create objects with owner flag (for DeliverPayload)
    /// Matches C++ static ObjectCreationList::create with createOwner (ObjectCreationList.h:116-121)
    ///
    /// Returns the first object created (or None if no objects created)
    pub fn create_with_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        self.create_internal_with_owner_flag(
            ctx,
            primary_obj,
            primary,
            secondary,
            create_owner,
            lifetime_frames,
        )
    }

    /// Create objects with angle and create-owner flag.
    /// Matches C++ call sites that pass both angle and createOwner.
    pub fn create_with_angle_and_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Real,
        create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        self.create_internal_with_angle_and_owner_flag(
            ctx,
            primary_obj,
            primary,
            secondary,
            angle,
            create_owner,
            lifetime_frames,
        )
    }

    /// Internal implementation - create with angle
    /// Matches C++ ObjectCreationList::createInternal (ObjectCreationList.cpp:1524-1536)
    fn create_internal_with_angle(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Real,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        debug_assert!(
            primary_obj.is_some(),
            "ObjectCreationList expects a non-null primary object for ownership context"
        );
        let mut first_object: Option<Arc<RwLock<Object>>> = None;

        for nugget in &self.nuggets {
            if let Some(obj) = nugget.create_with_angle(
                ctx,
                primary_obj,
                primary,
                secondary,
                angle,
                lifetime_frames,
            ) {
                if first_object.is_none() {
                    first_object = Some(obj);
                }
            }
        }

        first_object
    }

    /// Internal implementation - create with objects
    /// Matches C++ ObjectCreationList::createInternal (ObjectCreationList.cpp:1539-1551)
    fn create_internal_with_objects(
        &self,
        ctx: &CreationContext<'_>,
        primary: &Object,
        secondary: Option<&Object>,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        let mut first_object: Option<Arc<RwLock<Object>>> = None;

        for nugget in &self.nuggets {
            if let Some(obj) = nugget.create_with_objects(ctx, primary, secondary, lifetime_frames)
            {
                if first_object.is_none() {
                    first_object = Some(obj);
                }
            }
        }

        first_object
    }

    /// Internal implementation - create with owner flag
    /// Matches C++ ObjectCreationList::createInternal (ObjectCreationList.cpp:1509-1521)
    fn create_internal_with_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        debug_assert!(
            primary_obj.is_some(),
            "ObjectCreationList expects a non-null primary object for ownership context"
        );
        let mut first_object: Option<Arc<RwLock<Object>>> = None;

        for nugget in &self.nuggets {
            if let Some(obj) = nugget.create_with_owner_flag(
                ctx,
                primary_obj,
                primary,
                secondary,
                create_owner,
                lifetime_frames,
            ) {
                if first_object.is_none() {
                    first_object = Some(obj);
                }
            }
        }

        first_object
    }

    fn create_internal_with_angle_and_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Real,
        create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        debug_assert!(
            primary_obj.is_some(),
            "ObjectCreationList expects a non-null primary object for ownership context"
        );
        let mut first_object: Option<Arc<RwLock<Object>>> = None;

        for nugget in &self.nuggets {
            if let Some(obj) = nugget.create_with_angle_and_owner_flag(
                ctx,
                primary_obj,
                primary,
                secondary,
                angle,
                create_owner,
                lifetime_frames,
            ) {
                if first_object.is_none() {
                    first_object = Some(obj);
                }
            }
        }

        first_object
    }

    /// Get number of nuggets
    pub fn get_nugget_count(&self) -> usize {
        self.nuggets.len()
    }
}

impl Default for ObjectCreationList {
    fn default() -> Self {
        Self::new()
    }
}

/// ObjectCreationListStore - manages all OCL definitions
/// Matches C++ class ObjectCreationListStore (ObjectCreationList.h:163-194)
///
/// The store:
/// - Holds all OCL definitions loaded from INI
/// - Owns all nuggets (OCLs just reference them)
/// - Provides lookup by name
/// - Is a singleton (TheObjectCreationListStore)
pub struct ObjectCreationListStore {
    /// Map of OCL name to OCL definition
    ocls: HashMap<NameKeyType, Arc<ObjectCreationList>>,

    /// All nuggets owned by the store
    /// OCLs hold Arc references to these
    nuggets: Vec<Arc<dyn ObjectCreationNugget>>,
}

impl ObjectCreationListStore {
    /// Create new empty store
    /// Matches C++ ObjectCreationListStore::ObjectCreationListStore (ObjectCreationList.cpp:1558-1560)
    pub fn new() -> Self {
        Self {
            ocls: HashMap::new(),
            nuggets: Vec::new(),
        }
    }

    /// Find OCL by name
    /// Matches C++ ObjectCreationListStore::findObjectCreationList (ObjectCreationList.cpp:1574-1588)
    ///
    /// Returns None if "None" or not found
    pub fn find_object_creation_list(&self, name: &str) -> Option<Arc<ObjectCreationList>> {
        if name.eq_ignore_ascii_case("None") {
            return None;
        }

        let key = NameKeyGenerator::name_to_key(name);
        self.ocls.get(&key).cloned()
    }

    /// Add a nugget to the store
    /// Matches C++ ObjectCreationListStore::addObjectCreationNugget (ObjectCreationList.cpp:1591-1594)
    pub fn add_nugget(&mut self, nugget: Arc<dyn ObjectCreationNugget>) {
        self.nuggets.push(nugget);
    }

    /// Register an OCL by name
    /// Used during INI parsing
    pub fn register_ocl(
        &mut self,
        name: String,
        ocl: ObjectCreationList,
    ) -> Arc<ObjectCreationList> {
        let key = NameKeyGenerator::name_to_key(name.as_str());
        let handle = Arc::new(ocl);
        self.ocls.insert(key, handle.clone());
        handle
    }

    /// Get mutable reference to OCL (for building during parse)
    pub fn get_ocl_mut(&mut self, name: &str) -> Option<&mut ObjectCreationList> {
        let key = NameKeyGenerator::name_to_key(name);
        self.ocls.get_mut(&key).map(Arc::make_mut)
    }

    /// Get or create OCL by name
    pub fn get_or_create_ocl(&mut self, name: String) -> &mut ObjectCreationList {
        let key = NameKeyGenerator::name_to_key(name.as_str());
        let entry = self
            .ocls
            .entry(key)
            .or_insert_with(|| Arc::new(ObjectCreationList::new()));
        Arc::make_mut(entry)
    }

    /// Get number of registered OCLs
    pub fn get_ocl_count(&self) -> usize {
        self.ocls.len()
    }

    /// Get number of nuggets
    pub fn get_nugget_count(&self) -> usize {
        self.nuggets.len()
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.ocls.clear();
        self.nuggets.clear();
    }
}

impl Default for ObjectCreationListStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton store
/// Matches C++ extern ObjectCreationListStore *TheObjectCreationListStore (ObjectCreationList.h:197)
static GLOBAL_STORE: RwLock<Option<ObjectCreationListStore>> = RwLock::new(None);

/// Initialize the global store
pub fn init_object_creation_list_store() {
    let mut store = GLOBAL_STORE.write().unwrap();
    *store = Some(ObjectCreationListStore::new());
}

/// Get reference to global store
pub fn get_object_creation_list_store(
) -> std::sync::RwLockReadGuard<'static, Option<ObjectCreationListStore>> {
    GLOBAL_STORE.read().unwrap()
}

/// Get mutable reference to global store
pub fn get_object_creation_list_store_mut(
) -> std::sync::RwLockWriteGuard<'static, Option<ObjectCreationListStore>> {
    GLOBAL_STORE.write().unwrap()
}

fn parse_bool_token(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_coord3d_value(value: &str) -> Coord3D {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut z = 0.0;
    let mut saw_axis = false;

    for token in value.split_whitespace() {
        if let Some((axis, raw)) = token.split_once(':') {
            saw_axis = true;
            if let Ok(parsed) = raw.parse::<f32>() {
                match axis.to_ascii_uppercase().as_str() {
                    "X" => x = parsed,
                    "Y" => y = parsed,
                    "Z" => z = parsed,
                    _ => {}
                }
            }
        }
    }

    if !saw_axis {
        let numeric: Vec<f32> = value
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<f32>().ok())
            .collect();
        if numeric.len() >= 3 {
            x = numeric[0];
            y = numeric[1];
            z = numeric[2];
        }
    }

    Coord3D::new(x, y, z)
}

fn parse_disposition(value: &str) -> DebrisDisposition {
    let mut disposition = DebrisDisposition::new(0);
    for token in value.split_whitespace() {
        match token.to_ascii_uppercase().as_str() {
            "LIKE_EXISTING" => disposition.set(DebrisDisposition::LIKE_EXISTING),
            "ON_GROUND_ALIGNED" => disposition.set(DebrisDisposition::ON_GROUND_ALIGNED),
            "SEND_IT_FLYING" => disposition.set(DebrisDisposition::SEND_IT_FLYING),
            "SEND_IT_UP" => disposition.set(DebrisDisposition::SEND_IT_UP),
            "SEND_IT_OUT" => disposition.set(DebrisDisposition::SEND_IT_OUT),
            "RANDOM_FORCE" => disposition.set(DebrisDisposition::RANDOM_FORCE),
            "FLOATING" => disposition.set(DebrisDisposition::FLOATING),
            "INHERIT_VELOCITY" => disposition.set(DebrisDisposition::INHERIT_VELOCITY),
            "WHIRLING" => disposition.set(DebrisDisposition::WHIRLING),
            _ => {}
        }
    }

    if !disposition.has(
        DebrisDisposition::LIKE_EXISTING
            | DebrisDisposition::ON_GROUND_ALIGNED
            | DebrisDisposition::SEND_IT_FLYING
            | DebrisDisposition::SEND_IT_UP
            | DebrisDisposition::SEND_IT_OUT
            | DebrisDisposition::RANDOM_FORCE
            | DebrisDisposition::FLOATING
            | DebrisDisposition::INHERIT_VELOCITY
            | DebrisDisposition::WHIRLING,
    ) {
        DebrisDisposition::new(DebrisDisposition::ON_GROUND_ALIGNED)
    } else {
        disposition
    }
}

fn parse_name_list(value: &str) -> Vec<String> {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| part.trim().to_string())
        .collect()
}

fn apply_nugget_property(nugget: &mut GenericObjectCreationNugget, key: &str, value: &str) {
    match key.to_ascii_uppercase().as_str() {
        "OBJECTNAMES" | "MODELNAMES" => {
            let names = parse_name_list(value);
            if !names.is_empty() {
                nugget.names = names;
            }
        }
        "COUNT" => {
            if let Ok(count) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<i32>()
            {
                nugget.debris_to_generate = count.max(1);
            }
        }
        "OFFSET" => nugget.offset = parse_coord3d_value(value),
        "DISPOSITION" => nugget.disposition = parse_disposition(value),
        "DISPOSITIONINTENSITY" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.disposition_intensity = parsed;
            }
        }
        "MASS" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.mass = parsed;
            }
        }
        "MINFORCEMAGNITUDE" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.min_mag = parsed;
            }
        }
        "MAXFORCEMAGNITUDE" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.max_mag = parsed;
            }
        }
        "MINFORCEPITCH" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.min_pitch = parsed;
            }
        }
        "MAXFORCEPITCH" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.max_pitch = parsed;
            }
        }
        "YAWRATE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.yaw_rate = parsed;
            }
        }
        "ROLLRATE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.roll_rate = parsed;
            }
        }
        "PITCHRATE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.pitch_rate = parsed;
            }
        }
        "SPINRATE" => {
            if let Ok(parsed) = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .parse::<f32>()
            {
                nugget.spin_rate = parsed;
            }
        }
        "MINLIFETIME" => {
            if let Some(frames) = parse_duration_to_frames(value) {
                nugget.min_frames = frames;
            }
        }
        "MAXLIFETIME" => {
            if let Some(frames) = parse_duration_to_frames(value) {
                nugget.max_frames = frames;
            }
        }
        "PUTINCONTAINER" => nugget.put_in_container = value.trim().to_string(),
        "IGNOREPRIMARYOBSTACLE" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.ignore_primary_obstacle = parsed;
            }
        }
        "ORIENTINFORCEDIRECTION" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.orient_in_force_direction = parsed;
            }
        }
        "EXTRABOUNCINESS" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.extra_bounciness = parsed;
            }
        }
        "EXTRAFRICTION" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.extra_friction = parsed;
            }
        }
        "SPREADFORMATION" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.spread_formation = parsed;
            }
        }
        "MINDISTANCEAFORMATION" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.min_distance_a_formation = parsed;
            }
        }
        "MINDISTANCEBFORMATION" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.min_distance_b_formation = parsed;
            }
        }
        "MAXDISTANCEFORMATION" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.max_distance_formation = parsed;
            }
        }
        "FADEIN" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.fade_in = parsed;
            }
        }
        "FADEOUT" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.fade_out = parsed;
            }
        }
        "FADETIME" => {
            if let Some(frames) = parse_duration_to_frames(value) {
                nugget.fade_frames = frames;
            }
        }
        "FADESOUND" => nugget.fade_sound_name = value.trim().to_string(),
        "DIESONBADLAND" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.dies_on_bad_land = parsed;
            }
        }
        "CONTAININSIDESOURCEOBJECT" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.contain_inside_source_object = parsed;
            }
        }
        "INHERITSVETERANCY" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.inherit_veterancy = parsed;
            }
        }
        "SKIPIFSIGNIFICANTLYAIRBORNE" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.skip_if_significantly_airborne = parsed;
            }
        }
        "INVULNERABLETIME" => {
            if let Some(frames) = parse_duration_to_frames(value) {
                nugget.invulnerable_time = frames;
            }
        }
        "MINHEALTH" => {
            if let Some(parsed) = parse_first_percent(value) {
                nugget.min_health = parsed;
            }
        }
        "MAXHEALTH" => {
            if let Some(parsed) = parse_first_percent(value) {
                nugget.max_health = parsed;
            }
        }
        "REQUIRESLIVEPLAYER" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.requires_live_player = parsed;
            }
        }
        "PRESERVELAYER" => {
            if let Some(parsed) = parse_bool_token(value.split_whitespace().next().unwrap_or("")) {
                nugget.preserve_layer = parsed;
            }
        }
        "ANIMATIONSET" => {
            let mut parts = value.split_whitespace();
            let initial = parts.next().unwrap_or("").to_string();
            let flying = parts.next().unwrap_or("").to_string();
            let final_anim = parts.next().unwrap_or("").to_string();
            if !initial.is_empty() || !flying.is_empty() || !final_anim.is_empty() {
                nugget.anim_sets.push(super::nuggets::AnimSet {
                    anim_initial: initial,
                    anim_flying: flying,
                    anim_final: final_anim,
                });
            }
        }
        "FXFINAL" => {
            let fx_name = value.trim();
            nugget.fx_final = if fx_name.is_empty() {
                None
            } else {
                Some(fx_name.to_string())
            };
        }
        "OKTOCHANGEMODELCOLOR" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.ok_to_change_model_color = parsed;
            }
        }
        "MINLODREQUIRED" => match value.trim().to_ascii_uppercase().as_str() {
            "LOW" | "STATIC_GAME_LOD_LOW" => {
                nugget.min_lod_required = super::nuggets::StaticGameLODLevel::Low
            }
            "MEDIUM" | "STATIC_GAME_LOD_MEDIUM" => {
                nugget.min_lod_required = super::nuggets::StaticGameLODLevel::Medium
            }
            "HIGH" | "STATIC_GAME_LOD_HIGH" => {
                nugget.min_lod_required = super::nuggets::StaticGameLODLevel::High
            }
            _ => {}
        },
        "SHADOW" => {
            let upper = value.to_ascii_uppercase();
            nugget.shadow_type = if upper.contains("SHADOW_VOLUME") {
                super::nuggets::ShadowType::Volume
            } else if upper.contains("SHADOW_ADDITIVE_DECAL")
                || upper.contains("SHADOW_ALPHA_DECAL")
                || upper.contains("SHADOW_DECAL")
            {
                super::nuggets::ShadowType::Additive
            } else {
                super::nuggets::ShadowType::None
            };
        }
        "BOUNCESOUND" => nugget.bounce_sound = value.trim().to_string(),
        "PARTICLESYSTEM" => {
            nugget.particle_sys_name = value.trim().to_string();
        }
        _ => {}
    }
}

fn parse_first_real(value: &str) -> Option<f32> {
    value
        .split_whitespace()
        .next()
        .and_then(|token| token.parse::<f32>().ok())
}

fn parse_first_int(value: &str) -> Option<i32> {
    value
        .split_whitespace()
        .next()
        .and_then(|token| token.parse::<i32>().ok())
}

fn parse_first_percent(value: &str) -> Option<f32> {
    let token = value.split_whitespace().next()?;
    if let Some(raw) = token.strip_suffix('%') {
        return raw.parse::<f32>().ok().map(|v| (v * 0.01).clamp(0.0, 1.0));
    }
    token.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

fn parse_duration_to_frames(value: &str) -> Option<u32> {
    let token = value.split_whitespace().next()?;
    let lowered = token.to_ascii_lowercase();
    if let Some(raw) = lowered.strip_suffix("frames") {
        return raw.parse::<f32>().ok().map(|v| v.round().max(0.0) as u32);
    }
    if let Some(raw) = lowered.strip_suffix("frame") {
        return raw.parse::<f32>().ok().map(|v| v.round().max(0.0) as u32);
    }
    if let Some(raw) = lowered.strip_suffix('f') {
        return raw.parse::<f32>().ok().map(|v| v.round().max(0.0) as u32);
    }
    if let Some(raw) = lowered.strip_suffix("ms") {
        let ms: f32 = raw.parse().ok()?;
        return Some(((ms * 30.0) / 1000.0).round().max(0.0) as u32);
    }
    if let Some(raw) = lowered.strip_suffix('s') {
        let secs: f32 = raw.parse().ok()?;
        return Some((secs * 30.0).round().max(0.0) as u32);
    }

    lowered
        .parse::<f32>()
        .ok()
        .map(|ms| ((ms * 30.0) / 1000.0).round().max(0.0) as u32)
}

fn parse_color_rgba(value: &str) -> Option<u32> {
    if let Ok(raw) = value.trim().parse::<u32>() {
        return Some(raw);
    }

    let mut r = 255u32;
    let mut g = 255u32;
    let mut b = 255u32;
    let mut a = 255u32;
    for token in value.split_whitespace() {
        if let Some((k, v)) = token.split_once(':') {
            if let Ok(parsed) = v.parse::<u32>() {
                match k.to_ascii_uppercase().as_str() {
                    "R" => r = parsed.min(255),
                    "G" => g = parsed.min(255),
                    "B" => b = parsed.min(255),
                    "A" => a = parsed.min(255),
                    _ => {}
                }
            }
        }
    }

    Some((a << 24) | (b << 16) | (g << 8) | r)
}

fn parse_shadow_style(value: &str) -> u32 {
    let mut flags = 0u32;
    for token in value.split_whitespace() {
        match token.to_ascii_uppercase().as_str() {
            "SHADOW_DECAL" => flags |= SHADOW_DECAL,
            "SHADOW_VOLUME" => flags |= SHADOW_VOLUME,
            "SHADOW_PROJECTION" => flags |= SHADOW_PROJECTION,
            "SHADOW_DYNAMIC_PROJECTION" => flags |= SHADOW_DYNAMIC_PROJECTION,
            "SHADOW_DIRECTIONAL_PROJECTION" => flags |= SHADOW_DIRECTIONAL_PROJECTION,
            "SHADOW_ALPHA_DECAL" => flags |= SHADOW_ALPHA_DECAL,
            "SHADOW_ADDITIVE_DECAL" => flags |= SHADOW_ADDITIVE_DECAL,
            _ => {}
        }
    }

    if flags == 0 {
        SHADOW_ALPHA_DECAL
    } else {
        flags
    }
}

fn parse_attack_weapon_slot(value: &str) -> Option<crate::weapon::WeaponSlotType> {
    match value.trim().to_ascii_uppercase().as_str() {
        "PRIMARY" | "PRIMARY_WEAPON" => Some(crate::weapon::WeaponSlotType::Primary),
        "SECONDARY" | "SECONDARY_WEAPON" => Some(crate::weapon::WeaponSlotType::Secondary),
        "TERTIARY" | "TERTIARY_WEAPON" => Some(crate::weapon::WeaponSlotType::Tertiary),
        _ => None,
    }
}

fn parse_delivery_weapon_slot(value: &str) -> Option<WeaponSlotType> {
    match value.trim().to_ascii_uppercase().as_str() {
        "PRIMARY" | "PRIMARY_WEAPON" => Some(WeaponSlotType::Primary),
        "SECONDARY" | "SECONDARY_WEAPON" => Some(WeaponSlotType::Secondary),
        "TERTIARY" | "TERTIARY_WEAPON" => Some(WeaponSlotType::Tertiary),
        "NONE" | "-1" => None,
        _ => None,
    }
}

fn parse_payload(value: &str) -> Option<Payload> {
    let mut parts = value.split_whitespace();
    let name = parts.next()?.to_string();
    let count = parts
        .next()
        .and_then(|token| token.parse::<i32>().ok())
        .unwrap_or(1);
    Some(Payload {
        payload_name: name,
        payload_count: count.max(1),
    })
}

fn apply_delivery_property(
    nugget: &mut DeliverPayloadNugget,
    key: &str,
    value: &str,
    in_delivery_decal: bool,
) {
    if in_delivery_decal {
        match key.to_ascii_uppercase().as_str() {
            "TEXTURE" => {
                nugget.data.delivery_decal_template.texture_name = AsciiString::from(value)
            }
            "STYLE" => nugget.data.delivery_decal_template.shadow_type = parse_shadow_style(value),
            "OPACITYMIN" => {
                if let Some(parsed) = parse_first_real(value) {
                    nugget.data.delivery_decal_template.min_opacity = parsed * 0.01;
                    nugget.data.delivery_decal_template.opacity =
                        nugget.data.delivery_decal_template.min_opacity;
                }
            }
            "OPACITYMAX" => {
                if let Some(parsed) = parse_first_real(value) {
                    nugget.data.delivery_decal_template.max_opacity = parsed * 0.01;
                    nugget.data.delivery_decal_template.opacity =
                        nugget.data.delivery_decal_template.max_opacity;
                }
            }
            "OPACITYTHROBTIME" => {
                if let Some(frames) = parse_duration_to_frames(value) {
                    nugget.data.delivery_decal_template.opacity_throb_time = frames;
                }
            }
            "COLOR" => {
                if let Some(color) = parse_color_rgba(value) {
                    nugget.data.delivery_decal_template.color = color;
                }
            }
            "ONLYVISIBLETOOWNINGPLAYER" => {
                if let Some(parsed) = parse_bool_token(value) {
                    nugget
                        .data
                        .delivery_decal_template
                        .only_visible_to_owning_player = parsed;
                }
            }
            _ => {}
        }
        return;
    }

    match key.to_ascii_uppercase().as_str() {
        "TRANSPORT" => nugget.transport_name = value.trim().to_string(),
        "STARTATPREFERREDHEIGHT" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.start_at_preferred_height = parsed;
            }
        }
        "STARTATMAXSPEED" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.start_at_max_speed = parsed;
            }
        }
        "FORMATIONSIZE" => {
            if let Some(parsed) = parse_first_int(value) {
                nugget.formation_size = parsed.max(1) as u32;
            }
        }
        "FORMATIONSPACING" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.formation_spacing = parsed;
            }
        }
        "WEAPONCONVERGENCEFACTOR" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.convergence_factor = parsed;
            }
        }
        "WEAPONERRORRADIUS" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.error_radius = parsed;
            }
        }
        "DELAYDELIVERYMAX" => {
            if let Some(frames) = parse_duration_to_frames(value) {
                nugget.delay_delivery_frames_max = frames;
            }
        }
        "PAYLOAD" => {
            if let Some(payload) = parse_payload(value) {
                nugget.payload.push(payload);
            }
        }
        "PUTINCONTAINER" => nugget.put_in_container_name = value.trim().to_string(),
        "DELIVERYDISTANCE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.dist_to_target = parsed;
            }
        }
        "PREOPENDISTANCE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.pre_open_distance = parsed;
            }
        }
        "MAXATTEMPTS" => {
            if let Some(parsed) = parse_first_int(value) {
                nugget.data.max_attempts = parsed;
            }
        }
        "DROPOFFSET" => nugget.data.drop_offset = parse_coord3d_value(value),
        "DROPVARIANCE" => nugget.data.drop_variance = parse_coord3d_value(value),
        "DROPDELAY" => {
            if let Some(frames) = parse_duration_to_frames(value) {
                nugget.data.drop_delay = frames;
            }
        }
        "FIREWEAPON" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.data.fire_weapon = parsed;
            }
        }
        "SELFDESTRUCTOBJECT" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.data.self_destruct_object = parsed;
            }
        }
        "VISIBLENUMBONES" => {
            if let Some(parsed) = parse_first_int(value) {
                nugget.data.visible_num_bones = parsed;
            }
        }
        "VISIBLEDROPBONEBASENAME" => {
            nugget.data.visible_drop_bone_name = AsciiString::from(value.trim())
        }
        "VISIBLESUBOBJECTBASENAME" => {
            nugget.data.visible_sub_object_name = AsciiString::from(value.trim())
        }
        "VISIBLEPAYLOADTEMPLATENAME" => {
            nugget.data.visible_payload_template_name = AsciiString::from(value.trim())
        }
        "VISIBLEITEMSDROPPEDPERINTERVAL" => {
            if let Some(parsed) = parse_first_int(value) {
                nugget.data.visible_items_dropped_per_interval = parsed;
            }
        }
        "INHERITTRANSPORTVELOCITY" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.data.inherit_transport_velocity = parsed;
            }
        }
        "PARACHUTEDIRECTLY" => {
            if let Some(parsed) = parse_bool_token(value) {
                nugget.data.is_parachute_directly = parsed;
            }
        }
        "EXITPITCHRATE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.exit_pitch_rate = parsed;
            }
        }
        "STRAFELENGTH" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.strafe_length = parsed;
            }
        }
        "DIVESTARTDISTANCE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.dive_start_distance = parsed;
            }
        }
        "DIVEENDDISTANCE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.dive_end_distance = parsed;
            }
        }
        "STRAFINGWEAPONSLOT" => {
            nugget.data.strafing_weapon_slot = parse_delivery_weapon_slot(value)
        }
        "STRAFEWEAPONFX" => {
            nugget.data.strafe_fx = crate::helpers::TheFXListStore::find_fx_list(value.trim())
        }
        "VISIBLEPAYLOADWEAPONTEMPLATE" => {
            let weapon_name = value.trim();
            if weapon_name.eq_ignore_ascii_case("NONE") || weapon_name.is_empty() {
                nugget.data.visible_payload_weapon_template = None;
            } else {
                nugget.data.visible_payload_weapon_template =
                    crate::weapon::with_weapon_store(|store| {
                        store.find_weapon_template(weapon_name).cloned()
                    })
                    .ok()
                    .flatten();
            }
        }
        "DELIVERYDECALRADIUS" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.data.delivery_decal_radius = parsed;
            }
        }
        _ => {}
    }
}

fn apply_attack_property(
    nugget: &mut AttackNugget,
    key: &str,
    value: &str,
    in_delivery_decal: bool,
) {
    if in_delivery_decal {
        match key.to_ascii_uppercase().as_str() {
            "TEXTURE" => nugget.delivery_decal_template.texture_name = AsciiString::from(value),
            "STYLE" => nugget.delivery_decal_template.shadow_type = parse_shadow_style(value),
            "OPACITYMIN" => {
                if let Some(parsed) = parse_first_real(value) {
                    nugget.delivery_decal_template.min_opacity = parsed * 0.01;
                    nugget.delivery_decal_template.opacity =
                        nugget.delivery_decal_template.min_opacity;
                }
            }
            "OPACITYMAX" => {
                if let Some(parsed) = parse_first_real(value) {
                    nugget.delivery_decal_template.max_opacity = parsed * 0.01;
                    nugget.delivery_decal_template.opacity =
                        nugget.delivery_decal_template.max_opacity;
                }
            }
            "OPACITYTHROBTIME" => {
                if let Some(frames) = parse_duration_to_frames(value) {
                    nugget.delivery_decal_template.opacity_throb_time = frames;
                }
            }
            "COLOR" => {
                if let Some(color) = parse_color_rgba(value) {
                    nugget.delivery_decal_template.color = color;
                }
            }
            "ONLYVISIBLETOOWNINGPLAYER" => {
                if let Some(parsed) = parse_bool_token(value) {
                    nugget.delivery_decal_template.only_visible_to_owning_player = parsed;
                }
            }
            _ => {}
        }
        return;
    }

    match key.to_ascii_uppercase().as_str() {
        "NUMBEROFSHOTS" => {
            if let Some(parsed) = parse_first_int(value) {
                nugget.number_of_shots = parsed;
            }
        }
        "WEAPONSLOT" => {
            if let Some(slot) = parse_attack_weapon_slot(value) {
                nugget.weapon_slot = slot;
            }
        }
        "DELIVERYDECALRADIUS" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.delivery_decal_radius = parsed;
            }
        }
        _ => {}
    }
}

fn apply_force_property(nugget: &mut ApplyRandomForceNugget, key: &str, value: &str) {
    match key.to_ascii_uppercase().as_str() {
        "SPINRATE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.spin_rate = parsed;
            }
        }
        "MINFORCEMAGNITUDE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.min_mag = parsed;
            }
        }
        "MAXFORCEMAGNITUDE" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.max_mag = parsed;
            }
        }
        "MINFORCEPITCH" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.min_pitch = parsed;
            }
        }
        "MAXFORCEPITCH" => {
            if let Some(parsed) = parse_first_real(value) {
                nugget.max_pitch = parsed;
            }
        }
        _ => {}
    }
}

fn apply_fire_weapon_property(nugget: &mut FireWeaponNugget, key: &str, value: &str) {
    if key.eq_ignore_ascii_case("Weapon") {
        let weapon_name = value.trim();
        nugget.weapon = if weapon_name.is_empty() {
            None
        } else {
            Some(weapon_name.to_string())
        };
    }
}

enum NuggetBuilder {
    Generic(GenericObjectCreationNugget),
    DeliverPayload(DeliverPayloadNugget),
    FireWeapon(FireWeaponNugget),
    Attack(AttackNugget),
    ApplyRandomForce(ApplyRandomForceNugget),
}

impl NuggetBuilder {
    fn into_nugget(self) -> Option<Arc<dyn ObjectCreationNugget>> {
        match self {
            NuggetBuilder::Generic(nugget) => {
                if nugget.names.is_empty() {
                    None
                } else {
                    Some(Arc::new(nugget))
                }
            }
            NuggetBuilder::DeliverPayload(nugget) => {
                if nugget.transport_name.is_empty() {
                    None
                } else {
                    Some(Arc::new(nugget))
                }
            }
            NuggetBuilder::FireWeapon(nugget) => {
                if nugget.weapon.is_none() {
                    None
                } else {
                    Some(Arc::new(nugget))
                }
            }
            NuggetBuilder::Attack(nugget) => Some(Arc::new(nugget)),
            NuggetBuilder::ApplyRandomForce(nugget) => Some(Arc::new(nugget)),
        }
    }
}

pub fn load_object_creation_lists_from_str(content: &str) -> Result<usize, String> {
    let mut parsed: Vec<(String, ObjectCreationList)> = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_ocl: Option<ObjectCreationList> = None;
    let mut current_nugget: Option<NuggetBuilder> = None;
    let mut in_delivery_decal = false;

    for (idx, raw_line) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let mut words = line.split_whitespace();
        if let Some(keyword) = words.next() {
            if keyword.eq_ignore_ascii_case("ObjectCreationList") {
                if current_name.is_some() || current_nugget.is_some() || in_delivery_decal {
                    return Err(format!(
                        "Nested ObjectCreationList block at line {}",
                        line_no
                    ));
                }
                let name = words.collect::<Vec<_>>().join(" ");
                if name.trim().is_empty() {
                    return Err(format!(
                        "ObjectCreationList missing name at line {}",
                        line_no
                    ));
                }
                current_name = Some(name.trim().to_string());
                current_ocl = Some(ObjectCreationList::new());
                continue;
            }
        }

        if line.eq_ignore_ascii_case("End") {
            if in_delivery_decal {
                in_delivery_decal = false;
                continue;
            }

            if let Some(builder) = current_nugget.take() {
                if let Some(nugget) = builder.into_nugget() {
                    if let Some(ocl) = current_ocl.as_mut() {
                        ocl.add_nugget(nugget);
                    }
                }
                continue;
            }

            if let (Some(name), Some(ocl)) = (current_name.take(), current_ocl.take()) {
                parsed.push((name, ocl));
                continue;
            }

            return Err(format!("Unexpected End at line {}", line_no));
        }

        if current_ocl.is_none() {
            continue;
        }

        if current_nugget.is_none() {
            if line.eq_ignore_ascii_case("CreateObject") {
                let mut nugget = GenericObjectCreationNugget::default();
                nugget.name_are_objects = true;
                current_nugget = Some(NuggetBuilder::Generic(nugget));
                continue;
            }
            if line.eq_ignore_ascii_case("CreateDebris") {
                let mut nugget = GenericObjectCreationNugget::default();
                nugget.name_are_objects = false;
                current_nugget = Some(NuggetBuilder::Generic(nugget));
                continue;
            }
            if line.eq_ignore_ascii_case("DeliverPayload") {
                current_nugget = Some(NuggetBuilder::DeliverPayload(
                    DeliverPayloadNugget::default(),
                ));
                continue;
            }
            if line.eq_ignore_ascii_case("FireWeapon") {
                current_nugget = Some(NuggetBuilder::FireWeapon(FireWeaponNugget::default()));
                continue;
            }
            if line.eq_ignore_ascii_case("Attack") {
                current_nugget = Some(NuggetBuilder::Attack(AttackNugget::default()));
                continue;
            }
            if line.eq_ignore_ascii_case("ApplyRandomForce") {
                current_nugget = Some(NuggetBuilder::ApplyRandomForce(
                    ApplyRandomForceNugget::default(),
                ));
                continue;
            }

            continue;
        }

        if line.eq_ignore_ascii_case("DeliveryDecal") {
            if matches!(
                current_nugget,
                Some(NuggetBuilder::DeliverPayload(_)) | Some(NuggetBuilder::Attack(_))
            ) {
                in_delivery_decal = true;
            }
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            if let Some(builder) = current_nugget.as_mut() {
                match builder {
                    NuggetBuilder::Generic(nugget) => apply_nugget_property(nugget, key, value),
                    NuggetBuilder::DeliverPayload(nugget) => {
                        apply_delivery_property(nugget, key, value, in_delivery_decal)
                    }
                    NuggetBuilder::FireWeapon(nugget) => {
                        apply_fire_weapon_property(nugget, key, value)
                    }
                    NuggetBuilder::Attack(nugget) => {
                        apply_attack_property(nugget, key, value, in_delivery_decal)
                    }
                    NuggetBuilder::ApplyRandomForce(nugget) => {
                        apply_force_property(nugget, key, value)
                    }
                }
            }
            continue;
        }
    }

    if in_delivery_decal {
        return Err("Unterminated DeliveryDecal block".to_string());
    }
    if current_nugget.is_some() {
        return Err("Unterminated OCL nugget block".to_string());
    }
    if let Some(name) = current_name {
        return Err(format!("Unterminated ObjectCreationList block '{}'", name));
    }

    if parsed.is_empty() {
        return Ok(0);
    }

    let count_loaded = parsed.len();

    if get_object_creation_list_store().as_ref().is_none() {
        init_object_creation_list_store();
    }
    let mut store_guard = get_object_creation_list_store_mut();
    let store = store_guard
        .as_mut()
        .ok_or_else(|| "ObjectCreationListStore not initialized".to_string())?;

    for (name, ocl) in parsed {
        let nugget_clones: Vec<Arc<dyn ObjectCreationNugget>> = ocl.nuggets.to_vec();
        {
            let target_ocl = store.get_or_create_ocl(name);
            target_ocl.clear();
            for nugget in &nugget_clones {
                target_ocl.add_nugget(Arc::clone(nugget));
            }
        }
        for nugget in nugget_clones {
            store.nuggets.push(nugget);
        }
    }

    Ok(count_loaded)
}

pub fn load_object_creation_lists_from_path<P: AsRef<Path>>(path: P) -> Result<usize, String> {
    let content =
        fs::read_to_string(&path).map_err(|err| format!("Failed to read OCL file: {err}"))?;
    load_object_creation_lists_from_str(&content)
}

fn default_ocl_paths() -> [PathBuf; 3] {
    [
        PathBuf::from("Data/INI/ObjectCreationList.ini"),
        PathBuf::from("windows_game/extracted_big_files_v2/INIZH/Data/INI/ObjectCreationList.ini"),
        PathBuf::from("windows_game/extracted_big_files/INIZH/Data/INI/ObjectCreationList.ini"),
    ]
}

pub fn ensure_default_object_creation_lists_loaded() {
    static OCL_STORE_INITIALIZED: OnceLock<()> = OnceLock::new();
    OCL_STORE_INITIALIZED.get_or_init(|| {
        if get_object_creation_list_store()
            .as_ref()
            .map(|store| store.get_ocl_count() > 0)
            .unwrap_or(false)
        {
            return;
        }

        for path in default_ocl_paths() {
            if !path.exists() {
                continue;
            }

            match load_object_creation_lists_from_path(&path) {
                Ok(count) if count > 0 => {
                    log::info!(
                        "Loaded {} ObjectCreationList definitions from {}",
                        count,
                        path.display()
                    );
                    return;
                }
                Ok(_) => {}
                Err(err) => {
                    log::warn!(
                        "Failed to load OCL definitions from {}: {}",
                        path.display(),
                        err
                    );
                }
            }
        }

        log::warn!("ObjectCreationList definitions could not be loaded from default paths");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object_creation_list::nuggets::GenericObjectCreationNugget;
    use std::path::PathBuf;

    #[test]
    fn test_ocl_creation() {
        let mut ocl = ObjectCreationList::new();
        assert_eq!(ocl.get_nugget_count(), 0);

        let nugget = Arc::new(GenericObjectCreationNugget::default());
        ocl.add_nugget(nugget);
        assert_eq!(ocl.get_nugget_count(), 1);
    }

    #[test]
    fn test_ocl_clear() {
        let mut ocl = ObjectCreationList::new();
        let nugget = Arc::new(GenericObjectCreationNugget::default());
        ocl.add_nugget(nugget);
        assert_eq!(ocl.get_nugget_count(), 1);

        ocl.clear();
        assert_eq!(ocl.get_nugget_count(), 0);
    }

    #[test]
    fn test_store_basic() {
        let mut store = ObjectCreationListStore::new();
        assert_eq!(store.get_ocl_count(), 0);

        let ocl = ObjectCreationList::new();
        store.register_ocl("TestOCL".to_string(), ocl);
        assert_eq!(store.get_ocl_count(), 1);

        assert!(store.find_object_creation_list("TestOCL").is_some());
        assert!(store.find_object_creation_list("None").is_none());
        assert!(store.find_object_creation_list("NotFound").is_none());
    }

    #[test]
    fn test_store_add_nugget() {
        let mut store = ObjectCreationListStore::new();
        let nugget = Arc::new(GenericObjectCreationNugget::default());

        store.add_nugget(nugget);
        assert_eq!(store.get_nugget_count(), 1);
    }

    #[test]
    fn test_global_store_init() {
        init_object_creation_list_store();
        let store = get_object_creation_list_store();
        assert!(store.is_some());
    }

    #[test]
    fn test_parse_object_creation_list_from_str() {
        let data = r#"
ObjectCreationList OCL_Test
  CreateObject
    ObjectNames = UnitA UnitB
    Count = 2
    Disposition = ON_GROUND_ALIGNED INHERIT_VELOCITY
  End
End
"#;

        let count = load_object_creation_lists_from_str(data).expect("failed to parse OCL");
        assert_eq!(count, 1);

        let store = get_object_creation_list_store();
        let ocl = store
            .as_ref()
            .and_then(|s| s.find_object_creation_list("OCL_Test"))
            .expect("OCL_Test missing after parse");
        assert_eq!(ocl.get_nugget_count(), 1);
    }

    #[test]
    fn test_parse_advanced_nuggets_and_nested_delivery_decal() {
        let data = r#"
ObjectCreationList OCL_Advanced
  DeliverPayload
    DeliveryDecal
      Texture = SCCNukeLaunchDecal
      Style = SHADOW_ALPHA_DECAL
    End
    Transport = AmericaJetAurora
    Payload = AmericaJetAuroraBomb 1
  End
  FireWeapon
    Weapon = DemoTrapDetonationWeapon
  End
  Attack
    NumberOfShots = 1
    WeaponSlot = PRIMARY
    DeliveryDecal
      Texture = SCCNukeLaunchDecal
      Style = SHADOW_ALPHA_DECAL
    End
  End
  ApplyRandomForce
    SpinRate = 1.0
    MinForceMagnitude = 10.0
    MaxForceMagnitude = 20.0
  End
End
"#;

        let count =
            load_object_creation_lists_from_str(data).expect("failed to parse advanced OCL");
        assert_eq!(count, 1);

        let store = get_object_creation_list_store();
        let ocl = store
            .as_ref()
            .and_then(|s| s.find_object_creation_list("OCL_Advanced"))
            .expect("OCL_Advanced missing after parse");
        assert_eq!(ocl.get_nugget_count(), 4);
    }

    #[test]
    fn test_parse_stock_object_creation_list_file_when_present() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
        let stock =
            root.join("windows_game/extracted_big_files/INIZH/Data/INI/ObjectCreationList.ini");
        if !stock.exists() {
            return;
        }

        let count = load_object_creation_lists_from_path(&stock)
            .expect("failed to parse stock ObjectCreationList.ini");
        assert!(
            count >= 200,
            "expected at least 200 OCLs from stock data, got {}",
            count
        );
    }

    #[test]
    fn test_incremental_load_keeps_existing_ocls() {
        let a = r#"
ObjectCreationList OCL_A
  CreateObject
    ObjectNames = UnitA
  End
End
"#;
        let b = r#"
ObjectCreationList OCL_B
  CreateObject
    ObjectNames = UnitB
  End
End
"#;

        let count_a = load_object_creation_lists_from_str(a).expect("load A failed");
        assert_eq!(count_a, 1);
        let count_b = load_object_creation_lists_from_str(b).expect("load B failed");
        assert_eq!(count_b, 1);

        let store = get_object_creation_list_store();
        let store = store.as_ref().expect("store missing");
        assert!(store.find_object_creation_list("OCL_A").is_some());
        assert!(store.find_object_creation_list("OCL_B").is_some());
    }

    #[test]
    fn test_reload_same_ocl_replaces_nuggets() {
        let first = r#"
ObjectCreationList OCL_Reload
  CreateObject
    ObjectNames = UnitA
  End
End
"#;
        let second = r#"
ObjectCreationList OCL_Reload
  CreateObject
    ObjectNames = UnitA
  End
  CreateObject
    ObjectNames = UnitB
  End
End
"#;

        load_object_creation_lists_from_str(first).expect("first load failed");
        load_object_creation_lists_from_str(second).expect("second load failed");

        let store = get_object_creation_list_store();
        let ocl = store
            .as_ref()
            .and_then(|s| s.find_object_creation_list("OCL_Reload"))
            .expect("OCL_Reload missing after reload");
        assert_eq!(ocl.get_nugget_count(), 2);
    }

    #[test]
    fn test_duration_parser_matches_ini_duration_semantics() {
        assert_eq!(parse_duration_to_frames("1000"), Some(30));
        assert_eq!(parse_duration_to_frames("1s"), Some(30));
        assert_eq!(parse_duration_to_frames("500ms"), Some(15));
        assert_eq!(parse_duration_to_frames("10frames"), Some(10));
    }

    #[test]
    fn test_decal_color_parser_matches_common_delivery_parser_order() {
        assert_eq!(parse_color_rgba("R:1 G:2 B:3 A:4"), Some(0x0403_0201));
        assert_eq!(parse_color_rgba("16909060"), Some(16909060));
    }

    #[test]
    fn test_helper_ensure_ocl_lookup_does_not_create_placeholder() {
        let missing = "OCL_Missing_ParityTest_20260302_Batch83";
        assert!(
            crate::helpers::TheObjectCreationListStore::find_object_creation_list(missing)
                .is_none()
        );
        assert!(
            crate::helpers::TheObjectCreationListStore::ensure_object_creation_list(missing)
                .is_none()
        );
        assert!(
            crate::helpers::TheObjectCreationListStore::find_object_creation_list(missing)
                .is_none()
        );
    }
}
