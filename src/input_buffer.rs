use wasm_bindgen::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

// Simplified input buffer - operations are sent directly to GPU renderer

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

async fn get_renderer() -> Result<std::rc::Rc<std::cell::RefCell<crate::text_input::WebGPUResources>>, JsValue> {
    crate::text_input::get_or_init_webgpu_resources().await
}

// Batching state for rapid keystrokes
thread_local! {
    static PENDING_RENDER: RefCell<bool> = RefCell::new(false);
}

// Direct synchronous renderer operation with frame-based batching
fn direct_renderer_operation<F>(operation: F) 
where 
    F: FnOnce(&mut crate::gpu::fast_text::FastTextRenderer) -> Result<(), JsValue> + 'static,
{
    wasm_bindgen_futures::spawn_local(async move {
        match get_renderer().await {
            Ok(resources) => {
                let mut borrowed = resources.borrow_mut();
                if let Some(renderer) = borrowed.fast_text_renderer.as_mut() {
                    if let Err(e) = operation(renderer) {
                        // console_log!("Renderer operation error: {:?}", e);
                    } else {
                        // Batch rapid keystrokes - only render once per animation frame
                        let text = renderer.get_text();
                        let cursor_pos = renderer.get_cursor_position();
                        drop(borrowed);
                        
                        PENDING_RENDER.with(|pending| {
                            if !*pending.borrow() {
                                *pending.borrow_mut() = true;
                                
                                // Schedule render on next animation frame
                                let window = web_sys::window().unwrap();
                                let callback = Closure::wrap(Box::new(move || {
                                    PENDING_RENDER.with(|pending| {
                                        *pending.borrow_mut() = false;
                                    });
                                    
                                    if let Err(e) = crate::fast_text_input::render_from_buffer(&text, cursor_pos) {
                                        // console_log!("Render error: {:?}", e);
                                    }
                                }) as Box<dyn FnMut()>);
                                
                                let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
                                callback.forget();
                            }
                        });
                    }
                } else {
                    // console_log!("Fast text renderer not initialized");
                }
            }
            Err(_e) => {}, // console_log!("Failed to get renderer: {:?}", _e),
        }
    });
}

// High-performance input operations
#[wasm_bindgen]
pub fn insert_char_at_cursor(char_code: u32) {
    if let Some(ch) = char::from_u32(char_code) {
        // console_log!("insert_char_at_cursor: '{}'", ch);
        
        // Direct synchronous path - no async spawning
        direct_renderer_operation(move |renderer| {
            renderer.insert_char(ch)
        });
    }
}

#[wasm_bindgen]
pub fn delete_char_at_cursor() {
    // console_log!("delete_char_at_cursor");
    
    direct_renderer_operation(|renderer| {
        renderer.delete_char_before_cursor()
    });
}

#[wasm_bindgen]
pub fn move_cursor_left() {
    // console_log!("move_cursor_left");
    
    direct_renderer_operation(|renderer| {
        renderer.move_cursor_left();
        Ok(())
    });
}

#[wasm_bindgen]
pub fn move_cursor_right() {
    // console_log!("move_cursor_right");
    
    direct_renderer_operation(|renderer| {
        renderer.move_cursor_right();
        Ok(())
    });
}
// Legacy compatibility functions for fast_text_input.rs
static mut TEMP_BUFFER: [u8; 1024] = [0; 1024];

pub fn get_input_buffer_ptr() -> *mut u8 {
    std::ptr::addr_of_mut!(TEMP_BUFFER).cast::<u8>()
}

pub fn commit_input(length: usize, _cursor_pos: usize) {
    unsafe {
        let text_bytes = &TEMP_BUFFER[..length];
        if let Ok(text) = std::str::from_utf8(text_bytes) {
            // Set the text in the GPU renderer
            wasm_bindgen_futures::spawn_local(async move {
                match get_renderer().await {
                    Ok(resources) => {
                        let mut borrowed = resources.borrow_mut();
                        if let Some(renderer) = borrowed.fast_text_renderer.as_mut() {
                            renderer.set_text(text);
                            let text = renderer.get_text();
                            let cursor_pos = renderer.get_cursor_position();
                            drop(borrowed);
                            if let Err(e) = crate::fast_text_input::render_from_buffer(&text, cursor_pos) {
                                console_log!("Render error: {:?}", e);
                            }
                        }
                    }
                    Err(_e) => {}, // console_log!("Failed to get renderer: {:?}", _e),
                }
            });
        }
    }
}