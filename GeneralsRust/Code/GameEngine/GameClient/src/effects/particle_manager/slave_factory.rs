//! Receiver-owned authored slave creation, matching recursive C++ constructor order.
use super::*;
use std::collections::HashSet;

impl ParticleSystemManager {
    pub(super) fn create_authored_system_chain(
        &mut self,
        template: &Arc<ParticleSystemTemplate>,
        root_id: ParticleSystemId,
        create_slaves: bool,
        random: &mut dyn FnMut(f32, f32) -> f32,
        creation_frame: u32,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        // C++ has no cycle guard. Reject malformed chains before consuming IDs,
        // draws or list entries; iteration also avoids a content-driven stack.
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut next = Some(template.clone());
        while let Some(template) = next {
            if !seen.insert(Arc::as_ptr(&template)) {
                return Err(ParticleSystemError::InitializationFailed(
                    "cyclic particle slave template chain".into(),
                ));
            }
            next = if create_slaves {
                template.slave_template().cloned().or_else(|| {
                    let name = template.slave_system_name();
                    (!name.is_empty())
                        .then(|| self.find_template(name))
                        .flatten()
                })
            } else {
                None
            };
            chain.push(template);
        }

        // Reserve parent IDs before child construction (ParticleSys.cpp2950).
        let mut ids = Vec::with_capacity(chain.len());
        let mut next_id = self.next_system_id;
        for index in 0..chain.len() {
            let id = if index == 0 { root_id } else { next_id };
            if id == INVALID_PARTICLE_SYSTEM_ID {
                return Err(ParticleSystemError::InvalidSystemId(id));
            }
            if self.active_systems.contains_key(&id) || ids.contains(&id) {
                return Err(ParticleSystemError::InitializationFailed(format!(
                    "duplicate particle system ID {id}"
                )));
            }
            ids.push(id);
            next_id = next_id.max(id.saturating_add(1));
        }
        self.next_system_id = next_id;

        // Parent delay/wind draws precede all child draws. The C++ constructor
        // performs no further draws after createSlaveSystem (1123-1159).
        let mut systems: Vec<_> = chain
            .into_iter()
            .zip(&ids)
            .map(|(template, &id)| {
                ParticleSystem::new_with_random_at_frame(
                    template,
                    id,
                    false,
                    creation_frame,
                    random,
                )
            })
            .collect();
        for (index, system) in systems.iter_mut().enumerate() {
            system.set_master(index.checked_sub(1).map(|i| ids[i]));
            system.set_slave(ids.get(index + 1).copied());
        }
        // Recursive child ctors append before the parent's final admission.
        for (id, system) in ids.into_iter().zip(systems).rev() {
            self.active_systems.insert(id, Box::new(system));
            self.active_system_order.push(id);
        }
        self.system_count = self.active_systems.len();
        Ok(root_id)
    }
}

#[cfg(test)]
#[path = "slave_factory_tests.rs"]
mod tests;
