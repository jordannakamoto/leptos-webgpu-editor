use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys;
use getrandom::getrandom;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

fn random_f32() -> f32 {
    let mut bytes = [0u8; 4];
    getrandom(&mut bytes).unwrap();
    let val = u32::from_ne_bytes(bytes) as f32 / u32::MAX as f32;
    val * 2.0 - 1.0
}

fn generate_random_shape() -> (String, String) {
    let shape_type = (random_f32() * 3.0).floor() as i32;
    
    match shape_type {
        0 => {
            let x = random_f32() * 0.5;
            let y = random_f32() * 0.5;
            let size = 0.2 + random_f32() * 0.3;
            
            let vertex_code = format!(r#"
                @vertex
                fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {{
                    let pos = array<vec2<f32>, 3>(
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {})
                    );
                    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
                }}
            "#, x, y + size, x - size, y - size, x + size, y - size);
            
            let r = (random_f32() + 1.0) * 0.5;
            let g = (random_f32() + 1.0) * 0.5;
            let b = (random_f32() + 1.0) * 0.5;
            
            let fragment_code = format!(r#"
                @fragment
                fn fs_main() -> @location(0) vec4<f32> {{
                    return vec4<f32>({}, {}, {}, 1.0);
                }}
            "#, r, g, b);
            
            (vertex_code, fragment_code)
        },
        1 => {
            let x = random_f32() * 0.5;
            let y = random_f32() * 0.5;
            let size = 0.15 + random_f32() * 0.25;
            
            let vertex_code = format!(r#"
                @vertex
                fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {{
                    let pos = array<vec2<f32>, 6>(
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {})
                    );
                    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
                }}
            "#, x - size, y + size, x + size, y + size, x - size, y - size,
               x + size, y + size, x + size, y - size, x - size, y - size);
            
            let r = (random_f32() + 1.0) * 0.5;
            let g = (random_f32() + 1.0) * 0.5;
            let b = (random_f32() + 1.0) * 0.5;
            
            let fragment_code = format!(r#"
                @fragment
                fn fs_main() -> @location(0) vec4<f32> {{
                    return vec4<f32>({}, {}, {}, 1.0);
                }}
            "#, r, g, b);
            
            (vertex_code, fragment_code)
        },
        _ => {
            let x = random_f32() * 0.5;
            let y = random_f32() * 0.5;
            let size = 0.1 + random_f32() * 0.2;
            
            let vertex_code = format!(r#"
                @vertex
                fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {{
                    let pos = array<vec2<f32>, 18>(
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {}),
                        vec2<f32>({}, {})
                    );
                    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
                }}
            "#, 
            x, y, x + size * 0.866, y + size * 0.5, x, y + size,
            x, y, x, y + size, x - size * 0.866, y + size * 0.5,
            x, y, x - size * 0.866, y + size * 0.5, x - size * 0.866, y - size * 0.5,
            x, y, x - size * 0.866, y - size * 0.5, x, y - size,
            x, y, x, y - size, x + size * 0.866, y - size * 0.5,
            x, y, x + size * 0.866, y - size * 0.5, x + size * 0.866, y + size * 0.5);
            
            let r = (random_f32() + 1.0) * 0.5;
            let g = (random_f32() + 1.0) * 0.5;
            let b = (random_f32() + 1.0) * 0.5;
            
            let fragment_code = format!(r#"
                @fragment
                fn fs_main() -> @location(0) vec4<f32> {{
                    return vec4<f32>({}, {}, {}, 1.0);
                }}
            "#, r, g, b);
            
            (vertex_code, fragment_code)
        }
    }
}

async fn init_webgpu() -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document.get_element_by_id("webgpu-canvas").unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;
    
    let navigator = window.navigator();
    let gpu = navigator.gpu();
    
    if gpu.is_undefined() {
        console_log!("WebGPU not supported");
        return Err(JsValue::from_str("WebGPU not supported"));
    }
    
    let adapter_promise = gpu.request_adapter();
    let adapter = JsFuture::from(adapter_promise).await?;
    let adapter: web_sys::GpuAdapter = adapter.dyn_into()?;
    
    let device_promise = adapter.request_device();
    let device = JsFuture::from(device_promise).await?;
    let device: web_sys::GpuDevice = device.dyn_into()?;
    
    let context = canvas.get_context("webgpu")?.unwrap();
    let context: web_sys::GpuCanvasContext = context.dyn_into()?;
    
    let config = web_sys::GpuCanvasConfiguration::new(
        &device,
        web_sys::GpuTextureFormat::Bgra8unorm
    );
    config.set_alpha_mode(web_sys::GpuCanvasAlphaMode::Opaque);
    context.configure(&config)?;
    
    let (vertex_shader_code, fragment_shader_code) = generate_random_shape();
    
    let vertex_count = if vertex_shader_code.contains("array<vec2<f32>, 3>") {
        3
    } else if vertex_shader_code.contains("array<vec2<f32>, 6>") {
        6
    } else {
        18
    };
    
    let vertex_shader_desc = web_sys::GpuShaderModuleDescriptor::new(&vertex_shader_code);
    let vertex_shader = device.create_shader_module(&vertex_shader_desc);
    
    let fragment_shader_desc = web_sys::GpuShaderModuleDescriptor::new(&fragment_shader_code);
    let fragment_shader = device.create_shader_module(&fragment_shader_desc);
    
    let vertex_state = web_sys::GpuVertexState::new(&vertex_shader);
    vertex_state.set_entry_point("vs_main");
    
    let color_target = web_sys::GpuColorTargetState::new(web_sys::GpuTextureFormat::Bgra8unorm);
    
    let fragment_state = web_sys::GpuFragmentState::new(&fragment_shader, &js_sys::Array::of1(&color_target));
    fragment_state.set_entry_point("fs_main");
    
    let pipeline_desc = web_sys::GpuRenderPipelineDescriptor::new(&"auto".into(), &vertex_state);
    pipeline_desc.set_fragment(&fragment_state);
    pipeline_desc.set_primitive(&web_sys::GpuPrimitiveState::new());
    
    let pipeline = device.create_render_pipeline(&pipeline_desc)?;
    
    let command_encoder = device.create_command_encoder();
    let texture = context.get_current_texture()?;
    let texture_view = texture.create_view()?;
    
    let color_attachment = web_sys::GpuRenderPassColorAttachment::new(
        web_sys::GpuLoadOp::Clear,
        web_sys::GpuStoreOp::Store,
        &texture_view
    );
    color_attachment.set_clear_value(&js_sys::Array::of4(&JsValue::from(0.0), &JsValue::from(0.0), &JsValue::from(0.0), &JsValue::from(1.0)));
    
    let render_pass_desc = web_sys::GpuRenderPassDescriptor::new(&js_sys::Array::of1(&color_attachment));
    let render_pass = command_encoder.begin_render_pass(&render_pass_desc)?;
    
    render_pass.set_pipeline(&pipeline);
    render_pass.draw(vertex_count);
    render_pass.end();
    
    let command_buffer = command_encoder.finish();
    device.queue().submit(&js_sys::Array::of1(&command_buffer));
    
    console_log!("WebGPU random shape rendered successfully!");
    Ok(())
}

fn main() {
    leptos::mount::mount_to_body(|| {
        view! {
            <div>
                <h1>"WebGPU Random Shape Generator"</h1>
                <canvas id="webgpu-canvas" width="800" height="600" style="border: 1px solid black;"></canvas>
                <div>
                    <button on:click=move |_| {
                        wasm_bindgen_futures::spawn_local(async {
                            if let Err(e) = init_webgpu().await {
                                console_log!("Error initializing WebGPU: {:?}", e);
                            }
                        });
                    }>
                        "Generate Random Shape"
                    </button>
                </div>
                <p>"Click the button to generate a random shape (triangle, rectangle, or hexagon) with random colors and positions!"</p>
            </div>
        }
    })
}
