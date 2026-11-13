use crate::gpu::Gpu;
use crate::render::Globals;
use std::sync::Arc;
use std::time::Instant;
use tether_core::camera::{Camera, FRAME};
use tether_core::color::Palette;
use tether_core::mesh::Mesh;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub windowed: bool,
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut options = Self::default();
        for arg in args {
            if arg == "--windowed" {
                options.windowed = true;
            }
        }
        options
    }
}

pub fn is_fullscreen_toggle(key: &Key, mods: ModifiersState) -> bool {
    match key {
        Key::Named(NamedKey::F11) => true,
        Key::Named(NamedKey::Enter) => mods.alt_key(),
        Key::Character(c) => c.eq_ignore_ascii_case("f") && mods.control_key() && mods.super_key(),
        _ => false,
    }
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
        assert_eq!(Options::parse(args(&[])), Options { windowed: false });
    }

    #[test]
    fn options_read_windowed_flag() {
        assert_eq!(
            Options::parse(args(&["--windowed"])),
            Options { windowed: true }
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
