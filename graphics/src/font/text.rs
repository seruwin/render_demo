use crate::{
    AscendingError, Color, DrawOrder, GpuRenderer, Index, OrderedIndex,
    TextAtlas, TextVertex, Vec2, Vec3, Vec4,
};
use cosmic_text::{
    Attrs, Buffer, FontSystem, Metrics, SwashCache, SwashContent,
};

/// Controls the visible area of the text. Any text outside of the visible area will be clipped.
/// This is given by glyphon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextBounds(pub Vec4);

impl TextBounds {
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self(Vec4::new(left, top, right, bottom))
    }
}

impl Default for TextBounds {
    fn default() -> Self {
        Self(Vec4::new(f32::MIN, f32::MIN, f32::MAX, f32::MAX))
    }
}

pub struct Text {
    pub buffer: Buffer<'static>,
    pub pos: Vec3,
    pub size: Vec2,
    pub default_color: Color,
    pub bounds: TextBounds,
    pub store_id: Index,
    pub order: DrawOrder,
    /// if the shader should render with the camera's view.
    pub use_camera: bool,
    /// if anything got updated we need to update the buffers too.
    pub changed: bool,
}

impl Text {
    pub fn create_quad(
        &mut self,
        font_system: &FontSystem,
        cache: &mut SwashCache,
        atlas: &mut TextAtlas,
        renderer: &mut GpuRenderer,
    ) -> Result<(), AscendingError> {
        for run in self.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                if atlas.text.atlas.get(&glyph.cache_key).is_some()
                    || atlas.emoji.atlas.get(&glyph.cache_key).is_some()
                {
                    continue;
                }

                let image = cache
                    .get_image_uncached(font_system, glyph.cache_key)
                    .unwrap();
                let bitmap = image.data;
                let is_color = match image.content {
                    SwashContent::Color => true,
                    SwashContent::Mask => false,
                    SwashContent::SubpixelMask => false,
                };

                let width = image.placement.width;
                let height = image.placement.height;

                if width > 0 && height > 0 {
                    if is_color {
                        let _ = atlas
                            .emoji
                            .atlas
                            .upload(
                                glyph.cache_key,
                                &bitmap,
                                width,
                                height,
                                Vec2::new(
                                    image.placement.left as f32,
                                    image.placement.top as f32,
                                ),
                                renderer,
                            )
                            .ok_or(AscendingError::AtlasFull)?;
                    } else {
                        let _ = atlas
                            .text
                            .atlas
                            .upload(
                                glyph.cache_key,
                                &bitmap,
                                width,
                                height,
                                Vec2::new(
                                    image.placement.left as f32,
                                    image.placement.top as f32,
                                ),
                                renderer,
                            )
                            .ok_or(AscendingError::AtlasFull)?;
                    }
                }
            }
        }

        let mut text_buf = Vec::with_capacity(64 * 4);

        for run in self.buffer.layout_runs() {
            let line_y = run.line_y;

            for glyph in run.glyphs.iter() {
                let (allocation, is_color) = if let Some(allocation) =
                    atlas.text.atlas.get(&glyph.cache_key)
                {
                    (allocation, false)
                } else if let Some(allocation) =
                    atlas.emoji.atlas.get(&glyph.cache_key)
                {
                    (allocation, true)
                } else {
                    continue;
                };

                let position = allocation.data;
                let (u, v, width, height) = allocation.rect();
                let (mut u, mut v, mut width, mut height) =
                    (u as f32, v as f32, width as f32, height as f32);

                let (mut x, mut y) = (
                    (self.pos.x + glyph.x_int as f32 + position.x),
                    (self.pos.y + glyph.y_int as f32 - line_y),
                );

                let color = is_color
                    .then(|| Color::rgba(255, 255, 255, 255))
                    .unwrap_or(match glyph.color_opt {
                        Some(color) => color,
                        None => self.default_color,
                    });

                let screensize = renderer.size();
                //Bounds used from Glyphon
                let bounds_min_x = self.bounds.0.x.max(0.0);
                let bounds_min_y = self.bounds.0.w.max(0.0);
                let bounds_max_x = self.bounds.0.z.min(screensize.width);
                let bounds_max_y = self.bounds.0.y.min(screensize.height);

                // Starts beyond right edge or ends beyond left edge
                let max_x = x + width;
                if x > bounds_max_x || max_x < bounds_min_x {
                    continue;
                }

                // Starts beyond bottom edge or ends beyond top edge
                let max_y = y + height;
                if y > bounds_max_y || max_y < bounds_min_y {
                    continue;
                }

                // Clip left edge
                if x < bounds_min_x {
                    let right_shift = bounds_min_x - x;

                    x = bounds_min_x;
                    width = max_x - bounds_min_x;
                    u += right_shift;
                }

                // Clip right edge
                if x + width > bounds_max_x {
                    width = bounds_max_x - x;
                }

                // Clip top edge
                if y < bounds_min_y {
                    height -= bounds_min_y;
                    y = bounds_min_y;
                }

                // Clip top edge
                if y + height > bounds_max_y {
                    let bottom_shift = (y + height) - bounds_max_y;

                    v += bottom_shift;
                    height -= bottom_shift;
                }

                let default = TextVertex {
                    position: [x, y, self.pos.z],
                    hw: [width, height],
                    tex_coord: [u, v],
                    layer: allocation.layer as u32,
                    color: color.0,
                    use_camera: u32::from(self.use_camera),
                    is_color: is_color as u32,
                };

                text_buf.push(default);
            }
        }

        if let Some(store) = renderer.get_buffer_mut(&self.store_id) {
            store.store = bytemuck::cast_slice(&text_buf).to_vec();
            store.changed = true;
        }

        self.order = DrawOrder::new(false, &self.pos, 1);
        self.changed = false;
        Ok(())
    }

    pub fn new(
        renderer: &mut GpuRenderer,
        font_system: &'static FontSystem,
        metrics: Option<Metrics>,
        pos: Vec3,
        size: Vec2,
        bounds: Option<TextBounds>,
    ) -> Self {
        Self {
            buffer: Buffer::new(
                font_system,
                metrics.unwrap_or(Metrics::new(16.0, 16.0).scale(1.0)),
            ),
            pos,
            size,
            bounds: bounds.unwrap_or_default(),
            store_id: renderer.new_buffer(),
            order: DrawOrder::new(false, &pos, 1),
            changed: true,
            default_color: Color::rgba(0, 0, 0, 255),
            use_camera: false,
        }
    }

    /// resets the TextRender bytes to empty for new bytes
    pub fn set_text(&mut self, text: &str, attrs: Attrs<'static>) {
        self.buffer.set_text(text, attrs);
        self.changed = true;
    }

    pub fn set_buffer_size(&mut self, width: i32, height: i32) {
        self.buffer.set_size(width as f32, height as f32);
        self.changed = true;
    }

    /// resets the TextRender bytes to empty for new bytes
    pub fn clear(&mut self) {
        self.buffer.set_text("", cosmic_text::Attrs::new());
        self.changed = true;
    }

    /// used to check and update the vertex array.
    /// must call build_layout before you can Call this.
    pub fn update(
        &mut self,
        font_system: &FontSystem,
        cache: &mut SwashCache,
        atlas: &mut TextAtlas,
        renderer: &mut GpuRenderer,
    ) -> Result<OrderedIndex, AscendingError> {
        if self.changed {
            self.create_quad(font_system, cache, atlas, renderer)?;
        }

        Ok(OrderedIndex::new(self.order, self.store_id))
    }
}
