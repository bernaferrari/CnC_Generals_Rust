//! A real borrowed object/Drawable owner, independent of ambient registries.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptNamedPresentation};

pub(super) struct PresentationOwner {
    pub name: String,
    pub object: crate::object::Object,
    pub drawable: Option<crate::object::drawable::Drawable>,
}
impl PresentationOwner {
    pub fn new(name: &str, id: ObjectID) -> Self {
        Self {
            name: name.into(),
            object: crate::object::Object::new_raw(
                Arc::new(crate::common::DefaultThingTemplate::new(name.into())),
                id,
                crate::common::ObjectStatusMaskType::NONE,
                None,
            ),
            drawable: Some(crate::object::drawable::Drawable::new(
                1,
                id,
                "FlashModel".into(),
                crate::object::drawable::DrawableType::Static,
            )),
        }
    }
}
impl ScriptExecutionDriver for PresentationOwner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn named_presentation(
        &mut self,
        request: ScriptNamedPresentation<'_>,
        _: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        let name = match request {
            ScriptNamedPresentation::Flash { unit, .. }
            | ScriptNamedPresentation::CustomColor { unit, .. } => unit,
        };
        if name != self.name {
            return Some(Ok(()));
        }
        match request {
            ScriptNamedPresentation::Flash { seconds, white, .. } => {
                let color = if white {
                    Color::white()
                } else {
                    self.object.get_indicator_color()
                };
                if let Some(drawable) = &mut self.drawable {
                    super::super::apply_script_drawable_flash(drawable, seconds, color);
                }
            }
            ScriptNamedPresentation::CustomColor { color, .. } => self
                .object
                .set_custom_indicator_color(super::super::script_indicator_color(color)),
        }
        Some(Ok(()))
    }
}
