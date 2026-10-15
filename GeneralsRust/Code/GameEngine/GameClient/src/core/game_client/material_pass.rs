// Instance-owned second material pass: transient driving-world observations.
#[derive(Debug, Clone, Copy)]
pub struct NativeMaterialObservation {
    pub binding_key: PresentationDirectDrawableBindingKey,
    pub effectively_dead: bool,
    pub frenzy: bool,
    pub look: crate::drawable::drawable::StealthLook,
    pub is_disguised: bool,
    pub is_mine: bool,
    pub friendly_opacity: f32,
    /// Last logic write, including changed-look writes. It is admitted once,
    /// after look metadata, and is never replayed by the Draw operation.
    pub pending_opacity: Option<f32>,
}

impl GameClient {
    fn validate_material_observations(
        &self,
        facts: &[NativeMaterialObservation],
    ) -> GameClientResult<()> {
        let mut seen = std::collections::HashSet::new();
        for fact in facts {
            let key = fact.binding_key;
            if !seen.insert(key)
                || self
                    .presentation_direct_drawable_state(key.host_epoch, key.object_id)
                    .is_none_or(|state| state.binding_key != key)
                || !self
                    .drawables
                    .drawable_map
                    .get(&key.drawable_id)
                    .is_some_and(|drawable| drawable.as_any().is::<BasicDrawable>())
            {
                return Err(GameClientError::InvalidOperation(
                    "stale or duplicate native material observation".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn apply_presentation_material_command(
        &mut self,
        key: PresentationDirectDrawableBindingKey,
        opacity: f32,
    ) -> GameClientResult<()> {
        if self
            .presentation_direct_drawable_state(key.host_epoch, key.object_id)
            .is_none_or(|state| state.binding_key != key)
        {
            return Err(GameClientError::InvalidOperation(
                "stale native material command".into(),
            ));
        }
        let receiver = self
            .drawables
            .drawable_map
            .get_mut(&key.drawable_id)
            .and_then(|drawable| drawable.as_any_mut().downcast_mut::<BasicDrawable>())
            .ok_or_else(|| {
                GameClientError::InvalidOperation("native material receiver absent".into())
            })?;
        receiver.set_second_material_pass_opacity(opacity);
        Ok(())
    }

    /// Prevalidate before writing any receiver. Main acknowledges commands after
    /// this succeeds, independently of later device/particle failures.
    pub fn prepare_presentation_materials(
        &mut self,
        facts: &[NativeMaterialObservation],
    ) -> GameClientResult<()> {
        self.validate_material_observations(facts)?;
        for fact in facts {
            let receiver = self
                .drawables
                .drawable_map
                .get_mut(&fact.binding_key.drawable_id)
                .and_then(|drawable| drawable.as_any_mut().downcast_mut::<BasicDrawable>())
                .expect("prevalidated native material receiver");
            receiver.apply_stealth_look_metadata(
                fact.look,
                fact.is_disguised,
                fact.is_mine,
                fact.friendly_opacity,
            );
            if let Some(opacity) = fact.pending_opacity {
                receiver.set_second_material_pass_opacity(opacity);
            }
        }
        Ok(())
    }

    /// Completed presentation output only; it is never fed back into the owner.
    pub fn capture_presentation_material_opacities(
        &self,
    ) -> Vec<(PresentationDirectDrawableBindingKey, f32)> {
        self.drawables
            .native_drawable_order
            .iter()
            .filter_map(|id| {
                let key = self
                    .drawables
                    .presentation_direct_drawable_bindings
                    .get(id)?
                    .binding_key;
                if self
                    .presentation_direct_drawable_state(key.host_epoch, key.object_id)?
                    .binding_key
                    != key
                {
                    return None;
                }
                let receiver = self
                    .drawables
                    .drawable_map
                    .get(id)?
                    .as_any()
                    .downcast_ref::<BasicDrawable>()?;
                Some((key, receiver.material_pass_opacity()))
            })
            .collect()
    }

    fn advance_native_material_draw(
        &mut self,
        facts: &[NativeMaterialObservation],
    ) -> GameClientResult<()> {
        // Ordinary CU may physically retire an owner after admission and
        // before Draw. Its observation has no remaining receiver. A reused
        // live ID still undergoes the complete key check below.
        let live_facts: Vec<_> = facts
            .iter()
            .copied()
            .filter(|fact| {
                self.drawables
                    .drawable_map
                    .contains_key(&fact.binding_key.drawable_id)
            })
            .collect();
        self.validate_material_observations(&live_facts)?;
        let by_key: std::collections::HashMap<_, _> = live_facts
            .iter()
            .map(|fact| (fact.binding_key, fact))
            .collect();
        // Missing current-world input is an error, never an ambient object lookup
        // or a guessed alive observation. Validate the complete batch first.
        for id in &self.drawables.native_drawable_order {
            if let Some(binding) = self.drawables.presentation_direct_drawable_bindings.get(id) {
                if !by_key.contains_key(&binding.binding_key) {
                    return Err(GameClientError::InvalidOperation(
                        "missing native material observation".into(),
                    ));
                }
            }
        }
        for id in &self.drawables.native_drawable_order {
            let drawable = self
                .drawables
                .drawable_map
                .get_mut(id)
                .expect("native roster receiver");
            let Some(drawable) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() else {
                continue;
            };
            if let Some(binding) = self.drawables.presentation_direct_drawable_bindings.get(id) {
                let fact = by_key[&binding.binding_key];
                drawable.advance_material_draw(fact.effectively_dead, fact.frenzy);
            } else {
                // Retained Core/objectless route, distinct from direct native.
                drawable.advance_core_material_draw();
            }
        }
        Ok(())
    }
}
