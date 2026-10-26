use std::process::Command;
use std::time::Instant;
use wowie_avatar::model::CompanionMood;

/// Autonomous desktop roaming controller.
/// Allows the 3D companion to float and wander freely around the screen without user intervention.
#[derive(Debug, Clone)]
pub struct RoamController {
    pub enabled: bool,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub screen_w: f32,
    pub screen_h: f32,
    pub window_w: f32,
    pub window_h: f32,
    pub last_dispatch: Instant,
    pub wander_timer: f32,
    pub last_dispatched_x: i32,
    pub last_dispatched_y: i32,
    pub is_initialized: bool,
}

impl RoamController {
    #[must_use]
    pub fn new(window_w: f32, window_h: f32, enabled: bool) -> Self {
        let (screen_w, screen_h) = Self::detect_screen_resolution();
        let init_x = (screen_w - window_w) * 0.5;
        let init_y = (screen_h - window_h) * 0.4;

        Self {
            enabled,
            x: init_x,
            y: init_y,
            vx: 45.0,
            vy: 25.0,
            screen_w,
            screen_h,
            window_w,
            window_h,
            last_dispatch: Instant::now(),
            wander_timer: 0.0,
            last_dispatched_x: init_x as i32,
            last_dispatched_y: init_y as i32,
            is_initialized: false,
        }
    }

    /// Queries Hyprland for primary monitor resolution, with graceful 1920x1080 fallback.
    #[must_use]
    pub fn detect_screen_resolution() -> (f32, f32) {
        if let Ok(out) = Command::new("hyprctl").args(["monitors", "-j"]).output() {
            if out.status.success() {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    if let Some(arr) = json.as_array() {
                        if let Some(first) = arr.first() {
                            let width = first
                                .get("width")
                                .and_then(serde_json::Value::as_f64)
                                .map_or(1920.0, |v| v as f32);
                            let height = first
                                .get("height")
                                .and_then(serde_json::Value::as_f64)
                                .map_or(1080.0, |v| v as f32);
                            return (width, height);
                        }
                    }
                }
            }
        }
        (1920.0, 1080.0)
    }

    /// Toggles autonomous screen roaming on or off.
    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.enabled
    }

    /// Updates the autonomous motion based on companion mood and elapsed delta time.
    pub fn update(&mut self, mood: CompanionMood, delta_secs: f32) {
        if !self.enabled {
            return;
        }

        self.wander_timer += delta_secs;
        let margin_x = 20.0;
        let margin_y = 40.0;
        let min_x = margin_x;
        let max_x = (self.screen_w - self.window_w - margin_x).max(min_x);
        let min_y = margin_y;
        let max_y = (self.screen_h - self.window_h - margin_y).max(min_y);

        match mood {
            CompanionMood::Concerned => {
                // Distraction Alert / "Angry": Frantic pacing and running back and forth across the screen!
                let fast_speed = 360.0;
                if self.vx.abs() < 100.0 {
                    self.vx = fast_speed;
                } else {
                    self.vx = self.vx.signum() * fast_speed;
                }

                self.x += self.vx * delta_secs;
                // High frequency vertical agitated wobble
                self.y += (self.wander_timer * 12.0).sin() * 80.0 * delta_secs;

                // Bounce sharply off walls
                if self.x <= min_x {
                    self.x = min_x;
                    self.vx = fast_speed;
                } else if self.x >= max_x {
                    self.x = max_x;
                    self.vx = -fast_speed;
                }
                self.y = self.y.clamp(min_y, max_y);
            }
            CompanionMood::Celebrating => {
                // Celebration: Energetic joyful bounding and bouncing!
                let hop_speed_x = 120.0;
                if self.vx.abs() < 50.0 {
                    self.vx = hop_speed_x;
                } else {
                    self.vx = self.vx.signum() * hop_speed_x;
                }

                self.x += self.vx * delta_secs;
                let bounce_y = (self.wander_timer * 5.0).sin().abs() * -120.0;
                self.y = (max_y + bounce_y).clamp(min_y, max_y);

                if self.x <= min_x {
                    self.x = min_x;
                    self.vx = hop_speed_x;
                } else if self.x >= max_x {
                    self.x = max_x;
                    self.vx = -hop_speed_x;
                }
            }
            CompanionMood::Focused => {
                // Deep Focus: Settle down calmly in the bottom-right corner out of the user's way
                let target_x = max_x - 30.0;
                let target_y = max_y - 20.0;
                self.x += (target_x - self.x) * (delta_secs * 2.0).min(1.0);
                self.y += (target_y - self.y) * (delta_secs * 2.0).min(1.0);
            }
            CompanionMood::Relaxed | CompanionMood::Snoozed => {
                // Relaxed / Break: Slow, peaceful sleepy drifting near the bottom
                self.x += (self.wander_timer * 0.4).sin() * 30.0 * delta_secs;
                let target_y = max_y - 30.0;
                self.y += (target_y - self.y) * (delta_secs * 1.5).min(1.0);
            }
            CompanionMood::Neutral | CompanionMood::Thinking => {
                // Idle / Thinking: Gentle zero-gravity floating and drifting
                let drift_speed = 50.0;
                // Periodic smooth direction shifts
                if (self.wander_timer % 4.0) < delta_secs {
                    let angle = self.wander_timer * 1.3;
                    self.vx = angle.cos() * drift_speed;
                    self.vy = angle.sin() * (drift_speed * 0.6);
                }

                self.x += self.vx * delta_secs;
                self.y += self.vy * delta_secs;

                // Gentle bounce off screen bounds
                if self.x <= min_x {
                    self.x = min_x;
                    self.vx = self.vx.abs();
                } else if self.x >= max_x {
                    self.x = max_x;
                    self.vx = -self.vx.abs();
                }

                if self.y <= min_y {
                    self.y = min_y;
                    self.vy = self.vy.abs();
                } else if self.y >= max_y {
                    self.y = max_y;
                    self.vy = -self.vy.abs();
                }
            }
        }
    }

    /// Dispatches window position update to Hyprland if moved by at least 2 pixels and rate-limited.
    pub fn try_dispatch_move(&mut self) {
        if !self.enabled {
            return;
        }

        let now = Instant::now();
        if now.duration_since(self.last_dispatch).as_millis() < 40 {
            return; // Rate limit to ~25 Hz for low CPU usage
        }

        let cur_x = self.x.round() as i32;
        let cur_y = self.y.round() as i32;

        if (cur_x - self.last_dispatched_x).abs() < 2 && (cur_y - self.last_dispatched_y).abs() < 2
        {
            return; // No noticeable movement
        }

        self.last_dispatched_x = cur_x;
        self.last_dispatched_y = cur_y;
        self.last_dispatch = now;

        let cmd = format!(
            r#"return hl.dispatch(hl.dsp.window.move({{ match = {{ title = ".*Wowie Companion.*" }}, x = {cur_x}, y = {cur_y} }}))"#
        );
        let _ = Command::new("hyprctl").args(["repl", &cmd]).output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roam_controller_initialization_and_toggle() {
        let mut roam = RoamController::new(320.0, 380.0, true);
        assert!(roam.enabled);
        assert_eq!(roam.window_w, 320.0);
        assert_eq!(roam.window_h, 380.0);

        let active = roam.toggle();
        assert!(!active);
        assert!(!roam.enabled);

        let active_again = roam.toggle();
        assert!(active_again);
        assert!(roam.enabled);
    }

    #[test]
    fn test_roam_controller_concerned_angry_high_speed_pacing() {
        let mut roam = RoamController::new(300.0, 300.0, true);
        roam.screen_w = 1000.0;
        roam.screen_h = 1000.0;
        roam.x = 200.0;
        roam.vx = 50.0;

        // In Concerned state, speed must accelerate to angry pacing (>300 px/sec)
        roam.update(CompanionMood::Concerned, 0.1);
        assert!(roam.vx.abs() >= 350.0);
        assert!(roam.x > 200.0);

        // Advance to screen boundary to verify bouncing
        roam.x = 900.0;
        roam.update(CompanionMood::Concerned, 0.1);
        assert!(
            roam.vx < 0.0,
            "Angry pacing must bounce off right screen edge"
        );
    }

    #[test]
    fn test_roam_controller_focused_settling_into_corner() {
        let mut roam = RoamController::new(300.0, 300.0, true);
        roam.screen_w = 1000.0;
        roam.screen_h = 1000.0;
        roam.x = 100.0;
        roam.y = 100.0;

        // When focused, it must glide toward bottom-right anchor
        for _ in 0..10 {
            roam.update(CompanionMood::Focused, 0.1);
        }
        assert!(
            roam.x > 100.0,
            "Focused mood must guide companion toward screen corner"
        );
        assert!(
            roam.y > 100.0,
            "Focused mood must guide companion toward screen corner"
        );
    }

    #[test]
    fn test_roam_controller_disabled_no_op() {
        let mut roam = RoamController::new(300.0, 300.0, false);
        let init_x = roam.x;
        let init_y = roam.y;

        roam.update(CompanionMood::Concerned, 0.5);
        assert_eq!(roam.x, init_x, "Disabled roam must not alter position");
        assert_eq!(roam.y, init_y, "Disabled roam must not alter position");
    }
}
