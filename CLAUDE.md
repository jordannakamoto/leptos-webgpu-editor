# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust WebAssembly application that demonstrates WebGPU rendering capabilities within the Leptos reactive web framework. The application generates random geometric shapes (triangles, rectangles, hexagons) and renders them using WebGPU's hardware-accelerated graphics pipeline.

## Development Commands

- `trunk serve` - Start development server with hot reload
- `trunk build` - Build for production (outputs to `dist/`)
- `trunk build --release` - Optimized production build
- `cargo check` - Fast compilation check without building
- `cargo clippy` - Linting and suggestions

## Architecture

### Core Components

**Shape Generation (`src/main.rs:24-120`)**
- `generate_random_shape()` - Procedurally generates triangle, rectangle, or hexagon shapes
- Creates randomized vertex positions, colors, and sizes
- Generates WGSL (WebGPU Shading Language) vertex and fragment shaders dynamically

**WebGPU Pipeline (`src/main.rs:122-200`)**
- `init_webgpu()` - Initializes WebGPU context, creates render pipeline
- Sets up GPU adapter, device, and command queue
- Compiles shaders and executes rendering commands

**Leptos UI (`src/main.rs:202-255`)**
- Reactive web interface with canvas element (800x600px)
- Button-triggered shape generation
- Mounts to document body using CSR (Client-Side Rendering)

### Key Dependencies

- **Leptos 0.8.3** - Web framework with CSR features only
- **web-sys** - Extensive WebGPU API bindings for GPU operations
- **wasm-bindgen** - Rust-WASM interop layer
- **getrandom** - Cryptographically secure random number generation

## Development Notes

### WebGPU Setup
The project uses comprehensive WebGPU bindings including adapters, devices, command encoders, render pipelines, and shaders. All WebGPU initialization happens in `init_webgpu()` with proper error handling.

### Shape Rendering
Each shape type has distinct vertex generation logic:
- Triangles: 3 vertices with varied positioning
- Rectangles: 6 vertices (2 triangles) forming a quad
- Hexagons: 18 vertices (6 triangles) in radial pattern

### Build System
Uses Trunk for WASM bundling and asset management. The `index.html` template is minimal - Trunk injects the necessary WASM loading code during build.

### Random Generation
Custom `random_f32()` function uses `getrandom` to generate cryptographically secure random values, normalizing to -1.0 to 1.0 range for coordinate and color generation.