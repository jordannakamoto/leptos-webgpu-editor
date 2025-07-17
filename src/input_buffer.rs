use wasm_bindgen::prelude::*;

// Shared memory buffer for zero-copy input transfer
static mut INPUT_BUFFER: [u8; 1024] = [0; 1024];
static mut CURSOR_POSITION: usize = 0;
static mut TEXT_LENGTH: usize = 0;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub fn get_input_buffer_ptr() -> *mut u8 {
    unsafe { 
        std::ptr::addr_of_mut!(INPUT_BUFFER) as *mut u8
    }
}

#[wasm_bindgen]
pub fn get_cursor_position() -> usize {
    unsafe { CURSOR_POSITION }
}

#[wasm_bindgen]
pub fn set_cursor_position(pos: usize) {
    unsafe { CURSOR_POSITION = pos }
}

#[wasm_bindgen]
pub fn get_text_length() -> usize {
    unsafe { TEXT_LENGTH }
}

#[wasm_bindgen]
pub fn commit_input(length: usize, cursor_pos: usize) {
    unsafe {
        TEXT_LENGTH = length;
        CURSOR_POSITION = cursor_pos;
        
        // Convert to UTF-8 string
        let text_bytes = &INPUT_BUFFER[..length];
        if let Ok(text) = std::str::from_utf8(text_bytes) {
            // Trigger render with new text
            if let Err(e) = crate::fast_text_input::render_from_buffer(text, cursor_pos) {
                console_log!("Render error: {:?}", e);
            }
        }
    }
}

#[wasm_bindgen]
pub fn get_current_text() -> String {
    unsafe {
        let text_bytes = &INPUT_BUFFER[..TEXT_LENGTH];
        std::str::from_utf8(text_bytes).unwrap_or("").to_string()
    }
}

// High-performance input operations
#[wasm_bindgen]
pub fn insert_char_at_cursor(char_code: u32) {
    unsafe {
        if let Some(ch) = char::from_u32(char_code) {
            let mut text = get_current_text();
            console_log!("insert_char_at_cursor: '{}' into '{}' at position {}", ch, text, CURSOR_POSITION as usize);
            if CURSOR_POSITION <= text.len() {
                text.insert(CURSOR_POSITION, ch);
                console_log!("Result text: '{}'", text);
                
                // Write back to buffer
                let bytes = text.as_bytes();
                if bytes.len() <= 1024 {
                    INPUT_BUFFER[..bytes.len()].copy_from_slice(bytes);
                    TEXT_LENGTH = bytes.len();
                    CURSOR_POSITION += ch.len_utf8();
                    
                    // Trigger render with updated text
                    if let Err(e) = crate::fast_text_input::render_from_buffer(&text, CURSOR_POSITION) {
                        console_log!("Render error: {:?}", e);
                    }
                }
            }
        }
    }
}

#[wasm_bindgen]
pub fn delete_char_at_cursor() {
    unsafe {
        let mut text = get_current_text();
        console_log!("delete_char_at_cursor: '{}' at position {}", text, CURSOR_POSITION as usize);
        if CURSOR_POSITION > 0 && CURSOR_POSITION <= text.len() {
            // Find the previous character boundary
            let mut char_start = CURSOR_POSITION;
            while char_start > 0 && !text.is_char_boundary(char_start) {
                char_start -= 1;
            }
            
            if char_start > 0 {
                char_start -= 1;
                while char_start > 0 && !text.is_char_boundary(char_start) {
                    char_start -= 1;
                }
                
                text.remove(char_start);
                
                // Write back to buffer
                let bytes = text.as_bytes();
                if bytes.len() <= 1024 {
                    INPUT_BUFFER[..bytes.len()].copy_from_slice(bytes);
                    TEXT_LENGTH = bytes.len();
                    CURSOR_POSITION = char_start;
                    
                    // Trigger render with updated text
                    if let Err(e) = crate::fast_text_input::render_from_buffer(&text, CURSOR_POSITION) {
                        console_log!("Render error: {:?}", e);
                    }
                }
            }
        }
    }
}

#[wasm_bindgen]
pub fn move_cursor_left() {
    unsafe {
        if CURSOR_POSITION > 0 {
            let text = get_current_text();
            let mut new_pos = CURSOR_POSITION - 1;
            while new_pos > 0 && !text.is_char_boundary(new_pos) {
                new_pos -= 1;
            }
            CURSOR_POSITION = new_pos;
        }
    }
}

#[wasm_bindgen]
pub fn move_cursor_right() {
    unsafe {
        let text = get_current_text();
        if CURSOR_POSITION < text.len() {
            let mut new_pos = CURSOR_POSITION + 1;
            while new_pos < text.len() && !text.is_char_boundary(new_pos) {
                new_pos += 1;
            }
            CURSOR_POSITION = new_pos;
        }
    }
}