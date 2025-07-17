use wasm_bindgen::prelude::*;

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

// High-performance input operations
#[wasm_bindgen]
pub fn insert_char_at_cursor(char_code: u32) {
    if let Some(ch) = char::from_u32(char_code) {
        console_log!("insert_char_at_cursor: '{}'", ch);
        
        wasm_bindgen_futures::spawn_local(async move {
            match get_renderer().await {
                Ok(resources) => {
                    let mut borrowed = resources.borrow_mut();
                    if let Some(renderer) = borrowed.fast_text_renderer.as_mut() {
                        if let Err(e) = renderer.insert_char(ch) {
                            console_log!("Error inserting char: {:?}", e);
                        } else {
                            // Trigger render with updated text
                            let text = renderer.get_text();
                            let cursor_pos = renderer.get_cursor_position();
                            drop(borrowed); // Drop the borrow before calling render_from_buffer
                            if let Err(e) = crate::fast_text_input::render_from_buffer(&text, cursor_pos) {
                                console_log!("Render error: {:?}", e);
                            }
                        }
                    } else {
                        console_log!("Fast text renderer not initialized");
                    }
                }
                Err(e) => console_log!("Failed to get renderer: {:?}", e),
            }
        });
    }
}

#[wasm_bindgen]
pub fn delete_char_at_cursor() {
    console_log!("delete_char_at_cursor");
    
    wasm_bindgen_futures::spawn_local(async move {
        match get_renderer().await {
            Ok(resources) => {
                let mut borrowed = resources.borrow_mut();
                if let Some(renderer) = borrowed.fast_text_renderer.as_mut() {
                    if let Err(e) = renderer.delete_char_before_cursor() {
                        console_log!("Error deleting char: {:?}", e);
                    } else {
                        // Trigger render with updated text
                        let text = renderer.get_text();
                        let cursor_pos = renderer.get_cursor_position();
                        drop(borrowed); // Drop the borrow before calling render_from_buffer
                        if let Err(e) = crate::fast_text_input::render_from_buffer(&text, cursor_pos) {
                            console_log!("Render error: {:?}", e);
                        }
                    }
                } else {
                    console_log!("Fast text renderer not initialized");
                }
            }
            Err(e) => console_log!("Failed to get renderer: {:?}", e),
        }
    });
}

#[wasm_bindgen]
pub fn move_cursor_left() {
    console_log!("move_cursor_left");
    
    wasm_bindgen_futures::spawn_local(async move {
        match get_renderer().await {
            Ok(resources) => {
                let mut borrowed = resources.borrow_mut();
                if let Some(renderer) = borrowed.fast_text_renderer.as_mut() {
                    renderer.move_cursor_left();
                    // Trigger render with updated cursor position
                    let text = renderer.get_text();
                    let cursor_pos = renderer.get_cursor_position();
                    drop(borrowed); // Drop the borrow before calling render_from_buffer
                    if let Err(e) = crate::fast_text_input::render_from_buffer(&text, cursor_pos) {
                        console_log!("Render error: {:?}", e);
                    }
                } else {
                    console_log!("Fast text renderer not initialized");
                }
            }
            Err(e) => console_log!("Failed to get renderer: {:?}", e),
        }
    });
}

#[wasm_bindgen]
pub fn move_cursor_right() {
    console_log!("move_cursor_right");
    
    wasm_bindgen_futures::spawn_local(async move {
        match get_renderer().await {
            Ok(resources) => {
                let mut borrowed = resources.borrow_mut();
                if let Some(renderer) = borrowed.fast_text_renderer.as_mut() {
                    renderer.move_cursor_right();
                    // Trigger render with updated cursor position
                    let text = renderer.get_text();
                    let cursor_pos = renderer.get_cursor_position();
                    drop(borrowed); // Drop the borrow before calling render_from_buffer
                    if let Err(e) = crate::fast_text_input::render_from_buffer(&text, cursor_pos) {
                        console_log!("Render error: {:?}", e);
                    }
                } else {
                    console_log!("Fast text renderer not initialized");
                }
            }
            Err(e) => console_log!("Failed to get renderer: {:?}", e),
        }
    });
}
// Legacy compatibility functions for fast_text_input.rs
static mut TEMP_BUFFER: [u8; 1024] = [0; 1024];

pub fn get_input_buffer_ptr() -> *mut u8 {
    unsafe { std::ptr::addr_of_mut!(TEMP_BUFFER).cast::<u8>() }
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
                    Err(e) => console_log!("Failed to get renderer: {:?}", e),
                }
            });
        }
    }
}