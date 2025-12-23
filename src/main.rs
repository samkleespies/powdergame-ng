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
        if y >= self.height - 1 { return; } // Bottom boundary

        let particle = self.grid[idx];
        let down = self.get_index(x, y + 1);
        
        match particle.ptype {
            ParticleType::Sand => {
                // 1. Try down
                if self.grid[down].ptype == ParticleType::Empty || self.grid[down].ptype == ParticleType::Water {
                    self.swap(idx, down);
                } 
                // 2. Try down-left
                else if x > 0 && (self.grid[self.get_index(x - 1, y + 1)].ptype == ParticleType::Empty || self.grid[self.get_index(x - 1, y + 1)].ptype == ParticleType::Water) {
                    self.swap(idx, self.get_index(x - 1, y + 1));
                }
                // 3. Try down-right
                else if x < self.width - 1 && (self.grid[self.get_index(x + 1, y + 1)].ptype == ParticleType::Empty || self.grid[self.get_index(x + 1, y + 1)].ptype == ParticleType::Water) {
                    self.swap(idx, self.get_index(x + 1, y + 1));
                }
            },
            ParticleType::Water => {
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
                // 3. Move sideways with dispersion (faster settling)
                else {
                    let mut moves = Vec::new();
                    let dir = if rng.gen() { -1 } else { 1 };
                    
                    // Try the random direction first, then the other
                    for d in [dir, -dir] {
                        let spread_limit = 5; // How fast water spreads
                        for i in 1..=spread_limit {
                            let new_x = x as isize + d * i;
                            
                            if new_x < 0 || new_x >= self.width as isize {
                                break;
                            }
                            
                            let target_idx = self.get_index(new_x as usize, y);
                            if self.grid[target_idx].ptype == ParticleType::Empty {
                                moves.push(target_idx);
                            } else {
                                // Hit a wall or other particle
                                break;
                            }
                        }
                        
                        // If we found a valid move in this direction, take the furthest one and stop
                        if let Some(target_idx) = moves.last() {
                            self.swap(idx, *target_idx);
                            return; // Done moving
                        }
                    }
                }
            },
            ParticleType::Lava => {
                 // 1. Try down (burns water/sand - simplified: just overwrites or swaps? Let's swap for now)
                if self.grid[down].ptype == ParticleType::Empty {
                    self.swap(idx, down);
                } else if self.grid[down].ptype == ParticleType::Water {
                     // Turn water to stone/steam logic could go here. For now, swap.
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

    let mut world = World::new(WIDTH, HEIGHT);
    let mut last_update = Instant::now();
    let mut frames = 0;
    
    // Fixed time step variables
    let mut last_tick = Instant::now();
    let tick_interval = std::time::Duration::from_secs_f32(1.0 / TICK_RATE);
    let mut accumulator = std::time::Duration::new(0, 0);

    event_loop.run(move |event, _, control_flow| {
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
            if let Err(_) = pixels.render() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // FPS Counter
            frames += 1;
            if last_update.elapsed().as_secs_f32() >= 1.0 {
                let fps = frames as f32 / last_update.elapsed().as_secs_f32();
                window.set_title(&format!("Rust Powder Game - FPS: {:.1}", fps));
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
            }

            if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Material Selection
            if input.key_pressed(VirtualKeyCode::Key1) { world.selected_material = ParticleType::Sand; println!("Selected: Sand"); }
            if input.key_pressed(VirtualKeyCode::Key2) { world.selected_material = ParticleType::Water; println!("Selected: Water"); }
            if input.key_pressed(VirtualKeyCode::Key3) { world.selected_material = ParticleType::Lava; println!("Selected: Lava"); }
            if input.key_pressed(VirtualKeyCode::Key4) { world.selected_material = ParticleType::Wall; println!("Selected: Wall"); }
            if input.key_pressed(VirtualKeyCode::Key0) { world.selected_material = ParticleType::Empty; println!("Selected: Eraser"); }

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

            window.request_redraw();
        }
    });
}
