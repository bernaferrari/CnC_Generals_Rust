use serde_json::{Value as JsonValue, json};

pub(crate) fn push_view_json(
    root: &mut JsonValue,
    buffer: usize,
    byte_offset: u32,
    byte_length: u32,
) -> usize {
    let views = root
        .get_mut("bufferViews")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    views.push(json!({ "buffer": buffer, "byteOffset": byte_offset, "byteLength": byte_length }));
    views.len() - 1
}

pub(crate) fn push_view_json_with_target(
    root: &mut JsonValue,
    buffer: usize,
    byte_offset: u32,
    byte_length: u32,
    target: u32,
) -> usize {
    let views = root
        .get_mut("bufferViews")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    views.push(json!({ "buffer": buffer, "byteOffset": byte_offset, "byteLength": byte_length, "target": target }));
    views.len() - 1
}

pub(crate) fn push_accessor_vec3_json(root: &mut JsonValue, view: usize, count: u32) -> usize {
    push_accessor_vec3_json_with_bounds(root, view, count, None)
}

pub(crate) fn push_accessor_vec3_f32_json(root: &mut JsonValue, view: usize, count: u32) -> usize {
    push_accessor_vec3_json(root, view, count)
}

pub(crate) fn push_accessor_vec2_json(root: &mut JsonValue, view: usize, count: u32) -> usize {
    let accessors = root
        .get_mut("accessors")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    accessors
        .push(json!({ "bufferView": view, "componentType": 5126, "count": count, "type": "VEC2" }));
    accessors.len() - 1
}

pub(crate) fn push_accessor_scalar_u32_json(
    root: &mut JsonValue,
    view: usize,
    count: u32,
) -> usize {
    let accessors = root
        .get_mut("accessors")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    accessors.push(
        json!({ "bufferView": view, "componentType": 5125, "count": count, "type": "SCALAR" }),
    );
    accessors.len() - 1
}

pub(crate) fn push_accessor_scalar_u16_json(
    root: &mut JsonValue,
    view: usize,
    count: u32,
) -> usize {
    let accessors = root
        .get_mut("accessors")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    accessors.push(
        json!({ "bufferView": view, "componentType": 5123, "count": count, "type": "SCALAR" }),
    );
    accessors.len() - 1
}

pub(crate) fn push_accessor_vec4_u16_json(root: &mut JsonValue, view: usize, count: u32) -> usize {
    let accessors = root
        .get_mut("accessors")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    accessors
        .push(json!({ "bufferView": view, "componentType": 5123, "count": count, "type": "VEC4" }));
    accessors.len() - 1
}

pub(crate) fn push_accessor_vec4_f32_json(root: &mut JsonValue, view: usize, count: u32) -> usize {
    let accessors = root
        .get_mut("accessors")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    accessors
        .push(json!({ "bufferView": view, "componentType": 5126, "count": count, "type": "VEC4" }));
    accessors.len() - 1
}

pub(crate) fn push_accessor_mat4_f32_json(root: &mut JsonValue, view: usize, count: u32) -> usize {
    let accessors = root
        .get_mut("accessors")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    accessors
        .push(json!({ "bufferView": view, "componentType": 5126, "count": count, "type": "MAT4" }));
    accessors.len() - 1
}

pub(crate) fn push_accessor_vec3_json_with_bounds(
    root: &mut JsonValue,
    view: usize,
    count: u32,
    minmax: Option<([f32; 3], [f32; 3])>,
) -> usize {
    let accessors = root
        .get_mut("accessors")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    let mut acc =
        json!({ "bufferView": view, "componentType": 5126, "count": count, "type": "VEC3" });
    if let Some((min, max)) = minmax {
        if let Some(obj) = acc.as_object_mut() {
            obj.insert("min".into(), json!(min));
            obj.insert("max".into(), json!(max));
        }
    }
    accessors.push(acc);
    accessors.len() - 1
}

pub(crate) fn push_accessor_scalar_f32_json(
    root: &mut JsonValue,
    view: usize,
    count: u32,
) -> usize {
    let accessors = root
        .get_mut("accessors")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    accessors.push(
        json!({ "bufferView": view, "componentType": 5126, "count": count, "type": "SCALAR" }),
    );
    accessors.len() - 1
}

pub(crate) fn push_accessor_scalar_f32_with_bounds(
    root: &mut JsonValue,
    view: usize,
    count: u32,
    min_val: f32,
    max_val: f32,
) -> usize {
    let accessors = root
        .get_mut("accessors")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    accessors.push(json!({
        "bufferView": view,
        "componentType": 5126,
        "count": count,
        "type": "SCALAR",
        "min": [min_val],
        "max": [max_val]
    }));
    accessors.len() - 1
}

pub(crate) fn compute_min_max_vec3(
    vertices: &[crate::w3d::structs::W3dVector],
) -> Option<([f32; 3], [f32; 3])> {
    if vertices.is_empty() {
        return None;
    }
    let mut min = [vertices[0].x, vertices[0].y, vertices[0].z];
    let mut max = min;
    for v in vertices {
        if v.x < min[0] {
            min[0] = v.x;
        }
        if v.y < min[1] {
            min[1] = v.y;
        }
        if v.z < min[2] {
            min[2] = v.z;
        }
        if v.x > max[0] {
            max[0] = v.x;
        }
        if v.y > max[1] {
            max[1] = v.y;
        }
        if v.z > max[2] {
            max[2] = v.z;
        }
    }
    Some((min, max))
}
