use crate::{
    AsBufferPass, AscendingError, AtlasGroup, GpuRenderer, InstanceBuffer, Map,
    MapLayout, MapRenderPipeline, MapTextures, MapVertex, OrderedIndex,
    SetBuffers, StaticBufferObject, TextureGroup,
};

pub struct MapRenderer {
    pub maplower_buffer: InstanceBuffer<MapVertex>,
    pub mapupper_buffer: InstanceBuffer<MapVertex>,
    /// Texture Bind for maps tile placements.
    pub map_group: TextureGroup,
    /// contains the Map layer grids in pixel form.
    pub map_textures: MapTextures,
}

impl MapRenderer {
    pub fn new(
        renderer: &mut GpuRenderer,
        map_count: u32,
    ) -> Result<Self, AscendingError> {
        let map_textures = MapTextures::new(renderer, map_count);

        Ok(Self {
            maplower_buffer: InstanceBuffer::with_capacity(
                renderer.gpu_device(),
                30 * map_count as usize,
            ),
            mapupper_buffer: InstanceBuffer::with_capacity(
                renderer.gpu_device(),
                15 * map_count as usize,
            ),
            map_group: TextureGroup::from_view(
                renderer,
                &map_textures.texture_view,
                MapLayout,
            ),
            map_textures,
        })
    }

    pub fn add_buffer_store(
        &mut self,
        renderer: &GpuRenderer,
        index: (OrderedIndex, OrderedIndex),
    ) {
        self.maplower_buffer.add_buffer_store(renderer, index.0);
        self.mapupper_buffer.add_buffer_store(renderer, index.1);
    }

    pub fn finalize(&mut self, renderer: &mut GpuRenderer) {
        self.maplower_buffer.finalize(renderer);
        self.mapupper_buffer.finalize(renderer);
    }

    pub fn map_update(&mut self, map: &mut Map, renderer: &mut GpuRenderer) {
        let index = map.update(renderer, &mut self.map_textures);

        self.add_buffer_store(renderer, index);
    }

    pub fn get_unused_id(&mut self) -> Option<u32> {
        self.map_textures.get_unused_id()
    }

    pub fn mark_id_unused(&mut self, id: u32) {
        self.map_textures.mark_id_unused(id)
    }
}

pub trait RenderMap<'a, 'b>
where
    'b: 'a,
{
    fn render_lower_maps(
        &mut self,
        renderer: &'b GpuRenderer,
        buffer: &'b MapRenderer,
        atlas_group: &'b AtlasGroup,
    );

    fn render_upper_maps(
        &mut self,
        renderer: &'b GpuRenderer,
        buffer: &'b MapRenderer,
        atlas_group: &'b AtlasGroup,
    );
}

impl<'a, 'b> RenderMap<'a, 'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn render_lower_maps(
        &mut self,
        renderer: &'b GpuRenderer,
        buffer: &'b MapRenderer,
        atlas_group: &'b AtlasGroup,
    ) {
        if buffer.maplower_buffer.count() > 0 {
            self.set_buffers(renderer.buffer_object.as_buffer_pass());
            self.set_bind_group(1, &atlas_group.texture.bind_group, &[]);
            self.set_bind_group(2, &buffer.map_group.bind_group, &[]);
            self.set_vertex_buffer(1, buffer.maplower_buffer.instances(None));
            self.set_pipeline(
                renderer.get_pipelines(MapRenderPipeline).unwrap(),
            );
            self.draw_indexed(
                0..StaticBufferObject::index_count(),
                0,
                0..buffer.maplower_buffer.count(),
            );
        }
    }

    fn render_upper_maps(
        &mut self,
        renderer: &'b GpuRenderer,
        buffer: &'b MapRenderer,
        atlas_group: &'b AtlasGroup,
    ) {
        if buffer.mapupper_buffer.count() > 0 {
            self.set_buffers(renderer.buffer_object.as_buffer_pass());
            self.set_bind_group(1, &atlas_group.texture.bind_group, &[]);
            self.set_bind_group(2, &buffer.map_group.bind_group, &[]);
            self.set_vertex_buffer(1, buffer.mapupper_buffer.instances(None));
            self.set_pipeline(
                renderer.get_pipelines(MapRenderPipeline).unwrap(),
            );
            self.draw_indexed(
                0..StaticBufferObject::index_count(),
                0,
                0..buffer.mapupper_buffer.count(),
            );
        }
    }
}
