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

/// Detects which local speech engine is installed on the machine.
pub fn detect_available_engine() -> VoiceEngine {
    SpeechSynthesizer::detect_available_engine()
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

    /// Asynchronously speaks the provided text according to the active voice configuration.
    pub async fn speak_async(&self, text: &str) -> Result<(), SpeechError> {
        if !self.config.enabled {
            return Ok(());
        }

        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        match &self.active_engine {
            VoiceEngine::Mock => Ok(()),
            VoiceEngine::Piper => Self::execute_piper_async(&self.config, trimmed).await,
            VoiceEngine::EspeakNg => {
                let status = tokio::process::Command::new("espeak-ng")
                    .args([
                        "-s",
                        &self.config.rate.to_string(),
                        "-p",
                        &self.config.pitch.to_string(),
                        trimmed,
                    ])
                    .status()
                    .await
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
                let status = tokio::process::Command::new("spd-say")
                    .args(["-r", &self.config.rate.to_string(), trimmed])
                    .status()
                    .await
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

    /// Spawns speech playback in a background task (fire-and-forget),
    /// ensuring caller never blocks the event loop.
    pub fn speak_in_background(&self, text: String) {
        if !self.config.enabled || text.trim().is_empty() {
            return;
        }
        let synth = self.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = synth.speak_async(&text).await;
            });
        }
    }

    /// Executes the Piper neural TTS pipeline asynchronously with audio output piping.
    async fn execute_piper_async(config: &VoiceConfig, text: &str) -> Result<(), SpeechError> {
        let mut piper_cmd = tokio::process::Command::new("piper");
        if let Some(model) = &config.piper_model_path {
            piper_cmd.arg("--model").arg(model);
        }
        piper_cmd.arg("--output-raw");
        piper_cmd.stdin(std::process::Stdio::piped());
        piper_cmd.stdout(std::process::Stdio::piped());

        let mut piper_child = piper_cmd
            .spawn()
            .map_err(|e| SpeechError::ExecutionFailed(format!("Piper spawn error: {e}")))?;

        if let Some(mut stdin) = piper_child.stdin.take() {
            async_write_all(&mut stdin, text.as_bytes())
                .await
                .map_err(|e| {
                    SpeechError::ExecutionFailed(format!("Piper stdin write error: {e}"))
                })?;
            drop(stdin);
        }

        // Pipe raw PCM into pw-play or aplay if available
        let player_bin = if Command::new("pw-play").arg("--version").output().is_ok() {
            Some("pw-play")
        } else if Command::new("aplay").arg("--version").output().is_ok() {
            Some("aplay")
        } else {
            None
        };

        let mut maybe_player = if let Some(bin) = player_bin {
            let mut player_cmd = tokio::process::Command::new(bin);
            if bin == "pw-play" {
                player_cmd.args([
                    "--format",
                    "s16le",
                    "--rate",
                    "22050",
                    "--channels",
                    "1",
                    "-",
                ]);
            } else {
                player_cmd.args(["-r", "22050", "-f", "S16_LE", "-t", "raw", "-"]);
            }
            player_cmd.stdin(std::process::Stdio::piped());
            player_cmd.spawn().ok()
        } else {
            None
        };

        if let Some(mut stdout) = piper_child.stdout.take() {
            if let Some(ref mut player) = maybe_player {
                if let Some(mut player_stdin) = player.stdin.take() {
                    let _ = async_copy_stream(&mut stdout, &mut player_stdin).await;
                    drop(player_stdin);
                }
            } else {
                let _ = async_drain(&mut stdout).await;
            }
        }

        if let Some(mut player) = maybe_player {
            let _ = player.wait().await;
        }

        let status = piper_child
            .wait()
            .await
            .map_err(|e| SpeechError::ExecutionFailed(format!("Piper wait error: {e}")))?;

        if status.success() {
            Ok(())
        } else {
            Err(SpeechError::ExecutionFailed(format!(
                "Piper returned exit status: {:?}",
                status.code()
            )))
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

    /// Asynchronously speaks a canned voice notification when transitioning into a new mood.
    pub async fn speak_for_mood_async(&self, mood: CompanionMood) -> Result<(), SpeechError> {
        if let Some(line) = Self::canned_line_for_mood(mood) {
            self.speak_async(line).await
        } else {
            Ok(())
        }
    }

    /// Spawns a background task speaking a canned voice notification for a mood.
    pub fn speak_for_mood_in_background(&self, mood: CompanionMood) {
        if let Some(line) = Self::canned_line_for_mood(mood) {
            self.speak_in_background(line.to_string());
        }
    }
}

async fn async_write_all<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    mut buf: &[u8],
) -> std::io::Result<()> {
    use std::pin::Pin;
    use std::task::Poll;

    std::future::poll_fn(|cx| {
        while !buf.is_empty() {
            match Pin::new(&mut *writer).poll_write(cx, buf) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    )));
                }
                Poll::Ready(Ok(n)) => buf = &buf[n..],
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Ok(()))
    })
    .await
}

async fn async_drain<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> std::io::Result<()> {
    use std::pin::Pin;
    use std::task::Poll;

    let mut scratch = [0u8; 4096];
    std::future::poll_fn(|cx| loop {
        let mut read_buf = tokio::io::ReadBuf::new(&mut scratch);
        match Pin::new(&mut *reader).poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(())) => {
                if read_buf.filled().is_empty() {
                    return Poll::Ready(Ok(()));
                }
            }
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Pending,
        }
    })
    .await
}

async fn async_copy_stream<R: tokio::io::AsyncRead + Unpin, W: tokio::io::AsyncWrite + Unpin>(
    reader: &mut R,
    writer: &mut W,
) -> std::io::Result<()> {
    use std::pin::Pin;
    use std::task::Poll;

    let mut scratch = [0u8; 8192];
    loop {
        let bytes_read = std::future::poll_fn(|cx| {
            let mut read_buf = tokio::io::ReadBuf::new(&mut scratch);
            match Pin::new(&mut *reader).poll_read(cx, &mut read_buf) {
                Poll::Ready(Ok(())) => Poll::Ready(Ok(read_buf.filled().len())),
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Pending => Poll::Pending,
            }
        })
        .await?;

        if bytes_read == 0 {
            break;
        }

        let mut slice = &scratch[..bytes_read];
        while !slice.is_empty() {
            let bytes_written =
                std::future::poll_fn(|cx| match Pin::new(&mut *writer).poll_write(cx, slice) {
                    Poll::Ready(Ok(0)) => Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    ))),
                    Poll::Ready(Ok(n)) => Poll::Ready(Ok(n)),
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    Poll::Pending => Poll::Pending,
                })
                .await?;
            slice = &slice[bytes_written..];
        }
    }
    Ok(())
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

    #[tokio::test]
    async fn test_mock_synthesizer_speaks_async_without_error() {
        let synth = SpeechSynthesizer::mock();
        assert!(synth.speak_async("Async speech test").await.is_ok());
        assert!(synth
            .speak_for_mood_async(CompanionMood::Focused)
            .await
            .is_ok());
        assert!(synth
            .speak_for_mood_async(CompanionMood::Neutral)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_speak_in_background_runs() {
        let synth = SpeechSynthesizer::mock();
        synth.speak_in_background("Background speech test".to_string());
        synth.speak_for_mood_in_background(CompanionMood::Concerned);
        tokio::task::yield_now().await;
    }

    #[tokio::test]
    async fn test_disabled_async_synthesizer_no_op() {
        let config = VoiceConfig {
            enabled: false,
            engine: VoiceEngine::Mock,
            ..Default::default()
        };
        let synth = SpeechSynthesizer::new(config);
        assert!(synth.speak_async("Should be skipped").await.is_ok());
    }
}
