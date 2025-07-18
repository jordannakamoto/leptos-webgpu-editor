// GPU Context Management
// Handles WebGPU initialization, adapter/device creation, and canvas context configuration
// Main responsibilities:
// - Request and configure GPU adapter
// - Create and manage GPU device
// - Configure canvas rendering context
// - Manage GPU resources lifecycle

use wasm_bindgen::prelude::*;
use web_sys::{GpuAdapter, GpuDevice, GpuCanvasContext, HtmlCanvasElement, gpu_texture_usage};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[derive(Clone)]
pub struct GpuContext {
    pub adapter: GpuAdapter,
    pub device: GpuDevice,
    pub context: GpuCanvasContext,
    pub canvas: HtmlCanvasElement,
    pub offscreen_texture: web_sys::GpuTexture,
    pub offscreen_view: web_sys::GpuTextureView,
    pub queue: web_sys::GpuQueue,
    pub texture_view: web_sys::GpuTextureView,
}

impl GpuContext {
    pub async fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let window = web_sys::window().unwrap();
        let navigator = window.navigator();
        let gpu = navigator.gpu();
        
        // Request adapter
        let adapter_promise = gpu.request_adapter();
        let adapter = wasm_bindgen_futures::JsFuture::from(adapter_promise).await?;
        let adapter: GpuAdapter = adapter.into();
        
        // Request device
        let device_promise = adapter.request_device();
        let device = wasm_bindgen_futures::JsFuture::from(device_promise).await?;
        let device: GpuDevice = device.into();
        
        // Get canvas context
        let context = canvas.get_context("webgpu")?.unwrap();
        let context: GpuCanvasContext = context.dyn_into()?;
        
        // Configure canvas context with copy destination for double buffering
        let mut config = web_sys::GpuCanvasConfiguration::new(&device, web_sys::GpuTextureFormat::Bgra8unorm);
        config.set_usage(gpu_texture_usage::RENDER_ATTACHMENT | gpu_texture_usage::COPY_DST);
        config.set_alpha_mode(web_sys::GpuCanvasAlphaMode::Opaque);
        context.configure(&config);
        
        // Create persistent offscreen texture for double buffering
        let offscreen_texture = device.create_texture(&{
            let mut desc = web_sys::GpuTextureDescriptor::new(
                web_sys::GpuTextureFormat::Bgra8unorm,
                &{
                    let mut extent = web_sys::GpuExtent3dDict::new(canvas.width());
                    extent.set_height(canvas.height());
                    extent.set_depth_or_array_layers(1);
                    extent.into()
                },
                gpu_texture_usage::RENDER_ATTACHMENT | gpu_texture_usage::COPY_SRC,
            );
            desc.set_label("Offscreen Render Target");
            desc
        })?;
        
        let offscreen_view = offscreen_texture.create_view()?;
        
        let queue = device.queue();
        let texture_view = {
            let current_texture = context.get_current_texture()?;
            current_texture.create_view()?
        };
        
        Ok(Self {
            adapter,
            device,
            context,
            canvas: canvas.clone(),
            offscreen_texture,
            offscreen_view,
            queue,
            texture_view,
        })
    }
    
    pub fn get_current_texture_view(&self) -> Result<web_sys::GpuTextureView, JsValue> {
        let current_texture = self.context.get_current_texture()?;
        current_texture.create_view()
      }
}