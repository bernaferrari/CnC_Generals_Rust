//! Resolve an HLOD child name against a parsed dazzle prototype.

use super::W3DModel;
use super::w3d_dazzle_loader::W3dDazzleProto;

pub fn hlod_child_is_dazzle(model: &W3DModel, child_name: &str) -> Option<usize> {
    let leaf = child_name
        .split_once('.')
        .map(|(_, leaf)| leaf)
        .unwrap_or(child_name);
    model.dazzles.iter().position(|proto| {
        proto.name.eq_ignore_ascii_case(child_name) || proto.name.eq_ignore_ascii_case(leaf)
    })
}

pub fn dazzle_proto<'a>(model: &'a W3DModel, child_name: &str) -> Option<&'a W3dDazzleProto> {
    hlod_child_is_dazzle(model, child_name).and_then(|i| model.dazzles.get(i))
}
