/// Animation information for a model
#[derive(Debug, Clone)]
pub struct W3DAnimationInfo {
    /// Name of animation
    pub name: AsciiString,

    /// Distance covered by a single loop (for movement animations)
    pub distance_covered: Real,

    /// Natural duration in milliseconds
    pub natural_duration_ms: Real,

    /// Whether this is an idle animation (picks random anim when complete)
    pub is_idle_anim: bool,
}

impl W3DAnimationInfo {
    pub fn new(name: AsciiString, is_idle: bool, distance_covered: Real) -> Self {
        Self {
            name,
            distance_covered,
            natural_duration_ms: 0.0, // Calculated from animation data
            is_idle_anim: is_idle,
        }
    }
}

/// Animation mode for render objects
///
/// Reference: RenderObjClass::AnimMode in W3DModelDraw.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimMode {
    Manual,        // Manual control
    Loop,          // Loop continuously
    Once,          // Play once
    LoopPingPong,  // Loop with reverse playback
    LoopBackwards, // Loop playing backwards
    OnceBackwards, // Play once backwards
}

/// Particle system attachment to bone
#[derive(Debug, Clone)]
pub struct ParticleSysBoneInfo {
    /// Name of bone to attach to
    pub bone_name: AsciiString,

    /// Particle system template
    pub particle_system: AsciiString, // Reference to particle template
}

#[derive(Debug, Clone, PartialEq)]
struct ParticleSysTracker {
    id: u32,
    bone_index: i32,
    bone_name: AsciiString,
}

/// Pristine bone information (default pose)
#[derive(Debug, Clone)]
pub struct PristineBoneInfo {
    /// Transform matrix in default pose
    pub transform: Matrix3D,

    /// Bone index in skeleton
    pub bone_index: i32,
}

/// Turret information for model condition
#[derive(Debug, Clone)]
pub struct TurretInfo {
    /// Name key for turret angle bone
    pub turret_angle_name_key: NameKeyType,

    /// Name key for turret pitch bone
    pub turret_pitch_name_key: NameKeyType,

    /// Art-defined turret angle offset
    pub turret_art_angle: Real,

    /// Art-defined turret pitch offset
    pub turret_art_pitch: Real,

    /// Calculated bone index for angle
    pub turret_angle_bone: i32,

    /// Calculated bone index for pitch
    pub turret_pitch_bone: i32,
}

impl TurretInfo {
    pub fn new() -> Self {
        Self {
            turret_angle_name_key: 0,
            turret_pitch_name_key: 0,
            turret_art_angle: 0.0,
            turret_art_pitch: 0.0,
            turret_angle_bone: 0,
            turret_pitch_bone: 0,
        }
    }
}

impl Default for TurretInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Weapon barrel information
#[derive(Debug, Clone)]
pub struct WeaponBarrelInfo {
    /// Recoil bone index
    pub recoil_bone: i32,

    /// FX bone index
    pub fx_bone: i32,

    /// Muzzle flash bone index
    pub muzzle_flash_bone: i32,

    /// Projectile launch offset matrix
    pub projectile_offset_mtx: Matrix3D,
}

impl WeaponBarrelInfo {
    pub fn new() -> Self {
        Self {
            recoil_bone: 0,
            fx_bone: 0,
            muzzle_flash_bone: 0,
            projectile_offset_mtx: Matrix3D::IDENTITY,
        }
    }
}

impl Default for WeaponBarrelInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Hide/show sub-object directive
#[derive(Debug, Clone)]
pub struct HideShowSubObjInfo {
    /// Name of sub-object
    pub sub_obj_name: AsciiString,

    /// True to hide, false to show
    pub hide: bool,
}

/// Model condition state information
///
/// Defines which model and animations to use for a given set of model conditions.
///
/// Reference: ModelConditionInfo in W3DModelDraw.h
#[derive(Debug, Clone)]
pub struct ModelConditionInfo {
    /// Condition flags this state matches
    pub conditions_yes: Vec<ModelConditionFlags>,

    /// Model name to use
    pub model_name: AsciiString,

    /// Sub-objects to hide/show
    pub hide_show_list: Vec<HideShowSubObjInfo>,

    /// Public bones (accessible to code)
    pub public_bones: Vec<AsciiString>,

    /// Weapon fire FX bone names
    pub weapon_fire_fx_bone: [AsciiString; WEAPONSLOT_COUNT],

    /// Weapon recoil bone names
    pub weapon_recoil_bone: [AsciiString; WEAPONSLOT_COUNT],

    /// Weapon muzzle flash bone names
    pub weapon_muzzle_flash: [AsciiString; WEAPONSLOT_COUNT],

    /// Weapon projectile launch bone names
    pub weapon_projectile_launch_bone: [AsciiString; WEAPONSLOT_COUNT],

    /// Weapon projectile hide/show bone names.
    pub weapon_projectile_hide_show_bone: [AsciiString; WEAPONSLOT_COUNT],

    /// Animations for this state
    pub animations: Vec<W3DAnimationInfo>,

    /// Transition key
    pub transition_key: NameKeyType,

    /// Allow to finish key
    pub allow_to_finish_key: NameKeyType,

    /// Bit flags from INI `Flags`.
    pub flags: u32,

    /// Parse-time flags used to preserve C++ default-state animation behavior.
    pub ini_read_flags: u32,

    /// Animation mode
    pub anim_mode: AnimMode,

    /// Particle systems attached to bones
    pub particle_sys_bones: Vec<ParticleSysBoneInfo>,

    /// Animation speed randomization (min factor)
    pub anim_min_speed_factor: Real,

    /// Animation speed randomization (max factor)
    pub anim_max_speed_factor: Real,

    /// Transition source condition key.
    pub transition_from_key: NameKeyType,

    /// Transition destination condition key.
    pub transition_to_key: NameKeyType,

    /// Pristine bone transforms
    pub pristine_bones: HashMap<NameKeyType, PristineBoneInfo>,

    /// Turret information (up to MAX_TURRETS)
    pub turrets: Vec<TurretInfo>,

    /// Weapon barrel information per slot
    pub weapon_barrels: Vec<Vec<WeaponBarrelInfo>>,

    /// Runtime validation flags mirroring C++ `m_validStuff`.
    valid_stuff: u8,
}

impl ModelConditionInfo {
    pub fn new() -> Self {
        Self {
            conditions_yes: Vec::new(),
            model_name: AsciiString::new(),
            hide_show_list: Vec::new(),
            public_bones: Vec::new(),
            weapon_fire_fx_bone: [
                AsciiString::default(),
                AsciiString::default(),
                AsciiString::default(),
            ],
            weapon_recoil_bone: [
                AsciiString::default(),
                AsciiString::default(),
                AsciiString::default(),
            ],
            weapon_muzzle_flash: [
                AsciiString::default(),
                AsciiString::default(),
                AsciiString::default(),
            ],
            weapon_projectile_launch_bone: [
                AsciiString::default(),
                AsciiString::default(),
                AsciiString::default(),
            ],
            weapon_projectile_hide_show_bone: [
                AsciiString::default(),
                AsciiString::default(),
                AsciiString::default(),
            ],
            animations: Vec::new(),
            transition_key: 0,
            allow_to_finish_key: 0,
            flags: 0,
            ini_read_flags: 0,
            anim_mode: AnimMode::Once,
            particle_sys_bones: Vec::new(),
            anim_min_speed_factor: 1.0,
            anim_max_speed_factor: 1.0,
            transition_from_key: 0,
            transition_to_key: 0,
            pristine_bones: HashMap::new(),
            turrets: Vec::new(),
            weapon_barrels: vec![Vec::new(); WEAPONSLOT_COUNT],
            valid_stuff: 0,
        }
    }

    fn find_pristine_bone(&self, bone_key: NameKeyType) -> Option<&PristineBoneInfo> {
        if bone_key == NAMEKEY_INVALID {
            return None;
        }
        self.pristine_bones.get(&bone_key)
    }

    fn find_pristine_bone_by_name(
        &self,
        bone_name: &str,
    ) -> Option<(NameKeyType, &PristineBoneInfo)> {
        if bone_name.is_empty() {
            return None;
        }
        let bone_key = name_key_generate(bone_name);
        self.find_pristine_bone(bone_key)
            .map(|info| (bone_key, info))
    }

    fn pristine_bone_index_by_name(&self, bone_name: &str) -> i32 {
        self.find_pristine_bone_by_name(bone_name)
            .map(|(_, info)| info.bone_index)
            .unwrap_or(0)
    }

    /// C++ `ModelConditionInfo::validateCachedBones` (`W3DModelDraw.cpp:566-689`).
    fn validate_cached_bones(&mut self, scale: Real) {
        if (self.valid_stuff & MODEL_CONDITION_PRISTINE_BONES_VALID) != 0 {
            return;
        }
        // Tests / xfer may already have inserted bones without the valid bit.
        // C++ would have set the bit when it produced them; keep that map.
        if !self.pristine_bones.is_empty() {
            self.valid_stuff |= MODEL_CONDITION_PRISTINE_BONES_VALID;
            return;
        }
        self.pristine_bones.clear();
        // C++ sets the valid bit before the model load so a missing asset still
        // unblocks turret/barrel validation (indices stay 0).
        self.valid_stuff |= MODEL_CONDITION_PRISTINE_BONES_VALID;
        if self.model_name.is_empty() || self.model_name.is_none() {
            return;
        }

        let frame = if test_flag_bit(self.flags, ACBIT_PRISTINE_BONE_POS_IN_FINAL_FRAME) {
            PRISTINE_BONE_LAST_FRAME
        } else {
            0
        };
        let model = self.model_name.as_str().to_string();
        let public_bones = self.public_bones.clone();

        {
            let global = game_engine::common::global_data::read();
            for bone in &global.standard_public_bones {
                let _ = do_single_bone_name(
                    &mut self.pristine_bones,
                    &model,
                    scale,
                    frame,
                    bone,
                );
            }
        }
        for bone in &public_bones {
            let _ = do_single_bone_name(
                &mut self.pristine_bones,
                &model,
                scale,
                frame,
                bone.as_str(),
            );
        }
    }

    fn validate_turret_info(&mut self) {

        if (self.valid_stuff & MODEL_CONDITION_TURRETS_VALID) != 0 {
            return;
        }
        // C++ `validateTurretInfo` does not bail when the pristine map is empty
        // after `PRISTINE_BONES_VALID` is set. Missing bones stay index 0.


        let angle_keys: Vec<NameKeyType> = self
            .turrets
            .iter()
            .map(|turret| turret.turret_angle_name_key)
            .collect();
        let pitch_keys: Vec<NameKeyType> = self
            .turrets
            .iter()
            .map(|turret| turret.turret_pitch_name_key)
            .collect();

        let angle_bones: Vec<i32> = angle_keys
            .iter()
            .map(|key| {
                self.find_pristine_bone(*key)
                    .map(|bone| bone.bone_index)
                    .unwrap_or(0)
            })
            .collect();
        let pitch_bones: Vec<i32> = pitch_keys
            .iter()
            .map(|key| {
                self.find_pristine_bone(*key)
                    .map(|bone| bone.bone_index)
                    .unwrap_or(0)
            })
            .collect();

        for (index, turret) in self.turrets.iter_mut().enumerate() {
            turret.turret_angle_bone = angle_bones.get(index).copied().unwrap_or(0);
            turret.turret_pitch_bone = pitch_bones.get(index).copied().unwrap_or(0);
        }
        self.valid_stuff |= MODEL_CONDITION_TURRETS_VALID;
    }

    fn validate_weapon_barrel_info(&mut self) {
        if (self.valid_stuff & MODEL_CONDITION_BARRELS_VALID) != 0 {
            return;
        }
        // C++ still walks authored weapon-bone names after a failed model load.


        let mut validated_barrels = vec![Vec::new(); WEAPONSLOT_COUNT];

        for (slot, barrels) in validated_barrels.iter_mut().enumerate() {
            let fx_bone_name = self.weapon_fire_fx_bone[slot].as_str();
            let recoil_bone_name = self.weapon_recoil_bone[slot].as_str();
            let muzzle_flash_name = self.weapon_muzzle_flash[slot].as_str();
            let projectile_launch_name = self.weapon_projectile_launch_bone[slot].as_str();

            if fx_bone_name.is_empty()
                && recoil_bone_name.is_empty()
                && muzzle_flash_name.is_empty()
                && projectile_launch_name.is_empty()
            {
                continue;
            }

            let mut prev_fx_bone = 0;
            for index in 1..=99 {
                let mut info = WeaponBarrelInfo::new();

                if !recoil_bone_name.is_empty() {
                    let name = format!("{recoil_bone_name}{index:02}");
                    info.recoil_bone = self.pristine_bone_index_by_name(&name);
                }

                if !muzzle_flash_name.is_empty() {
                    let name = format!("{muzzle_flash_name}{index:02}");
                    info.muzzle_flash_bone = self.pristine_bone_index_by_name(&name);
                }

                if !fx_bone_name.is_empty() {
                    let name = format!("{fx_bone_name}{index:02}");
                    info.fx_bone = self.pristine_bone_index_by_name(&name);
                    if info.fx_bone == 0 && info.muzzle_flash_bone != 0 {
                        info.fx_bone = prev_fx_bone;
                    }
                }

                let mut projectile_launch_bone = 0;
                if !projectile_launch_name.is_empty() {
                    let name = format!("{projectile_launch_name}{index:02}");
                    if let Some((_, bone)) = self.find_pristine_bone_by_name(&name) {
                        projectile_launch_bone = bone.bone_index;
                        info.projectile_offset_mtx = bone.transform;
                    }
                }

                if info.fx_bone == 0
                    && info.recoil_bone == 0
                    && info.muzzle_flash_bone == 0
                    && projectile_launch_bone == 0
                {
                    break;
                }

                prev_fx_bone = info.fx_bone;
                barrels.push(info);
            }

            if barrels.is_empty() {
                let mut info = WeaponBarrelInfo::new();

                if !recoil_bone_name.is_empty() {
                    info.recoil_bone = self.pristine_bone_index_by_name(recoil_bone_name);
                }

                if !muzzle_flash_name.is_empty() {
                    info.muzzle_flash_bone = self.pristine_bone_index_by_name(muzzle_flash_name);
                }

                if !projectile_launch_name.is_empty() {
                    if let Some((_, bone)) = self.find_pristine_bone_by_name(projectile_launch_name)
                    {
                        info.projectile_offset_mtx = bone.transform;
                    }
                }

                if !fx_bone_name.is_empty() {
                    info.fx_bone = self.pristine_bone_index_by_name(fx_bone_name);
                }

                if info.fx_bone != 0
                    || info.recoil_bone != 0
                    || info.muzzle_flash_bone != 0
                    || info.projectile_offset_mtx != Matrix3D::IDENTITY
                {
                    barrels.push(info);
                }
            }
        }

        self.weapon_barrels = validated_barrels;
        self.valid_stuff |= MODEL_CONDITION_BARRELS_VALID;
    }

    fn validate_public_bones(&mut self, extra_public_bones: &[AsciiString]) {
        if (self.valid_stuff & MODEL_CONDITION_PUBLIC_BONES_VALID) != 0 {
            return;
        }
        for bone in extra_public_bones {
            add_public_bone(&mut self.public_bones, bone.as_str());
        }
        self.valid_stuff |= MODEL_CONDITION_PUBLIC_BONES_VALID;
    }

    fn validate_runtime_caches(&mut self, extra_public_bones: &[AsciiString]) {
        self.validate_runtime_caches_scaled(extra_public_bones, 1.0);
    }

    fn validate_runtime_caches_scaled(&mut self, extra_public_bones: &[AsciiString], scale: Real) {
        self.validate_public_bones(extra_public_bones);
        self.validate_cached_bones(scale);
        self.validate_turret_info();
        self.validate_weapon_barrel_info();
    }

    fn refresh_projectile_valid_bit(&mut self) {
        self.valid_stuff &= !MODEL_CONDITION_HAS_PROJECTILE_BONES;
        if self
            .weapon_projectile_launch_bone
            .iter()
            .any(|name| !name.is_empty())
        {
            self.valid_stuff |= MODEL_CONDITION_HAS_PROJECTILE_BONES;
        }
    }
}

impl Default for ModelConditionInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Sentinel frame: pose the last clip frame (`PRISTINE_BONE_POS_IN_FINAL_FRAME`).
const PRISTINE_BONE_LAST_FRAME: i32 = -1;

pub type PristineBoneLookupHook =
    std::sync::Arc<dyn Fn(&str, Real, i32, &str) -> Option<(i32, Matrix3D)> + Send + Sync>;

static PRISTINE_BONE_LOOKUP: std::sync::LazyLock<std::sync::RwLock<Option<PristineBoneLookupHook>>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(None));

/// Register a W3D backend that can pose a state model and return a bone.
/// `frame == -1` means last clip frame. `None` unregisters.
pub fn register_pristine_bone_lookup_hook(hook: Option<PristineBoneLookupHook>) {
    if let Ok(mut guard) = PRISTINE_BONE_LOOKUP.write() {
        *guard = hook;
    }
}

fn lookup_pristine_bone(
    model: &str,
    scale: Real,
    frame: i32,
    bone_name: &str,
) -> Option<(i32, Matrix3D)> {
    let guard = PRISTINE_BONE_LOOKUP.read().ok()?;
    let hook = guard.as_ref()?;
    hook(model, scale, frame, bone_name)
}

/// C++ `doSingleBoneName`: base name plus numbered `name01`..`name99` variants.
fn do_single_bone_name(
    map: &mut HashMap<NameKeyType, PristineBoneInfo>,
    model: &str,
    scale: Real,
    frame: i32,
    bone_name: &str,
) -> bool {
    if bone_name.is_empty() || bone_name.eq_ignore_ascii_case("none") {
        return false;
    }
    let bone_name = bone_name.to_ascii_lowercase();
    let mut found = false;
    if let Some((bone_index, transform)) = lookup_pristine_bone(model, scale, frame, &bone_name) {
        map.insert(
            name_key_generate(&bone_name),
            PristineBoneInfo {
                transform,
                bone_index,
            },
        );
        found = true;
    }
    for index in 1..=99 {
        let numbered = format!("{bone_name}{index:02}");
        if let Some((bone_index, transform)) = lookup_pristine_bone(model, scale, frame, &numbered)
        {
            map.insert(
                name_key_generate(&numbered),
                PristineBoneInfo {
                    transform,
                    bone_index,
                },
            );
            found = true;
        } else {
            break;
        }
    }
    found
}

