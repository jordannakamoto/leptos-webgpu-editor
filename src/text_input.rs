use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, HtmlElement, KeyboardEvent};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[component]
pub fn TextInput() -> impl IntoView {
    let (text_content, set_text_content) = signal("Hello World".to_string());
    let (cursor_pos, set_cursor_pos) = signal(0_usize);
    
    // Auto-render when text changes
    Effect::new(move |_| {
        let text = text_content.get();
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = render_text_with_input(&text).await {
                console_log!("WebGPU text error: {:?}", e);
            }
        });
    });
    
    view! {
        <div>
            <canvas 
                id="webgpu-canvas" 
                width="800" 
                height="600" 
                style="border: 1px solid black; outline: none;" 
                tabindex="0"
                on:keydown=move |ev: KeyboardEvent| {
                    ev.prevent_default();
                    let key = ev.key();
                    
                    set_text_content.update(|text| {
                        let mut cursor = cursor_pos.get();
                        
                        match key.as_str() {
                            "Backspace" => {
                                if cursor > 0 {
                                    text.remove(cursor - 1);
                                    cursor -= 1;
                                }
                            }
                            "Delete" => {
                                if cursor < text.len() {
                                    text.remove(cursor);
                                }
                            }
                            "ArrowLeft" => {
                                if cursor > 0 {
                                    cursor -= 1;
                                }
                            }
                            "ArrowRight" => {
                                if cursor < text.len() {
                                    cursor += 1;
                                }
                            }
                            "Enter" => {
                                text.insert(cursor, '\n');
                                cursor += 1;
                            }
                            key if key.len() == 1 => {
                                text.insert_str(cursor, &key);
                                cursor += key.len();
                            }
                            _ => {}
                        }
                        
                        set_cursor_pos.set(cursor);
                    });
                }
            ></canvas>
            <div style="margin-top: 10px; font-size: 12px; color: #666;">
                "Click canvas and type to edit text directly"
            </div>
        </div>
    }
}

async fn render_text_with_input(text: &str) -> Result<(), JsValue> {
    console_log!("Rendering text: '{}'", text);

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas: HtmlCanvasElement = document
        .get_element_by_id("webgpu-canvas")
        .unwrap()
        .dyn_into::<HtmlCanvasElement>()
        .unwrap();

    let css_width = 800.0;
    let css_height = 600.0;
    let canvas_elem = canvas.unchecked_ref::<HtmlElement>();
    canvas_elem.style().set_property("width", &format!("{}px", css_width))?;
    canvas_elem.style().set_property("height", &format!("{}px", css_height))?;

    let dpr = window.device_pixel_ratio();
    canvas.set_width((css_width * dpr) as u32);
    canvas.set_height((css_height * dpr) as u32);

    let context = crate::gpu::context::GpuContext::new(&canvas).await?;
    let mut text_renderer = crate::gpu::text::TextRenderer::new()?;
    text_renderer.create_text_pipeline(&context.device)?;

    let view = context.get_current_texture_view()?;
    let canvas_width = canvas.width() as f32;
    let canvas_height = canvas.height() as f32;
    
    text_renderer.render_text(
        &context.device,
        &view,
        text,
        50.0,
        100.0,
        canvas_width,
        canvas_height,
    )?;

    Ok(())
}