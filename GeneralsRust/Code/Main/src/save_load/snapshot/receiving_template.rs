//! Detached receiving definition resolution for transactional save preflight.
use crate::game_logic::{GameLogic, ThingTemplate};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::borrow::Cow;

/// Validate construction-critical turret data without publishing an object.
pub(super) fn validate_turret_source(world: &GameLogic, name: &str) -> SaveLoadResult<()> {
    let Some(content) = &world.receiving_module_content else {
        return Ok(());
    };
    let plan = content.admitted_template(name).ok_or_else(|| {
        SaveLoadError::Corrupted(format!(
            "Saved Object '{name}' is absent from receiving content"
        ))
    })?;
    crate::game_logic::thing::ReceivingClientTemplate::new(plan)
        .validate_turret_slots()
        .map_err(|error| SaveLoadError::Corrupted(format!("Saved Object '{name}': {error}")))
}

/// A materialized host-template cache is not the receiving content catalog.
/// Resolve excluded/lazy Object definitions without admitting a live template,
/// running a constructor, or discovering a different session's asset manager.
/// Legacy programmatic worlds retain their optional cached-template contract.
pub(super) fn resolve<'a>(
    world: &'a GameLogic,
    name: &str,
) -> SaveLoadResult<Option<Cow<'a, ThingTemplate>>> {
    let Some(content) = &world.receiving_module_content else {
        return Ok(world.templates.get(name).map(Cow::Borrowed));
    };
    let missing = || {
        SaveLoadError::Corrupted(format!(
            "Saved Object '{name}' is absent from receiving content"
        ))
    };
    let plan = content.admitted_template(name).ok_or_else(missing)?;
    let definition = world
        .receiving_object_definitions
        .as_ref()
        .and_then(|definitions| definitions.get(name))
        .ok_or_else(missing)?;
    if let Some(template) = world.templates.get(name).filter(|template| {
        template
            .receiving_object_definition
            .as_ref()
            .is_some_and(|existing| std::sync::Arc::ptr_eq(existing, definition))
    }) {
        return Ok(Some(Cow::Borrowed(template)));
    }
    let mut template = GameLogic::build_template_from_object_definition_with_receiving(
        name,
        definition,
        definition.get_primary_texture(),
        Some(content),
        world.locomotor_definitions.as_ref(),
    )
    .map_err(SaveLoadError::Corrupted)?;
    template.receiving_object_definition = Some(std::sync::Arc::clone(definition));
    template.receiving_client_template =
        Some(crate::game_logic::thing::ReceivingClientTemplate::new(plan));
    Ok(Some(Cow::Owned(template)))
}

#[cfg(test)]
#[path = "receiving_template_tests.rs"]
mod tests;
