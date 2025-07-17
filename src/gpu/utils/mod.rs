// WebGPU Utilities
// Helper functions for common WebGPU operations
// - Clear render passes
// - Command buffer submission
// - Error handling helpers

use web_sys::{GpuDevice, GpuTextureView};

pub fn clear_screen(device: &GpuDevice, view: &GpuTextureView, color: [f64; 4]) -> Result<(), wasm_bindgen::JsValue> {
    let command_encoder = device.create_command_encoder();
    
    // Color attachment
    let color_attachments = js_sys::Array::new();
    let mut color_attachment = web_sys::GpuRenderPassColorAttachment::new(web_sys::GpuLoadOp::Clear, web_sys::GpuStoreOp::Store, &view);
    let mut clear_color = web_sys::GpuColorDict::new(color[3], color[2], color[1], color[0]);
    color_attachment.set_clear_value(&clear_color);
    color_attachments.push(&color_attachment);
    
    let render_pass_descriptor = web_sys::GpuRenderPassDescriptor::new(&color_attachments);
    let render_pass = command_encoder.begin_render_pass(&render_pass_descriptor)?;
    render_pass.end();
    
    // Submit
    let command_buffer = command_encoder.finish();
    let command_buffers = js_sys::Array::new();
    command_buffers.push(&command_buffer);
    device.queue().submit(&command_buffers);
    
    Ok(())
}