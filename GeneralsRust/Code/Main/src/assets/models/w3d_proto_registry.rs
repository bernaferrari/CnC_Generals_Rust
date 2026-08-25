//! Extra C++ `PrototypeClass` kinds the live Main registry must retain.
//!
//! `WW3DAssetManager` registers Collection / Box / Ring / Sphere / Aggregate /
//! Null / DistLOD / Dazzle / ParticleEmitter in addition to Mesh/HLod/HModel.

use super::W3DModel;
use super::w3d_collection_aggregate::{W3dAggregateProto, W3dCollectionProto, W3dDistLodProto};
use super::w3d_dazzle_loader::W3dDazzleProto;
use super::w3d_emitter_loader::W3dEmitterProto;
use super::w3d_primitive_protos::{W3dBoxProto, W3dNullProto, W3dRingProto, W3dSphereProto};

/// Extra prototype kinds stored on [`W3DModel`] after the live loader parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3dExtraPrototypeKind {
    Emitter { emitter_index: usize },
    Dazzle { dazzle_index: usize },
    Box { box_index: usize },
    Ring { ring_index: usize },
    Sphere { sphere_index: usize },
    Null { null_index: usize },
    Collection { collection_index: usize },
    Aggregate { aggregate_index: usize },
    DistLod { dist_lod_index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct W3dExtraPrototypeRef {
    pub full_name: &'static str,
}

pub fn extra_prototypes(model: &W3DModel) -> Vec<(String, W3dExtraPrototypeKind)> {
    let mut out = Vec::new();
    push_named(
        &mut out,
        model.emitters.iter().map(|p| p.name.as_str()),
        |i| W3dExtraPrototypeKind::Emitter { emitter_index: i },
    );
    push_named(
        &mut out,
        model.dazzles.iter().map(|p| p.name.as_str()),
        |i| W3dExtraPrototypeKind::Dazzle { dazzle_index: i },
    );
    push_named(&mut out, model.boxes.iter().map(|p| p.name.as_str()), |i| {
        W3dExtraPrototypeKind::Box { box_index: i }
    });
    push_named(&mut out, model.rings.iter().map(|p| p.name.as_str()), |i| {
        W3dExtraPrototypeKind::Ring { ring_index: i }
    });
    push_named(
        &mut out,
        model.spheres.iter().map(|p| p.name.as_str()),
        |i| W3dExtraPrototypeKind::Sphere { sphere_index: i },
    );
    push_named(&mut out, model.nulls.iter().map(|p| p.name.as_str()), |i| {
        W3dExtraPrototypeKind::Null { null_index: i }
    });
    push_named(
        &mut out,
        model.collections.iter().map(|p| p.name.as_str()),
        |i| W3dExtraPrototypeKind::Collection {
            collection_index: i,
        },
    );
    push_named(
        &mut out,
        model.aggregates.iter().map(|p| p.name.as_str()),
        |i| W3dExtraPrototypeKind::Aggregate { aggregate_index: i },
    );
    push_named(
        &mut out,
        model.dist_lods.iter().map(|p| p.name.as_str()),
        |i| W3dExtraPrototypeKind::DistLod { dist_lod_index: i },
    );
    out
}

fn push_named<'a, I, F>(out: &mut Vec<(String, W3dExtraPrototypeKind)>, names: I, kind: F)
where
    I: IntoIterator<Item = &'a str>,
    F: Fn(usize) -> W3dExtraPrototypeKind,
{
    for (index, name) in names.into_iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        out.push((name.to_string(), kind(index)));
    }
}

pub fn emitter_at(model: &W3DModel, index: usize) -> Option<&W3dEmitterProto> {
    model.emitters.get(index)
}

pub fn dazzle_at(model: &W3DModel, index: usize) -> Option<&W3dDazzleProto> {
    model.dazzles.get(index)
}

pub fn box_at(model: &W3DModel, index: usize) -> Option<&W3dBoxProto> {
    model.boxes.get(index)
}

pub fn ring_at(model: &W3DModel, index: usize) -> Option<&W3dRingProto> {
    model.rings.get(index)
}

pub fn sphere_at(model: &W3DModel, index: usize) -> Option<&W3dSphereProto> {
    model.spheres.get(index)
}

pub fn null_at(model: &W3DModel, index: usize) -> Option<&W3dNullProto> {
    model.nulls.get(index)
}

pub fn collection_at(model: &W3DModel, index: usize) -> Option<&W3dCollectionProto> {
    model.collections.get(index)
}

pub fn aggregate_at(model: &W3DModel, index: usize) -> Option<&W3dAggregateProto> {
    model.aggregates.get(index)
}

pub fn dist_lod_at(model: &W3DModel, index: usize) -> Option<&W3dDistLodProto> {
    model.dist_lods.get(index)
}
