use fontdue::{Font, FontSettings, layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle}};
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
            sdf_atlas: None,
            atlas_width: 512,
            atlas_height: 512,
            atlas_texture: None,
            bind_group: None,
        })
    }

    pub fn create_text_pipeline(&mut self, device: &GpuDevice) -> Result<(), JsValue> {
        let vertex_shader = device.create_shader_module(&web_sys::GpuShaderModuleDescriptor::new(r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn main(@builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput {
    // Create a simple quad
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(-0.05, -0.05), vec2<f32>(0.05, -0.05), vec2<f32>(-0.05, 0.05),
        vec2<f32>(0.05, -0.05), vec2<f32>(0.05, 0.05), vec2<f32>(-0.05, 0.05)
    );
    var tex = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 0.0)
    );
    
    // Offset each instance (glyph) horizontally
    let offset_x = (f32(instance_index) - 5.0) * 0.08; // Center around 0, space out characters
    
    var output: VertexOutput;
    output.position = vec4<f32>(pos[vertex_index].x + offset_x, pos[vertex_index].y, 0.0, 1.0);
    output.tex_coord = tex[vertex_index];
    return output;
}
"#));

        let fragment_shader = device.create_shader_module(&web_sys::GpuShaderModuleDescriptor::new(r#"
@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    // For now, create a simple rectangular shape for each glyph
    // This will be replaced with actual SDF texture sampling
    let center = vec2<f32>(0.5, 0.5);
    let size = vec2<f32>(0.8, 0.8);
    
    let edge = step(0.1, tex_coord.x) * step(tex_coord.x, 0.9) * 
               step(0.1, tex_coord.y) * step(tex_coord.y, 0.9);
    
    return vec4<f32>(0.2, 0.8, 1.0, edge); // Blue rectangles for now
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
        
        self.pipeline = Some(device.create_render_pipeline(&pipeline_desc)?);
        Ok(())
    }

    fn generate_sdf_atlas(&mut self, text: &str) -> Result<(), JsValue> {
        console_log!("Generating SDF atlas for: '{}'", text);
        
        let atlas_size = (self.atlas_width * self.atlas_height) as usize;
        let mut atlas_data = vec![0u8; atlas_size];
        
        // Generate bitmap for each unique character and create simple SDF-like data
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
            
            // Copy bitmap into atlas with simple SDF effect
            for y in 0..metrics.height.min(char_size as usize) {
                for x in 0..metrics.width.min(char_size as usize) {
                    let atlas_x = char_x as usize + x;
                    let atlas_y = char_y as usize + y;
                    let atlas_idx = atlas_y * self.atlas_width as usize + atlas_x;
                    
                    if atlas_idx < atlas_data.len() && y < bitmap.len() / metrics.width {
                        let bitmap_idx = y * metrics.width + x;
                        if bitmap_idx < bitmap.len() {
                            // Simple conversion: fontdue bitmap to SDF-like value
                            atlas_data[atlas_idx] = bitmap[bitmap_idx];
                        }
                    }
                }
            }
        }
        
        self.sdf_atlas = Some(atlas_data);
        console_log!("SDF atlas generated: {}x{} with {} characters", self.atlas_width, self.atlas_height, unique_chars.len());
        Ok(())
    }

    fn create_texture_and_bind_group(&mut self, _device: &GpuDevice) -> Result<(), JsValue> {
        console_log!("Texture creation temporarily skipped - using placeholder");
        // TODO: Implement proper texture creation when WebGPU API is available
        Ok(())
    }

    fn generate_text_vertices(&self, text: &str, x: f32, y: f32) -> Vec<f32> {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        let fonts = &[&self.font];
        layout.reset(&LayoutSettings::default());
        layout.append(fonts, &TextStyle::new(text, 48.0, 0));

        let mut vertices = Vec::new();
        
        for glyph in layout.glyphs() {
            let (metrics, bitmap) = self.font.rasterize(glyph.parent, glyph.key.px);
            
            if !bitmap.is_empty() {
                // Convert screen coordinates to normalized device coordinates
                let left = (glyph.x as f32 + x) / 400.0 - 1.0;   // Assuming 800px canvas width
                let right = (glyph.x as f32 + metrics.width as f32 + x) / 400.0 - 1.0;
                let top = 1.0 - (glyph.y as f32 + y) / 300.0;    // Assuming 600px canvas height, flip Y
                let bottom = 1.0 - (glyph.y as f32 + metrics.height as f32 + y) / 300.0;

                // Generate two triangles for the quad
                // Triangle 1
                vertices.extend_from_slice(&[
                    left, bottom,   0.0, 1.0,  // bottom-left
                    right, bottom,  1.0, 1.0,  // bottom-right
                    left, top,      0.0, 0.0,  // top-left
                ]);
                
                // Triangle 2
                vertices.extend_from_slice(&[
                    right, bottom,  1.0, 1.0,  // bottom-right
                    right, top,     1.0, 0.0,  // top-right
                    left, top,      0.0, 0.0,  // top-left
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
        let glyph_count = vertices.len() / 24; // 4 floats per vertex, 6 vertices per glyph
        
        console_log!("Rendering {} glyphs for text: '{}'", glyph_count, text);

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
        
        // Use instanced rendering to draw multiple glyphs
        // 6 vertices per quad, glyph_count instances (one per character)
        render_pass.draw_with_instance_count(6, glyph_count as u32);
        render_pass.end();
        
        device.queue().submit(&js_sys::Array::of1(&command_encoder.finish()));
        Ok(())
    }
}