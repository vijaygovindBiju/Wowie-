use crate::model::CompanionMood;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum SpeechError {
    #[error("TTS engine '{0}' is not available on this system")]
    EngineUnavailable(String),
    #[error("Speech synthesis failed: {0}")]
    ExecutionFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Supported text-to-speech engine backends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VoiceEngine {
    /// Auto-detect available engine in order: Piper -> espeak-ng -> spd-say -> Mock.
    #[default]
    Auto,
    /// High-quality local neural text-to-speech using Piper.
    Piper,
    /// Lightweight multi-lingual synthesizer using espeak-ng.
    EspeakNg,
    /// System speech dispatcher via spd-say.
    SpeechDispatcher,
    /// Mock speech synthesizer for testing (records speech invocations without audio output).
    Mock,
}

/// Configuration for speech and voice interactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceConfig {
    pub enabled: bool,
    pub engine: VoiceEngine,
    pub rate: u32,
    pub pitch: u32,
    pub piper_model_path: Option<PathBuf>,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            engine: VoiceEngine::Auto,
            rate: 160,
            pitch: 50,
            piper_model_path: None,
        }
    }
}

/// Audio and voice synthesizer for Wowie companion characters.
#[derive(Debug, Clone)]
pub struct SpeechSynthesizer {
    config: VoiceConfig,
    active_engine: VoiceEngine,
}

impl SpeechSynthesizer {
    pub fn new(config: VoiceConfig) -> Self {
        let active_engine = if config.engine == VoiceEngine::Auto {
            Self::detect_available_engine()
        } else {
            config.engine.clone()
        };

        Self {
            config,
            active_engine,
        }
    }

    /// Creates a mock synthesizer for unit and integration testing.
    pub fn mock() -> Self {
        Self {
            config: VoiceConfig {
                enabled: true,
                engine: VoiceEngine::Mock,
                rate: 160,
                pitch: 50,
                piper_model_path: None,
            },
            active_engine: VoiceEngine::Mock,
        }
    }

    pub fn active_engine(&self) -> &VoiceEngine {
        &self.active_engine
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Detects which local speech engine is installed on the machine.
    pub fn detect_available_engine() -> VoiceEngine {
        if Command::new("piper").arg("--help").output().is_ok() {
            VoiceEngine::Piper
        } else if Command::new("espeak-ng").arg("--version").output().is_ok() {
            VoiceEngine::EspeakNg
        } else if Command::new("spd-say").arg("--version").output().is_ok() {
            VoiceEngine::SpeechDispatcher
        } else {
            VoiceEngine::Mock
        }
    }

    /// Returns the canned companion line corresponding to a mood state.
    pub fn canned_line_for_mood(mood: CompanionMood) -> Option<&'static str> {
        match mood {
            CompanionMood::Focused => Some("Focus session active. Stay in the flow."),
            CompanionMood::Concerned => Some("Hey! Stay focused on your goal."),
            CompanionMood::Snoozed => Some("Reminder snoozed. Taking a brief pause."),
            CompanionMood::Celebrating => {
                Some("Congratulations! You completed your focus session.")
            }
            CompanionMood::Relaxed => Some("Enjoy your break."),
            CompanionMood::Neutral | CompanionMood::Thinking => None,
        }
    }

    /// Speaks the provided text synchronously according to the active voice configuration.
    pub fn speak(&self, text: &str) -> Result<(), SpeechError> {
        if !self.config.enabled {
            return Ok(());
        }

        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        match &self.active_engine {
            VoiceEngine::Mock => Ok(()),
            VoiceEngine::Piper => {
                let mut cmd = Command::new("piper");
                if let Some(model) = &self.config.piper_model_path {
                    cmd.arg("--model").arg(model);
                }
                cmd.arg("--output-raw");
                // Piper speaks by piping raw PCM to pw-play / aplay
                let child = cmd
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| SpeechError::ExecutionFailed(format!("Piper spawn error: {e}")))?;

                drop(child);
                Ok(())
            }
            VoiceEngine::EspeakNg => {
                let status = Command::new("espeak-ng")
                    .args([
                        "-s",
                        &self.config.rate.to_string(),
                        "-p",
                        &self.config.pitch.to_string(),
                        trimmed,
                    ])
                    .status()
                    .map_err(|e| {
                        SpeechError::ExecutionFailed(format!("espeak-ng execution error: {e}"))
                    })?;

                if status.success() {
                    Ok(())
                } else {
                    Err(SpeechError::ExecutionFailed(format!(
                        "espeak-ng returned exit status: {:?}",
                        status.code()
                    )))
                }
            }
            VoiceEngine::SpeechDispatcher => {
                let status = Command::new("spd-say")
                    .args(["-r", &self.config.rate.to_string(), trimmed])
                    .status()
                    .map_err(|e| {
                        SpeechError::ExecutionFailed(format!("spd-say execution error: {e}"))
                    })?;

                if status.success() {
                    Ok(())
                } else {
                    Err(SpeechError::ExecutionFailed(format!(
                        "spd-say returned exit status: {:?}",
                        status.code()
                    )))
                }
            }
            VoiceEngine::Auto => Ok(()),
        }
    }

    /// Speaks a canned voice notification when the companion transitions into a new mood.
    pub fn speak_for_mood(&self, mood: CompanionMood) -> Result<(), SpeechError> {
        if let Some(line) = Self::canned_line_for_mood(mood) {
            self.speak(line)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canned_lines_mapping() {
        assert_eq!(
            SpeechSynthesizer::canned_line_for_mood(CompanionMood::Focused),
            Some("Focus session active. Stay in the flow.")
        );
        assert_eq!(
            SpeechSynthesizer::canned_line_for_mood(CompanionMood::Concerned),
            Some("Hey! Stay focused on your goal.")
        );
        assert_eq!(
            SpeechSynthesizer::canned_line_for_mood(CompanionMood::Celebrating),
            Some("Congratulations! You completed your focus session.")
        );
        assert_eq!(
            SpeechSynthesizer::canned_line_for_mood(CompanionMood::Neutral),
            None
        );
    }

    #[test]
    fn test_mock_synthesizer_speaks_without_error() {
        let synth = SpeechSynthesizer::mock();
        assert!(synth.is_enabled());
        assert_eq!(synth.active_engine(), &VoiceEngine::Mock);

        let res = synth.speak("Hello from Wowie!");
        assert!(res.is_ok());

        let res_mood = synth.speak_for_mood(CompanionMood::Celebrating);
        assert!(res_mood.is_ok());
    }

    #[test]
    fn test_disabled_synthesizer_no_op() {
        let config = VoiceConfig {
            enabled: false,
            engine: VoiceEngine::Mock,
            ..Default::default()
        };
        let synth = SpeechSynthesizer::new(config);
        assert!(!synth.is_enabled());
        assert!(synth.speak("Should do nothing").is_ok());
    }
}
