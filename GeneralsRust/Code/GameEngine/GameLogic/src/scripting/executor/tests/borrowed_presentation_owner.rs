//! A real borrowed object/Drawable owner, independent of ambient registries.
use super::*;
use crate::scripting::engine::ScriptExecutionDriver;
use generals_sim::scripting::object_appearance_actions::ObjectAppearanceAction;
use generals_sim::scripting::object_status_actions::ObjectStatusTarget;

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
    fn object_appearance(
        &mut self,
        action: &ObjectAppearanceAction,
        _: Option<ObjectID>,
        _: Option<u32>,
        _: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        use ObjectAppearanceAction::{CustomColor, Flash};
        let name = match action {
            Flash {
                target: ObjectStatusTarget::Named(unit),
                ..
            }
            | CustomColor { unit, .. } => unit,
            _ => return None,
        };
        if name != &self.name {
            return Some(Ok(()));
        }
        match action {
            Flash { seconds, white, .. } => {
                let color = if *white {
                    Color::white()
                } else {
                    self.object.get_indicator_color()
                };
                if let Some(drawable) = &mut self.drawable {
                    super::super::apply_script_drawable_flash(drawable, *seconds, color);
                }
            }
            CustomColor { color, .. } => self
                .object
                .set_custom_indicator_color(super::super::script_indicator_color(*color)),
            _ => unreachable!("the appearance action was resolved to a named object"),
        }
        Some(Ok(()))
    }
}
