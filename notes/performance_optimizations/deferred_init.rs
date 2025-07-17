use wasm_bindgen::prelude::*;
// Remove unused imports
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

pub struct DeferredInitializer {
    tasks: Vec<Box<dyn FnOnce() -> Result<(), JsValue>>>,
    idle_deadline: Option<f64>,
}

impl DeferredInitializer {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            idle_deadline: None,
        }
    }

    pub fn add_task<F>(&mut self, task: F) 
    where 
        F: FnOnce() -> Result<(), JsValue> + 'static 
    {
        self.tasks.push(Box::new(task));
    }

    pub fn run_deferred(&mut self) {
        let window = web_sys::window().unwrap();
        
        // Use requestIdleCallback if available, otherwise use setTimeout
        if js_sys::Reflect::has(&window, &"requestIdleCallback".into()).unwrap_or(false) {
            self.run_with_idle_callback();
        } else {
            self.run_with_timeout();
        }
    }

    fn run_with_idle_callback(&mut self) {
        let window = web_sys::window().unwrap();
        let tasks = std::mem::take(&mut self.tasks);
        
        let callback = Closure::wrap(Box::new(move |deadline: JsValue| {
            let deadline_obj = deadline.dyn_into::<js_sys::Object>().unwrap();
            let time_remaining = js_sys::Reflect::get(&deadline_obj, &"timeRemaining".into())
                .unwrap()
                .dyn_into::<js_sys::Function>()
                .unwrap();
            
            let mut remaining_tasks = tasks;
            
            while !remaining_tasks.is_empty() {
                let time_left: f64 = time_remaining.call0(&deadline_obj)
                    .unwrap()
                    .as_f64()
                    .unwrap_or(0.0);
                
                if time_left <= 1.0 {
                    // Not enough time, schedule for next idle period
                    let next_callback = Closure::wrap(Box::new(move |_: JsValue| {
                        for task in remaining_tasks {
                            if let Err(e) = task() {
                                console_log!("Deferred task failed: {:?}", e);
                            }
                        }
                    }) as Box<dyn FnMut(_)>);
                    
                    schedule_idle_callback(&next_callback);
                    next_callback.forget();
                    break;
                }
                
                if let Some(task) = remaining_tasks.pop() {
                    if let Err(e) = task() {
                        console_log!("Deferred task failed: {:?}", e);
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);
        
        schedule_idle_callback(&callback);
        callback.forget();
    }

    fn run_with_timeout(&mut self) {
        let window = web_sys::window().unwrap();
        let tasks = std::mem::take(&mut self.tasks);
        
        let callback = Closure::wrap(Box::new(move || {
            console_log!("Running {} deferred tasks", tasks.len());
            for task in tasks {
                if let Err(e) = task() {
                    console_log!("Deferred task failed: {:?}", e);
                }
            }
        }) as Box<dyn FnMut()>);
        
        window.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            16 // ~1 frame delay
        ).unwrap();
        
        callback.forget();
    }
}

fn schedule_idle_callback(callback: &Closure<dyn FnMut(JsValue)>) {
    let window = web_sys::window().unwrap();
    let request_idle_callback = js_sys::Reflect::get(&window, &"requestIdleCallback".into())
        .unwrap()
        .dyn_into::<js_sys::Function>()
        .unwrap();
    
    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &"timeout".into(), &5000.into()).unwrap(); // 5 second timeout
    
    request_idle_callback.call2(&window, callback.as_ref().unchecked_ref(), &options).unwrap();
}

// Global deferred initializer
thread_local! {
    static DEFERRED_INIT: RefCell<Option<DeferredInitializer>> = RefCell::new(None);
}

pub fn get_deferred_initializer() -> Rc<RefCell<DeferredInitializer>> {
    DEFERRED_INIT.with(|init| {
        if init.borrow().is_none() {
            *init.borrow_mut() = Some(DeferredInitializer::new());
        }
        // Note: This is a simplified version - in practice you'd want proper reference counting
        Rc::new(RefCell::new(DeferredInitializer::new()))
    })
}

pub fn defer_webgpu_init() {
    console_log!("Deferring WebGPU initialization");
    
    let initializer = get_deferred_initializer();
    
    initializer.borrow_mut().add_task(Box::new(|| {
        console_log!("Initializing WebGPU resources in background");
        
        // Initialize WebGPU context
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = crate::text_input::get_or_init_webgpu_resources().await {
                console_log!("Failed to initialize WebGPU: {:?}", e);
            } else {
                console_log!("WebGPU initialized successfully in background");
            }
        });
        
        Ok(())
    }));
    
    initializer.borrow_mut().add_task(Box::new(|| {
        console_log!("Preloading font atlas");
        
        // Preload font atlas
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = crate::cache_manager::get_cache_manager().await {
                console_log!("Failed to initialize cache manager: {:?}", e);
            } else {
                console_log!("Cache manager initialized");
            }
        });
        
        Ok(())
    }));
    
    initializer.borrow_mut().run_deferred();
}

pub fn defer_heavy_operations() {
    console_log!("Deferring heavy operations");
    
    let initializer = get_deferred_initializer();
    
    initializer.borrow_mut().add_task(Box::new(|| {
        console_log!("Compiling shaders in background");
        // Shader compilation would go here
        Ok(())
    }));
    
    initializer.borrow_mut().add_task(Box::new(|| {
        console_log!("Preparing vertex buffers");
        // Buffer preparation would go here
        Ok(())
    }));
    
    initializer.borrow_mut().run_deferred();
}