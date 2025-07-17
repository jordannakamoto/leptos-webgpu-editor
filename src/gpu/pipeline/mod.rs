// Render Pipeline Management
// Creates and manages WebGPU render pipelines
// Responsibilities:
// - Create render pipelines from shader modules
// - Configure pipeline layout and vertex attributes
// - Manage pipeline state and bindings
// - Execute render passes

use wasm_bindgen::prelude::*;
use web_sys::{GpuDevice, GpuRenderPipeline, GpuRenderPassEncoder};

pub struct RenderPipeline {
    pipeline: GpuRenderPipeline,
}

impl RenderPipeline {
    pub fn new(device: &GpuDevice, vertex_shader: &web_sys::GpuShaderModule, fragment_shader: &web_sys::GpuShaderModule) -> Result<Self, JsValue> {
        // For now, create a simple pipeline - will be expanded with proper configuration
        let pipeline_layout = device.create_pipeline_layout(&web_sys::GpuPipelineLayoutDescriptor::new(&js_sys::Array::new()));
        
        // Vertex stage
        let vertex_state = web_sys::GpuVertexState::new(vertex_shader);
        vertex_state.set_entry_point("main");
        
        let mut pipeline_desc = web_sys::GpuRenderPipelineDescriptor::new(&pipeline_layout, &vertex_state);

        // Fragment stage
        let targets = js_sys::Array::new();
        let color_target = web_sys::GpuColorTargetState::new(web_sys::GpuTextureFormat::Bgra8unorm);
        targets.push(&color_target);
        
        let fragment_state = web_sys::GpuFragmentState::new(fragment_shader, &targets);
        fragment_state.set_entry_point("main");
        pipeline_desc.fragment(&fragment_state);
        
        // Primitive state
        let mut primitive = web_sys::GpuPrimitiveState::new();
        primitive.topology(web_sys::GpuPrimitiveTopology::TriangleList);
        pipeline_desc.primitive(&primitive);
        
        let pipeline = device.create_render_pipeline(&pipeline_desc)?;
        
        Ok(Self { pipeline })
    }
    
    pub fn bind(&self, render_pass: &GpuRenderPassEncoder) {
        render_pass.set_pipeline(&self.pipeline);
    }
    
    pub fn inner(&self) -> &GpuRenderPipeline {
        &self.pipeline
    }
}