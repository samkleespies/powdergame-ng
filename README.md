# Rust Powder Game

A highly performant cellular automata "falling sand" game written in Rust.

## Controls
- **Mouse Left**: Paint particles
- **[**: Decrease brush size
- **]**: Increase brush size
- **1**: Sand
- **2**: Water
- **3**: Lava
- **4**: Wall
- **0**: Eraser (Empty)
- **Esc**: Exit

## Architecture
- **Language**: Rust
- **Rendering**: `pixels` (Hardware accelerated buffer)
- **Windowing**: `winit`
- **Physics**: Custom cellular automata grid
- **Performance**: Optimized flat `Vec` storage, minimal allocations per frame.

## Running
```bash
cargo run --release
```
