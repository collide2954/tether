use crate::render::{Globals, Renderer};
use std::sync::Arc;
use tether_core::math::Vec2;
use tether_core::mesh::Mesh;
use winit::event_loop::OwnedDisplayHandle;
use winit::raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle};
use winit::window::Window;

#[derive(Clone)]
struct Display(OwnedDisplayHandle);

impl std::fmt::Debug for Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Display")
    }
}

impl HasDisplayHandle for Display {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.0.display_handle()
    }
}

pub fn backends() -> wgpu::Backends {
    if cfg!(target_os = "windows") {
        wgpu::Backends::DX12 | wgpu::Backends::GL
    } else if cfg!(target_os = "macos") {
        wgpu::Backends::METAL
    } else {
        wgpu::Backends::VULKAN | wgpu::Backends::GL
    }
}

pub async fn capture(width: u32, height: u32, globals: &Globals, mesh: &Mesh) -> Vec<u8> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = backends();
    let instance = wgpu::Instance::new(descriptor.with_env());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        })
        .await
        .expect("find a GPU adapter");
    let (device, queue) = adapter
        .request_device(&Default::default())
        .await
        .expect("open the GPU device");
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut renderer = Renderer::new(&device, format);
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("capture"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let row = width * 4;
    let padded =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let view = texture.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    renderer.render(&device, &queue, &mut encoder, &view, globals, mesh);
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        extent,
    );
    queue.submit([encoder.finish()]);
    readback.map_async(wgpu::MapMode::Read, .., |result| {
        result.expect("map the readback buffer");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("wait for the GPU");
    let data = readback
        .slice(..)
        .get_mapped_range()
        .expect("read the readback buffer");
    data.chunks(padded as usize)
        .flat_map(|line| &line[..row as usize])
        .copied()
        .collect()
}

pub struct Gpu {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

impl Gpu {
    pub async fn new(window: Arc<Window>, display: OwnedDisplayHandle) -> Self {
        let mut descriptor =
            wgpu::InstanceDescriptor::new_with_display_handle(Box::new(Display(display)));
        descriptor.backends = backends();
        let instance = wgpu::Instance::new(descriptor.with_env());
        let surface = instance
            .create_surface(window.clone())
            .expect("create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("find a GPU adapter");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tether"),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .expect("open the GPU device");
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let renderer = Renderer::new(&device, format);
        Self {
            instance,
            window,
            surface,
            device,
            queue,
            config,
            renderer,
        }
    }

    pub fn size(&self) -> Vec2 {
        Vec2::new(self.config.width as f32, self.config.height as f32)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(&mut self, globals: &Globals, mesh: &Mesh) {
        let (texture, suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => (t, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => (t, true),
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(self.window.clone())
                    .expect("create surface");
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let view = texture.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.renderer.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            globals,
            mesh,
        );
        self.queue.submit([encoder.finish()]);
        self.queue.present(texture);
        let size = self.window.inner_size();
        if suboptimal && (size.width, size.height) != (self.config.width, self.config.height) {
            self.resize(size.width, size.height);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_uses_direct3d_12_with_gl_fallback() {
        assert_eq!(backends(), wgpu::Backends::DX12 | wgpu::Backends::GL);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn macos_uses_metal() {
        assert_eq!(backends(), wgpu::Backends::METAL);
    }

    #[test]
    #[cfg(all(unix, not(target_os = "macos")))]
    fn linux_uses_vulkan_with_gl_fallback() {
        assert_eq!(backends(), wgpu::Backends::VULKAN | wgpu::Backends::GL);
    }
}
