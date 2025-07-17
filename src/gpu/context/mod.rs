// GPU Context Management
// Handles WebGPU initialization, adapter/device creation, and canvas context configuration
// Main responsibilities:
// - Request and configure GPU adapter
// - Create and manage GPU device
// - Configure canvas rendering context
// - Manage GPU resources lifecycle

use wasm_bindgen::prelude::*;
use web_sys::{GpuAdapter, GpuDevice, GpuCanvasContext, HtmlCanvasElement, gpu_texture_usage};

pub struct GpuContext {
    pub adapter: GpuAdapter,
    pub device: GpuDevice,
    pub context: GpuCanvasContext,
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
        
        // Configure canvas context
        let mut config = web_sys::GpuCanvasConfiguration::new(&device, web_sys::GpuTextureFormat::Bgra8unorm);
        config.usage(gpu_texture_usage::RENDER_ATTACHMENT);
        context.configure(&config);
        
        Ok(Self {
            adapter,
            device,
            context,
        })
    }
    
    pub fn get_current_texture_view(&self) -> Result<web_sys::GpuTextureView, JsValue> {
        let current_texture = self.context.get_current_texture()?;
        current_texture.create_view()
      }
}