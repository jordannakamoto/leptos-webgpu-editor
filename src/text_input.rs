use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
use web_sys::{HtmlCanvasElement, HtmlElement, KeyboardEvent};
use std::cell::RefCell;
use std::rc::Rc;

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
    
    // Synchronous rendering without async overhead
    Effect::new(move |_| {
        let text = text_content.get();
        
        // Skip the async overhead - render synchronously
        if let Err(e) = render_text_sync(&text) {
            console_log!("WebGPU text error: {:?}", e);
        }
    });
    
    view! {
        <div>
            <canvas 
                id="webgpu-canvas" 
                width="800" 
                height="600" 
                style="border: 1px solid black; outline: none; background-color: white;" 
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

// Global WebGPU resources cache
struct WebGPUResources {
    context: Option<crate::gpu::context::GpuContext>,
    text_renderer: Option<crate::gpu::text::TextRenderer>,
    is_initialized: bool,
    is_rendering: bool,
    first_render: bool,
}

static mut WEBGPU_RESOURCES: Option<Rc<RefCell<WebGPUResources>>> = None;

async fn get_or_init_webgpu_resources() -> Result<Rc<RefCell<WebGPUResources>>, JsValue> {
    unsafe {
        if let Some(resources) = &WEBGPU_RESOURCES {
            return Ok(resources.clone());
        }
        
        let resources = Rc::new(RefCell::new(WebGPUResources {
            context: None,
            text_renderer: None,
            is_initialized: false,
            is_rendering: false,
            first_render: true,
        }));
        
        // Initialize WebGPU context and text renderer once
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
        
        // Pre-generate full character set for instant rendering
        let full_charset = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:,.<>?/~`'\" \n\t";
        text_renderer.generate_sdf_atlas(full_charset)?;
        text_renderer.create_texture_and_bind_group(&context.device)?;

        {
            let mut res = resources.borrow_mut();
            res.context = Some(context);
            res.text_renderer = Some(text_renderer);
            res.is_initialized = true;
        }

        WEBGPU_RESOURCES = Some(resources.clone());
        Ok(resources)
    }
}

fn render_text_sync(text: &str) -> Result<(), JsValue> {
    unsafe {
        if let Some(resources) = &WEBGPU_RESOURCES {
            let mut res = resources.borrow_mut();
            let context = res.context.as_ref().unwrap();
            let text_renderer = res.text_renderer.as_mut().unwrap();
            
            let canvas_width = context.canvas.width() as f32;
            let canvas_height = context.canvas.height() as f32;
            
            text_renderer.render_text(
                &context.device,
                context,
                text,
                50.0,
                100.0,
                canvas_width,
                canvas_height,
            )?;
        }
    }
    Ok(())
}