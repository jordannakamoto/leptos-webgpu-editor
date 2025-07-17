use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, HtmlElement};

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
    
    // Get canvas and set up DPI scaling
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("webgpu-canvas")
        .unwrap()
        .dyn_into::<HtmlCanvasElement>()
        .unwrap();
    
    // Apply DPI scaling
    let dpr = window.device_pixel_ratio();
    let display_width = canvas.client_width() as f64;
    let display_height = canvas.client_height() as f64;
    canvas.set_width((display_width * dpr) as u32);
    canvas.set_height((display_height * dpr) as u32);
    
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
    let canvas: HtmlCanvasElement = document
        .get_element_by_id("webgpu-canvas")
        .unwrap()
        .dyn_into::<HtmlCanvasElement>()
        .unwrap();

    // ✅ Set CSS size explicitly to prevent layout shift
    let css_width = 800.0;
    let css_height = 600.0;
    let canvas_elem = canvas.unchecked_ref::<HtmlElement>();
canvas_elem
    .style()
    .set_property("width", &format!("{}px", css_width))?;
canvas_elem
    .style()
    .set_property("height", &format!("{}px", css_height))?;

    // ✅ Set internal resolution with device pixel ratio
    let dpr = window.device_pixel_ratio();
    canvas.set_width((css_width * dpr) as u32);
    canvas.set_height((css_height * dpr) as u32);

    // Initialize WebGPU context
    let context = gpu::context::GpuContext::new(&canvas).await?;

    // Create text renderer and pipeline
    let mut text_renderer = gpu::text::TextRenderer::new()?;
    text_renderer.create_text_pipeline(&context.device)?;

    // Get current texture view
    let view = context.get_current_texture_view()?;

    // ✅ Pass actual canvas dimensions (in pixels) to renderer
    let canvas_width = canvas.width() as f32;
    let canvas_height = canvas.height() as f32;
    text_renderer.render_text(
        &context.device,
        &view,
        "Hello World",
        100.0,
        100.0,
        canvas_width,
        canvas_height,
    )?;

    console_log!("Text rendered successfully!");
    Ok(())
}

fn main() {
    leptos::mount::mount_to_body(|| {
        view! {
            <div>
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