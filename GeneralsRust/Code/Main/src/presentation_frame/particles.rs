use super::*;

/// Snapshot-owned combat particle system for presentation/client observe path.
/// Fail-closed: not full W3D GPU particle parity (hq-gq7n residual).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationParticleSystem {
    pub id: u32,
    pub kind: CombatParticleKind,
    pub template_name: String,
    pub position: Vec3,
    pub source_object: Option<ObjectId>,
    pub target_object: Option<ObjectId>,
    pub spawned_frame: u32,
    pub active: bool,
    pub client_system_id: Option<u32>,
    /// C++ Weapon.ini FireFX / DetonationFX residual (empty = preset only).
    #[serde(default)]
    pub fx_list_name: String,
    /// C++ Weapon.ini FireOCL / ProjectileDetonationOCL residual (empty = none).
    #[serde(default)]
    pub ocl_list_name: String,
}

impl PresentationParticleSystem {
    pub fn from_combat_entry(entry: &CombatParticleSystemEntry) -> Self {
        Self {
            id: entry.id,
            kind: entry.kind,
            template_name: entry.template_name.clone(),
            position: entry.position,
            source_object: entry.source_object,
            target_object: entry.target_object,
            spawned_frame: entry.spawned_frame,
            active: entry.active,
            client_system_id: entry.client_system_id,
            fx_list_name: entry.fx_list_name.clone(),
            ocl_list_name: entry.ocl_list_name.clone(),
        }
    }
}

impl PresentationFrame {
    /// Freeze named damage, death and bone effects that have no particle row yet.
    pub(super) fn append_object_fx_particles(&mut self) -> usize {
        let frame = self.frame.0;
        let mut next_id = self
            .particle_systems
            .iter()
            .map(|p| p.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
        let mut appended = 0usize;
        for o in &self.objects {
            let candidates: [(Option<&str>, CombatParticleKind); 3] = [
                (
                    o.damage_fx_name.as_deref(),
                    CombatParticleKind::WeaponImpact,
                ),
                (
                    o.death_fx_name.as_deref(),
                    CombatParticleKind::DeathExplosion,
                ),
                (o.bone_fx_name.as_deref(), CombatParticleKind::WeaponImpact),
            ];
            for (name, kind) in candidates {
                let Some(name) = name else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let already = self.particle_systems.iter().any(|p| {
                    (!p.fx_list_name.is_empty() && p.fx_list_name == name)
                        || (p.template_name == name && p.source_object == Some(o.id))
                });
                if already {
                    continue;
                }
                self.particle_systems.push(PresentationParticleSystem {
                    id: next_id,
                    kind,
                    template_name: name.to_string(),
                    position: o.position,
                    source_object: Some(o.id),
                    target_object: None,
                    spawned_frame: frame,
                    active: true,
                    client_system_id: None,
                    fx_list_name: name.to_string(),
                    ocl_list_name: String::new(),
                });
                next_id = next_id.saturating_add(1).max(1);
                appended += 1;
            }
        }
        appended
    }
}
