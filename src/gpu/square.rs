use wasm_bindgen::prelude::*;
use web_sys::{GpuDevice, GpuRenderPipeline};

pub fn create_square_pipeline(device: &GpuDevice) -> Result<GpuRenderPipeline, JsValue> {
    let vertex_shader = device.create_shader_module(&web_sys::GpuShaderModuleDescriptor::new(r#"
@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(-0.5, -0.5), vec2<f32>(0.5, -0.5), vec2<f32>(-0.5, 0.5),
        vec2<f32>(0.5, -0.5), vec2<f32>(0.5, 0.5), vec2<f32>(-0.5, 0.5)
    );
    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}
"#));

    let fragment_shader = device.create_shader_module(&web_sys::GpuShaderModuleDescriptor::new(r#"
@fragment
fn main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
"#));

    let pipeline_layout = device.create_pipeline_layout(&web_sys::GpuPipelineLayoutDescriptor::new(&js_sys::Array::new()));
    let vertex_state = web_sys::GpuVertexState::new(&vertex_shader);
    vertex_state.set_entry_point("main");
    
    let targets = js_sys::Array::new();
    targets.push(&web_sys::GpuColorTargetState::new(web_sys::GpuTextureFormat::Bgra8unorm));
    let fragment_state = web_sys::GpuFragmentState::new(&fragment_shader, &targets);
    fragment_state.set_entry_point("main");
    
    let primitive = web_sys::GpuPrimitiveState::new();
    primitive.set_topology(web_sys::GpuPrimitiveTopology::TriangleList);
    
    let pipeline_desc = web_sys::GpuRenderPipelineDescriptor::new(&pipeline_layout, &vertex_state);
    pipeline_desc.set_fragment(&fragment_state);
    pipeline_desc.set_primitive(&primitive);
    
    device.create_render_pipeline(&pipeline_desc)
}

pub fn draw_square(device: &GpuDevice, view: &web_sys::GpuTextureView, pipeline: &GpuRenderPipeline) -> Result<(), JsValue> {
    let command_encoder = device.create_command_encoder();
    
    let color_attachments = js_sys::Array::new();
    let clear_color = web_sys::GpuColorDict::new(1.0, 0.1, 0.2, 0.3);
    let color_attachment = web_sys::GpuRenderPassColorAttachment::new(
        web_sys::GpuLoadOp::Clear,
        web_sys::GpuStoreOp::Store,
        &view
    );
    color_attachment.set_clear_value(&clear_color);
    color_attachments.push(&color_attachment);
    
    let render_pass_descriptor = web_sys::GpuRenderPassDescriptor::new(&color_attachments);
    let render_pass = command_encoder.begin_render_pass(&render_pass_descriptor)?;
    
    render_pass.set_pipeline(&pipeline);
    render_pass.draw(6);
    render_pass.end();
    
    device.queue().submit(&js_sys::Array::of1(&command_encoder.finish()));
    Ok(())
}