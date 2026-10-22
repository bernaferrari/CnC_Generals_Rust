//! Source ObjectDrawInterface declaration order, independent of visible models.
use super::*;

impl DrawModuleDefinition {
    pub(crate) fn has_object_draw_interface(&self) -> bool {
        // W3DModelDraw is the sole original override; these declarations
        // derive from it transitively. Tree/Prop state promotion is irrelevant.
        const CLASSES: &[&str] = &[
            "W3DDependencyModelDraw",
            "W3DModelDraw",
            "W3DOverlordAircraftDraw",
            "W3DOverlordTankDraw",
            "W3DOverlordTruckDraw",
            "W3DPoliceCarDraw",
            "W3DScienceModelDraw",
            "W3DSupplyDraw",
            "W3DTankDraw",
            "W3DTankTruckDraw",
            "W3DTruckDraw",
        ];
        let Some(class) = self.declaration.split_whitespace().next() else {
            return false;
        };
        CLASSES
            .iter()
            .any(|known| class.eq_ignore_ascii_case(known))
    }
}

impl ObjectDefinition {
    /// No model selection occurs here. A first interface selecting Model=None
    /// must remain first; a later visible module cannot become the camera.
    pub(crate) fn first_object_draw_module(&self) -> Option<(u32, &DrawModuleDefinition)> {
        let (index, module) = self
            .draw_modules
            .iter()
            .enumerate()
            .find(|(_, module)| module.has_object_draw_interface())?;
        Some((u32::try_from(index).ok()?, module))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_interface_model_none_does_not_fall_through_to_later_visible_module() {
        let mut first = DrawModuleDefinition::new("W3DModelDraw First".into());
        let mut state = DrawConditionStateDefinition::default_state();
        state.set_model("NONE");
        first.condition_states.push(state);
        let mut later = DrawModuleDefinition::new("W3DTankDraw Later".into());
        let mut state = DrawConditionStateDefinition::default_state();
        state.set_model("LaterVisible");
        later.condition_states.push(state);
        let modules = [first, later];
        let selected = ObjectDefinition::select_first_object_draw_from_modules(&modules, 0)
            .unwrap()
            .unwrap();
        assert_eq!(selected.module_index, 0);
        assert!(selected.model_key.is_empty());
        let visible = ObjectDefinition::select_draw_models_from_modules(&modules, 0).unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].module_index, 1);
    }
    #[test]
    fn source_interface_selection_is_independent_of_model_and_tree_promotion() {
        let mut definition = ObjectDefinition::new("CameraOwner".into());
        let mut tree = DrawModuleDefinition::new("W3DTreeDraw TreeTag".into());
        tree.condition_states
            .push(DrawConditionStateDefinition::default_state());
        definition.draw_modules.push(tree);
        // No state is deliberately retained here: eligibility is the original
        // declared interface, not the availability of an initialized renderobj.
        definition
            .draw_modules
            .push(DrawModuleDefinition::new("W3DModelDraw FirstTag".into()));
        definition
            .draw_modules
            .push(DrawModuleDefinition::new("W3DTankDraw LaterTag".into()));
        assert_eq!(definition.first_object_draw_module().unwrap().0, 1);
        assert!(
            !DrawModuleDefinition::new("W3DModelDrawImpostor Tag".into())
                .has_object_draw_interface()
        );
        assert!(
            DrawModuleDefinition::new("w3dpolicecardraw Tag".into()).has_object_draw_interface()
        );
    }
    #[test]
    fn animation_speed_range_keeps_default_inheritance_and_exact_state_override() {
        let text = r#"Object CameraRange
 Draw = W3DModelDraw ModuleTag_01
  DefaultConditionState
   Model = CameraModel
   AnimationSpeedFactorRange = 1.25 1.25
  End
  ConditionState = MOVING
   Model = CameraModel
  End
  ConditionState = ATTACKING
   Model = CameraModel
   AnimationSpeedFactorRange = 0.5 2.0
  End
 End
End
"#;
        let mut parser = IniParser::new();
        parser.parse_ini_content(text, "camera-range.ini").unwrap();
        let definition = parser.get_definition("CameraRange").unwrap();
        let get = |bits| {
            ObjectDefinition::select_first_object_draw_from_modules(&definition.draw_modules, bits)
                .unwrap()
                .unwrap()
                .animation_speed_range_bits
                .unwrap()
                .map(f32::from_bits)
        };
        assert_eq!(get(0), [1.25, 1.25]);
        assert_eq!(
            get(1u128 << game_engine::common::bit_flags::ModelConditionFlags::MOVING),
            [1.25, 1.25]
        );
        assert_eq!(
            get(1u128 << game_engine::common::bit_flags::ModelConditionFlags::ATTACKING),
            [0.5, 2.]
        );
    }
    #[test]
    fn animation_speed_range_transition_inherits_default_before_local_override() {
        // C++ PARSE_TRANSITION copies the complete DefaultConditionState before
        // parsing local fields, including the ordered speed-range endpoints.
        let text = r#"Object TransitionRange
 Draw = W3DModelDraw ModuleTag_01
  DefaultConditionState
   Model = CameraModel
   AnimationSpeedFactorRange = 2.0 0.5
  End
  TransitionState = A B
   Model = CameraModel
  End
  TransitionState = B C
   Model = CameraModel
   AnimationSpeedFactorRange = 0.25 0.75
  End
 End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(text, "transition-range.ini")
            .unwrap();
        let definition = parser.get_definition("TransitionRange").unwrap();
        let states = &definition.draw_modules[0].condition_states;
        assert_eq!(states.len(), 3);
        assert!(states[0].is_default);
        assert!(states[1].is_transition);
        assert!(states[2].is_transition);
        let range = |index: usize| {
            states[index]
                .animation_speed_range_bits
                .unwrap()
                .map(f32::from_bits)
        };
        assert_eq!(range(0), [2.0, 0.5]);
        assert_eq!(range(1), [2.0, 0.5]);
        assert_eq!(range(2), [0.25, 0.75]);
    }
}

impl ObjectDefinition {
    pub(crate) fn select_first_object_draw_from_modules(
        modules: &[DrawModuleDefinition],
        bits: u128,
    ) -> std::result::Result<Option<AuthoredDrawModel>, ()> {
        let Some((index, module)) = modules
            .iter()
            .enumerate()
            .find(|(_, module)| module.has_object_draw_interface())
        else {
            return Ok(None);
        };
        let (state_index, state) = module
            .selected_condition_state_for_conditions(bits)?
            .ok_or(())?;
        Ok(Some(module.authored_draw_model(
            u32::try_from(index).map_err(|_| ())?,
            u32::try_from(state_index).map_err(|_| ())?,
            state,
        )))
    }
}

impl ObjectDefinition {
    pub(crate) fn select_object_draws_from_modules(
        modules: &[DrawModuleDefinition],
        bits: u128,
    ) -> std::result::Result<Vec<AuthoredDrawModel>, ()> {
        modules
            .iter()
            .enumerate()
            .filter(|(_, module)| module.has_object_draw_interface())
            .map(|(index, module)| {
                let (state_index, state) = module
                    .selected_condition_state_for_conditions(bits)?
                    .ok_or(())?;
                Ok(module.authored_draw_model(
                    u32::try_from(index).map_err(|_| ())?,
                    u32::try_from(state_index).map_err(|_| ())?,
                    state,
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod native_constructor_roster_diagnostic_tests {
    use super::*;
    #[test]
    fn non_camera_render_modules_are_not_object_draw_interfaces() {
        for class in ["W3DTreeDraw", "W3DPropDraw"] {
            let mut module = DrawModuleDefinition::new(format!("{class} DrawTag"));
            let mut state = DrawConditionStateDefinition::default_state();
            state.set_model("DiagnosticTree");
            module.condition_states.push(state);
            let modules = [module];
            assert!(
                !modules[0].has_object_draw_interface(),
                "camera interface stays narrow"
            );
            let rendering = ObjectDefinition::select_draw_models_from_modules(&modules, 0).unwrap();
            assert_eq!(
                rendering.len(),
                1,
                "actual render selection owns a non-camera Draw"
            );
            assert_eq!(rendering[0].module_index, 0);
            assert!(
                ObjectDefinition::select_object_draws_from_modules(&modules, 0)
                    .unwrap()
                    .is_empty(),
                "this camera-only projection cannot be used as complete native constructor roster"
            );
        }
    }
}
