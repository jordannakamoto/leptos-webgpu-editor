use wasm_bindgen::prelude::*;
use web_sys::{GpuDevice, GpuTextureView, GpuRenderPipeline, GpuBuffer, GpuComputePipeline};
use std::collections::HashMap;
use fontdue::{Font, FontSettings, layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle}};
use sdf_glyph_renderer::BitmapGlyph;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// Persistent GPU buffer for glyph instances
struct GlyphInstanceBuffer {
    buffer: GpuBuffer,
    capacity: usize,
    used: usize,
}

// Cached render command for reuse
struct CachedRenderCommand {
    vertex_count: u32,
    instance_count: u32,
    is_dirty: bool,
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

// Dirty region tracking for incremental updates
#[derive(Debug, Clone)]
struct DirtyRegion {
    start_char: usize,
    end_char: usize,
    needs_update: bool,
}

pub struct FastTextRenderer {
    // GPU resources
    device: GpuDevice,
    render_pipeline: Option<GpuRenderPipeline>,
    compute_pipeline: Option<GpuComputePipeline>,
    
    // Persistent buffers
    glyph_buffer: Option<GlyphInstanceBuffer>,
    position_buffer: Option<GpuBuffer>,
    vertex_buffer: Option<GpuBuffer>,
    atlas_texture: Option<web_sys::GpuTexture>,
    bind_group: Option<web_sys::GpuBindGroup>,
    
    // Font and SDF atlas
    font: Font,
    sdf_atlas: Option<Vec<u8>>,
    glyph_map: HashMap<char, GlyphInfo>,
    
    // Caching and optimization
    cached_commands: HashMap<String, CachedRenderCommand>,
    dirty_regions: Vec<DirtyRegion>,
    last_text: String,
    
    // Text state management
    text_buffer: Vec<char>,
    cursor_position: usize,
    
    // Configuration
    max_glyphs: usize,
    atlas_size: u32,
    buffer_size: usize,
}

impl FastTextRenderer {
    pub fn new(device: GpuDevice, max_glyphs: usize) -> Result<Self, JsValue> {
        console_log!("Creating FastTextRenderer with {} max glyphs", max_glyphs);
        
        // Load font
        let font_data = include_bytes!("../assets/fonts/Spectral-ExtraLight.ttf");
        if font_data.is_empty() {
            return Err(JsValue::from_str("Font file is empty or not found"));
        }
        
        let font = Font::from_bytes(font_data as &[u8], FontSettings::default())
            .map_err(|e| JsValue::from_str(&format!("Failed to load font: {:?}", e)))?;
        
        Ok(Self {
            device,
            render_pipeline: None,
            compute_pipeline: None,
            glyph_buffer: None,
            position_buffer: None,
            vertex_buffer: None,
            atlas_texture: None,
            bind_group: None,
            font,
            sdf_atlas: None,
            glyph_map: HashMap::new(),
            cached_commands: HashMap::new(),
            dirty_regions: Vec::new(),
            last_text: String::new(),
            text_buffer: Vec::new(),
            cursor_position: 0,
            max_glyphs,
            atlas_size: 1024, // Larger atlas for better performance
            buffer_size: 4,
        })
    }
    
    pub fn initialize(&mut self) -> Result<(), JsValue> {
        console_log!("Initializing FastTextRenderer GPU resources");
        
        // Create persistent vertex buffer (ring buffer style)
        let vertex_buffer_size = self.max_glyphs * 6 * 4 * 4; // 6 vertices * 4 floats * 4 bytes
        let vertex_buffer = self.device.create_buffer(&{
            let mut desc = web_sys::GpuBufferDescriptor::new(
                vertex_buffer_size as f64,
                web_sys::gpu_buffer_usage::VERTEX | web_sys::gpu_buffer_usage::COPY_DST,
            );
            desc.set_label("Persistent Vertex Buffer");
            desc.set_mapped_at_creation(false);
            desc
        })?;
        
        self.vertex_buffer = Some(vertex_buffer);
        
        // Create glyph instance buffer
        let instance_buffer_size = self.max_glyphs * 16; // 4 floats per instance
        let instance_buffer = self.device.create_buffer(&{
            let mut desc = web_sys::GpuBufferDescriptor::new(
                instance_buffer_size as f64,
                web_sys::gpu_buffer_usage::STORAGE | web_sys::gpu_buffer_usage::COPY_DST,
            );
            desc.set_label("Glyph Instance Buffer");
            desc.set_mapped_at_creation(false);
            desc
        })?;
        
        self.glyph_buffer = Some(GlyphInstanceBuffer {
            buffer: instance_buffer,
            capacity: self.max_glyphs,
            used: 0,
        });
        
        // Create position compute buffer
        let position_buffer = self.device.create_buffer(&{
            let mut desc = web_sys::GpuBufferDescriptor::new(
                (self.max_glyphs * 8) as f64, // 2 floats per position
                web_sys::gpu_buffer_usage::STORAGE | web_sys::gpu_buffer_usage::COPY_DST,
            );
            desc.set_label("Position Buffer");
            desc.set_mapped_at_creation(false);
            desc
        })?;
        
        self.position_buffer = Some(position_buffer);
        
        // Create compute pipeline for glyph positioning
        self.create_compute_pipeline()?;
        
        // Create render pipeline
        self.create_render_pipeline()?;
        
        console_log!("FastTextRenderer initialized successfully");
        Ok(())
    }
    
    fn create_compute_pipeline(&mut self) -> Result<(), JsValue> {
        let compute_shader = self.device.create_shader_module(&{
            let mut desc = web_sys::GpuShaderModuleDescriptor::new(r#"
@group(0) @binding(0) var<storage, read> glyph_data: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> positions: array<vec2<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&glyph_data)) {
        return;
    }
    
    let glyph = glyph_data[index];
    let char_code = u32(glyph.x);
    let x_offset = glyph.y;
    let y_offset = glyph.z;
    let scale = glyph.w;
    
    // Calculate position based on character metrics
    // This runs in parallel on GPU for all glyphs
    positions[index] = vec2<f32>(x_offset, y_offset);
}
"#);
            desc.set_label("Glyph Positioning Compute Shader");
            desc
        });
        
        // Create bind group layout entries
        let entries = js_sys::Array::new();
        
        // Glyph data buffer (read-only storage)
        let glyph_entry = web_sys::GpuBindGroupLayoutEntry::new(0, web_sys::gpu_shader_stage::COMPUTE);
        let glyph_buffer_layout = web_sys::GpuBufferBindingLayout::new();
        glyph_buffer_layout.set_type(web_sys::GpuBufferBindingType::ReadOnlyStorage);
        glyph_entry.set_buffer(&glyph_buffer_layout);
        entries.push(&glyph_entry);
        
        // Position buffer (read-write storage)
        let position_entry = web_sys::GpuBindGroupLayoutEntry::new(1, web_sys::gpu_shader_stage::COMPUTE);
        let position_buffer_layout = web_sys::GpuBufferBindingLayout::new();
        position_buffer_layout.set_type(web_sys::GpuBufferBindingType::Storage);
        position_entry.set_buffer(&position_buffer_layout);
        entries.push(&position_entry);
        
        // Create bind group layout
        let bind_group_layout_desc = web_sys::GpuBindGroupLayoutDescriptor::new(&entries);
        bind_group_layout_desc.set_label("Compute Bind Group Layout");
        let bind_group_layout = self.device.create_bind_group_layout(&bind_group_layout_desc)?;
        
        // Create pipeline layout
        let layouts = js_sys::Array::new();
        layouts.push(&bind_group_layout);
        let pipeline_layout_desc = web_sys::GpuPipelineLayoutDescriptor::new(&layouts);
        pipeline_layout_desc.set_label("Compute Pipeline Layout");
        let pipeline_layout = self.device.create_pipeline_layout(&pipeline_layout_desc);
        
        // Create compute stage
        let compute_stage = web_sys::GpuProgrammableStage::new(&compute_shader);
        compute_stage.set_entry_point("main");
        
        // Create compute pipeline
        let compute_pipeline_desc = web_sys::GpuComputePipelineDescriptor::new(&pipeline_layout, &compute_stage);
        compute_pipeline_desc.set_label("Glyph Positioning Pipeline");
        let compute_pipeline = self.device.create_compute_pipeline(&compute_pipeline_desc);
        
        self.compute_pipeline = Some(compute_pipeline);
        Ok(())
    }
    
    fn create_render_pipeline(&mut self) -> Result<(), JsValue> {
        let vertex_shader = self.device.create_shader_module(&{
            let mut desc = web_sys::GpuShaderModuleDescriptor::new(r#"
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
"#);
            desc.set_label("Fast Text Vertex Shader");
            desc
        });
        
        let fragment_shader = self.device.create_shader_module(&{
            let mut desc = web_sys::GpuShaderModuleDescriptor::new(r#"
@group(0) @binding(0) var sdf_texture: texture_2d<f32>;
@group(0) @binding(1) var sdf_sampler: sampler;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    let distance = textureSample(sdf_texture, sdf_sampler, tex_coord).r;
    
    // Dynamic width based on derivatives for better quality at all scales
    var width = fwidth(distance);
    
    // For very small text, we need a bit more smoothing
    // For large text, we want it sharper
    width = clamp(width * 1.2, 0.001, 0.3);
    
    // Use smoothstep for antialiasing - note: 1.0 - smoothstep to invert
    let alpha = 1.0 - smoothstep(0.5 - width, 0.5 + width, distance);
    
    if (alpha < 0.001) {
        discard;
    }
    
    return vec4<f32>(1.0, 1.0, 1.0, alpha); // White text
}
"#);
            desc.set_label("Fast Text Fragment Shader");
            desc
        });
        
        // Create bind group layout for fragment shader
        let fragment_entries = js_sys::Array::new();
        
        // SDF texture binding
        let texture_entry = web_sys::GpuBindGroupLayoutEntry::new(0, web_sys::gpu_shader_stage::FRAGMENT);
        let texture_binding = web_sys::GpuTextureBindingLayout::new();
        texture_entry.set_texture(&texture_binding);
        fragment_entries.push(&texture_entry);
        
        // Sampler binding
        let sampler_entry = web_sys::GpuBindGroupLayoutEntry::new(1, web_sys::gpu_shader_stage::FRAGMENT);
        let sampler_binding = web_sys::GpuSamplerBindingLayout::new();
        sampler_entry.set_sampler(&sampler_binding);
        fragment_entries.push(&sampler_entry);
        
        let bind_group_layout_desc = web_sys::GpuBindGroupLayoutDescriptor::new(&fragment_entries);
        bind_group_layout_desc.set_label("Fast Text Bind Group Layout");
        let bind_group_layout = self.device.create_bind_group_layout(&bind_group_layout_desc)?;
        
        // Create pipeline layout
        let layouts = js_sys::Array::new();
        layouts.push(&bind_group_layout);
        let pipeline_layout_desc = web_sys::GpuPipelineLayoutDescriptor::new(&layouts);
        pipeline_layout_desc.set_label("Fast Text Pipeline Layout");
        let pipeline_layout = self.device.create_pipeline_layout(&pipeline_layout_desc);
        
        // Create vertex attributes
        let vertex_attributes = js_sys::Array::new();
        let pos_attr = web_sys::GpuVertexAttribute::new(web_sys::GpuVertexFormat::Float32x2, 0.0, 0);
        vertex_attributes.push(&pos_attr);
        let tex_attr = web_sys::GpuVertexAttribute::new(web_sys::GpuVertexFormat::Float32x2, 8.0, 1);
        vertex_attributes.push(&tex_attr);
        
        // Create vertex buffer layout
        let vertex_buffer_layout = web_sys::GpuVertexBufferLayout::new(16.0, &vertex_attributes);
        vertex_buffer_layout.set_step_mode(web_sys::GpuVertexStepMode::Vertex);
        
        let vertex_buffers = js_sys::Array::new();
        vertex_buffers.push(&vertex_buffer_layout);
        
        // Create vertex state
        let vertex_state = web_sys::GpuVertexState::new(&vertex_shader);
        vertex_state.set_entry_point("main");
        vertex_state.set_buffers(&vertex_buffers);
        
        // Create fragment state
        let targets = js_sys::Array::new();
        let color_target_state = web_sys::GpuColorTargetState::new(web_sys::GpuTextureFormat::Bgra8unorm);
        
        // Add blend state for transparency
        let color_component = web_sys::GpuBlendComponent::new();
        color_component.set_operation(web_sys::GpuBlendOperation::Add);
        color_component.set_src_factor(web_sys::GpuBlendFactor::SrcAlpha);
        color_component.set_dst_factor(web_sys::GpuBlendFactor::OneMinusSrcAlpha);
        
        let alpha_component = web_sys::GpuBlendComponent::new();
        alpha_component.set_operation(web_sys::GpuBlendOperation::Add);
        alpha_component.set_src_factor(web_sys::GpuBlendFactor::One);
        alpha_component.set_dst_factor(web_sys::GpuBlendFactor::OneMinusSrcAlpha);
        
        let blend_state = web_sys::GpuBlendState::new(&color_component, &alpha_component);
        color_target_state.set_blend(&blend_state);
        
        targets.push(&color_target_state);
        let fragment_state = web_sys::GpuFragmentState::new(&fragment_shader, &targets);
        fragment_state.set_entry_point("main");
        
        // Create primitive state
        let primitive = web_sys::GpuPrimitiveState::new();
        primitive.set_topology(web_sys::GpuPrimitiveTopology::TriangleList);
        
        // Create render pipeline
        let render_pipeline_desc = web_sys::GpuRenderPipelineDescriptor::new(&pipeline_layout, &vertex_state);
        render_pipeline_desc.set_fragment(&fragment_state);
        render_pipeline_desc.set_primitive(&primitive);
        render_pipeline_desc.set_label("Fast Text Render Pipeline");
        
        let render_pipeline = self.device.create_render_pipeline(&render_pipeline_desc)?;
        
        self.render_pipeline = Some(render_pipeline);
        Ok(())
    }
    
    pub fn update_text(&mut self, new_text: &str) -> Result<(), JsValue> {
        // Calculate dirty regions by comparing with previous text
        self.calculate_dirty_regions(new_text);
        
        // Only update changed regions
        if !self.dirty_regions.is_empty() {
            self.update_dirty_regions(new_text)?;
        }
        
        self.last_text = new_text.to_string();
        Ok(())
    }
    
    fn calculate_dirty_regions(&mut self, new_text: &str) {
        self.dirty_regions.clear();
        
        let old_chars: Vec<char> = self.last_text.chars().collect();
        let new_chars: Vec<char> = new_text.chars().collect();
        
        let mut start = 0;
        let mut end = 0;
        let mut in_dirty_region = false;
        
        let max_len = old_chars.len().max(new_chars.len());
        
        for i in 0..max_len {
            let old_char = old_chars.get(i);
            let new_char = new_chars.get(i);
            
            if old_char != new_char {
                if !in_dirty_region {
                    start = i;
                    in_dirty_region = true;
                }
                end = i;
            } else if in_dirty_region {
                self.dirty_regions.push(DirtyRegion {
                    start_char: start,
                    end_char: end + 1,
                    needs_update: true,
                });
                in_dirty_region = false;
            }
        }
        
        if in_dirty_region {
            self.dirty_regions.push(DirtyRegion {
                start_char: start,
                end_char: end + 1,
                needs_update: true,
            });
        }
        
        console_log!("Found {} dirty regions", self.dirty_regions.len());
    }
    
    fn update_dirty_regions(&mut self, text: &str) -> Result<(), JsValue> {
        // This would update only the changed character data
        // For now, simplified implementation
        console_log!("Updating dirty regions for text: {}", text);
        Ok(())
    }
    
    pub fn generate_sdf_atlas(&mut self, text: &str) -> Result<(), JsValue> {
        console_log!("Generating SDF atlas for: '{}'", text);
        
        let atlas_size = (self.atlas_size * self.atlas_size) as usize;
        let mut atlas_data = vec![128u8; atlas_size]; // Initialize with middle gray
        
        let mut unique_chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        unique_chars.sort();
        unique_chars.dedup();
        
        console_log!("Processing {} unique characters", unique_chars.len());
        
        let char_size = 64;
        let chars_per_row = self.atlas_size / char_size;
        
        self.glyph_map.clear();
        
        for (i, &ch) in unique_chars.iter().enumerate() {
            let char_x = (i as u32 % chars_per_row) * char_size;
            let char_y = (i as u32 / chars_per_row) * char_size;
            
            let (metrics, bitmap) = self.font.rasterize(ch, 12.0);
            
            if !bitmap.is_empty() && metrics.width > 0 && metrics.height > 0 {
                // Generate SDF
                let bitmap_glyph = match BitmapGlyph::from_unbuffered(&bitmap, metrics.width, metrics.height, self.buffer_size) {
                    Ok(glyph) => glyph,
                    Err(e) => {
                        console_log!("SDF glyph creation failed for '{}': {:?}", ch, e);
                        continue;
                    }
                };
                
                let sdf_radius = 4.0;
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
                        let atlas_idx = atlas_y * self.atlas_size as usize + atlas_x;
                        
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
    
    pub fn render(&mut self, text: &str, x: f32, y: f32, screen_width: f32, screen_height: f32, context: &crate::gpu::context::GpuContext) -> Result<(), JsValue> {
        // Update text if changed
        if text != self.last_text {
            self.update_text(text)?;
        }
        
        // Use the main render_text function
        self.render_text(text, x, y, screen_width, screen_height, context)
    }
    
    pub fn render_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        screen_width: f32,
        screen_height: f32,
        context: &crate::gpu::context::GpuContext,
    ) -> Result<(), JsValue> {
        // Update text if changed
        if text != self.last_text {
            self.update_text(text)?;
        }
        
        // Ensure we have atlas and bind group BEFORE generating vertices
        if self.sdf_atlas.is_none() || !text.chars().all(|c| c.is_whitespace() || self.glyph_map.contains_key(&c)) {
            console_log!("Generating SDF atlas for text: '{}'", text);
            self.generate_sdf_atlas(text)?;
            self.atlas_texture = None;
            self.bind_group = None;
        }

        if self.atlas_texture.is_none() {
            console_log!("Creating texture and bind group");
            self.create_texture_and_bind_group()?;
        }
        
        // Generate vertices for text
        let vertices = self.generate_text_vertices(text, x, y, screen_width, screen_height);
        let vertex_count = vertices.len() / 4;
        
        console_log!("Generated {} vertices for text: '{}'", vertex_count, text);
        
        if vertex_count == 0 {
            console_log!("No vertices generated, returning early");
            return Ok(());
        }
        
        // Atlas and bind group already created above
        
        let pipeline = self.render_pipeline.as_ref()
            .ok_or_else(|| JsValue::from_str("Render pipeline not created"))?;
        
        // Use persistent vertex buffer
        let vertex_buffer = self.vertex_buffer.as_ref()
            .ok_or_else(|| JsValue::from_str("Persistent vertex buffer not initialized"))?;
        
        let vertex_bytes: Vec<u8> = vertices.iter()
            .flat_map(|&f| f.to_le_bytes())
            .collect();
        let vertex_data = unsafe {
            js_sys::Uint8Array::view(&vertex_bytes)
        };
        self.device.queue().write_buffer_with_u32_and_u8_array(vertex_buffer, 0, &vertex_data)?;
        
        // Create command encoder and render pass
        let command_encoder = self.device.create_command_encoder();
        
        let color_attachments = js_sys::Array::new();
        let clear_color = web_sys::GpuColorDict::new(0.1, 0.1, 0.1, 1.0); // Dark gray background
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
        
        render_pass.set_vertex_buffer(0, Some(vertex_buffer));
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
        
        self.device.queue().submit(&js_sys::Array::of1(&command_encoder.finish()));
        Ok(())
    }
    
    fn execute_cached_render(&self, _cached: &CachedRenderCommand, _context: &crate::gpu::context::GpuContext) -> Result<(), JsValue> {
        // This function is deprecated - use render_text instead
        console_log!("execute_cached_render is deprecated - use render_text instead");
        Ok(())
    }
    
    pub fn create_texture_and_bind_group(&mut self) -> Result<(), JsValue> {
        if let Some(ref atlas_data) = self.sdf_atlas {
            let mut extent = web_sys::GpuExtent3dDict::new(self.atlas_size);
            extent.set_height(self.atlas_size);
            extent.set_depth_or_array_layers(1);
            
            let texture_desc = web_sys::GpuTextureDescriptor::new(
                web_sys::GpuTextureFormat::R8unorm,
                &extent,
                web_sys::gpu_texture_usage::TEXTURE_BINDING | web_sys::gpu_texture_usage::COPY_DST,
            );
            
            let texture = self.device.create_texture(&texture_desc)?;
            
            let data_layout = web_sys::GpuTexelCopyBufferLayout::new();
            data_layout.set_bytes_per_row(self.atlas_size);
            data_layout.set_rows_per_image(self.atlas_size);
            
            let mut copy_size = web_sys::GpuExtent3dDict::new(self.atlas_size);
            copy_size.set_height(self.atlas_size);
            copy_size.set_depth_or_array_layers(1);
            
            let destination = web_sys::GpuTexelCopyTextureInfo::new(&texture);
            
            self.device.queue().write_texture_with_u8_slice_and_gpu_extent_3d_dict(
                &destination,
                atlas_data.as_slice(),
                &data_layout,
                &copy_size,
            )?;
            
            let sampler_desc = web_sys::GpuSamplerDescriptor::new();
            sampler_desc.set_mag_filter(web_sys::GpuFilterMode::Linear);
            sampler_desc.set_min_filter(web_sys::GpuFilterMode::Linear);
            sampler_desc.set_mipmap_filter(web_sys::GpuMipmapFilterMode::Linear);
            let sampler = self.device.create_sampler_with_descriptor(&sampler_desc);
            
            // Create bind group for fragment shader
            let bind_entries = js_sys::Array::new();
            
            let texture_bind_entry = web_sys::GpuBindGroupEntry::new(0, &texture.create_view()?.into());
            bind_entries.push(&texture_bind_entry);
            
            let sampler_bind_entry = web_sys::GpuBindGroupEntry::new(1, &sampler);
            bind_entries.push(&sampler_bind_entry);
            
            // We need to get the bind group layout from the render pipeline
            if let Some(ref pipeline) = self.render_pipeline {
                let bind_group_layout = pipeline.get_bind_group_layout(0);
                let bind_group_desc = web_sys::GpuBindGroupDescriptor::new(&bind_entries, &bind_group_layout);
                let bind_group = self.device.create_bind_group(&bind_group_desc);
                
                self.atlas_texture = Some(texture);
                self.bind_group = Some(bind_group);
            } else {
                return Err(JsValue::from_str("Render pipeline must be created before texture"));
            }
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
        layout.append(fonts, &TextStyle::new(text, 12.0, 0));
    
        let mut vertices = Vec::new();
        
        for glyph in layout.glyphs() {
            console_log!("Processing glyph: '{}' at position ({}, {})", glyph.parent, glyph.x, glyph.y);
            if let Some(glyph_info) = self.glyph_map.get(&glyph.parent) {
                console_log!("Found glyph info for: '{}' at atlas ({}, {})", glyph.parent, glyph_info.atlas_x, glyph_info.atlas_y);
                // Use layout positions directly - fontdue handles baseline alignment
                let glyph_left = glyph.x as f32;
                let glyph_top = glyph.y as f32;
                console_log!("Screen position: ({}, {})", glyph_left, glyph_top);
                
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
                let u_left = glyph_info.atlas_x / self.atlas_size as f32;
                let u_right = (glyph_info.atlas_x + glyph_info.sdf_width) / self.atlas_size as f32;
                let v_top = glyph_info.atlas_y / self.atlas_size as f32;
                let v_bottom = (glyph_info.atlas_y + glyph_info.sdf_height) / self.atlas_size as f32;
    
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
        
        console_log!("Generated {} total vertices", vertices.len());
        vertices
    }
    
    fn generate_and_cache_render_command(&mut self, cache_key: &str, context: &crate::gpu::context::GpuContext) -> Result<(), JsValue> {
        console_log!("Generating new render command for key: {}", cache_key);
        
        // Generate SDF atlas if needed
        let current_text = self.last_text.clone();
        if self.sdf_atlas.is_none() || !current_text.chars().all(|c| c.is_whitespace() || self.glyph_map.contains_key(&c)) {
            self.generate_sdf_atlas(&current_text)?;
            // Recreate texture with new atlas
            self.atlas_texture = None;
            self.bind_group = None;
        }

        if self.atlas_texture.is_none() {
            self.create_texture_and_bind_group()?;
        }
        
        // Calculate vertex count based on text length
        let text_len = self.last_text.len();
        let vertex_count = (text_len * 6) as u32; // 6 vertices per character (2 triangles)
        
        // Generate new render command and cache it
        let cached_command = CachedRenderCommand {
            vertex_count,
            instance_count: text_len as u32,
            is_dirty: false,
        };
        
        self.cached_commands.insert(cache_key.to_string(), cached_command);
        
        // For now, just return Ok - the actual rendering happens in render_text
        Ok(())
    }

    // Character-based text operations
    pub fn insert_char(&mut self, ch: char) -> Result<(), JsValue> {
        if self.cursor_position <= self.text_buffer.len() {
            self.text_buffer.insert(self.cursor_position, ch);
            self.cursor_position += 1;
            self.update_last_text();
        }
        Ok(())
    }

    pub fn delete_char_before_cursor(&mut self) -> Result<(), JsValue> {
        if self.cursor_position > 0 {
            self.text_buffer.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
            self.update_last_text();
        }
        Ok(())
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.text_buffer.len() {
            self.cursor_position += 1;
        }
    }

    pub fn get_cursor_position(&self) -> usize {
        self.cursor_position
    }

    pub fn get_text(&self) -> String {
        self.text_buffer.iter().collect()
    }

    pub fn set_text(&mut self, text: &str) {
        self.text_buffer = text.chars().collect();
        self.cursor_position = self.text_buffer.len();
        self.update_last_text();
    }

    fn update_last_text(&mut self) {
        self.last_text = self.text_buffer.iter().collect();
    }
}