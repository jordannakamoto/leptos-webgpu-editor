use fontdue::{Font, FontSettings, layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle}};
use sdf_glyph_renderer::BitmapGlyph;
use wasm_bindgen::prelude::*;
use web_sys::{GpuDevice, GpuTextureView, GpuRenderPipeline};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

pub struct TextRenderer {
    font: Font,
    pipeline: Option<GpuRenderPipeline>,
    bind_group_layout: Option<web_sys::GpuBindGroupLayout>,
    sdf_atlas: Option<Vec<u8>>, // Will store SDF texture data
    atlas_width: u32,
    atlas_height: u32,
    atlas_texture: Option<web_sys::GpuTexture>,
    bind_group: Option<web_sys::GpuBindGroup>,
}

impl TextRenderer {
    pub fn new() -> Result<Self, JsValue> {
        // Load the Minipax font
        let font_data = include_bytes!("../assets/fonts/Minipax-Regular.otf");
        let font = Font::from_bytes(font_data as &[u8], FontSettings::default())
            .map_err(|e| JsValue::from_str(&format!("Failed to load Minipax font: {:?}", e)))?;
        
        Ok(Self {
            font,
            pipeline: None,
            bind_group_layout: None,
            sdf_atlas: None,
            atlas_width: 512,
            atlas_height: 512,
            atlas_texture: None,
            bind_group: None,
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
    // Sample the SDF texture
    let distance = textureSample(sdf_texture, sdf_sampler, tex_coord).r;
    
    // The edge of the glyph is at a distance of 0.5 after R8unorm normalization
    // We want to smoothly transition from transparent to opaque around this value
    let smoothing = 0.05;
    let alpha = smoothstep(0.5 - smoothing, 0.5 + smoothing, distance);

    // If alpha is 0, discard the fragment to avoid issues with the clear color
    if (alpha < 0.01) {
        discard;
    }

    // Output white text with the calculated alpha
    return vec4<f32>(1.0, 1.0, 1.0, alpha);
}
"#));

        // Create bind group layout for SDF texture and sampler
        let entries = js_sys::Array::new();
        
        // Texture binding
        let texture_entry = web_sys::GpuBindGroupLayoutEntry::new(0, web_sys::gpu_shader_stage::FRAGMENT);
        let texture_binding = web_sys::GpuTextureBindingLayout::new();
        texture_entry.set_texture(&texture_binding);
        entries.push(&texture_entry);
        
        // Sampler binding
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
        
        // Define vertex buffer layout
        let vertex_attributes = js_sys::Array::new();
        
        // Position attribute at location 0
        let pos_attr = web_sys::GpuVertexAttribute::new(web_sys::GpuVertexFormat::Float32x2, 0.0, 0);
        vertex_attributes.push(&pos_attr);
        
        // Texture coordinate attribute at location 1  
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
        
        // Enable alpha blending for transparent text
        let color_target_state = web_sys::GpuColorTargetState::new(web_sys::GpuTextureFormat::Bgra8unorm);
        
        // Define the blend state for transparency
        let mut color_component = web_sys::GpuBlendComponent::new();
        color_component.operation(web_sys::GpuBlendOperation::Add);
        color_component.src_factor(web_sys::GpuBlendFactor::SrcAlpha);
        color_component.dst_factor(web_sys::GpuBlendFactor::OneMinusSrcAlpha);
        
        // Alpha component
        let mut alpha_component = web_sys::GpuBlendComponent::new();
        alpha_component.operation(web_sys::GpuBlendOperation::Add);
        alpha_component.src_factor(web_sys::GpuBlendFactor::One);
        alpha_component.dst_factor(web_sys::GpuBlendFactor::OneMinusSrcAlpha);
        
        let blend_state = web_sys::GpuBlendState::new(&alpha_component, &color_component);
        
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

    fn generate_sdf_atlas(&mut self, text: &str) -> Result<(), JsValue> {
        console_log!("Generating SDF atlas for: '{}'", text);
        
        let atlas_size = (self.atlas_width * self.atlas_height) as usize;
        let mut atlas_data = vec![0u8; atlas_size];
        
        // Generate bitmap for each unique character and create proper SDF data
        let mut unique_chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        unique_chars.sort();
        unique_chars.dedup();
        
        console_log!("Processing {} unique characters", unique_chars.len());
        
        let char_size = 64; // Size of each character in the atlas
        let chars_per_row = self.atlas_width / char_size;
        
        for (i, &ch) in unique_chars.iter().enumerate() {
            let char_x = (i as u32 % chars_per_row) * char_size;
            let char_y = (i as u32 / chars_per_row) * char_size;
            
            // Rasterize the character using fontdue
            let (metrics, bitmap) = self.font.rasterize(ch, 48.0);
            
            console_log!("Character '{}': {}x{} pixels", ch, metrics.width, metrics.height);
            
            if !bitmap.is_empty() {
                // Debug: Check bitmap data
                let bitmap_samples: Vec<u8> = bitmap.iter().take(10).cloned().collect();
                console_log!("Bitmap sample (first 10 pixels): {:?}", bitmap_samples);
                // Generate proper SDF from the bitmap using sdf_glyph_renderer
                let buffer_size = 8;
                let bitmap_glyph = match BitmapGlyph::from_unbuffered(&bitmap, metrics.width, metrics.height, buffer_size) {
                    Ok(glyph) => glyph,
                    Err(e) => {
                        console_log!("SDF glyph creation failed for '{}': {:?}", ch, e);
                        continue;
                    }
                };
                let sdf_data = bitmap_glyph.render_sdf(4); // Try smaller radius
                
                // SDF dimensions include buffer padding
                let sdf_width = metrics.width + 2 * buffer_size;
                let sdf_height = metrics.height + 2 * buffer_size;
                console_log!("SDF data: {}x{} = {} (actual: {})", sdf_width, sdf_height, sdf_width * sdf_height, sdf_data.len());
                
                // Debug: Check raw SDF values from different areas
                let total_len = sdf_data.len();
                let indices = [0, total_len/8, total_len/4, total_len/2, total_len*3/4, total_len-1];
                let sdf_samples: Vec<f64> = indices.iter()
                    .filter_map(|&i| if i < total_len { Some(sdf_data[i]) } else { None })
                    .collect();
                console_log!("Raw SDF samples (varied positions): {:?}", sdf_samples);
                
                // Copy SDF data into atlas (use SDF dimensions, not original metrics)
                let copy_width = sdf_width.min(char_size as usize);
                let copy_height = sdf_height.min(char_size as usize);
                
                console_log!("Copying {}x{} SDF data to atlas at ({}, {})", copy_width, copy_height, char_x, char_y);
                
                let mut pixels_written = 0;
                let mut sample_values = Vec::new();
                
                for y in 0..copy_height {
                    for x in 0..copy_width {
                        let atlas_x = char_x as usize + x;
                        let atlas_y = char_y as usize + y;
                        let atlas_idx = atlas_y * self.atlas_width as usize + atlas_x;
                        
                        if atlas_idx < atlas_data.len() {
                            let sdf_idx = y * sdf_width + x;
                            if sdf_idx < sdf_data.len() {
                                // Convert SDF value to 0-255 range
                                let sdf_value = ((sdf_data[sdf_idx] + 1.0) * 127.5).clamp(0.0, 255.0) as u8;
                                atlas_data[atlas_idx] = sdf_value;
                                pixels_written += 1;
                                
                                // Sample some values for debugging
                                if sample_values.len() < 5 && sdf_value != 127 { // Collect non-neutral values
                                    sample_values.push(sdf_value);
                                }
                            }
                        }
                    }
                }
                
                console_log!("Wrote {} pixels, sample values: {:?}", pixels_written, sample_values);
            }
        }
        
        self.sdf_atlas = Some(atlas_data);
        console_log!("SDF atlas generated: {}x{} with {} characters", self.atlas_width, self.atlas_height, unique_chars.len());
        Ok(())
    }

    fn create_texture_and_bind_group(&mut self, device: &GpuDevice) -> Result<(), JsValue> {
        console_log!("Creating GPU texture for SDF atlas");
        
        if let Some(ref atlas_data) = self.sdf_atlas {
            // Create texture descriptor
            let mut extent = web_sys::GpuExtent3dDict::new(self.atlas_width);
            extent.set_height(self.atlas_height);
            extent.set_depth_or_array_layers(1);
            
            let texture_desc = web_sys::GpuTextureDescriptor::new(
                web_sys::GpuTextureFormat::R8unorm,
                &extent,
                web_sys::gpu_texture_usage::TEXTURE_BINDING | web_sys::gpu_texture_usage::COPY_DST,
            );
            texture_desc.set_label("SDF Atlas Texture");
            
            let texture = device.create_texture(&texture_desc)?;
            
            // Upload atlas data to GPU
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
            
            // Create sampler
            let sampler = device.create_sampler();
            
            // Use the bind group layout from the pipeline
            let bind_group_layout = self.bind_group_layout.as_ref()
                .ok_or_else(|| JsValue::from_str("Pipeline must be created before texture. Call create_text_pipeline first."))?;
            
            // Create bind group
            let bind_entries = js_sys::Array::new();
            
            let texture_bind_entry = web_sys::GpuBindGroupEntry::new(0, &texture.create_view()?.into());
            bind_entries.push(&texture_bind_entry);
            
            let sampler_bind_entry = web_sys::GpuBindGroupEntry::new(1, &sampler);
            bind_entries.push(&sampler_bind_entry);
            
            let bind_group_desc = web_sys::GpuBindGroupDescriptor::new(&bind_entries, bind_group_layout);
            let bind_group = device.create_bind_group(&bind_group_desc);
            
            self.atlas_texture = Some(texture);
            self.bind_group = Some(bind_group);
            
            console_log!("GPU texture and bind group created successfully");
        }
        
        Ok(())
    }

    fn generate_text_vertices(&self, text: &str, x: f32, y: f32) -> Vec<f32> {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        let fonts = &[&self.font];
        
        // Pass the desired baseline y-position to the layout settings
        let mut layout_settings = LayoutSettings::default();
        layout_settings.y = y; // The `y` coordinate now defines the baseline
        layout.reset(&layout_settings);
        layout.append(fonts, &TextStyle::new(text, 48.0, 0));

        let mut vertices = Vec::new();
        let char_size = 64; // Size of each character in the atlas
        let chars_per_row = self.atlas_width / char_size;
        
        // Create mapping of characters to atlas positions
        let mut unique_chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        unique_chars.sort();
        unique_chars.dedup();
        
        for glyph in layout.glyphs() {
            let (metrics, bitmap) = self.font.rasterize(glyph.parent, glyph.key.px);
            
            if !bitmap.is_empty() {
                // The layout's `glyph.y` is now the authoritative position
                // It represents the top of the bounding box, correctly aligned to the baseline
                let glyph_top = glyph.y as f32;
                let glyph_bottom = glyph_top + metrics.height as f32;

                // Convert screen coordinates to normalized device coordinates
                let left = (glyph.x as f32 + x) / 400.0 - 1.0;   // Assuming 800px canvas width
                let right = (glyph.x as f32 + metrics.width as f32 + x) / 400.0 - 1.0;
                // The global `y` offset is no longer added here because the layout handled it
                let top = 1.0 - glyph_top / 300.0;    // Assuming 600px canvas height, flip Y
                let bottom = 1.0 - glyph_bottom / 300.0;

                // Find character's position in atlas
                let char_index = unique_chars.iter().position(|&c| c == glyph.parent).unwrap_or(0);
                let atlas_x = (char_index % chars_per_row as usize) as f32 * char_size as f32;
                let atlas_y = (char_index / chars_per_row as usize) as f32 * char_size as f32;
                
                // SDF dimensions include buffer padding
                let buffer_size = 8.0;
                let sdf_width = metrics.width as f32 + 2.0 * buffer_size;
                let sdf_height = metrics.height as f32 + 2.0 * buffer_size;
                
                // Convert atlas coordinates to UV coordinates (0.0-1.0) using SDF dimensions
                let u_left = atlas_x / self.atlas_width as f32;
                let u_right = (atlas_x + sdf_width) / self.atlas_width as f32;
                let v_top = atlas_y / self.atlas_height as f32;
                let v_bottom = (atlas_y + sdf_height) / self.atlas_height as f32;

                // Generate two triangles for the quad with proper atlas UV coordinates
                // Triangle 1
                vertices.extend_from_slice(&[
                    left, bottom,   u_left, v_bottom,   // bottom-left
                    right, bottom,  u_right, v_bottom,  // bottom-right
                    left, top,      u_left, v_top,      // top-left
                ]);
                
                // Triangle 2
                vertices.extend_from_slice(&[
                    right, bottom,  u_right, v_bottom,  // bottom-right
                    right, top,     u_right, v_top,     // top-right
                    left, top,      u_left, v_top,      // top-left
                ]);
            }
        }
        
        vertices
    }

    pub fn render_text(
        &mut self,
        device: &GpuDevice,
        view: &GpuTextureView,
        text: &str,
        x: f32,
        y: f32,
    ) -> Result<(), JsValue> {
        // Generate SDF atlas if not already created
        if self.sdf_atlas.is_none() {
            self.generate_sdf_atlas(text)?;
        }

        // Create GPU texture if not already created
        if self.atlas_texture.is_none() {
            self.create_texture_and_bind_group(device)?;
        }

        let pipeline = self.pipeline.as_ref()
            .ok_or_else(|| JsValue::from_str("Text pipeline not created. Call create_text_pipeline first."))?;

        // Generate vertices for the text glyphs
        let vertices = self.generate_text_vertices(text, x, y);
        let vertex_count = vertices.len() / 4; // 4 floats per vertex (position + tex_coord)
        
        console_log!("Rendering {} vertices for text: '{}'", vertex_count, text);

        // Create vertex buffer
        let vertex_buffer_desc = web_sys::GpuBufferDescriptor::new(
            (vertices.len() * 4) as f64, // size in bytes
            web_sys::gpu_buffer_usage::VERTEX | web_sys::gpu_buffer_usage::COPY_DST,
        );
        let vertex_buffer = device.create_buffer(&vertex_buffer_desc)?;
        
        // Upload vertex data  
        let vertex_bytes: Vec<u8> = vertices.iter()
            .flat_map(|&f| f.to_le_bytes())
            .collect();
        let vertex_data = unsafe {
            js_sys::Uint8Array::view(&vertex_bytes)
        };
        device.queue().write_buffer_with_u32_and_u8_array(&vertex_buffer, 0, &vertex_data)?;

        let command_encoder = device.create_command_encoder();
        
        let color_attachments = js_sys::Array::new();
        let clear_color = web_sys::GpuColorDict::new(0.0, 0.0, 0.0, 1.0); // Black background
        let color_attachment = web_sys::GpuRenderPassColorAttachment::new(
            web_sys::GpuLoadOp::Clear,
            web_sys::GpuStoreOp::Store,
            view
        );
        color_attachment.set_clear_value(&clear_color);
        color_attachments.push(&color_attachment);
        
        let render_pass_descriptor = web_sys::GpuRenderPassDescriptor::new(&color_attachments);
        let render_pass = command_encoder.begin_render_pass(&render_pass_descriptor)?;
        
        render_pass.set_pipeline(pipeline);
        
        // Set the bind group for SDF texture sampling
        if let Some(ref bind_group) = self.bind_group {
            render_pass.set_bind_group(0, Some(bind_group));
        }
        
        // Set vertex buffer
        render_pass.set_vertex_buffer(0, Some(&vertex_buffer));
        
        // Draw vertices
        render_pass.draw(vertex_count as u32);
        render_pass.end();
        
        device.queue().submit(&js_sys::Array::of1(&command_encoder.finish()));
        Ok(())
    }
}