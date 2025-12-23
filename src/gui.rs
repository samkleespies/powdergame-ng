use egui::{ClippedPrimitive, Context, TexturesDelta};
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use pixels::{wgpu, PixelsContext};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;

pub struct Gui {
    pub egui_ctx: Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    renderer: Renderer,
    paint_jobs: Vec<ClippedPrimitive>,
    textures: TexturesDelta,
}

impl Gui {
    pub fn new<T>(event_loop: &EventLoopWindowTarget<T>, rpass: &PixelsContext, width: u32, height: u32, scale_factor: f32) -> Self {
        let max_texture_size = rpass.device.limits().max_texture_dimension_2d as usize;

        let egui_ctx = Context::default();
        let mut egui_state = egui_winit::State::new(event_loop);
        egui_state.set_max_texture_side(max_texture_size);
        egui_state.set_pixels_per_point(scale_factor);

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };
        let renderer = Renderer::new(&rpass.device, rpass.texture_format, None, 1);
        let textures = TexturesDelta::default();

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            renderer,
            paint_jobs: Vec::new(),
            textures,
        }
    }

    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        let response = self.egui_state.on_event(&self.egui_ctx, event);
        if response.repaint {
            return true;
        }
        false
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    pub fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.pixels_per_point = scale_factor as f32;
        self.egui_state.set_pixels_per_point(scale_factor as f32);
    }

    pub fn prepare(&mut self, window: &Window) {
        let raw_input = self.egui_state.take_egui_input(window);
        self.egui_ctx.begin_frame(raw_input);
    }

    pub fn ui<F: FnOnce(&Context)>(&mut self, run_ui: F) {
        run_ui(&self.egui_ctx);
    }

    pub fn render(&mut self, window: &Window, encoder: &mut wgpu::CommandEncoder, render_target: &wgpu::TextureView, context: &PixelsContext) {
        let full_output = self.egui_ctx.end_frame();
        self.paint_jobs = self.egui_ctx.tessellate(full_output.shapes);
        self.textures = full_output.textures_delta;

        self.egui_state.handle_platform_output(window, &self.egui_ctx, full_output.platform_output);

        for (id, image_delta) in &self.textures.set {
            self.renderer.update_texture(&context.device, &context.queue, *id, image_delta);
        }

        self.renderer.update_buffers(
            &context.device,
            &context.queue,
            encoder,
            &self.paint_jobs,
            &self.screen_descriptor,
        );

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.renderer.render(&mut rpass, &self.paint_jobs, &self.screen_descriptor);
        }

        for id in &self.textures.free {
            self.renderer.free_texture(id);
        }
    }
}