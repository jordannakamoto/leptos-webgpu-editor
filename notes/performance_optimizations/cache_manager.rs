use wasm_bindgen::prelude::*;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CachedGlyph {
    pub atlas_x: f32,
    pub atlas_y: f32,
    pub width: f32,
    pub height: f32,
    pub sdf_width: f32,
    pub sdf_height: f32,
}

#[derive(Serialize, Deserialize)]
pub struct FontAtlasCache {
    pub atlas_data: Vec<u8>,
    pub atlas_size: u32,
    pub glyph_map: HashMap<char, CachedGlyph>,
    pub font_version: String,
}

pub struct CacheManager {
    memory_cache: HashMap<String, FontAtlasCache>,
}

impl CacheManager {
    pub fn new() -> Self {
        Self { 
            memory_cache: HashMap::new(),
        }
    }

    pub async fn init(&mut self) -> Result<(), JsValue> {
        console_log!("Initializing cache manager");
        Ok(())
    }

    pub async fn get_font_atlas(&self, font_id: &str) -> Result<Option<FontAtlasCache>, JsValue> {
        // For now, use in-memory cache
        // In a real implementation, you'd use localStorage or IndexedDB
        if let Some(cache) = self.memory_cache.get(font_id) {
            console_log!("Found cached font atlas for {}", font_id);
            Ok(Some(cache.clone()))
        } else {
            Ok(None)
        }
    }

    pub async fn store_font_atlas(&mut self, font_id: &str, cache: FontAtlasCache) -> Result<(), JsValue> {
        // Store in memory cache
        self.memory_cache.insert(font_id.to_string(), cache);
        console_log!("Stored font atlas cache for {}", font_id);
        Ok(())
    }

    pub async fn clear_cache(&mut self) -> Result<(), JsValue> {
        self.memory_cache.clear();
        console_log!("Cache cleared");
        Ok(())
    }

    pub async fn get_cache_size(&self) -> Result<usize, JsValue> {
        Ok(self.memory_cache.len())
    }
}

// Global cache manager instance
use std::cell::RefCell;
thread_local! {
    static CACHE_MANAGER: RefCell<Option<CacheManager>> = RefCell::new(None);
}

pub async fn get_cache_manager() -> Result<(), JsValue> {
    CACHE_MANAGER.with(|cache| {
        if cache.borrow().is_none() {
            let mut manager = CacheManager::new();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = manager.init().await {
                    console_log!("Failed to initialize cache: {:?}", e);
                }
                CACHE_MANAGER.with(|cache| {
                    *cache.borrow_mut() = Some(manager);
                });
            });
        }
    });
    Ok(())
}

pub async fn cache_font_atlas(font_id: &str, atlas_data: Vec<u8>, atlas_size: u32, glyph_map: HashMap<char, CachedGlyph>) -> Result<(), JsValue> {
    let cache = FontAtlasCache {
        atlas_data,
        atlas_size,
        glyph_map,
        font_version: "1.0".to_string(),
    };
    
    CACHE_MANAGER.with(|cache_manager| {
        if let Some(manager) = cache_manager.borrow().as_ref() {
            let manager_clone = manager.clone();
            let font_id_clone = font_id.to_string();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = manager_clone.store_font_atlas(&font_id_clone, &cache).await {
                    console_log!("Failed to cache font atlas: {:?}", e);
                }
            });
        }
    });
    Ok(())
}

pub async fn get_cached_font_atlas(font_id: &str) -> Result<Option<FontAtlasCache>, JsValue> {
    CACHE_MANAGER.with(|cache_manager| {
        if let Some(manager) = cache_manager.borrow().as_ref() {
            // For now, return None - in a real implementation, we'd await the async call
            // This is a simplified version due to the complexity of async in thread_local
            Ok(None)
        } else {
            Ok(None)
        }
    })
}