use pixels::{SurfaceTexture, PixelsBuilder};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
use rand::Rng;
use std::time::Instant;

const WIDTH: usize = 200;
const HEIGHT: usize = 150;
const SCALE: f64 = 4.0;
const TICK_RATE: f32 = 60.0; // Fixed physics updates per second

#[derive(Clone, Copy, PartialEq)]
enum ParticleType {
    Empty,
    Sand,
    Water,
    Lava,
    Wall,
    Stone,
    Steam,
}

#[derive(Clone, Copy)]
struct Particle {
    ptype: ParticleType,
    color: [u8; 4],
    updated: bool, // Prevent double updates per frame
}

impl Particle {
    fn new(ptype: ParticleType) -> Self {
        let mut rng = rand::thread_rng();
        let color = match ptype {
            ParticleType::Empty => [0, 0, 0, 0],
            ParticleType::Sand => [225 + rng.gen_range(0..25), 180 + rng.gen_range(0..25), 120, 255], // Sandy Brown
            ParticleType::Water => [0, 100 + rng.gen_range(0..50), 255, 200], // Blue
            ParticleType::Lava => [255, 69 + rng.gen_range(0..50), 0, 255], // Orange/Red
            ParticleType::Wall => [100, 100, 100, 255], // Grey
            ParticleType::Stone => [70 + rng.gen_range(0..20), 70 + rng.gen_range(0..20), 70 + rng.gen_range(0..20), 255], // Dark Grey
            ParticleType::Steam => [200, 200, 200, 150 + rng.gen_range(0..50)], // Semi-transparent White
        };
        
        Self {
            ptype,
            color,
            updated: false,
        }
    }
}

struct World {
    width: usize,
    height: usize,
    grid: Vec<Particle>,
    selected_material: ParticleType,
    brush_size: usize,
}

impl World {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            grid: vec![Particle::new(ParticleType::Empty); width * height],
            selected_material: ParticleType::Sand,
            brush_size: 2,
        }
    }

    fn get_index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    fn update(&mut self) {
        let mut rng = rand::thread_rng();
        
        // Reset update flags
        for p in self.grid.iter_mut() {
            p.updated = false;
        }

        // Iterate from bottom up, left to right
        for y in (0..self.height).rev() {
            // Randomize x direction processing to prevent bias
            let x_iter: Vec<usize> = if rng.gen() {
                (0..self.width).collect()
            } else {
                (0..self.width).rev().collect()
            };

            for x in x_iter {
                let idx = self.get_index(x, y);
                if self.grid[idx].ptype == ParticleType::Empty || self.grid[idx].ptype == ParticleType::Wall || self.grid[idx].updated {
                    continue;
                }

                self.update_particle(x, y, idx, &mut rng);
            }
        }
    }

    fn update_particle(&mut self, x: usize, y: usize, idx: usize, rng: &mut rand::rngs::ThreadRng) {
        let particle = self.grid[idx];
        
        // Steam Rises (Handle first because it goes UP)
        if particle.ptype == ParticleType::Steam {
            if y == 0 {
                self.grid[idx] = Particle::new(ParticleType::Empty); // Dissipate at top
                return;
            }
            
            let up = self.get_index(x, y - 1);
            if self.grid[up].ptype == ParticleType::Empty {
                self.swap(idx, up);
            } else if rng.gen_bool(0.5) {
                // Try move side-up
                 let dir = if rng.gen() { -1 } else { 1 };
                 let new_x = x as isize + dir;
                 if new_x >= 0 && new_x < self.width as isize {
                     let side_up = self.get_index(new_x as usize, y - 1);
                     if self.grid[side_up].ptype == ParticleType::Empty {
                         self.swap(idx, side_up);
                     } else {
                         // Or just sideways if blocked
                         let side = self.get_index(new_x as usize, y);
                         if self.grid[side].ptype == ParticleType::Empty {
                             self.swap(idx, side);
                         }
                     }
                 }
            } else if rng.gen_bool(0.1) {
                // Randomly disappear
                 self.grid[idx] = Particle::new(ParticleType::Empty);
            }
            return;
        }

        if y >= self.height - 1 { return; } // Bottom boundary for falling particles

        let down = self.get_index(x, y + 1);
        
        match particle.ptype {
            ParticleType::Sand => {
                let down_type = self.grid[down].ptype;
                // 1. Fall down (through Empty or Water)
                if down_type == ParticleType::Empty || down_type == ParticleType::Water {
                    self.swap(idx, down);
                } 
                // 2. Try down-left
                else if x > 0 {
                    let dl = self.get_index(x - 1, y + 1);
                    if self.grid[dl].ptype == ParticleType::Empty || self.grid[dl].ptype == ParticleType::Water {
                        self.swap(idx, dl);
                    }
                }
                // 3. Try down-right
                else if x < self.width - 1 {
                    let dr = self.get_index(x + 1, y + 1);
                    if self.grid[dr].ptype == ParticleType::Empty || self.grid[dr].ptype == ParticleType::Water {
                        self.swap(idx, dr);
                    }
                }
            },
            ParticleType::Stone => {
                // Stone is heavy, sinks in water and lava
                let down_type = self.grid[down].ptype;
                if down_type == ParticleType::Empty || down_type == ParticleType::Water || down_type == ParticleType::Lava {
                    self.swap(idx, down);
                }
            },
            ParticleType::Water => {
                // Interaction: Turn to steam if touching Lava
                // Check neighbors for Lava
                let mut touching_lava = false;
                if self.grid[down].ptype == ParticleType::Lava { touching_lava = true; }
                else if x > 0 && self.grid[self.get_index(x-1, y)].ptype == ParticleType::Lava { touching_lava = true; }
                else if x < self.width-1 && self.grid[self.get_index(x+1, y)].ptype == ParticleType::Lava { touching_lava = true; }
                else if y > 0 && self.grid[self.get_index(x, y-1)].ptype == ParticleType::Lava { touching_lava = true; }
                
                if touching_lava {
                    self.grid[idx] = Particle::new(ParticleType::Steam);
                    return;
                }

                // 1. Try down
                if self.grid[down].ptype == ParticleType::Empty {
                    self.swap(idx, down);
                }
                // 2. Try down-left / down-right
                else if x > 0 && self.grid[self.get_index(x - 1, y + 1)].ptype == ParticleType::Empty {
                    self.swap(idx, self.get_index(x - 1, y + 1));
                }
                else if x < self.width - 1 && self.grid[self.get_index(x + 1, y + 1)].ptype == ParticleType::Empty {
                    self.swap(idx, self.get_index(x + 1, y + 1));
                }
                // 3. Move sideways with dispersion
                else {
                     let mut moves = Vec::new();
                    let dir = if rng.gen() { -1 } else { 1 };
                    
                    // Try the random direction first, then the other
                    for d in [dir, -dir] {
                        let spread_limit = 5; 
                        for i in 1..=spread_limit {
                            let new_x = x as isize + d * i;
                            
                            if new_x < 0 || new_x >= self.width as isize { break; }
                            
                            let target_idx = self.get_index(new_x as usize, y);
                            if self.grid[target_idx].ptype == ParticleType::Empty {
                                moves.push(target_idx);
                            } else {
                                break;
                            }
                        }
                        
                        if let Some(target_idx) = moves.last() {
                            self.swap(idx, *target_idx);
                            return; 
                        }
                    }
                }
            },
            ParticleType::Lava => {
                 // Interaction: Turn to Stone if touching Water
                 // Also turn the Water into Steam (handled by Water update mostly, but Lava should solidify)
                 let mut touching_water = false;
                 // Check adjacent
                 if self.grid[down].ptype == ParticleType::Water { touching_water = true; }
                 else if y > 0 && self.grid[self.get_index(x, y-1)].ptype == ParticleType::Water { touching_water = true; }
                 else if x > 0 && self.grid[self.get_index(x-1, y)].ptype == ParticleType::Water { touching_water = true; }
                 else if x < self.width-1 && self.grid[self.get_index(x+1, y)].ptype == ParticleType::Water { touching_water = true; }

                 if touching_water {
                     self.grid[idx] = Particle::new(ParticleType::Stone);
                     // Optionally turn the water into steam here too to ensure reaction is symmetric
                     // But Water update handles its own death.
                     return;
                 }

                 // 1. Try down
                if self.grid[down].ptype == ParticleType::Empty {
                    self.swap(idx, down);
                } 
                // 2. Move sideways slowly
                else if rng.gen_bool(0.2) {
                    let dir = if rng.gen() { -1 } else { 1 };
                    let new_x = x as isize + dir;
                    if new_x >= 0 && new_x < self.width as isize {
                        let side_idx = self.get_index(new_x as usize, y);
                         if self.grid[side_idx].ptype == ParticleType::Empty {
                            self.swap(idx, side_idx);
                        }
                    }
                }
            },
            _ => {}
        }
    }

    fn swap(&mut self, idx1: usize, idx2: usize) {
        self.grid.swap(idx1, idx2);
        self.grid[idx2].updated = true;
    }

    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let p = self.grid[i];
            pixel.copy_from_slice(&p.color);
        }
    }

    fn paint(&mut self, x: usize, y: usize) {
        let size = self.brush_size as isize;
        for dy in -size..=size {
            for dx in -size..=size {
                if dx*dx + dy*dy > size*size { continue; } // Circle brush
                
                let px = x as isize + dx;
                let py = y as isize + dy;
                
                if px >= 0 && px < self.width as isize && py >= 0 && py < self.height as isize {
                    let idx = self.get_index(px as usize, py as usize);
                    if self.grid[idx].ptype == ParticleType::Empty || self.selected_material == ParticleType::Empty {
                        self.grid[idx] = Particle::new(self.selected_material);
                    }
                }
            }
        }
    }
}

mod gui;
use gui::Gui;

fn main() {
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    
    let window = WindowBuilder::new()
        .with_title("Rust Powder Game")
        .with_inner_size(LogicalSize::new(WIDTH as f64 * SCALE, HEIGHT as f64 * SCALE))
        .build(&event_loop)
        .unwrap();

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        PixelsBuilder::new(WIDTH as u32, HEIGHT as u32, surface_texture)
            .present_mode(pixels::wgpu::PresentMode::Mailbox)
            .build()
            .unwrap()
    };

    let mut gui = Gui::new(
        &event_loop,
        pixels.context(),
        window.inner_size().width,
        window.inner_size().height,
        window.scale_factor() as f32, // Revert to correct scaling
    );

    let mut world = World::new(WIDTH, HEIGHT);
    let mut last_update = Instant::now();
    let mut frames = 0;
    
    // Fixed time step variables
    let mut last_tick = Instant::now();
    let tick_interval = std::time::Duration::from_secs_f32(1.0 / TICK_RATE);
    let mut accumulator = std::time::Duration::new(0, 0);

    // UI State
    let mut show_tools = false;

    event_loop.run(move |event, _, control_flow| {
        // Update gui state
        if let Event::WindowEvent { event: ref window_event, .. } = event {
            if gui.handle_event(window_event) {
                window.request_redraw();
            }
        }

        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            // Calculate elapsed time since last frame
            let now = Instant::now();
            let delta = now.duration_since(last_tick);
            last_tick = now;
            accumulator += delta;

            // Update physics at fixed rate
            while accumulator >= tick_interval {
                world.update();
                accumulator -= tick_interval;
            }

            world.draw(pixels.frame_mut());
            
            // Prepare GUI
            gui.prepare(&window);
            
            // Draw UI
            gui.ui(|ctx| {
                let mut style = (*ctx.style()).clone();
                style.visuals.window_fill = egui::Color32::from_black_alpha(220);
                style.visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::WHITE;
                ctx.set_style(style);

                // Minimal Toggle Button Area
                egui::Area::new("toggle_area")
                    .fixed_pos(egui::pos2(10.0, 10.0))
                    .show(ctx, |ui| {
                         if ui.button(if show_tools { "❌ Close" } else { "🛠 Tools" }).clicked() {
                             show_tools = !show_tools;
                         }
                    });

                // The Tool Window
                if show_tools {
                    egui::Window::new("Tools")
                        .anchor(egui::Align2::LEFT_TOP, [10.0, 50.0])
                        .resizable(false)
                        .collapsible(false)
                        .title_bar(false) // Clean look
                        .show(ctx, |ui| {
                            ui.heading("Elements");
                            ui.separator();
                            
                            let mut selected = world.selected_material;
                            
                            egui::Grid::new("elements_grid").striped(true).show(ui, |ui| {
                                if ui.selectable_value(&mut selected, ParticleType::Sand, "Sand").clicked() { world.selected_material = ParticleType::Sand; }
                                if ui.selectable_value(&mut selected, ParticleType::Water, "Water").clicked() { world.selected_material = ParticleType::Water; }
                                ui.end_row();
                                if ui.selectable_value(&mut selected, ParticleType::Lava, "Lava").clicked() { world.selected_material = ParticleType::Lava; }
                                if ui.selectable_value(&mut selected, ParticleType::Stone, "Stone").clicked() { world.selected_material = ParticleType::Stone; }
                                ui.end_row();
                                if ui.selectable_value(&mut selected, ParticleType::Steam, "Steam").clicked() { world.selected_material = ParticleType::Steam; }
                                if ui.selectable_value(&mut selected, ParticleType::Wall, "Wall").clicked() { world.selected_material = ParticleType::Wall; }
                                ui.end_row();
                            });
                            
                            ui.separator();
                            if ui.selectable_value(&mut selected, ParticleType::Empty, "Eraser").clicked() { world.selected_material = ParticleType::Empty; }
                            
                            // Ensure internal state matches UI if changed externally (though we just set it above)
                            if selected != world.selected_material {
                                world.selected_material = selected;
                            }

                            ui.add_space(10.0);
                            ui.heading("Brush Size");
                            ui.add(egui::Slider::new(&mut world.brush_size, 1..=10).text("px"));
                            
                            ui.add_space(5.0);
                            ui.label("Shortcuts: 1-4, 0, [ ]");
                        });
                }
            });

            // Render
            let render_result = pixels.render_with(|encoder, render_target, context| {
                context.scaling_renderer.render(encoder, render_target);
                gui.render(&window, encoder, render_target, context);
                Ok(())
            });

            if let Err(_) = render_result {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // FPS Counter
            frames += 1;
            if last_update.elapsed().as_secs_f32() >= 1.0 {
                let fps = frames as f32 / last_update.elapsed().as_secs_f32();
                // window.set_title(&format!("Rust Powder Game - FPS: {:.1}", fps));
                frames = 0;
                last_update = Instant::now();
            }
        }

        // Handle input events
        if input.update(&event) {
            // Resize the window
            if let Some(size) = input.window_resized() {
                if let Err(_) = pixels.resize_surface(size.width, size.height) {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                gui.resize(size.width, size.height);
            }
            
            // Handle scale factor change
            if let Some(scale_factor) = input.scale_factor() {
                gui.scale_factor(scale_factor);
            }

            if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Only process game input if UI is NOT consuming the pointer
            if !gui.egui_ctx.is_pointer_over_area() {
                // Material Selection Keys (still keep them)
                if input.key_pressed(VirtualKeyCode::Key1) { world.selected_material = ParticleType::Sand; }
                if input.key_pressed(VirtualKeyCode::Key2) { world.selected_material = ParticleType::Water; }
                if input.key_pressed(VirtualKeyCode::Key3) { world.selected_material = ParticleType::Lava; }
                if input.key_pressed(VirtualKeyCode::Key4) { world.selected_material = ParticleType::Wall; }
                if input.key_pressed(VirtualKeyCode::Key0) { world.selected_material = ParticleType::Empty; }

                // Brush Size
                if input.key_pressed(VirtualKeyCode::LBracket) { if world.brush_size > 1 { world.brush_size -= 1; } }
                if input.key_pressed(VirtualKeyCode::RBracket) { world.brush_size += 1; }

                // Mouse Interaction
                if input.mouse_held(0) {
                    if let Some(mouse_pos) = input.mouse() {
                        if let Ok((gx, gy)) = pixels.window_pos_to_pixel(mouse_pos) {
                            world.paint(gx, gy);
                        }
                    }
                }
            }

            window.request_redraw();
        }
    });
}
