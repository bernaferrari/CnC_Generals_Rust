//! Opaque paired admission from one ordered archive-source stream.
use super::ReceivingObjectDefinitions;
use game_engine::common::{
    ini::INILoadType,
    name_key_generator::NameKeyNamespace,
    thing::{module_factory::ModuleFactory, receiving_module_content::ReceivingModuleContent},
};

pub(crate) struct ReceivingObjectContent {
    modules: ReceivingModuleContent,
    projections: ReceivingObjectDefinitions,
}
impl ReceivingObjectContent {
    pub(crate) fn from_sources<I>(sources: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        Self::from_sources_with_sciences(
            sources,
            std::sync::Arc::new(game_engine::common::rts::science::ScienceNameFacts::default()),
        )
    }
    pub(crate) fn from_sources_with_sciences<I>(
        sources: I,
        sciences: std::sync::Arc<game_engine::common::rts::science::ScienceNameFacts>,
    ) -> Result<Self, String>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let sources = sources.into_iter().collect::<Vec<_>>();
        let mut factory =
            ModuleFactory::new_receiving_with_sciences(NameKeyNamespace::new(), sciences);
        gamelogic::contain_module_overrides::register_native_module_overrides(&mut factory)?;
        let mut modules = ReceivingModuleContent::new(factory);
        for (path, text) in &sources {
            modules
                .load_ini_text(text, INILoadType::Overwrite)
                .map_err(|error| format!("Receiving Object source '{path}': {error}"))?;
        }
        modules.validate_production_prerequisites()?;
        let projections = ReceivingObjectDefinitions::from_boot_sources(&sources)?;
        Ok(Self {
            modules,
            projections,
        })
    }
    pub(crate) fn module_content(&self) -> &ReceivingModuleContent {
        &self.modules
    }
    pub(crate) fn projections(&self) -> &ReceivingObjectDefinitions {
        &self.projections
    }
    pub(crate) fn into_parts(self) -> (ReceivingModuleContent, ReceivingObjectDefinitions) {
        (self.modules, self.projections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    // W3DTankDraw.cpp58 + INI.cpp1709: authored dist/sec becomes dist/frame.
    const FIRST_RATE_PER_FRAME: f32 = 2.5 * (1.0 / 30.0);
    const PEER_RATE_PER_FRAME: f32 = 7.5 * (1.0 / 30.0);
    fn sources(rate: f32) -> [(String, String); 2] {
        [
            (
                "Data/INI/Default/Object.ini".into(),
                format!(
                    "Object Actual\nScale = {rate}\nDraw = W3DTankDraw TankTag\nTreadAnimationRate = {rate}\nEnd\nEnd"
                ),
            ),
            (
                "Data/INI/Object/Actual.ini".into(),
                "Object Actual\nScale = 4\nEnd\nChildObject ActualChild Actual\nEnd".into(),
            ),
        ]
    }
    fn rate(content: &ReceivingObjectContent, name: &str) -> f32 {
        content
            .module_content()
            .admitted_template(name)
            .unwrap()
            .draw_modules()
            .iter()
            .next()
            .unwrap()
            .data
            .as_any()
            .downcast_ref::<gamelogic::object::draw::W3DTankDrawModuleData>()
            .unwrap()
            .tread_animation_rate
    }
    #[test]
    fn one_ordered_source_pair_keeps_typed_parent_data_and_host_projection_together() {
        let first = ReceivingObjectContent::from_sources(sources(2.5)).unwrap();
        let peer = ReceivingObjectContent::from_sources(sources(7.5)).unwrap();
        assert_eq!(first.projections().get("Actual").unwrap().scale, 4.0);
        assert_eq!(first.projections().get("ActualChild").unwrap().scale, 4.0);
        assert_eq!(rate(&first, "Actual"), FIRST_RATE_PER_FRAME);
        assert_eq!(rate(&peer, "Actual"), PEER_RATE_PER_FRAME);
        let parent = first.module_content().admitted_template("Actual").unwrap();
        let child = first
            .module_content()
            .admitted_template("ActualChild")
            .unwrap();
        assert!(
            Arc::ptr_eq(
                &parent.draw_modules().iter().next().unwrap().data,
                &child.draw_modules().iter().next().unwrap().data
            ),
            "child shares actual declaration data, never reparses parent history"
        );
        let (_, mut projections) = first.into_parts();
        projections.reset_map_overrides();
        assert_eq!(
            projections.get("Actual").unwrap().scale,
            4.0,
            "final boot stream, not an intermediate file, is the base"
        );
    }
    #[test]
    fn rejected_paired_source_tail_cannot_publish_partial_typed_or_projection_content() {
        let accepted = ReceivingObjectContent::from_sources(sources(2.5)).unwrap();
        let retained = accepted
            .module_content()
            .admitted_template("Actual")
            .unwrap();
        let mut bad = sources(7.5).to_vec();
        bad.push((
            "Data/INI/Object/Bad.ini".into(),
            "Object Bad\nUnknownReceivingField = Yes\nEnd".into(),
        ));
        let error = ReceivingObjectContent::from_sources(bad).err().unwrap();
        assert!(
            error.contains("Bad.ini") && error.contains("Unknown Object field"),
            "{error}"
        );
        assert!(Arc::ptr_eq(
            &retained,
            &accepted
                .module_content()
                .admitted_template("Actual")
                .unwrap()
        ));
        assert_eq!(rate(&accepted, "Actual"), FIRST_RATE_PER_FRAME);
        assert_eq!(accepted.projections().get("Actual").unwrap().scale, 4.0);
        assert!(accepted.projections().get("Bad").is_none());
    }
    #[test]
    fn default_draw_is_inherited_only_until_an_actual_child_draw_is_declared() {
        let text = "Object DefaultThingTemplate\nDraw = W3DModelDraw DefaultDraw\nDefaultConditionState\nModel = ActualDefaultModel\nEnd\nEnd\nEnd\nObject Plain\nEnd\nObject Changed\nDraw = W3DModelDraw ChangedDraw\nDefaultConditionState\nModel = ActualChangedModel\nEnd\nEnd\nEnd";
        let content =
            ReceivingObjectContent::from_sources([("defaults.ini".into(), text.into())]).unwrap();
        let default = content
            .module_content()
            .admitted_template("DefaultThingTemplate")
            .unwrap();
        let plain = content.module_content().admitted_template("Plain").unwrap();
        assert_eq!(plain.draw_modules().iter().count(), 1);
        assert!(Arc::ptr_eq(
            &default.draw_modules().iter().next().unwrap().data,
            &plain.draw_modules().iter().next().unwrap().data
        ));
        let plain_projection = content.projections().get("Plain").unwrap();
        assert_eq!(
            plain_projection.model_name.as_deref(),
            Some("ActualDefaultModel")
        );
        assert_eq!(plain_projection.draw_modules.len(), 1);
        let changed = content.projections().get("Changed").unwrap();
        assert_eq!(changed.model_name.as_deref(), Some("ActualChangedModel"));
        assert_eq!(changed.draw_modules.len(), 1);
        assert!(changed.draw_modules[0].declaration.contains("ChangedDraw"));
    }
}
