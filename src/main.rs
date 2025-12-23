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
    Wood,
    Oil,
    Fire,
    Smoke,
    Gunpowder,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Empty,
    Solid,      // Walls, Wood (doesn't move)
    Powder,     // Sand, Gunpowder, Stone (moves down, piles up)
    Liquid,     // Water, Oil, Lava (moves down, flows sideways)
    Gas,        // Steam, Smoke, Fire (moves up/randomly)
}

impl ParticleType {
    fn density(&self) -> u8 {
        match self {
            ParticleType::Wall | ParticleType::Wood => 255, // Immovable
            ParticleType::Stone => 100,
            ParticleType::Sand | ParticleType::Gunpowder => 80,
            ParticleType::Water => 50,
            ParticleType::Lava => 60, // Slightly heavier than water usually, or make it displace? Let's say 60.
            ParticleType::Oil => 40,  // Floats on water
            ParticleType::Steam | ParticleType::Smoke | ParticleType::Fire => 10,
            ParticleType::Empty => 0,
        }
    }

    fn state(&self) -> State {
        match self {
            ParticleType::Empty => State::Empty,
            ParticleType::Wall | ParticleType::Wood => State::Solid,
            ParticleType::Sand | ParticleType::Stone | ParticleType::Gunpowder => State::Powder,
            ParticleType::Water | ParticleType::Lava | ParticleType::Oil => State::Liquid,
            ParticleType::Steam | ParticleType::Smoke | ParticleType::Fire => State::Gas,
        }
    }
}

#[derive(Clone, Copy)]
struct Particle {
    ptype: ParticleType,
    color: [u8; 4],
    vel_x: f32,
    vel_y: f32,
    updated: bool,
}

impl Particle {
    fn new(ptype: ParticleType) -> Self {
        let mut rng = rand::thread_rng();
        let color = match ptype {
            ParticleType::Empty => [0, 0, 0, 0],
            ParticleType::Sand => [225 + rng.gen_range(0..25), 180 + rng.gen_range(0..25), 120, 255],
            ParticleType::Water => [0, 100 + rng.gen_range(0..50), 255, 200],
            ParticleType::Lava => [255, 69 + rng.gen_range(0..50), 0, 255],
            ParticleType::Wall => [100, 100, 100, 255],
            ParticleType::Stone => [70 + rng.gen_range(0..20), 70 + rng.gen_range(0..20), 70 + rng.gen_range(0..20), 255],
            ParticleType::Steam => [200, 200, 200, 100 + rng.gen_range(0..50)],
            ParticleType::Wood => [139, 69 + rng.gen_range(0..20), 19, 255], // SaddleBrown
            ParticleType::Oil => [50 + rng.gen_range(0..20), 50 + rng.gen_range(0..20), 0, 200], // Dark oily
            ParticleType::Fire => [255, 100 + rng.gen_range(0..100), 0, 255], // Orange-Yellow
            ParticleType::Smoke => [50 + rng.gen_range(0..20), 50 + rng.gen_range(0..20), 50 + rng.gen_range(0..20), 150], // Grey semi-transparent
            ParticleType::Gunpowder => [40 + rng.gen_range(0..20), 40 + rng.gen_range(0..20), 40 + rng.gen_range(0..20), 255], // Dark grey/black dust
        };
        
        Self {
            ptype,
            color,
            vel_x: 0.0,
            vel_y: 0.0,
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
        let mut particle = self.grid[idx];
        
        if self.handle_interactions(x, y, idx, &mut particle, rng) {
             self.grid[idx] = particle;
             return; 
        }

        // Apply Gravity
        let state = particle.ptype.state();
        if state == State::Solid || state == State::Empty { return; }

        let gravity = if state == State::Gas { -0.05 } else { 0.1 };
        particle.vel_y += gravity;
        
        // Terminals
        if particle.vel_y > 4.0 { particle.vel_y = 4.0; }
        if particle.vel_y < -2.0 { particle.vel_y = -2.0; }

        // Random Brownian motion / Spreading
        if state == State::Liquid {
            // Liquid "Flattening" Logic: actively seek empty space if supported
            if particle.vel_x.abs() < 0.5 {
                // Check if supported below (or on floor)
                let supported = if y < self.height - 1 {
                    let down_idx = self.get_index(x, y + 1);
                    self.grid[down_idx].ptype != ParticleType::Empty
                } else {
                    true
                };

                if supported {
                    let left_blocked = if x > 0 {
                        self.grid[self.get_index(x - 1, y)].ptype != ParticleType::Empty
                    } else { true };
                    
                    let right_blocked = if x < self.width - 1 {
                        self.grid[self.get_index(x + 1, y)].ptype != ParticleType::Empty
                    } else { true };

                    if !left_blocked && right_blocked {
                        particle.vel_x = -2.0; // Flow left strongly
                    } else if !right_blocked && left_blocked {
                        particle.vel_x = 2.0; // Flow right strongly
                    } else if !left_blocked && !right_blocked {
                        // Spread randomly if both open
                        particle.vel_x = if rng.gen_bool(0.5) { 1.5 } else { -1.5 };
                    } else {
                        // Trapped/Settled: small jitter to keep it "alive"
                        if rng.gen_bool(0.1) {
                             particle.vel_x = rng.gen_range(-0.2..=0.2);
                        }
                    }
                }
            }
             particle.vel_x *= 0.9; // Friction
        } else if state == State::Gas {
             particle.vel_x += rng.gen_range(-0.2..=0.2);
             particle.vel_x *= 0.95;
        } else {
            // Powders have high friction, small jitter for settling
            if rng.gen_bool(0.05) {
                particle.vel_x += rng.gen_range(-0.5..=0.5);
            }
            particle.vel_x *= 0.8; 
        }

        self.grid[idx] = particle; // Save velocity changes

        // Move based on velocity
        self.apply_movement(x, y, idx, state, rng);
    }

    fn handle_interactions(&mut self, x: usize, y: usize, _idx: usize, p: &mut Particle, rng: &mut rand::rngs::ThreadRng) -> bool {
        let mut changed = false;

        // Fire lifecycle
        if p.ptype == ParticleType::Fire {
            if rng.gen_bool(0.05) {
                p.ptype = ParticleType::Smoke;
                p.color = Particle::new(ParticleType::Smoke).color;
                p.vel_y = 0.0;
                changed = true;
            }
        } else if p.ptype == ParticleType::Smoke || p.ptype == ParticleType::Steam {
            if rng.gen_bool(0.02) {
                p.ptype = ParticleType::Empty;
                p.color = [0, 0, 0, 0];
                changed = true;
            }
        }

        let neighbors = [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1)
        ];

        for (nx, ny) in neighbors {
            if nx >= self.width || ny >= self.height { continue; }
            let n_idx = self.get_index(nx, ny);
            let n_type = self.grid[n_idx].ptype;

            if p.ptype == ParticleType::Fire {
                if n_type == ParticleType::Water {
                    p.ptype = ParticleType::Smoke;
                    p.color = Particle::new(ParticleType::Smoke).color;
                    changed = true;
                } else if matches!(n_type, ParticleType::Wood | ParticleType::Oil | ParticleType::Gunpowder) {
                    if rng.gen_bool(0.05) {
                        self.grid[n_idx] = Particle::new(ParticleType::Fire);
                        self.grid[n_idx].updated = true;
                    }
                }
            } else if p.ptype == ParticleType::Lava {
                if n_type == ParticleType::Water {
                    p.ptype = ParticleType::Stone;
                    p.color = Particle::new(ParticleType::Stone).color;
                    self.grid[n_idx] = Particle::new(ParticleType::Steam);
                    self.grid[n_idx].updated = true;
                    changed = true;
                } else if matches!(n_type, ParticleType::Wood | ParticleType::Oil | ParticleType::Gunpowder) {
                    if rng.gen_bool(0.05) {
                        self.grid[n_idx] = Particle::new(ParticleType::Fire);
                        self.grid[n_idx].updated = true;
                    }
                }
            }
        }

        changed
    }

    fn apply_movement(&mut self, x: usize, y: usize, idx: usize, _state: State, rng: &mut rand::rngs::ThreadRng) {
        // --- 1. Vertical Movement ---
        let p = self.grid[idx];
        let mut curr_idx = idx;
        let mut curr_x = x;
        let mut curr_y = y;

        if p.vel_y.abs() >= 0.1 {
            let steps = p.vel_y.abs().ceil() as usize;
            let dir_y = if p.vel_y > 0.0 { 1 } else { -1 };
            let steps = steps.clamp(1, 3); // Max speed limit per frame to prevent tunneling

            for _ in 0..steps {
                let next_y_isize = curr_y as isize + dir_y;
                
                // Boundary check
                if next_y_isize >= self.height as isize || next_y_isize < 0 {
                    self.grid[curr_idx].vel_y = 0.0;
                    break;
                }
                
                let next_y = next_y_isize as usize;
                let target_idx = self.get_index(curr_x, next_y);

                if self.try_swap(curr_idx, target_idx) {
                    // Success
                    curr_idx = target_idx;
                    curr_y = next_y;
                } else {
                    // Blocked!
                    
                    // --- SLOPE DEFLECTION PHYSICS ---
                    // We hit something vertically. Let's convert that Y momentum into X momentum
                    // if there is a slope.
                    
                    // NEW: Dynamic slide factor based on state
                    let slide_factor = if p.ptype.state() == State::Liquid { 0.8 } else { 0.1 };
                    
                    let left_blocked = if curr_x > 0 { !self.is_empty_or_liquid(curr_x - 1, next_y) } else { true };
                    let right_blocked = if curr_x < self.width - 1 { !self.is_empty_or_liquid(curr_x + 1, next_y) } else { true };
                    
                    if !left_blocked && right_blocked {
                         // Slope points Left
                         self.grid[curr_idx].vel_x -= p.vel_y.abs() * slide_factor;
                    } else if !right_blocked && left_blocked {
                         // Slope points Right
                         self.grid[curr_idx].vel_x += p.vel_y.abs() * slide_factor;
                    } else if !left_blocked && !right_blocked {
                        // Peak? Pick random side
                        if rng.gen() {
                            self.grid[curr_idx].vel_x += p.vel_y.abs() * slide_factor;
                        } else {
                            self.grid[curr_idx].vel_x -= p.vel_y.abs() * slide_factor;
                        }
                    }

                    self.grid[curr_idx].vel_y = 0.0; // Stop falling
                    break;
                }
            }
        }

        // --- 2. Horizontal Movement ---
        // Refresh particle ref in case it moved/changed
        let p_x = self.grid[curr_idx];
        
        if p_x.vel_x.abs() >= 0.1 {
            let steps_x = p_x.vel_x.abs().ceil() as usize;
            let dir_x = if p_x.vel_x > 0.0 { 1 } else { -1 };
            let steps_x = steps_x.clamp(1, 3);

            for _ in 0..steps_x {
                let next_x_isize = curr_x as isize + dir_x;

                if next_x_isize >= self.width as isize || next_x_isize < 0 {
                     self.grid[curr_idx].vel_x *= -0.5; // Bounce off walls?
                     break;
                }

                let next_x = next_x_isize as usize;
                let target_idx = self.get_index(next_x, curr_y);

                if self.try_swap(curr_idx, target_idx) {
                    curr_idx = target_idx;
                    curr_x = next_x;
                } else {
                    // Hit a wall horizontally
                    self.grid[curr_idx].vel_x = 0.0;
                    break;
                }
            }
        }
    }

    fn is_empty_or_liquid(&self, x: usize, y: usize) -> bool {
        let idx = self.get_index(x, y);
        let t = self.grid[idx].ptype;
        t == ParticleType::Empty || t.state() == State::Liquid || t.state() == State::Gas
    }

    fn try_swap(&mut self, idx1: usize, idx2: usize) -> bool {
        let p1 = self.grid[idx1];
        let p2 = self.grid[idx2];
        
        let d1 = p1.ptype.density();
        let d2 = p2.ptype.density();

        // Rules:
        // 1. Can always move into Empty (density 0)
        // 2. If Density 1 > Density 2 AND falling (vel_y > 0), swap (sink)
        // 3. If Density 1 < Density 2 AND rising (vel_y < 0), swap (float)
        
        let should_swap = if p2.ptype == ParticleType::Empty {
            true
        } else if p1.ptype.state() == State::Solid {
            false // Solids don't move into things
        } else if p1.vel_y > 0.0 {
            d1 > d2 // Sinking
        } else {
            d1 < d2 // Rising
        };

        if should_swap {
             self.grid.swap(idx1, idx2);
             self.grid[idx2].updated = true; // p1 is now at idx2
             return true;
        }
        false
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
                                if ui.selectable_value(&mut selected, ParticleType::Wood, "Wood").clicked() { world.selected_material = ParticleType::Wood; }
                                if ui.selectable_value(&mut selected, ParticleType::Oil, "Oil").clicked() { world.selected_material = ParticleType::Oil; }
                                ui.end_row();
                                if ui.selectable_value(&mut selected, ParticleType::Fire, "Fire").clicked() { world.selected_material = ParticleType::Fire; }
                                if ui.selectable_value(&mut selected, ParticleType::Gunpowder, "Gunpowder").clicked() { world.selected_material = ParticleType::Gunpowder; }
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
                let _fps = frames as f32 / last_update.elapsed().as_secs_f32();
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
                // Material Selection Keys
                if input.key_pressed(VirtualKeyCode::Key1) { world.selected_material = ParticleType::Sand; }
                if input.key_pressed(VirtualKeyCode::Key2) { world.selected_material = ParticleType::Water; }
                if input.key_pressed(VirtualKeyCode::Key3) { world.selected_material = ParticleType::Lava; }
                if input.key_pressed(VirtualKeyCode::Key4) { world.selected_material = ParticleType::Wall; }
                if input.key_pressed(VirtualKeyCode::Key5) { world.selected_material = ParticleType::Wood; }
                if input.key_pressed(VirtualKeyCode::Key6) { world.selected_material = ParticleType::Oil; }
                if input.key_pressed(VirtualKeyCode::Key7) { world.selected_material = ParticleType::Fire; }
                if input.key_pressed(VirtualKeyCode::Key8) { world.selected_material = ParticleType::Gunpowder; }
                if input.key_pressed(VirtualKeyCode::Key9) { world.selected_material = ParticleType::Stone; }
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
