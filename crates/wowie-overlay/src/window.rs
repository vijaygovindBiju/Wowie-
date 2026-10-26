use crate::backend::{select_rendering_backend, GpuAccelerationPipeline};
use crate::error::OverlayError;
use crate::renderer::Renderer3D;
use crate::state::{OverlayConfig, OverlayControlEvent};
use std::num::NonZeroU32;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};
use wowie_avatar::model::{CompanionMood, Model3D};
use wowie_avatar::CharacterRegistry;

pub struct DesktopOverlay {
    config: OverlayConfig,
    model: Model3D,
    registry: Option<CharacterRegistry>,
    bridge_rx: Option<Receiver<OverlayControlEvent>>,
}

impl std::fmt::Debug for DesktopOverlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopOverlay")
            .field("config", &self.config)
            .field("model", &self.model)
            .field("registry", &self.registry)
            .finish_non_exhaustive()
    }
}

impl DesktopOverlay {
    pub fn new(
        config: OverlayConfig,
        model: Model3D,
        registry: Option<CharacterRegistry>,
        bridge_rx: Option<Receiver<OverlayControlEvent>>,
    ) -> Self {
        Self {
            config,
            model,
            registry,
            bridge_rx,
        }
    }

    /// Check if Wayland or X11 display server is available in the current environment
    pub fn is_display_available() -> bool {
        std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok()
    }

    /// Pre-register Hyprland window rules BEFORE creating the window so it maps borderless and transparent
    pub fn pre_register_hyprland_rules(enabled: bool) {
        if !enabled {
            return;
        }

        let is_hyprland = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
            || std::env::var("XDG_CURRENT_DESKTOP")
                .is_ok_and(|d| d.to_lowercase().contains("hyprland"));

        if is_hyprland {
            let lua_rule = r#"hl.window_rule({ name = "wowie-overlay", match = {title = ".*Wowie Companion.*"}, float = true, pin = true, no_blur = true, border_size = 0, no_shadow = true, rounding = 0, opaque = false })"#;
            let _ = std::process::Command::new("hyprctl")
                .args(["repl", lua_rule])
                .output();
        }
    }

    /// Run the overlay window event loop. Blocks until window is closed.
    pub fn run(self) -> Result<(), OverlayError> {
        if !Self::is_display_available() {
            return Err(OverlayError::DisplayServerUnavailable(
                "Neither WAYLAND_DISPLAY nor DISPLAY is set".to_string(),
            ));
        }

        // Register Hyprland rules beforehand so the compositor doesn't create borders/blur on initial map
        Self::pre_register_hyprland_rules(self.config.enable_hyprland_auto_rules);

        let event_loop = EventLoop::new().map_err(|e| {
            OverlayError::DisplayServerUnavailable(format!("Event loop initialization failed: {e}"))
        })?;

        let mut app = OverlayApp::new(self.config, self.model, self.registry, self.bridge_rx);

        event_loop
            .run_app(&mut app)
            .map_err(|e| OverlayError::Runtime(format!("Event loop execution error: {e}")))?;

        Ok(())
    }
}

struct OverlayApp {
    config: OverlayConfig,
    model: Model3D,
    registry: Option<CharacterRegistry>,
    bridge_rx: Option<Receiver<OverlayControlEvent>>,
    pub gpu_pipeline: GpuAccelerationPipeline,
    window: Option<Arc<Window>>,
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    renderer: Renderer3D,
    start_time: Instant,
    last_frame_time: Instant,
    current_mood: CompanionMood,
    status_text: String,
    is_visible: bool,
    mouse_pressed: bool,
    last_mouse_pos: (f64, f64),
    should_exit: bool,
    has_applied_post_map_rules: bool,
}

impl OverlayApp {
    fn new(
        config: OverlayConfig,
        model: Model3D,
        registry: Option<CharacterRegistry>,
        bridge_rx: Option<Receiver<OverlayControlEvent>>,
    ) -> Self {
        let w = config.width as usize;
        let h = config.height as usize;
        let backend = select_rendering_backend(config.prefer_gpu);
        let gpu_pipeline =
            GpuAccelerationPipeline::with_backend(backend, config.width, config.height);
        Self {
            config,
            model,
            registry,
            bridge_rx,
            gpu_pipeline,
            window: None,
            context: None,
            surface: None,
            renderer: Renderer3D::new(w, h),
            start_time: Instant::now(),
            last_frame_time: Instant::now(),
            current_mood: CompanionMood::Neutral,
            status_text: "Ready".to_string(),
            is_visible: true,
            mouse_pressed: false,
            last_mouse_pos: (0.0, 0.0),
            should_exit: false,
            has_applied_post_map_rules: false,
        }
    }

    fn apply_hyprland_rules(&self) {
        if !self.config.enable_hyprland_auto_rules {
            return;
        }

        let is_hyprland = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
            || std::env::var("XDG_CURRENT_DESKTOP")
                .is_ok_and(|d| d.to_lowercase().contains("hyprland"));

        if is_hyprland {
            // Modern Hyprland (0.56+) with Lua configuration
            let lua_rule = r#"hl.window_rule({ name = "wowie-overlay", match = {title = ".*Wowie Companion.*"}, float = true, pin = true, no_blur = true, border_size = 0, no_shadow = true, rounding = 0, opaque = false })"#;
            let _ = std::process::Command::new("hyprctl")
                .args(["repl", lua_rule])
                .output();

            // Explicit dynamic property dispatch to ensure compositor strips borders, blur, and shadows
            for (prop, val) in [
                ("noblur", "1"),
                ("noborder", "1"),
                ("noshadow", "1"),
                ("opaque", "0"),
                ("rounding", "0"),
            ] {
                let cmd = format!(
                    r#"return hl.dispatch(hl.dsp.window.set_prop({{ match = {{ title = ".*Wowie Companion.*" }}, prop = "{prop}", value = {val} }}))"#
                );
                let _ = std::process::Command::new("hyprctl")
                    .args(["repl", &cmd])
                    .output();
            }
        }
    }

    fn cycle_mood(&mut self) {
        self.current_mood = match self.current_mood {
            CompanionMood::Neutral => CompanionMood::Focused,
            CompanionMood::Focused => CompanionMood::Concerned,
            CompanionMood::Concerned => CompanionMood::Relaxed,
            CompanionMood::Relaxed => CompanionMood::Snoozed,
            CompanionMood::Snoozed => CompanionMood::Celebrating,
            CompanionMood::Celebrating => CompanionMood::Thinking,
            CompanionMood::Thinking => CompanionMood::Neutral,
        };
        self.status_text = format!("{:?}", self.current_mood);
    }

    fn cycle_character(&mut self) {
        if let Some(reg) = &self.registry {
            let chars = reg.list_characters();
            if chars.is_empty() {
                return;
            }

            let active_id = reg.get_active_character().map(|c| c.id);
            let next_char = if let Some(aid) = active_id {
                let pos = chars.iter().position(|c| c.id == aid).unwrap_or(0);
                &chars[(pos + 1) % chars.len()]
            } else {
                &chars[0]
            };

            let _ = reg.set_active_character(next_char.id);
            if let Ok(m) = reg.load_character_model(next_char.id) {
                self.model = m;
            }
        }
    }

    fn process_control_messages(&mut self) {
        if let Some(rx) = &self.bridge_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    OverlayControlEvent::SetMood(m) => {
                        self.current_mood = m;
                        self.status_text = format!("{m:?}");
                    }
                    OverlayControlEvent::SetModel(boxed_model) => {
                        self.model = *boxed_model;
                    }
                    OverlayControlEvent::SetStatus(s) => {
                        self.status_text = s;
                    }
                    OverlayControlEvent::ToggleVisibility => {
                        self.is_visible = !self.is_visible;
                    }
                    OverlayControlEvent::Quit => {
                        self.should_exit = true;
                    }
                }
            }
        }
    }

    fn render_frame(&mut self) {
        self.process_control_messages();

        let Some(_window) = &self.window else { return };
        let Some(surface) = &mut self.surface else {
            return;
        };

        if !self.is_visible {
            // Clear surface to fully transparent when hidden
            if let Ok(mut buffer) = surface.buffer_mut() {
                buffer.fill(0x0000_0000);
                let _ = buffer.present();
            }
            return;
        }

        let elapsed = self.start_time.elapsed().as_secs_f32();

        // 1. Render 3D model
        self.renderer
            .render_model(&self.model, self.current_mood, elapsed);

        // 2. Render HUD glass pill if enabled
        if self.config.show_hud {
            self.renderer.draw_hud_pill(
                &self.model.metadata.title,
                self.current_mood,
                &self.status_text,
            );
        }

        // 3. Blit to softbuffer window surface
        if let Ok(mut buffer) = surface.buffer_mut() {
            let len = buffer.len().min(self.renderer.color_buffer.len());
            buffer[..len].copy_from_slice(&self.renderer.color_buffer[..len]);
            let _ = buffer.present();
        }

        if !self.has_applied_post_map_rules {
            self.has_applied_post_map_rules = true;
            self.apply_hyprland_rules();
        }
    }
}

impl ApplicationHandler for OverlayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut window_attributes = WindowAttributes::default()
            .with_title("Wowie Companion Overlay")
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height))
            .with_transparent(true)
            .with_decorations(false);

        #[cfg(target_os = "linux")]
        {
            use winit::platform::wayland::WindowAttributesExtWayland;
            window_attributes = window_attributes.with_name("wowie-overlay", "wowie-overlay");
        }

        let window = match event_loop.create_window(window_attributes) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("Failed to create winit overlay window: {e}");
                event_loop.exit();
                return;
            }
        };

        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to create softbuffer context: {e}");
                event_loop.exit();
                return;
            }
        };

        let mut surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to create softbuffer surface: {e}");
                event_loop.exit();
                return;
            }
        };

        let size = window.inner_size();
        let width = NonZeroU32::new(size.width.max(1)).unwrap();
        let height = NonZeroU32::new(size.height.max(1)).unwrap();
        let _ = surface.resize(width, height);

        self.renderer
            .resize(size.width as usize, size.height as usize);
        self.gpu_pipeline.resize(size.width, size.height);
        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);

        self.apply_hyprland_rules();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.should_exit {
            event_loop.exit();
            return;
        }

        self.process_control_messages();

        let now = Instant::now();
        let target_interval = if self.is_visible {
            std::time::Duration::from_millis(u64::from(1000 / self.config.fps_limit.max(1)))
        } else {
            // When hidden, sleep heavily to conserve CPU
            std::time::Duration::from_millis(200)
        };

        let elapsed = now.duration_since(self.last_frame_time);
        if let Some(remaining) = target_interval.checked_sub(elapsed) {
            std::thread::sleep(remaining);
        }
        self.last_frame_time = Instant::now();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.should_exit {
            event_loop.exit();
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) = (
                    NonZeroU32::new(size.width.max(1)),
                    NonZeroU32::new(size.height.max(1)),
                ) {
                    if let Some(surface) = &mut self.surface {
                        let _ = surface.resize(w, h);
                    }
                    self.renderer
                        .resize(size.width as usize, size.height as usize);
                    self.gpu_pipeline.resize(size.width, size.height);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                self.render_frame();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                match logical_key {
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    Key::Character(c) if c.eq_ignore_ascii_case("q") => event_loop.exit(),
                    Key::Character(c) if c.eq_ignore_ascii_case("h") => {
                        self.is_visible = !self.is_visible;
                    }
                    Key::Character(c) if c.eq_ignore_ascii_case("u") => {
                        self.config.show_hud = !self.config.show_hud;
                    }
                    Key::Character(c) if c.eq_ignore_ascii_case("m") => {
                        self.cycle_mood();
                    }
                    Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                        self.cycle_character();
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.renderer.camera.yaw -= 0.15;
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        self.renderer.camera.yaw += 0.15;
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.renderer.camera.pitch = (self.renderer.camera.pitch + 0.1).min(1.2);
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.renderer.camera.pitch = (self.renderer.camera.pitch - 0.1).max(-1.2);
                    }
                    _ => {}
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                self.mouse_pressed = state == ElementState::Pressed;
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.mouse_pressed {
                    let dx = position.x - self.last_mouse_pos.0;
                    let dy = position.y - self.last_mouse_pos.1;
                    self.renderer.camera.yaw += (dx * 0.01) as f32;
                    self.renderer.camera.pitch =
                        (self.renderer.camera.pitch - (dy * 0.01) as f32).clamp(-1.2, 1.2);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                self.last_mouse_pos = (position.x, position.y);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y * 0.05) as f32,
                };
                self.renderer.camera.distance =
                    (self.renderer.camera.distance - scroll_y * 0.3).clamp(1.5, 10.0);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}
