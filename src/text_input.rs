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
    
    // Initialize WebGPU after DOM is fully rendered
    Effect::new(move |_| {
        let text = text_content.get();
        
        wasm_bindgen_futures::spawn_local(async move {
            // Wait for next animation frame to ensure DOM is rendered
            let promise = js_sys::Promise::new(&mut |resolve, _| {
                if let Some(window) = web_sys::window() {
                    let _ = window.request_animation_frame(&resolve);
                }
            });
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            
            match get_or_init_webgpu_resources().await {
                Ok(resources) => {
                    if let Err(e) = render_text_with_resources(&text, &resources) {
                        console_log!("WebGPU text error: {:?}", e);
                    }
                }
                Err(e) => {
                    console_log!("WebGPU initialization error: {:?}", e);
                }
            }
        });
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
pub struct WebGPUResources {
    pub context: Option<crate::gpu::context::GpuContext>,
    pub text_renderer: Option<crate::gpu::text::TextRenderer>,
    pub fast_text_renderer: Option<crate::gpu::fast_text::FastTextRenderer>,
    pub is_initialized: bool,
    pub is_rendering: bool,
    pub first_render: bool,
}

thread_local! {
    static WEBGPU_RESOURCES: RefCell<Option<Rc<RefCell<WebGPUResources>>>> = RefCell::new(None);
}

pub async fn get_or_init_webgpu_resources() -> Result<Rc<RefCell<WebGPUResources>>, JsValue> {
    // Check if already initialized
    if let Some(existing) = WEBGPU_RESOURCES.with(|res| res.borrow().clone()) {
        return Ok(existing);
    }
        
    let res = Rc::new(RefCell::new(WebGPUResources {
            context: None,
            text_renderer: None,
            fast_text_renderer: None,
            is_initialized: false,
            is_rendering: false,
            first_render: true,
        }));
        
        // Initialize WebGPU context and text renderer once
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window available"))?;
        let document = window.document().ok_or_else(|| JsValue::from_str("No document available"))?;
        let canvas: HtmlCanvasElement = document
            .get_element_by_id("fast-webgpu-canvas")
            .ok_or_else(|| JsValue::from_str("Canvas element not found"))?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| JsValue::from_str("Element is not a canvas"))?;

        let css_width = 800.0;
        let css_height = 600.0;
        let canvas_elem = canvas.unchecked_ref::<HtmlElement>();
        canvas_elem.style().set_property("width", &format!("{}px", css_width))?;
        canvas_elem.style().set_property("height", &format!("{}px", css_height))?;

        let dpr = window.device_pixel_ratio();
        canvas.set_width((css_width * dpr) as u32);
        canvas.set_height((css_height * dpr) as u32);

        let context = crate::gpu::context::GpuContext::new(&canvas).await?;
        
        // Initialize fast text renderer for high performance
        let mut fast_text_renderer = crate::gpu::fast_text::FastTextRenderer::new(
            context.device.clone(),
            10000, // Support up to 10k characters
        )?;
        fast_text_renderer.initialize()?;
        
        // Keep old renderer as fallback
        let mut text_renderer = crate::gpu::text::TextRenderer::new()?;
        text_renderer.create_text_pipeline(&context.device)?;
        
        // Pre-generate full character set for instant rendering
        let full_charset = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:,.<>?/~`'\" \n\t";
        text_renderer.generate_sdf_atlas(full_charset)?;
        text_renderer.create_texture_and_bind_group(&context.device)?;

        {
            let mut resources_mut = res.borrow_mut();
            resources_mut.context = Some(context);
            resources_mut.text_renderer = Some(text_renderer);
            resources_mut.fast_text_renderer = Some(fast_text_renderer);
            resources_mut.is_initialized = true;
        }

    // Store in the thread local
    WEBGPU_RESOURCES.with(|r| *r.borrow_mut() = Some(res.clone()));
    
    Ok(res)
}

fn render_text_with_resources(text: &str, resources: &Rc<RefCell<WebGPUResources>>) -> Result<(), JsValue> {
    // Check fast renderer first
    {
        let mut res = resources.borrow_mut();
        if let Some(fast_renderer) = res.fast_text_renderer.as_mut() {
            fast_renderer.update_text(text)?;
        }
    }

    // Render phase
    {
        let res = resources.borrow();
        if let Some(context) = &res.context {
            if res.fast_text_renderer.is_some() {
                console_log!("Rendering via fast text renderer.");
                // TODO: Insert actual draw call here
            } else if let Some(text_renderer) = &res.text_renderer {
                let width = context.canvas.width() as f32;
                let height = context.canvas.height() as f32;
                console_log!("Rendering fallback text at {} x {}", width, height);
                // TODO: text_renderer.render_text(...)
            }
        }
    }

    Ok(())
}