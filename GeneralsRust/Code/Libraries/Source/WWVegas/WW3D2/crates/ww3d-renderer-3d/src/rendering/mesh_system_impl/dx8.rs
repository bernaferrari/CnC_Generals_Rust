#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;

/// Polygon renderer class - manages GPU rendering for mesh polygons
#[derive(Debug)]
pub struct DX8PolygonRendererClass {
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub primitive_type: wgpu::PrimitiveTopology,
    pub texture_category: Option<Arc<DX8TextureCategoryClass>>,
    pub material_pass: Option<Arc<MaterialPassClass>>,
    pub shader: ShaderClass,
    pub vertex_material: Option<Arc<VertexMaterialClass>>,
}

impl Default for DX8PolygonRendererClass {
    fn default() -> Self {
        Self::new()
    }
}

impl DX8PolygonRendererClass {
    pub fn new() -> Self {
        Self {
            vertex_buffer: None,
            index_buffer: None,
            vertex_count: 0,
            index_count: 0,
            primitive_type: wgpu::PrimitiveTopology::TriangleList,
            texture_category: None,
            material_pass: None,
            shader: ShaderClass::default(),
            vertex_material: None,
        }
    }

    /// Render a material pass
    pub fn render_material_pass<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        _transform: &Mat4,
        _render_info: &RenderInfoClass,
    ) -> W3dResult<()> {
        if let Some(vertex_buffer) = &self.vertex_buffer {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        }
        if let Some(index_buffer) = &self.index_buffer {
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.index_count, 0, 0..1);
        } else {
            render_pass.draw(0..self.vertex_count, 0..1);
        }
        Ok(())
    }

    pub fn set_texture_category(&mut self, category: Arc<DX8TextureCategoryClass>) {
        self.texture_category = Some(category);
    }

    pub fn get_texture_category(&self) -> Option<&Arc<DX8TextureCategoryClass>> {
        self.texture_category.as_ref()
    }
}

/// Texture category for organizing rendering by texture/shader combinations
#[derive(Debug)]
pub struct DX8TextureCategoryClass {
    pub pass: u32,
    pub textures: Vec<Option<Arc<TextureClass>>>,
    pub shader: ShaderClass,
    pub material: Option<Arc<VertexMaterialClass>>,
    pub polygon_renderers: Vec<Arc<DX8PolygonRendererClass>>,
    pub(super) render_tasks: Mutex<Vec<MeshRenderTask>>,
}

impl DX8TextureCategoryClass {
    pub fn new(
        textures: Vec<Option<Arc<TextureClass>>>,
        shader: ShaderClass,
        material: Option<Arc<VertexMaterialClass>>,
        pass: u32,
    ) -> Self {
        Self {
            pass,
            textures,
            shader,
            material,
            polygon_renderers: Vec::new(),
            render_tasks: Mutex::new(Vec::new()),
        }
    }

    pub fn add_polygon_renderer(&mut self, renderer: Arc<DX8PolygonRendererClass>) {
        self.polygon_renderers.push(renderer);
    }

    pub fn add_render_task(
        &mut self,
        polygon_renderer: Arc<DX8PolygonRendererClass>,
        mesh: Arc<MeshClass>,
    ) {
        if let Ok(mut guard) = self.render_tasks.lock() {
            guard.push(MeshRenderTask {
                polygon_renderer,
                mesh,
            });
        }
    }

    pub fn clear_render_tasks(&mut self) {
        if let Ok(mut guard) = self.render_tasks.lock() {
            guard.clear();
        }
    }

    pub fn has_render_tasks(&self) -> bool {
        self.render_tasks
            .lock()
            .map(|tasks| !tasks.is_empty())
            .unwrap_or(false)
    }
}

/// Render task for mesh rendering
#[derive(Debug, Clone)]
pub struct MeshRenderTask {
    pub polygon_renderer: Arc<DX8PolygonRendererClass>,
    pub mesh: Arc<MeshClass>,
}

/// FVF (Flexible Vertex Format) category container
#[derive(Debug)]
pub struct DX8FVFCategoryContainer {
    pub texture_categories: HashMap<(u32, String), Arc<DX8TextureCategoryClass>>,
}

impl Default for DX8FVFCategoryContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl DX8FVFCategoryContainer {
    pub fn new() -> Self {
        Self {
            texture_categories: HashMap::new(),
        }
    }

    pub fn get_or_create_texture_category(
        &mut self,
        textures: Vec<Option<Arc<TextureClass>>>,
        shader: ShaderClass,
        material: Option<Arc<VertexMaterialClass>>,
        pass: u32,
    ) -> Arc<DX8TextureCategoryClass> {
        let key = (
            pass,
            format!(
                "{:?}",
                (
                    shader,
                    material
                        .as_ref()
                        .map(|m| m.name.clone())
                        .unwrap_or_default()
                )
            ),
        );

        if let Some(category) = self.texture_categories.get(&key) {
            return category.clone();
        }

        let category = Arc::new(DX8TextureCategoryClass::new(
            textures, shader, material, pass,
        ));

        self.texture_categories.insert(key, category.clone());
        category
    }
}
