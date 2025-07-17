use fontdue::{Font, FontSettings, layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle}};
use sdf_glyph_renderer::BitmapGlyph;
use wasm_bindgen::prelude::*;
use web_sys::{GpuDevice, GpuTextureView, GpuRenderPipeline};
use std::collections::HashMap;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// Store glyph atlas info
struct GlyphInfo {
    atlas_x: f32,
    atlas_y: f32,
    width: f32,
    height: f32,
    sdf_width: f32,
    sdf_height: f32,
}

pub struct TextRenderer {
    font: Font,
    pipeline: Option<GpuRenderPipeline>,
    bind_group_layout: Option<web_sys::GpuBindGroupLayout>,
    sdf_atlas: Option<Vec<u8>>,
    atlas_width: u32,
    atlas_height: u32,
    atlas_texture: Option<web_sys::GpuTexture>,
    bind_group: Option<web_sys::GpuBindGroup>,
    glyph_map: HashMap<char, GlyphInfo>,
    buffer_size: usize,
}

impl TextRenderer {
    pub fn new() -> Result<Self, JsValue> {
        let font_data = include_bytes!("../assets/fonts/Spectral-ExtraLight.ttf");
        
        if font_data.is_empty() {
            return Err(JsValue::from_str("Font file is empty or not found"));
        }
        
        let font = Font::from_bytes(font_data as &[u8], FontSettings::default())
            .map_err(|e| JsValue::from_str(&format!("Failed to load font Spectral-ExtraLight.ttf: {:?}", e)))?;
        
        Ok(Self {
            font,
            pipeline: None,
            bind_group_layout: None,
            sdf_atlas: None,
            atlas_width: 512,
            atlas_height: 512,
            atlas_texture: None,
            bind_group: None,
            glyph_map: HashMap::new(),
            buffer_size: 8,
        })
    }

    pub fn create_text_pipeline(&mut self, device: &GpuDevice) -> Result<(), JsValue> {
        let vertex_shader = device.create_shader_module(&web_sys::GpuShaderModuleDescriptor::new(r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coord: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.tex_coord = input.tex_coord;
    return output;
}
"#));

        let fragment_shader = device.create_shader_module(&web_sys::GpuShaderModuleDescriptor::new(r#"
@group(0) @binding(0) var sdf_texture: texture_2d<f32>;
@group(0) @binding(1) var sdf_sampler: sampler;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    let distance = textureSample(sdf_texture, sdf_sampler, tex_coord).r;
    
    // Dynamic width based on derivatives for better quality at all scales
    // This automatically adjusts based on how zoomed in/out the text is
    var width = fwidth(distance);
    
    // For very small text, we need a bit more smoothing
    // For large text, we want it sharper
    width = clamp(width * 0.7, 0.0001, 0.5);
    
    // Use smoothstep for antialiasing
    let alpha = 1.0 - smoothstep(0.5 - width, 0.5 + width, distance);
    
    if (alpha < 0.001) {
        discard;
    }
    
    return vec4<f32>(0.0, 0.0, 0.0, alpha); // Black text
}
"#));

        // Create bind group layout
        let entries = js_sys::Array::new();
        
        let texture_entry = web_sys::GpuBindGroupLayoutEntry::new(0, web_sys::gpu_shader_stage::FRAGMENT);
        let texture_binding = web_sys::GpuTextureBindingLayout::new();
        texture_entry.set_texture(&texture_binding);
        entries.push(&texture_entry);
        
        let sampler_entry = web_sys::GpuBindGroupLayoutEntry::new(1, web_sys::gpu_shader_stage::FRAGMENT);
        let sampler_binding = web_sys::GpuSamplerBindingLayout::new();
        sampler_entry.set_sampler(&sampler_binding);
        entries.push(&sampler_entry);
        
        let bind_group_layout_desc = web_sys::GpuBindGroupLayoutDescriptor::new(&entries);
        let bind_group_layout = device.create_bind_group_layout(&bind_group_layout_desc)?;
        
        let layouts = js_sys::Array::new();
        layouts.push(&bind_group_layout);
        let pipeline_layout = device.create_pipeline_layout(&web_sys::GpuPipelineLayoutDescriptor::new(&layouts));
        
        self.bind_group_layout = Some(bind_group_layout);
        
        // Vertex buffer layout
        let vertex_attributes = js_sys::Array::new();
        
        let pos_attr = web_sys::GpuVertexAttribute::new(web_sys::GpuVertexFormat::Float32x2, 0.0, 0);
        vertex_attributes.push(&pos_attr);
        
        let tex_attr = web_sys::GpuVertexAttribute::new(web_sys::GpuVertexFormat::Float32x2, 8.0, 1);
        vertex_attributes.push(&tex_attr);
        
        let vertex_buffer_layout = web_sys::GpuVertexBufferLayout::new(16.0, &vertex_attributes);
        vertex_buffer_layout.set_step_mode(web_sys::GpuVertexStepMode::Vertex);
        
        let vertex_buffers = js_sys::Array::new();
        vertex_buffers.push(&vertex_buffer_layout);
        
        let vertex_state = web_sys::GpuVertexState::new(&vertex_shader);
        vertex_state.set_entry_point("main");
        vertex_state.set_buffers(&vertex_buffers);
        
        let targets = js_sys::Array::new();
        
        let color_target_state = web_sys::GpuColorTargetState::new(web_sys::GpuTextureFormat::Bgra8unorm);
        
        // Blend state for transparency
        let mut color_component = web_sys::GpuBlendComponent::new();
        color_component.operation(web_sys::GpuBlendOperation::Add);
        color_component.src_factor(web_sys::GpuBlendFactor::SrcAlpha);
        color_component.dst_factor(web_sys::GpuBlendFactor::OneMinusSrcAlpha);
        
        let mut alpha_component = web_sys::GpuBlendComponent::new();
        alpha_component.operation(web_sys::GpuBlendOperation::Add);
        alpha_component.src_factor(web_sys::GpuBlendFactor::One);
        alpha_component.dst_factor(web_sys::GpuBlendFactor::OneMinusSrcAlpha);
        
        let blend_state = web_sys::GpuBlendState::new(&color_component, &alpha_component);
        
        color_target_state.set_blend(&blend_state);
        targets.push(&color_target_state);
        let fragment_state = web_sys::GpuFragmentState::new(&fragment_shader, &targets);
        fragment_state.set_entry_point("main");
        
        let primitive = web_sys::GpuPrimitiveState::new();
        primitive.set_topology(web_sys::GpuPrimitiveTopology::TriangleList);
        
        let pipeline_desc = web_sys::GpuRenderPipelineDescriptor::new(&pipeline_layout, &vertex_state);
        pipeline_desc.set_fragment(&fragment_state);
        pipeline_desc.set_primitive(&primitive);
        
        self.pipeline = Some(device.create_render_pipeline(&pipeline_desc)?);
        Ok(())
    }

    pub fn generate_sdf_atlas(&mut self, text: &str) -> Result<(), JsValue> {
        console_log!("Generating SDF atlas for: '{}'", text);
        
        let atlas_size = (self.atlas_width * self.atlas_height) as usize;
        let mut atlas_data = vec![128u8; atlas_size]; // Initialize with middle gray
        
        let mut unique_chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        unique_chars.sort();
        unique_chars.dedup();
        
        console_log!("Processing {} unique characters", unique_chars.len());
        
        let char_size = 64;
        let chars_per_row = self.atlas_width / char_size;
        
        self.glyph_map.clear();
        
        for (i, &ch) in unique_chars.iter().enumerate() {
            let char_x = (i as u32 % chars_per_row) * char_size;
            let char_y = (i as u32 / chars_per_row) * char_size;
            
            let (metrics, bitmap) = self.font.rasterize(ch, 48.0);
            
            if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                // Generate SDF
                let bitmap_glyph = match BitmapGlyph::from_unbuffered(&bitmap, metrics.width, metrics.height, self.buffer_size) {
                    Ok(glyph) => glyph,
                    Err(e) => {
                        console_log!("SDF glyph creation failed for '{}': {:?}", ch, e);
                        continue;
                    }
                };
                
                let sdf_radius = 8.0;
                let sdf_data = bitmap_glyph.render_sdf(sdf_radius as usize);
                
                let sdf_width = metrics.width + 2 * self.buffer_size;
                let sdf_height = metrics.height + 2 * self.buffer_size;
                
                // Store glyph info
                self.glyph_map.insert(ch, GlyphInfo {
                    atlas_x: char_x as f32,
                    atlas_y: char_y as f32,
                    width: metrics.width as f32,
                    height: metrics.height as f32,
                    sdf_width: sdf_width as f32,
                    sdf_height: sdf_height as f32,
                });
                
                // Copy SDF data into atlas
                let copy_width = sdf_width.min(char_size as usize);
                let copy_height = sdf_height.min(char_size as usize);
                
                for y in 0..copy_height {
                    for x in 0..copy_width {
                        let atlas_x = char_x as usize + x;
                        let atlas_y = char_y as usize + y;
                        let atlas_idx = atlas_y * self.atlas_width as usize + atlas_x;
                        
                        if atlas_idx < atlas_data.len() {
                            let sdf_idx = y * sdf_width + x;
                            if sdf_idx < sdf_data.len() {
                                let normalized_distance = sdf_data[sdf_idx] / sdf_radius;
                                let sdf_value = ((normalized_distance + 1.0) * 127.5).clamp(0.0, 255.0) as u8;
                                atlas_data[atlas_idx] = sdf_value;
                            }
                        }
                    }
                }
            }
        }
        
        self.sdf_atlas = Some(atlas_data);
        console_log!("SDF atlas generated successfully");
        Ok(())
    }

    pub fn create_texture_and_bind_group(&mut self, device: &GpuDevice) -> Result<(), JsValue> {
        if let Some(ref atlas_data) = self.sdf_atlas {
            let mut extent = web_sys::GpuExtent3dDict::new(self.atlas_width);
            extent.set_height(self.atlas_height);
            extent.set_depth_or_array_layers(1);
            
            let texture_desc = web_sys::GpuTextureDescriptor::new(
                web_sys::GpuTextureFormat::R8unorm,
                &extent,
                web_sys::gpu_texture_usage::TEXTURE_BINDING | web_sys::gpu_texture_usage::COPY_DST,
            );
            
            let texture = device.create_texture(&texture_desc)?;
            
            let data_layout = web_sys::GpuTexelCopyBufferLayout::new();
            data_layout.set_bytes_per_row(self.atlas_width);
            data_layout.set_rows_per_image(self.atlas_height);
            
            let mut copy_size = web_sys::GpuExtent3dDict::new(self.atlas_width);
            copy_size.set_height(self.atlas_height);
            copy_size.set_depth_or_array_layers(1);
            
            let destination = web_sys::GpuTexelCopyTextureInfo::new(&texture);
            
            device.queue().write_texture_with_u8_slice_and_gpu_extent_3d_dict(
                &destination,
                atlas_data.as_slice(),
                &data_layout,
                &copy_size,
            )?;
            
            let sampler_desc = web_sys::GpuSamplerDescriptor::new();
            sampler_desc.set_mag_filter(web_sys::GpuFilterMode::Linear);
            sampler_desc.set_min_filter(web_sys::GpuFilterMode::Linear);
            sampler_desc.set_mipmap_filter(web_sys::GpuMipmapFilterMode::Linear);
            let sampler = device.create_sampler_with_descriptor(&sampler_desc);
            
            let bind_group_layout = self.bind_group_layout.as_ref()
                .ok_or_else(|| JsValue::from_str("Pipeline must be created before texture"))?;
            
            let bind_entries = js_sys::Array::new();
            
            let texture_bind_entry = web_sys::GpuBindGroupEntry::new(0, &texture.create_view()?.into());
            bind_entries.push(&texture_bind_entry);
            
            let sampler_bind_entry = web_sys::GpuBindGroupEntry::new(1, &sampler);
            bind_entries.push(&sampler_bind_entry);
            
            let bind_group_desc = web_sys::GpuBindGroupDescriptor::new(&bind_entries, bind_group_layout);
            let bind_group = device.create_bind_group(&bind_group_desc);
            
            self.atlas_texture = Some(texture);
            self.bind_group = Some(bind_group);
        }
        
        Ok(())
    }

    fn generate_text_vertices(&self, text: &str, x: f32, y: f32, screen_width: f32, screen_height: f32) -> Vec<f32> {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        let fonts = &[&self.font];
        
        let mut layout_settings = LayoutSettings::default();
        layout_settings.x = x;
        layout_settings.y = y;
        layout.reset(&layout_settings);
        layout.append(fonts, &TextStyle::new(text, 48.0, 0));
    
        let mut vertices = Vec::new();
        
        for glyph in layout.glyphs() {
            if let Some(glyph_info) = self.glyph_map.get(&glyph.parent) {
                // Use layout positions directly - fontdue handles baseline alignment
                let glyph_left = glyph.x as f32;
                let glyph_top = glyph.y as f32;
                
                // Account for the SDF buffer padding
                let buffer_offset = self.buffer_size as f32;
                let screen_left = glyph_left - buffer_offset;
                let screen_top = glyph_top - buffer_offset;
                let screen_right = screen_left + glyph_info.sdf_width;
                let screen_bottom = screen_top + glyph_info.sdf_height;
                
                // Convert to NDC (-1 to 1 range)
                let left = (screen_left / screen_width) * 2.0 - 1.0;
                let right = (screen_right / screen_width) * 2.0 - 1.0;
                let top = 1.0 - (screen_top / screen_height) * 2.0;
                let bottom = 1.0 - (screen_bottom / screen_height) * 2.0;
                
                // UV coordinates
                let u_left = glyph_info.atlas_x / self.atlas_width as f32;
                let u_right = (glyph_info.atlas_x + glyph_info.sdf_width) / self.atlas_width as f32;
                let v_top = glyph_info.atlas_y / self.atlas_height as f32;
                let v_bottom = (glyph_info.atlas_y + glyph_info.sdf_height) / self.atlas_height as f32;
    
                // Two triangles for the quad
                vertices.extend_from_slice(&[
                    left, bottom,   u_left, v_bottom,
                    right, bottom,  u_right, v_bottom,
                    left, top,      u_left, v_top,
                    
                    right, bottom,  u_right, v_bottom,
                    right, top,     u_right, v_top,
                    left, top,      u_left, v_top,
                ]);
            }
        }
        
        vertices
    }
    
    pub fn render_text(
        &mut self,
        device: &GpuDevice,
        context: &crate::gpu::context::GpuContext,
        text: &str,
        x: f32,
        y: f32,
        screen_width: f32,
        screen_height: f32,
    ) -> Result<(), JsValue> {
        if self.sdf_atlas.is_none() || !text.chars().all(|c| c.is_whitespace() || self.glyph_map.contains_key(&c)) {
            self.generate_sdf_atlas(text)?;
            // Recreate texture with new atlas
            self.atlas_texture = None;
            self.bind_group = None;
        }

        if self.atlas_texture.is_none() {
            self.create_texture_and_bind_group(device)?;
        }

        let pipeline = self.pipeline.as_ref()
            .ok_or_else(|| JsValue::from_str("Text pipeline not created"))?;

        let vertices = self.generate_text_vertices(text, x, y, screen_width, screen_height);
        let vertex_count = vertices.len() / 4;
        
        if vertex_count == 0 {
            return Ok(());
        }

        let vertex_buffer_desc = web_sys::GpuBufferDescriptor::new(
            (vertices.len() * 4) as f64,
            web_sys::gpu_buffer_usage::VERTEX | web_sys::gpu_buffer_usage::COPY_DST,
        );
        let vertex_buffer = device.create_buffer(&vertex_buffer_desc)?;
        
        let vertex_bytes: Vec<u8> = vertices.iter()
            .flat_map(|&f| f.to_le_bytes())
            .collect();
        let vertex_data = unsafe {
            js_sys::Uint8Array::view(&vertex_bytes)
        };
        device.queue().write_buffer_with_u32_and_u8_array(&vertex_buffer, 0, &vertex_data)?;

        let command_encoder = device.create_command_encoder();
        
        // Render to offscreen texture first
        let color_attachments = js_sys::Array::new();
        let clear_color = web_sys::GpuColorDict::new(1.0, 1.0, 1.0, 1.0);
        let color_attachment = web_sys::GpuRenderPassColorAttachment::new(
            web_sys::GpuLoadOp::Clear,
            web_sys::GpuStoreOp::Store,
            &context.offscreen_view
        );
        color_attachment.set_clear_value(&clear_color);
        color_attachments.push(&color_attachment);
        
        let render_pass_descriptor = web_sys::GpuRenderPassDescriptor::new(&color_attachments);
        let render_pass = command_encoder.begin_render_pass(&render_pass_descriptor)?;
        
        render_pass.set_pipeline(pipeline);
        
        if let Some(ref bind_group) = self.bind_group {
            render_pass.set_bind_group(0, Some(bind_group));
        }
        
        render_pass.set_vertex_buffer(0, Some(&vertex_buffer));
        render_pass.draw(vertex_count as u32);
        render_pass.end();
        
        // Copy from offscreen texture to swapchain
        let swapchain_texture = context.context.get_current_texture()?;
        command_encoder.copy_texture_to_texture_with_u32_sequence(
            &{
                let source = web_sys::GpuTexelCopyTextureInfo::new(&context.offscreen_texture);
                source
            },
            &{
                let dest = web_sys::GpuTexelCopyTextureInfo::new(&swapchain_texture);
                dest
            },
            &{
                let mut copy_size = web_sys::GpuExtent3dDict::new(context.canvas.width());
                copy_size.set_height(context.canvas.height());
                copy_size.set_depth_or_array_layers(1);
                copy_size.into()
            },
        )?;
        
        device.queue().submit(&js_sys::Array::of1(&command_encoder.finish()));
        Ok(())
    }
}