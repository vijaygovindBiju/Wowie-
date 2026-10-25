use crate::model::{CharacterProfile, CompanionEvent, CompanionMood};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct CompanionRuntime {
    active_profile: Arc<Mutex<Option<CharacterProfile>>>,
    current_mood: Arc<Mutex<CompanionMood>>,
}

impl Default for CompanionRuntime {
    fn default() -> Self {
        Self::new(None)
    }
}

impl CompanionRuntime {
    pub fn new(profile: Option<CharacterProfile>) -> Self {
        Self {
            active_profile: Arc::new(Mutex::new(profile)),
            current_mood: Arc::new(Mutex::new(CompanionMood::Neutral)),
        }
    }

    pub fn set_profile(&self, profile: Option<CharacterProfile>) {
        *self.active_profile.lock().unwrap() = profile;
    }

    pub fn active_profile(&self) -> Option<CharacterProfile> {
        self.active_profile.lock().unwrap().clone()
    }

    pub fn current_mood(&self) -> CompanionMood {
        *self.current_mood.lock().unwrap()
    }

    pub fn handle_event(&self, event: CompanionEvent) -> CompanionMood {
        let mood = event.to_mood();
        *self.current_mood.lock().unwrap() = mood;
        mood
    }

    /// Companion display name or default fallback.
    pub fn companion_name(&self) -> String {
        self.active_profile
            .lock()
            .unwrap()
            .as_ref()
            .map_or_else(|| "Wowie".to_string(), |p| p.display_name.clone())
    }

    /// Multi-line visual presentation representing the companion's current mood.
    /// Acts as an expressive desktop terminal companion and visual fallback.
    pub fn render_ascii_art(&self) -> &'static str {
        let mood = self.current_mood();
        match mood {
            CompanionMood::Neutral => {
                r#"
       /\_/\  
      ( o.o )  [Ready]
       > ^ <   "Let's make progress today!"
                "#
            }
            CompanionMood::Focused => {
                r#"
       /\_/\  
      ( -.- )= [Deep Focus]
       > ^ <   "Coding in the zone... keep going!"
                "#
            }
            CompanionMood::Concerned => {
                r#"
       /\_/\  
      ( O.O )! [Distraction Check]
       > ^ <   "Looks like we drifted off-task. Need a break?"
                "#
            }
            CompanionMood::Relaxed => {
                r#"
       /\_/\  
      ( u.u )_旦~ [Break Time]
       > ^ <   "Resting the mind. Refresh and recharge."
                "#
            }
            CompanionMood::Celebrating => {
                r#"
     * \_/\ * 
    * ( ^.^ ) * [Goal Achieved!]
       > ^ <   "Spectacular work! Session completed!"
                "#
            }
            CompanionMood::Thinking => {
                r#"
       /\_/\  
      ( ?.? )? [Analyzing]
       > ^ <   "Evaluating desktop activity signals..."
                "#
            }
            CompanionMood::Snoozed => {
                r#"
       /\_/\  
      ( -.- ) zZz [Snoozed]
       > ^ <   "Coaching paused. Taking a brief snooze."
                "#
            }
        }
    }

    /// Formats a companion notification message with personality.
    pub fn format_message(&self, text: &str) -> String {
        let name = self.companion_name();
        format!("[{name}] {text}")
    }
}
