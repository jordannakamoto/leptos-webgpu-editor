use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

mod gpu;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

async fn render_square() -> Result<(), JsValue> {
    console_log!("Starting square render...");
    
    // Get canvas
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("webgpu-canvas")
        .unwrap()
        .dyn_into::<HtmlCanvasElement>()
        .unwrap();
    
    // Initialize GPU context
    let context = gpu::context::GpuContext::new(&canvas).await?;
    
    // Create square pipeline
    let pipeline = gpu::square::create_square_pipeline(&context.device)?;
    
    // Get current texture view
    let view = context.get_current_texture_view()?;
    
    // Draw square
    gpu::square::draw_square(&context.device, &view, &pipeline)?;
    
    console_log!("Square rendered successfully!");
    Ok(())
}

async fn render_text() -> Result<(), JsValue> {
    console_log!("Starting text render...");
    
    // Get canvas
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("webgpu-canvas")
        .unwrap()
        .dyn_into::<HtmlCanvasElement>()
        .unwrap();
    
    // Initialize GPU context
    let context = gpu::context::GpuContext::new(&canvas).await?;
    
    // Create text renderer
    let mut text_renderer = gpu::text::TextRenderer::new()?;
    text_renderer.create_text_pipeline(&context.device)?;
    
    // Get current texture view
    let view = context.get_current_texture_view()?;
    
    // Render text
    text_renderer.render_text(&context.device, &view, "Hello World", 0.0, 0.0)?;
    
    console_log!("Text rendered successfully!");
    Ok(())
}

fn main() {
    leptos::mount::mount_to_body(|| {
        view! {
            <div>
                <h1>"WebGPU Renderer"</h1>
                <canvas id="webgpu-canvas" width="800" height="600" style="border: 1px solid black;"></canvas>
                <div>
                    <button on:click=move |_| {
                        wasm_bindgen_futures::spawn_local(async {
                            if let Err(e) = render_square().await {
                                console_log!("WebGPU error: {:?}", e);
                            }
                        });
                    }>
                        "Draw Square"
                    </button>
                    <button on:click=move |_| {
                        wasm_bindgen_futures::spawn_local(async {
                            if let Err(e) = render_text().await {
                                console_log!("WebGPU text error: {:?}", e);
                            }
                        });
                    }>
                        "Draw Text"
                    </button>
                </div>
            </div>
        }
    })
}