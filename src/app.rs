use crate::gpu::Gpu;
use crate::render::Globals;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tether_core::camera::{Camera, FRAME};
use tether_core::color::Palette;
use tether_core::math::Vec2;
use tether_core::mesh::Mesh;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub windowed: bool,
    pub capture: Option<Capture>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Capture {
    pub path: PathBuf,
    pub at: f32,
    pub width: u32,
    pub height: u32,
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut windowed = false;
        let mut path = None;
        let mut at = 0.0;
        let mut size = (1920, 1080);
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--windowed" => windowed = true,
                "--capture" => path = args.next().map(PathBuf::from),
                "--at" => {
                    if let Some(value) = args.next().and_then(|s| s.parse().ok()) {
                        at = value;
                    }
                }
                "--size" => {
                    if let Some(value) = args.next().and_then(|s| parse_size(&s)) {
                        size = value;
                    }
                }
                _ => {}
            }
        }
        let capture = path.map(|path| Capture {
            path,
            at,
            width: size.0,
            height: size.1,
        });
        Self { windowed, capture }
    }
}

fn parse_size(text: &str) -> Option<(u32, u32)> {
    let (w, h) = text.split_once('x')?;
    let (w, h): (u32, u32) = (w.parse().ok()?, h.parse().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

pub fn is_fullscreen_toggle(key: &Key, mods: ModifiersState) -> bool {
    match key {
        Key::Named(NamedKey::F11) => true,
        Key::Named(NamedKey::Enter) => mods.alt_key(),
        Key::Character(c) => c.eq_ignore_ascii_case("f") && mods.control_key() && mods.super_key(),
        _ => false,
    }
}

pub fn capture(capture: &Capture) -> Result<(), String> {
    let size = Vec2::new(capture.width as f32, capture.height as f32);
    let camera = Camera::fit(FRAME, size);
    let palette = Palette::paper();
    let mesh = Mesh::new(camera.pixel_size());
    let globals = Globals::new(&camera, palette.ground, capture.at);
    let rgba = pollster::block_on(crate::gpu::capture(
        capture.width,
        capture.height,
        &globals,
        &mesh,
    ));
    let png = tether_core::png::encode_rgba(capture.width, capture.height, &rgba);
    std::fs::write(&capture.path, png).map_err(|e| e.to_string())
}

pub fn run(options: Options) -> Result<(), winit::error::EventLoopError> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        display: event_loop.owned_display_handle(),
        options,
        modifiers: ModifiersState::empty(),
        running: None,
    };
    event_loop.run_app(&mut app)
}

struct App {
    display: OwnedDisplayHandle,
    options: Options,
    modifiers: ModifiersState,
    running: Option<Running>,
}

struct Running {
    window: Arc<Window>,
    gpu: Gpu,
    mesh: Mesh,
    palette: Palette,
    start: Instant,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        let mut attributes = Window::default_attributes()
            .with_title("Tether")
            .with_inner_size(LogicalSize::new(1600.0, 900.0))
            .with_min_inner_size(LogicalSize::new(480.0, 270.0));
        if !self.options.windowed {
            attributes = attributes.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let window = Arc::new(event_loop.create_window(attributes).expect("create window"));
        let gpu = pollster::block_on(Gpu::new(window.clone(), self.display.clone()));
        window.request_redraw();
        self.running = Some(Running {
            window,
            gpu,
            mesh: Mesh::default(),
            palette: Palette::paper(),
            start: Instant::now(),
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(running) = self.running.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => running.gpu.resize(size.width, size.height),
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                if is_fullscreen_toggle(&event.logical_key, self.modifiers) {
                    running.toggle_fullscreen();
                }
            }
            WindowEvent::RedrawRequested => {
                running.frame();
                running.window.request_redraw();
            }
            _ => {}
        }
    }
}

impl Running {
    fn toggle_fullscreen(&self) {
        let next = match self.window.fullscreen() {
            Some(_) => None,
            None => Some(Fullscreen::Borderless(None)),
        };
        self.window.set_fullscreen(next);
    }

    fn frame(&mut self) {
        let camera = Camera::fit(FRAME, self.gpu.size());
        self.mesh.feather = camera.pixel_size();
        self.mesh.clear();
        let time = self.start.elapsed().as_secs_f32();
        let globals = Globals::new(&camera, self.palette.ground, time);
        self.gpu.render(&globals, &self.mesh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::SmolStr;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn options_default_to_fullscreen() {
        assert_eq!(Options::parse(args(&[])), Options::default());
        assert!(!Options::default().windowed);
    }

    #[test]
    fn options_read_windowed_flag() {
        assert!(Options::parse(args(&["--windowed"])).windowed);
    }

    #[test]
    fn options_read_capture_with_defaults() {
        let capture = Options::parse(args(&["--capture", "out.png"])).capture;
        assert_eq!(
            capture,
            Some(Capture {
                path: PathBuf::from("out.png"),
                at: 0.0,
                width: 1920,
                height: 1080,
            })
        );
    }

    #[test]
    fn options_read_capture_time_and_size() {
        let options = Options::parse(args(&[
            "--size",
            "800x600",
            "--capture",
            "a.png",
            "--at",
            "2.5",
        ]));
        let capture = options.capture.expect("capture");
        assert_eq!((capture.at, capture.width, capture.height), (2.5, 800, 600));
    }

    #[test]
    fn options_ignore_capture_settings_without_a_path() {
        assert_eq!(
            Options::parse(args(&["--at", "3", "--size", "10x10"])).capture,
            None
        );
    }

    #[test]
    fn options_keep_defaults_for_bad_values() {
        let options = Options::parse(args(&["--capture", "a.png", "--at", "x", "--size", "9"]));
        let capture = options.capture.expect("capture");
        assert_eq!(
            (capture.at, capture.width, capture.height),
            (0.0, 1920, 1080)
        );
    }

    #[test]
    fn f11_toggles_fullscreen() {
        assert!(is_fullscreen_toggle(
            &Key::Named(NamedKey::F11),
            ModifiersState::empty()
        ));
    }

    #[test]
    fn alt_enter_toggles_fullscreen() {
        let enter = Key::Named(NamedKey::Enter);
        assert!(is_fullscreen_toggle(&enter, ModifiersState::ALT));
        assert!(!is_fullscreen_toggle(&enter, ModifiersState::empty()));
    }

    #[test]
    fn control_command_f_toggles_fullscreen() {
        let f = Key::Character(SmolStr::new("f"));
        assert!(is_fullscreen_toggle(
            &f,
            ModifiersState::CONTROL | ModifiersState::SUPER
        ));
        assert!(!is_fullscreen_toggle(&f, ModifiersState::CONTROL));
        assert!(!is_fullscreen_toggle(&f, ModifiersState::empty()));
    }

    #[test]
    fn other_keys_do_not_toggle_fullscreen() {
        let a = Key::Character(SmolStr::new("a"));
        assert!(!is_fullscreen_toggle(&a, ModifiersState::ALT));
        assert!(!is_fullscreen_toggle(
            &Key::Named(NamedKey::Escape),
            ModifiersState::empty()
        ));
    }
}
