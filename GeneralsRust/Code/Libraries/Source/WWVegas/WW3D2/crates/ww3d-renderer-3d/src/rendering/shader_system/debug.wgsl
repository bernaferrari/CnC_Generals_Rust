// Debug Visualization Shaders
//
// Wireframe, normals, LOD visualization, and other debug modes.

// ============================================================================
// Wireframe Rendering
// ============================================================================

struct WireframeUniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    color: vec3<f32>,
    line_width: f32,
}

@group(0) @binding(0)
var<uniform> wireframe_uniforms: WireframeUniforms;

struct WireframeVertexInput {
    @location(0) position: vec3<f32>,
}

struct WireframeVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn wireframe_vertex_main(in: WireframeVertexInput) -> WireframeVertexOutput {
    var out: WireframeVertexOutput;

    let world_pos = wireframe_uniforms.model * vec4<f32>(in.position, 1.0);
    out.clip_position = wireframe_uniforms.view_proj * world_pos;
    out.color = wireframe_uniforms.color;

    return out;
}

@fragment
fn wireframe_fragment_main(in: WireframeVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}

// ============================================================================
// Normal Visualization
// ============================================================================

struct NormalsVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct NormalsVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
}

@vertex
fn normals_vertex_main(in: NormalsVertexInput) -> NormalsVertexOutput {
    var out: NormalsVertexOutput;

    let world_pos = wireframe_uniforms.model * vec4<f32>(in.position, 1.0);
    out.clip_position = wireframe_uniforms.view_proj * world_pos;

    // Transform normal to world space
    let normal_matrix = mat3x3<f32>(
        wireframe_uniforms.model[0].xyz,
        wireframe_uniforms.model[1].xyz,
        wireframe_uniforms.model[2].xyz
    );
    out.normal = normalize(normal_matrix * in.normal);

    return out;
}

@fragment
fn normals_fragment_main(in: NormalsVertexOutput) -> @location(0) vec4<f32> {
    // Map normal from [-1, 1] to [0, 1] for color visualization
    // Red = X axis, Green = Y axis, Blue = Z axis
    let normal_color = in.normal * 0.5 + 0.5;
    return vec4<f32>(normal_color, 1.0);
}

// ============================================================================
// LOD Visualization
// ============================================================================

struct LodUniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    lod_color: vec3<f32>,
    lod_level: f32,
}

@group(0) @binding(0)
var<uniform> lod_uniforms: LodUniforms;

struct LodVertexInput {
    @location(0) position: vec3<f32>,
}

struct LodVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn lod_vertex_main(in: LodVertexInput) -> LodVertexOutput {
    var out: LodVertexOutput;

    let world_pos = lod_uniforms.model * vec4<f32>(in.position, 1.0);
    out.clip_position = lod_uniforms.view_proj * world_pos;
    out.color = lod_uniforms.lod_color;

    return out;
}

@fragment
fn lod_fragment_main(in: LodVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}

// ============================================================================
// Texture Coordinate Visualization
// ============================================================================

struct UvVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

struct UvVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn uv_vertex_main(in: UvVertexInput) -> UvVertexOutput {
    var out: UvVertexOutput;

    let world_pos = wireframe_uniforms.model * vec4<f32>(in.position, 1.0);
    out.clip_position = wireframe_uniforms.view_proj * world_pos;
    out.uv = in.uv;

    return out;
}

@fragment
fn uv_fragment_main(in: UvVertexOutput) -> @location(0) vec4<f32> {
    // Red = U coordinate, Green = V coordinate
    return vec4<f32>(in.uv.x, in.uv.y, 0.0, 1.0);
}

// ============================================================================
// Vertex Color Visualization
// ============================================================================

struct VertexColorInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
}

struct VertexColorOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vertex_color_vertex_main(in: VertexColorInput) -> VertexColorOutput {
    var out: VertexColorOutput;

    let world_pos = wireframe_uniforms.model * vec4<f32>(in.position, 1.0);
    out.clip_position = wireframe_uniforms.view_proj * world_pos;
    out.color = in.color;

    return out;
}

@fragment
fn vertex_color_fragment_main(in: VertexColorOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}

// ============================================================================
// Collision Mesh Rendering
// ============================================================================

struct CollisionUniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    color: vec3<f32>,
    alpha: f32,
}

@group(0) @binding(0)
var<uniform> collision_uniforms: CollisionUniforms;

struct CollisionVertexInput {
    @location(0) position: vec3<f32>,
}

struct CollisionVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn collision_vertex_main(in: CollisionVertexInput) -> CollisionVertexOutput {
    var out: CollisionVertexOutput;

    let world_pos = collision_uniforms.model * vec4<f32>(in.position, 1.0);
    out.clip_position = collision_uniforms.view_proj * world_pos;
    out.color = vec4<f32>(collision_uniforms.color, collision_uniforms.alpha);

    return out;
}

@fragment
fn collision_fragment_main(in: CollisionVertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

// ============================================================================
// Bounding Box Rendering
// ============================================================================

struct BoundingBoxUniforms {
    view_proj: mat4x4<f32>,
    min_point: vec3<f32>,
    _padding1: f32,
    max_point: vec3<f32>,
    _padding2: f32,
    color: vec3<f32>,
    line_width: f32,
}

@group(0) @binding(0)
var<uniform> bbox_uniforms: BoundingBoxUniforms;

// Bounding box is rendered as 12 lines (line list topology)
@vertex
fn bbox_vertex_main(@builtin(vertex_index) vertex_index: u32) -> WireframeVertexOutput {
    var out: WireframeVertexOutput;

    // Generate box vertices (8 corners)
    let corners = array<vec3<f32>, 8>(
        vec3<f32>(bbox_uniforms.min_point.x, bbox_uniforms.min_point.y, bbox_uniforms.min_point.z),
        vec3<f32>(bbox_uniforms.max_point.x, bbox_uniforms.min_point.y, bbox_uniforms.min_point.z),
        vec3<f32>(bbox_uniforms.max_point.x, bbox_uniforms.max_point.y, bbox_uniforms.min_point.z),
        vec3<f32>(bbox_uniforms.min_point.x, bbox_uniforms.max_point.y, bbox_uniforms.min_point.z),
        vec3<f32>(bbox_uniforms.min_point.x, bbox_uniforms.min_point.y, bbox_uniforms.max_point.z),
        vec3<f32>(bbox_uniforms.max_point.x, bbox_uniforms.min_point.y, bbox_uniforms.max_point.z),
        vec3<f32>(bbox_uniforms.max_point.x, bbox_uniforms.max_point.y, bbox_uniforms.max_point.z),
        vec3<f32>(bbox_uniforms.min_point.x, bbox_uniforms.max_point.y, bbox_uniforms.max_point.z),
    );

    // Map vertex index to corner (for line list rendering)
    let corner_indices = array<u32, 24>(
        0u, 1u, 1u, 2u, 2u, 3u, 3u, 0u,  // Bottom face
        4u, 5u, 5u, 6u, 6u, 7u, 7u, 4u,  // Top face
        0u, 4u, 1u, 5u, 2u, 6u, 3u, 7u   // Vertical edges
    );

    let corner = corners[corner_indices[vertex_index]];
    out.clip_position = bbox_uniforms.view_proj * vec4<f32>(corner, 1.0);
    out.color = bbox_uniforms.color;

    return out;
}

// ============================================================================
// Grid Rendering
// ============================================================================

struct GridUniforms {
    view_proj: mat4x4<f32>,
    grid_size: f32,
    grid_divisions: u32,
    color: vec3<f32>,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> grid_uniforms: GridUniforms;

@vertex
fn grid_vertex_main(@builtin(vertex_index) vertex_index: u32) -> WireframeVertexOutput {
    var out: WireframeVertexOutput;

    let half_size = grid_uniforms.grid_size * 0.5;
    let step = grid_uniforms.grid_size / f32(grid_uniforms.grid_divisions);

    // Generate grid lines (on XZ plane, Y = 0)
    let total_lines = (grid_uniforms.grid_divisions + 1u) * 2u;
    let line_index = vertex_index / 2u;
    let point_index = vertex_index % 2u;

    var position: vec3<f32>;

    if (line_index < grid_uniforms.grid_divisions + 1u) {
        // Lines parallel to X axis
        let z = -half_size + f32(line_index) * step;
        if (point_index == 0u) {
            position = vec3<f32>(-half_size, 0.0, z);
        } else {
            position = vec3<f32>(half_size, 0.0, z);
        }
    } else {
        // Lines parallel to Z axis
        let adjusted_index = line_index - (grid_uniforms.grid_divisions + 1u);
        let x = -half_size + f32(adjusted_index) * step;
        if (point_index == 0u) {
            position = vec3<f32>(x, 0.0, -half_size);
        } else {
            position = vec3<f32>(x, 0.0, half_size);
        }
    }

    out.clip_position = grid_uniforms.view_proj * vec4<f32>(position, 1.0);
    out.color = grid_uniforms.color;

    return out;
}

// ============================================================================
// Axes Rendering (XYZ as RGB)
// ============================================================================

struct AxesUniforms {
    view_proj: mat4x4<f32>,
    origin: vec3<f32>,
    length: f32,
}

@group(0) @binding(0)
var<uniform> axes_uniforms: AxesUniforms;

@vertex
fn axes_vertex_main(@builtin(vertex_index) vertex_index: u32) -> WireframeVertexOutput {
    var out: WireframeVertexOutput;

    // 6 vertices for 3 axes (line list)
    let axis_index = vertex_index / 2u;
    let point_index = vertex_index % 2u;

    var position: vec3<f32>;
    var color: vec3<f32>;

    if (axis_index == 0u) {
        // X axis (Red)
        if (point_index == 0u) {
            position = axes_uniforms.origin;
        } else {
            position = axes_uniforms.origin + vec3<f32>(axes_uniforms.length, 0.0, 0.0);
        }
        color = vec3<f32>(1.0, 0.0, 0.0);
    } else if (axis_index == 1u) {
        // Y axis (Green)
        if (point_index == 0u) {
            position = axes_uniforms.origin;
        } else {
            position = axes_uniforms.origin + vec3<f32>(0.0, axes_uniforms.length, 0.0);
        }
        color = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        // Z axis (Blue)
        if (point_index == 0u) {
            position = axes_uniforms.origin;
        } else {
            position = axes_uniforms.origin + vec3<f32>(0.0, 0.0, axes_uniforms.length);
        }
        color = vec3<f32>(0.0, 0.0, 1.0);
    }

    out.clip_position = axes_uniforms.view_proj * vec4<f32>(position, 1.0);
    out.color = color;

    return out;
}

// ============================================================================
// Overdraw Visualization
// ============================================================================

// Uses additive blending to show overdraw
// Each fragment adds a small amount of red
@fragment
fn overdraw_fragment_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.1, 0.0, 0.0, 1.0);
}
