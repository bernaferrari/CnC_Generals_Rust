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

    fn resolve_material_draw_facts(
        &self,
        facts: &[NativeMaterialObservation],
    ) -> GameClientResult<HashMap<PresentationDirectDrawableBindingKey, NativeMaterialObservation>>
    {
        // CU may retire an owner before Draw; live reused IDs still undergo
        // the complete key check. No registry or guessed-alive fallback.
        let live: Vec<_> = facts
            .iter()
            .copied()
            .filter(|fact| {
                self.drawables
                    .drawable_map
                    .contains_key(&fact.binding_key.drawable_id)
            })
            .collect();
        self.validate_material_observations(&live)?;
        let by_key: HashMap<_, _> = live
            .into_iter()
            .map(|fact| (fact.binding_key, fact))
            .collect();
        for id in &self.drawables.native_drawable_order {
            if let Some(binding) = self.drawables.presentation_direct_drawable_bindings.get(id) {
                if !by_key.contains_key(&binding.binding_key) {
                    return Err(GameClientError::InvalidOperation(
                        "missing native material observation".into(),
                    ));
                }
            }
        }
        Ok(by_key)
    }

    fn advance_material_drawable(
        &mut self,
        id: DrawableId,
        by_key: &HashMap<PresentationDirectDrawableBindingKey, NativeMaterialObservation>,
    ) -> GameClientResult<()> {
        let receiver = self.drawables.drawable_map.get_mut(&id).ok_or_else(|| {
            GameClientError::InvalidOperation(
                "native Draw receiver retired during traversal".into(),
            )
        })?;
        let Some(receiver) = receiver.as_any_mut().downcast_mut::<BasicDrawable>() else {
            return Ok(());
        };
        if let Some(binding) = self
            .drawables
            .presentation_direct_drawable_bindings
            .get(&id)
        {
            let fact = by_key.get(&binding.binding_key).ok_or_else(|| {
                GameClientError::InvalidOperation(
                    "native Draw binding changed during traversal".into(),
                )
            })?;
            receiver.advance_material_draw(fact.effectively_dead, fact.frenzy);
        } else {
            receiver.advance_core_material_draw();
        }
        Ok(())
    }

    fn advance_native_material_draw(
        &mut self,
        facts: &[NativeMaterialObservation],
    ) -> GameClientResult<()> {
        let by_key = self.resolve_material_draw_facts(facts)?;
        for id in self.drawables.native_drawable_order.clone() {
            self.advance_material_drawable(id, &by_key)?;
        }
        Ok(())
    }
}
