//! Resolve an HLOD child name against a parsed emitter prototype.

use super::W3DModel;
use super::w3d_emitter_loader::W3dEmitterProto;

pub fn hlod_child_is_emitter(model: &W3DModel, child_name: &str) -> Option<usize> {
    find_named(&model.emitters, child_name, |proto| proto.name.as_str())
}

fn find_named<T>(items: &[T], child_name: &str, name_of: impl Fn(&T) -> &str) -> Option<usize> {
    let leaf = child_name
        .split_once('.')
        .map(|(_, leaf)| leaf)
        .unwrap_or(child_name);
    items.iter().position(|item| {
        let name = name_of(item);
        name.eq_ignore_ascii_case(child_name) || name.eq_ignore_ascii_case(leaf)
    })
}

pub fn emitter_proto<'a>(model: &'a W3DModel, child_name: &str) -> Option<&'a W3dEmitterProto> {
    hlod_child_is_emitter(model, child_name).and_then(|i| model.emitters.get(i))
}
