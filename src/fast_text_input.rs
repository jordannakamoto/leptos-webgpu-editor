use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, HtmlTextAreaElement};
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
pub fn FastTextInput() -> impl IntoView {
    let (text_content, set_text_content) = signal("Hello World".to_string());
    let (cursor_pos, set_cursor_pos) = signal(0_usize);
    let (render_frame, set_render_frame) = signal(0u32);
    
    // Animation frame throttling
    let animation_frame_id = Rc::new(RefCell::new(None::<i32>));
    
    // Setup input capture and rendering
    Effect::new(move |_| {
        setup_input_capture();
        let _ = set_render_frame; // Use the signal to avoid warnings
    });
    
    view! {
        <div style="position: relative; width: 800px; height: 600px;">
            // Hidden textarea for native input capture
            <textarea
                id="hidden-input"
                style="
                    position: absolute;
                    left: 0;
                    top: 0;
                    width: 1px;
                    height: 1px;
                    opacity: 0;
                    z-index: -1;
                    resize: none;
                    border: none;
                    outline: none;
                    background: transparent;
                "
                autocomplete="off"
                spellcheck="false"
            ></textarea>
            
            // WebGPU canvas
            <canvas 
                id="fast-webgpu-canvas" 
                width="800" 
                height="600" 
                style="
                    border: 1px solid black; 
                    outline: none; 
                    background-color: #1a1a1a;
                    display: block;
                    cursor: text;
                " 
                tabindex="0"
                on:click=move |_| {
                    // Focus the hidden textarea when canvas is clicked
                    focus_hidden_input();
                }
            ></canvas>
            
            // Status display
            <div style="margin-top: 10px; font-size: 12px; color: #666;">
                "High-performance WebGPU text editor - Click to focus"
            </div>
        </div>
    }
}

#[wasm_bindgen]
pub fn setup_input_capture() {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    
    if let Some(textarea) = document.get_element_by_id("hidden-input") {
        let textarea: HtmlTextAreaElement = textarea.dyn_into().unwrap();
        
        // Get shared memory buffer pointer
        let buffer_ptr = crate::input_buffer::get_input_buffer_ptr();
        let memory = wasm_bindgen::memory();
        
        // Setup input event listener that syncs with GPU renderer
        let input_callback = Closure::wrap(Box::new(move || {
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            
            if let Some(textarea) = document.get_element_by_id("hidden-input") {
                let textarea: HtmlTextAreaElement = textarea.dyn_into().unwrap();
                let value = textarea.value();
                let cursor_pos = textarea.selection_start().unwrap_or(Some(0)).unwrap_or(0) as usize;
                
                // Sync textarea with GPU renderer
                wasm_bindgen_futures::spawn_local(async move {
                    match crate::text_input::get_or_init_webgpu_resources().await {
                        Ok(resources) => {
                            let mut borrowed = resources.borrow_mut();
                            if let Some(renderer) = borrowed.fast_text_renderer.as_mut() {
                                // Set the text from textarea to GPU renderer
                                renderer.set_text(&value);
                                let text = renderer.get_text();
                                let cursor_pos = renderer.get_cursor_position();
                                drop(borrowed);
                                
                                // Render the synchronized text
                                if let Err(e) = crate::fast_text_input::render_from_buffer(&text, cursor_pos) {
                                    console_log!("Render error: {:?}", e);
                                }
                            }
                        }
                        Err(e) => console_log!("Failed to get renderer: {:?}", e),
                    }
                });
            }
        }) as Box<dyn FnMut()>);
        
        textarea.set_oninput(Some(input_callback.as_ref().unchecked_ref()));
        input_callback.forget();
        
        // Handle special keys
        let keydown_callback = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
            match event.key().as_str() {
                "ArrowLeft" => {
                    event.prevent_default();
                    crate::input_buffer::move_cursor_left();
                    sync_textarea_with_gpu_renderer();
                }
                "ArrowRight" => {
                    event.prevent_default();
                    crate::input_buffer::move_cursor_right();
                    sync_textarea_with_gpu_renderer();
                }
                "Backspace" => {
                    event.prevent_default();
                    crate::input_buffer::delete_char_at_cursor();
                    sync_textarea_with_gpu_renderer();
                }
                _ => {}
            }
        }) as Box<dyn FnMut(_)>);
        
        textarea.set_onkeydown(Some(keydown_callback.as_ref().unchecked_ref()));
        keydown_callback.forget();
    }
}

fn sync_textarea_with_gpu_renderer() {
    wasm_bindgen_futures::spawn_local(async move {
        match crate::text_input::get_or_init_webgpu_resources().await {
            Ok(resources) => {
                let borrowed = resources.borrow();
                if let Some(renderer) = borrowed.fast_text_renderer.as_ref() {
                    let text = renderer.get_text();
                    let cursor_pos = renderer.get_cursor_position();
                    drop(borrowed);
                    
                    // Update textarea to match GPU renderer state
                    if let Some(window) = web_sys::window() {
                        if let Some(document) = window.document() {
                            if let Some(textarea) = document.get_element_by_id("hidden-input") {
                                let textarea: HtmlTextAreaElement = textarea.dyn_into().unwrap();
                                textarea.set_value(&text);
                                let _ = textarea.set_selection_start(Some(cursor_pos as u32));
                                let _ = textarea.set_selection_end(Some(cursor_pos as u32));
                            }
                        }
                    }
                }
            }
            Err(e) => console_log!("Failed to get renderer: {:?}", e),
        }
    });
}

#[wasm_bindgen]
pub fn focus_hidden_input() {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    
    if let Some(textarea) = document.get_element_by_id("hidden-input") {
        let textarea: HtmlTextAreaElement = textarea.dyn_into().unwrap();
        let _ = textarea.focus();
    }
}

pub fn setup_render_loop(set_render_frame: WriteSignal<u32>) {
    let callback = Rc::new(RefCell::new(None::<Closure<dyn FnMut()>>));
    let callback_clone = callback.clone();
    
    let render_fn = move || {
        set_render_frame.update(|f| *f += 1);
        
        let callback_clone = callback_clone.clone();
        let window = web_sys::window().unwrap();
        if let Some(cb) = callback_clone.borrow().as_ref() {
            let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
        }
    };
    
    *callback.borrow_mut() = Some(Closure::wrap(Box::new(render_fn) as Box<dyn FnMut()>));
    
    // Start the render loop
    let window = web_sys::window().unwrap();
    if let Some(cb) = callback.borrow().as_ref() {
        let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
    }
}

// Called from input_buffer.rs when input is committed
pub fn render_from_buffer(text: &str, cursor_pos: usize) -> Result<(), JsValue> {
    console_log!("render_from_buffer called with text: '{}' (length: {})", text, text.len());
    // Initialize WebGPU if not already done
    let text_owned = text.to_string();
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = render_fast_text(&text_owned, cursor_pos).await {
            console_log!("Fast render error: {:?}", e);
        }
    });
    
    Ok(())
}

async fn render_fast_text(text: &str, _cursor_pos: usize) -> Result<(), JsValue> {
    console_log!("render_fast_text called with text: '{}' (length: {})", text, text.len());
    
    // Get or initialize WebGPU resources
    let resources = crate::text_input::get_or_init_webgpu_resources().await?;
    
    {
        let mut res = resources.borrow_mut();
        if let Some(fast_renderer) = res.fast_text_renderer.as_mut() {
            // Update text with cursor position
            fast_renderer.update_text(text)?;
        }
    }
    
    {
        let context_clone = {
            let res = resources.borrow();
            if let Some(context) = res.context.as_ref() {
                if res.fast_text_renderer.is_some() {
                    Some(context.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };
        
        if let Some(context) = context_clone {
            let mut res = resources.borrow_mut();
            if let Some(fast_renderer) = res.fast_text_renderer.as_mut() {
                fast_renderer.render(
                    text,           // Use the actual text parameter
                    100.0,          // x
                    100.0,          // y
                    800.0,          // screen_width
                    600.0,          // screen_height
                    &context
                )?;
            }
        }
    }
    
    Ok(())
}