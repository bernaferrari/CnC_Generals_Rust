//! Helpers to convert asset prototypes into runtime LOD objects.

use super::lod_object::{
    CompositeMeshLodGeometry, LodLevel, LodObject, MeshInstanceTemplate, MeshLodGeometry,
    TransformNodeInstance,
};
use glam::{Mat4, Vec3};
use std::collections::HashSet;
use std::sync::Arc;
use ww3d_assets::{
    assets::AssetManager,
    prototypes::{
        CollectionPrototype, HlodAggregateEntry, HlodPrototype, LodModelPrototype, MeshPrototype,
    },
    AggregatePrototype,
};

/// Build a `LodObject` from an asset prototype.
pub fn build_lod_object_from_prototype(
    prototype: &LodModelPrototype,
    assets: &AssetManager,
) -> Option<LodObject> {
    let mut lod_object = LodObject::new(0, Vec3::ZERO);
    lod_object.set_label(prototype.name.clone());

    let mut has_levels = false;
    for entry in &prototype.lods {
        let mesh_name = entry.render_obj_name.trim();
        if mesh_name.is_empty() {
            continue;
        }

        let mut visited = HashSet::new();
        let runtime = collect_runtime_data(mesh_name, assets, 0, Mat4::IDENTITY, &mut visited);
        if runtime.meshes.is_empty() {
            continue;
        }

        let mut bounds_collection = Vec::new();
        for template in &runtime.meshes {
            if let Some(mesh_proto) = assets
                .get_prototype(&template.name)
                .and_then(|proto| proto.as_any().downcast_ref::<MeshPrototype>())
            {
                if let Some(bounds) = compute_mesh_bounds(mesh_proto, template.transform) {
                    bounds_collection.push((template.clone(), bounds));
                }
            }
        }

        if bounds_collection.is_empty() {
            continue;
        }

        let mut geometry_info = assemble_geometry(runtime, &bounds_collection);

        let distance_threshold = entry.lod_max.max(entry.lod_min).max(0.0);
        let screen_threshold = distance_threshold;

        let mut lod_level = LodLevel::new(distance_threshold, screen_threshold);
        let geometry: Arc<dyn super::lod_object::LodGeometry> =
            if geometry_info.instances.len() == 1 {
                let single = geometry_info
                    .instances
                    .pop()
                    .expect("expected single instance");
                Arc::new(MeshLodGeometry::new(
                    single.name,
                    single.transform,
                    geometry_info.center,
                    geometry_info.radius,
                    geometry_info.triangle_count,
                    geometry_info.vertex_count,
                    geometry_info.transform_nodes,
                    geometry_info.snap_points,
                ))
            } else {
                Arc::new(CompositeMeshLodGeometry::new(
                    geometry_info.instances,
                    geometry_info.center,
                    geometry_info.radius,
                    geometry_info.triangle_count,
                    geometry_info.vertex_count,
                    geometry_info.transform_nodes,
                    geometry_info.snap_points,
                ))
            };
        lod_level = lod_level.with_mesh(geometry);
        lod_level.render_cost = geometry_info.triangle_count as f32;
        lod_level.render_value = geometry_info.vertex_count as f32;
        lod_object.add_lod_level(lod_level);
        has_levels = true;
    }

    if has_levels {
        Some(lod_object)
    } else {
        None
    }
}

pub fn build_lod_object_from_hlod_prototype(
    prototype: &HlodPrototype,
    assets: &AssetManager,
) -> Option<LodObject> {
    let mut lod_object = LodObject::new(0, Vec3::ZERO);
    lod_object.set_label(prototype.name.clone());

    let mut has_levels = false;
    for entry in &prototype.lods {
        let mut runtime = RuntimeAggregate::default();

        for sub in &entry.models {
            runtime.merge(collect_runtime_data(
                sub.name.as_str(),
                assets,
                0,
                Mat4::IDENTITY,
                &mut HashSet::new(),
            ));
        }

        for aggregate in &prototype.aggregates {
            merge_hlod_aggregate(aggregate, assets, &mut runtime);
        }

        if runtime.meshes.is_empty() {
            continue;
        }

        let mut bounds_collection = Vec::new();
        for template in &runtime.meshes {
            if let Some(mesh_proto) = assets
                .get_prototype(&template.name)
                .and_then(|proto| proto.as_any().downcast_ref::<MeshPrototype>())
            {
                if let Some(bounds) = compute_mesh_bounds(mesh_proto, template.transform) {
                    bounds_collection.push((template.clone(), bounds));
                }
            }
        }

        if bounds_collection.is_empty() {
            continue;
        }

        let geometry_info = assemble_geometry(runtime, &bounds_collection);

        let mut lod_level = LodLevel::new(entry.max_screen_size, entry.max_screen_size);
        let geometry: Arc<dyn super::lod_object::LodGeometry> =
            if geometry_info.instances.len() == 1 {
                let single = geometry_info
                    .instances
                    .into_iter()
                    .next()
                    .expect("expected single instance");
                Arc::new(MeshLodGeometry::new(
                    single.name,
                    single.transform,
                    geometry_info.center,
                    geometry_info.radius,
                    geometry_info.triangle_count,
                    geometry_info.vertex_count,
                    geometry_info.transform_nodes,
                    geometry_info.snap_points,
                ))
            } else {
                Arc::new(CompositeMeshLodGeometry::new(
                    geometry_info.instances,
                    geometry_info.center,
                    geometry_info.radius,
                    geometry_info.triangle_count,
                    geometry_info.vertex_count,
                    geometry_info.transform_nodes,
                    geometry_info.snap_points,
                ))
            };

        lod_level = lod_level.with_mesh(geometry);
        lod_level.render_cost = geometry_info.triangle_count as f32;
        lod_level.render_value = geometry_info.vertex_count as f32;
        lod_object.add_lod_level(lod_level);
        has_levels = true;
    }

    if has_levels {
        Some(lod_object)
    } else {
        None
    }
}

fn collect_runtime_data(
    name: &str,
    assets: &AssetManager,
    depth: usize,
    parent_transform: Mat4,
    visited: &mut HashSet<String>,
) -> RuntimeAggregate {
    const MAX_DEPTH: usize = 32;
    if depth > MAX_DEPTH {
        return RuntimeAggregate::default();
    }

    let trimmed = name.trim();
    if trimmed.is_empty() {
        return RuntimeAggregate::default();
    }

    if !visited.insert(trimmed.to_string()) {
        return RuntimeAggregate::default();
    }

    let mut aggregate = RuntimeAggregate::default();

    if assets
        .get_prototype(trimmed)
        .and_then(|proto| proto.as_any().downcast_ref::<MeshPrototype>())
        .is_some()
    {
        aggregate.meshes.push(MeshInstanceTemplate {
            name: trimmed.to_string(),
            transform: parent_transform,
        });
    } else if let Some(collection) = assets
        .get_prototype(trimmed)
        .and_then(|proto| proto.as_any().downcast_ref::<CollectionPrototype>())
    {
        for placeholder in &collection.placeholders {
            let child_transform = parent_transform * placeholder.transform;
            let child = collect_runtime_data(
                placeholder.name.as_str(),
                assets,
                depth + 1,
                child_transform,
                visited,
            );
            aggregate.merge(child);
        }

        for obj_name in &collection.object_names {
            let child =
                collect_runtime_data(obj_name, assets, depth + 1, parent_transform, visited);
            aggregate.merge(child);
        }

        for node in &collection.transform_nodes {
            let world_transform = parent_transform * node.transform;
            aggregate.transform_nodes.push(TransformNodeInstance {
                name: node.name.clone(),
                transform: world_transform,
            });
            let child =
                collect_runtime_data(&node.name, assets, depth + 1, world_transform, visited);
            aggregate.merge(child);
        }

        for snap in &collection.snap_points {
            let position = parent_transform.transform_point3(Vec3::new(snap.x, snap.y, snap.z));
            aggregate.snap_points.push(position);
        }
    } else if let Some(hlod) = assets
        .get_prototype(trimmed)
        .and_then(|proto| proto.as_any().downcast_ref::<HlodPrototype>())
    {
        for lod_entry in &hlod.lods {
            for sub in &lod_entry.models {
                let child = collect_runtime_data(
                    sub.name.as_str(),
                    assets,
                    depth + 1,
                    parent_transform,
                    visited,
                );
                aggregate.merge(child);
            }
        }

        for aggregate_entry in &hlod.aggregates {
            merge_hlod_aggregate(aggregate_entry, assets, &mut aggregate);
        }

        for proxy in &hlod.proxy_entries {
            aggregate.transform_nodes.push(TransformNodeInstance {
                name: proxy.name.clone(),
                transform: parent_transform,
            });
        }
    } else if let Some(aggregate_proto) = assets
        .get_prototype(trimmed)
        .and_then(|proto| proto.as_any().downcast_ref::<AggregatePrototype>())
    {
        if !aggregate_proto.base_model_name.trim().is_empty() {
            let child = collect_runtime_data(
                aggregate_proto.base_model_name.trim(),
                assets,
                depth + 1,
                parent_transform,
                visited,
            );
            aggregate.merge(child);
        }

        for sub in &aggregate_proto.subobjects {
            let child = collect_runtime_data(
                sub.subobject_name.trim(),
                assets,
                depth + 1,
                parent_transform,
                visited,
            );
            aggregate.merge(child);
        }
    } else if let Some(lod_model) = assets
        .get_prototype(trimmed)
        .and_then(|proto| proto.as_any().downcast_ref::<LodModelPrototype>())
    {
        for entry in &lod_model.lods {
            let child = collect_runtime_data(
                &entry.render_obj_name,
                assets,
                depth + 1,
                parent_transform,
                visited,
            );
            aggregate.merge(child);
        }
    }

    visited.remove(trimmed);
    aggregate
}

#[derive(Default)]
struct RuntimeAggregate {
    meshes: Vec<MeshInstanceTemplate>,
    transform_nodes: Vec<TransformNodeInstance>,
    snap_points: Vec<Vec3>,
}

impl RuntimeAggregate {
    fn merge(&mut self, mut other: RuntimeAggregate) {
        self.meshes.append(&mut other.meshes);
        self.transform_nodes.append(&mut other.transform_nodes);
        self.snap_points.append(&mut other.snap_points);
    }
}

#[derive(Debug, Clone)]
struct MeshBounds {
    min: Vec3,
    max: Vec3,
    triangles: usize,
    vertices: usize,
}

fn compute_mesh_bounds(mesh: &MeshPrototype, transform: Mat4) -> Option<MeshBounds> {
    if mesh.vertices.is_empty() {
        return None;
    }

    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);

    for vertex in &mesh.vertices {
        let point = transform.transform_point3(Vec3::new(vertex.x, vertex.y, vertex.z));
        min = min.min(point);
        max = max.max(point);
    }

    Some(MeshBounds {
        min,
        max,
        triangles: mesh.triangles.len(),
        vertices: mesh.vertices.len(),
    })
}

struct GeometryBuild {
    instances: Vec<MeshInstanceTemplate>,
    transform_nodes: Vec<TransformNodeInstance>,
    snap_points: Vec<Vec3>,
    center: Vec3,
    radius: f32,
    triangle_count: usize,
    vertex_count: usize,
}

fn assemble_geometry(
    runtime: RuntimeAggregate,
    bounds_collection: &[(MeshInstanceTemplate, MeshBounds)],
) -> GeometryBuild {
    let mut agg_min = Vec3::splat(f32::MAX);
    let mut agg_max = Vec3::splat(f32::MIN);
    let mut total_triangles = 0usize;
    let mut total_vertices = 0usize;

    let mut instances = Vec::with_capacity(bounds_collection.len());
    for (template, bounds) in bounds_collection {
        agg_min = agg_min.min(bounds.min);
        agg_max = agg_max.max(bounds.max);
        total_triangles += bounds.triangles;
        total_vertices += bounds.vertices;
        instances.push(template.clone());
    }

    let center = (agg_min + agg_max) * 0.5;
    let corners = [
        Vec3::new(agg_min.x, agg_min.y, agg_min.z),
        Vec3::new(agg_max.x, agg_min.y, agg_min.z),
        Vec3::new(agg_min.x, agg_max.y, agg_min.z),
        Vec3::new(agg_max.x, agg_max.y, agg_min.z),
        Vec3::new(agg_min.x, agg_min.y, agg_max.z),
        Vec3::new(agg_max.x, agg_min.y, agg_max.z),
        Vec3::new(agg_min.x, agg_max.y, agg_max.z),
        Vec3::new(agg_max.x, agg_max.y, agg_max.z),
    ];
    let mut radius: f32 = 0.0;
    for corner in &corners {
        radius = radius.max((*corner - center).length());
    }

    GeometryBuild {
        instances,
        transform_nodes: runtime.transform_nodes,
        snap_points: runtime.snap_points,
        center,
        radius,
        triangle_count: total_triangles,
        vertex_count: total_vertices,
    }
}

fn merge_hlod_aggregate(
    aggregate: &HlodAggregateEntry,
    assets: &AssetManager,
    runtime: &mut RuntimeAggregate,
) {
    for model in &aggregate.models {
        let mut visited = HashSet::new();
        runtime.merge(collect_runtime_data(
            model.name.as_str(),
            assets,
            0,
            Mat4::IDENTITY,
            &mut visited,
        ));
    }
}
