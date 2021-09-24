#![allow(dead_code)]
use camera::controls::{OrbitControls, OrbitSettings};
use camera::Projection;
use input::{Bindings, FrameTime, InputHandler};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod graphics;
mod state;

use graphics::*;
use state::*;

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
enum Action {
    Quit,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
enum Axis {
    Forward,
    Sideward,
    Yaw,
    Pitch,
}

#[tokio::main]
async fn main() -> Result<(), RendererError> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Demo")
        .build(&event_loop)
        .unwrap();

    let backends = wgpu::Backends::PRIMARY;
    let instance = wgpu::Instance::new(backends);

    let mut renderer = instance
        .create_renderer(
            window,
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
            },
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::TEXTURE_BINDING_ARRAY,
                limits: wgpu::Limits::default(),
                label: None,
            },
            None,
            wgpu::PresentMode::Fifo,
        )
        .await
        .unwrap();

    let mut layout_storage = LayoutStorage::new();
    let mut sprite_atlas = Atlas::new(renderer.device(), 2048);
    let texture = Texture::from_file("images/Tree.png")?;

    let mut encoder = renderer
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("command encoder"),
        });

    let allocation = sprite_atlas
        .upload(&texture, renderer.device(), &mut encoder)
        .ok_or_else(|| OtherError::new("failed to upload image"))?;
    let mut sprite = Sprite::new(allocation);
    renderer.queue().submit(std::iter::once(encoder.finish()));

    sprite.pos[0] = 64;
    sprite.pos[1] = 64;
    sprite.pos[2] = 1;
    sprite.uv = [0, 0, 80, 80];
    sprite.changed = true;

    let sprite_texture =
        TextureGroup::from_atlas(renderer.device(), &mut layout_storage, &sprite_atlas);

    let sprite_pipeline = SpriteRenderPipeline::new(
        renderer.device(),
        renderer.surface_format(),
        &mut layout_storage,
    )?;

    let settings = OrbitSettings {
        zoom_speed: 0.1,
        ..Default::default()
    };

    let controls = OrbitControls::new(settings, [0.0; 3], 3.0);
    let camera = Camera::new(
        &renderer,
        &mut layout_storage,
        Projection::Perspective {
            fov: (90.0_f32).to_radians(),
            aspect_ratio: 1920.0 / 1080.0,
            near: 0.1,
            far: 100.0,
        },
        controls,
    );

    let sprite_buffer = SpriteBuffer::new(renderer.device());
    let mut state = State {
        sprite,
        sprite_pipeline,
        sprite_atlas,
        layout_storage,
        sprite_texture,
        camera,
        sprite_buffer,
    };

    let mut views = HashMap::new();

    let size = wgpu::Extent3d {
        width: renderer.size().width,
        height: renderer.size().height,
        depth_or_array_layers: 1,
    };

    let texture = renderer.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("depth texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut size = renderer.size();

    views.insert("depthbuffer".to_string(), view);

    let mut bindings = Bindings::<Action, Axis>::new();
    bindings.insert_action(
        Action::Quit,
        vec![winit::event::VirtualKeyCode::Q.into()].into_iter(),
    );
    let mut input_handler = InputHandler::new(bindings);

    let mut frame_time = FrameTime::new();

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
                ..
            } if window_id == renderer.window().id() => {
                if let WindowEvent::CloseRequested = *event {
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }

        if size != renderer.size() {
            size = renderer.size();

            state.camera.set_projection(Projection::Perspective {
                fov: (90.0_f32).to_radians(),
                aspect_ratio: (size.width as f32) / (size.height as f32),
                near: 0.1,
                far: 100.0,
            });

            let size = wgpu::Extent3d {
                width: renderer.size().width,
                height: renderer.size().height,
                depth_or_array_layers: 1,
            };

            let texture = renderer.device().create_texture(&wgpu::TextureDescriptor {
                label: Some("depth texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            views.insert("depthbuffer".to_string(), view);
        }

        input_handler.update(renderer.window(), &event, 1.0);

        let frame = match renderer.update(&event).unwrap() {
            Some(frame) => frame,
            _ => return,
        };

        if input_handler.is_action_down(&Action::Quit) {
            *control_flow = ControlFlow::Exit;
        }

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        views.insert("framebuffer".to_string(), view);

        state.sprite.update();
        let indices = vec![0, 1, 2, 1, 2, 3];
        let build = VertexBufferBuilder::new(state.sprite.buffer.clone()).with_indices(indices);
        let vertex = build.build(renderer.device());

        // Start encoding commands.
        let mut encoder =
            renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("command encoder"),
                });

        //gotta update our buffers before we render.
        state.sprite_buffer.copy_to_vertex(
            &mut encoder,
            vertex.vertex_buffer(),
            vertex.vertices().len(),
        );

        state.sprite_buffer.copy_to_indice(
            &mut encoder,
            vertex.indice_buffer(),
            vertex.indices().len(),
        );

        // Run the render pass.
        state.render(&mut encoder, &views);

        // Submit our command queue.
        renderer.queue().submit(std::iter::once(encoder.finish()));

        views.remove("framebuffer");

        input_handler.end_frame();
        frame_time.update();
    });
}
