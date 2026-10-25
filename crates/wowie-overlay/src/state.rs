use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use wowie_avatar::model::{CompanionMood, Model3D};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    pub width: u32,
    pub height: u32,
    pub fps_limit: u32,
    pub always_on_top: bool,
    pub enable_hyprland_auto_rules: bool,
    pub show_hud: bool,
    #[serde(default)]
    pub prefer_gpu: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            width: 320,
            height: 380,
            fps_limit: 30,
            always_on_top: true,
            enable_hyprland_auto_rules: true,
            show_hud: true,
            prefer_gpu: false,
        }
    }
}

#[derive(Debug)]
pub enum OverlayControlEvent {
    SetMood(CompanionMood),
    SetModel(Box<Model3D>),
    SetStatus(String),
    ToggleVisibility,
    Quit,
}

#[derive(Debug, Clone)]
pub struct OverlayBridge {
    sender: Sender<OverlayControlEvent>,
    current_mood: Arc<Mutex<CompanionMood>>,
}

impl OverlayBridge {
    pub fn new() -> (Self, Receiver<OverlayControlEvent>) {
        let (tx, rx) = channel();
        (
            Self {
                sender: tx,
                current_mood: Arc::new(Mutex::new(CompanionMood::Neutral)),
            },
            rx,
        )
    }

    pub fn set_mood(&self, mood: CompanionMood) {
        *self.current_mood.lock().unwrap() = mood;
        let _ = self.sender.send(OverlayControlEvent::SetMood(mood));
    }

    pub fn set_model(&self, model: Model3D) {
        let _ = self
            .sender
            .send(OverlayControlEvent::SetModel(Box::new(model)));
    }

    pub fn set_status(&self, status: impl Into<String>) {
        let _ = self
            .sender
            .send(OverlayControlEvent::SetStatus(status.into()));
    }

    pub fn toggle_visibility(&self) {
        let _ = self.sender.send(OverlayControlEvent::ToggleVisibility);
    }

    pub fn quit(&self) {
        let _ = self.sender.send(OverlayControlEvent::Quit);
    }

    pub fn current_mood(&self) -> CompanionMood {
        *self.current_mood.lock().unwrap()
    }
}
