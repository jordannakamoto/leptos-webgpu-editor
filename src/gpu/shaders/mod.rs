// Shader Management
// Handles shader compilation and generation
// Responsibilities:
// - Generate WGSL shader code for different shapes
// - Compile shaders into GPU shader modules
// - Manage shader variants and caching
// - Provide shader utilities

use wasm_bindgen::prelude::*;
use web_sys::{GpuDevice, GpuShaderModule};

pub struct ShaderManager;

impl ShaderManager {
    pub fn create_basic_vertex_shader(device: &GpuDevice) -> Result<GpuShaderModule, JsValue> {
        let shader_code = r#"
@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-0.5, -0.5),
        vec2<f32>(0.5, -0.5),
        vec2<f32>(0.0, 0.5)
    );
    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}
"#;
        
        let mut descriptor = web_sys::GpuShaderModuleDescriptor::new(shader_code);
        Ok(device.create_shader_module(&descriptor))
    }
    
    pub fn create_basic_fragment_shader(device: &GpuDevice) -> Result<GpuShaderModule, JsValue> {
        let shader_code = r#"
@fragment
fn main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
"#;
        
        let mut descriptor = web_sys::GpuShaderModuleDescriptor::new(shader_code);
        Ok(device.create_shader_module(&descriptor))
    }
    
    pub fn generate_shape_vertex_shader(shape_type: &str, vertices: &[(f32, f32)], color: &[f32; 4]) -> String {
        // Generate WGSL shader code based on shape type
        // This will be expanded to handle different shapes dynamically
        format!(r#"
@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {{
    // Vertex data for {} shape
    var pos = array<vec2<f32>, {}>(
        {}
    );
    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}}
"#, shape_type, vertices.len(), 
    vertices.iter()
        .map(|(x, y)| format!("vec2<f32>({}, {})", x, y))
        .collect::<Vec<_>>()
        .join(",\n        ")
    )
    }
    
    pub fn generate_shape_fragment_shader(color: &[f32; 4]) -> String {
        format!(r#"
@fragment
fn main() -> @location(0) vec4<f32> {{
    return vec4<f32>({}, {}, {}, {});
}}
"#, color[0], color[1], color[2], color[3])
    }
}