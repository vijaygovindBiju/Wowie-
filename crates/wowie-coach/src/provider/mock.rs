//! Deterministic offline mock provider for testing the AI Coach dialogue engine.
//!
//! Provides zero-dependency, 100% offline simulation supporting:
//! - Canned response mode
//! - FIFO queue mode (multi-turn sequences)
//! - Error injection mode (timeouts, network errors, rate limits, malformed JSON, disallowed actions)
//! - Heuristic pattern matching mode (keyword-based deterministic evaluation)
//! - Call inspection (call count, prompt history recording, state reset)

use crate::error::CoachError;
use crate::models::action::ActionCandidate;
use crate::models::response::{CoachRawResponse, RelevanceClassification};
use crate::prompt::CoachPrompt;
use crate::provider::trait_def::CoachProvider;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

/// Type alias for custom evaluation closures.
pub type CustomEvaluator =
    Box<dyn Fn(&CoachPrompt) -> Result<CoachRawResponse, CoachError> + Send + Sync>;

/// Operating mode and behavior of `MockCoachProvider`.
pub enum MockBehavior {
    /// Canned response returned on every evaluation.
    Canned(Result<CoachRawResponse, CoachError>),

    /// Sequential FIFO queue of responses/errors popped in order.
    Queue {
        items: VecDeque<Result<CoachRawResponse, CoachError>>,
        on_empty: QueueEmptyBehavior,
    },

    /// Deterministic heuristic matching based on prompt content.
    Heuristic,

    /// Custom closure evaluation for advanced test fixtures.
    Custom(CustomEvaluator),
}

impl std::fmt::Debug for MockBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Canned(res) => f.debug_tuple("Canned").field(res).finish(),
            Self::Queue { items, on_empty } => f
                .debug_struct("Queue")
                .field("len", &items.len())
                .field("on_empty", on_empty)
                .finish(),
            Self::Heuristic => write!(f, "Heuristic"),
            Self::Custom(_) => write!(f, "Custom(<closure>)"),
        }
    }
}

/// Policy when the mock FIFO queue is exhausted.
#[derive(Debug, Clone, PartialEq)]
pub enum QueueEmptyBehavior {
    /// Return an error indicating the queue is exhausted.
    Error,
    /// Fall back to a default canned response.
    Fallback(CoachRawResponse),
    /// Repeat the last response popped from the queue.
    RepeatLast,
}

#[derive(Debug)]
struct MockInnerState {
    behavior: MockBehavior,
    call_count: usize,
    received_prompts: Vec<CoachPrompt>,
    simulated_delay: Option<Duration>,
    last_response: Option<Result<CoachRawResponse, CoachError>>,
}

/// Zero-dependency, offline deterministic coach provider fixture.
#[derive(Debug)]
pub struct MockCoachProvider {
    name: &'static str,
    model_name: String,
    state: Mutex<MockInnerState>,
}

impl MockCoachProvider {
    /// Creates a mock provider returning a fixed `CoachRawResponse` on every evaluation.
    pub fn canned(response: CoachRawResponse) -> Self {
        Self::new(MockBehavior::Canned(Ok(response)))
    }

    /// Creates a mock provider returning a default `Relevant` response on every evaluation.
    pub fn canned_relevant() -> Self {
        let resp = CoachRawResponse {
            relevance: RelevanceClassification::Relevant,
            confidence: 0.95,
            uncertainty: Some(0.05),
            rationale: "Activity is directly relevant to active goal.".to_string(),
            question: None,
            action_candidate: Some(ActionCandidate::None),
            ..Default::default()
        };
        Self::canned(resp)
    }

    /// Creates a mock provider returning a `PossiblyUnrelated` response with an action candidate.
    pub fn canned_distracted(action: ActionCandidate) -> Self {
        let resp = CoachRawResponse {
            relevance: RelevanceClassification::PossiblyUnrelated,
            confidence: 0.85,
            uncertainty: Some(0.15),
            rationale: "Observed desktop activity appears unrelated to commitment.".to_string(),
            question: Some("Are you taking a break or researching?".to_string()),
            action_candidate: Some(action),
            ..Default::default()
        };
        Self::canned(resp)
    }

    /// Creates a mock provider returning an injected `CoachError` on every evaluation.
    pub fn canned_error(error: CoachError) -> Self {
        Self::new(MockBehavior::Canned(Err(error)))
    }

    /// Creates a mock provider returning responses from a FIFO queue.
    pub fn queue(items: impl IntoIterator<Item = Result<CoachRawResponse, CoachError>>) -> Self {
        Self::new(MockBehavior::Queue {
            items: items.into_iter().collect(),
            on_empty: QueueEmptyBehavior::Error,
        })
    }

    /// Creates an empty FIFO queue mock provider.
    pub fn empty_queue() -> Self {
        Self::queue(vec![])
    }

    /// Creates a mock provider using heuristic keyword matching.
    pub fn heuristic() -> Self {
        Self::new(MockBehavior::Heuristic)
    }

    /// Creates a mock provider simulating a transient error: fails `fail_count` times, then succeeds.
    pub fn transient_failure(
        fail_count: usize,
        error: CoachError,
        then_succeed: CoachRawResponse,
    ) -> Self {
        let mut items = Vec::with_capacity(fail_count + 1);
        for _ in 0..fail_count {
            items.push(Err(error.clone()));
        }
        items.push(Ok(then_succeed));
        Self::queue(items)
    }

    fn new(behavior: MockBehavior) -> Self {
        Self {
            name: "mock_coach_provider",
            model_name: "mock-model-v1".to_string(),
            state: Mutex::new(MockInnerState {
                behavior,
                call_count: 0,
                received_prompts: Vec::new(),
                simulated_delay: None,
                last_response: None,
            }),
        }
    }

    /// Sets a simulated delay before returning responses.
    #[must_use]
    pub fn with_delay(self, delay: Duration) -> Self {
        if let Ok(mut state) = self.state.lock() {
            state.simulated_delay = Some(delay);
        }
        self
    }

    /// Sets the custom provider name.
    #[must_use]
    pub fn with_name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    /// Sets the custom model name.
    #[must_use]
    pub fn with_model_name(mut self, model_name: impl Into<String>) -> Self {
        self.model_name = model_name.into();
        self
    }

    /// Pushes a response to the FIFO queue (thread-safe via interior mutability).
    pub fn push_response(&self, response: CoachRawResponse) {
        if let Ok(mut state) = self.state.lock() {
            if let MockBehavior::Queue { items, .. } = &mut state.behavior {
                items.push_back(Ok(response));
            } else {
                let mut items = VecDeque::new();
                items.push_back(Ok(response));
                state.behavior = MockBehavior::Queue {
                    items,
                    on_empty: QueueEmptyBehavior::Error,
                };
            }
        }
    }

    /// Pushes an error to the FIFO queue.
    pub fn push_error(&self, error: CoachError) {
        if let Ok(mut state) = self.state.lock() {
            if let MockBehavior::Queue { items, .. } = &mut state.behavior {
                items.push_back(Err(error));
            } else {
                let mut items = VecDeque::new();
                items.push_back(Err(error));
                state.behavior = MockBehavior::Queue {
                    items,
                    on_empty: QueueEmptyBehavior::Error,
                };
            }
        }
    }

    /// Pushes a raw JSON string to the FIFO queue, parsing and validating it.
    pub fn push_json(&self, raw_json: &str) -> Result<(), CoachError> {
        let raw = CoachRawResponse::from_json(raw_json)?;
        self.push_response(raw);
        Ok(())
    }

    /// Sets the behavior when the queue is exhausted.
    pub fn set_on_empty(&self, on_empty: QueueEmptyBehavior) {
        if let Ok(mut state) = self.state.lock() {
            if let MockBehavior::Queue {
                on_empty: target, ..
            } = &mut state.behavior
            {
                *target = on_empty;
            }
        }
    }

    /// Returns the number of times `evaluate` was called.
    pub fn call_count(&self) -> usize {
        self.state.lock().map_or(0, |s| s.call_count)
    }

    /// Returns a copy of all prompts received in order.
    pub fn received_prompts(&self) -> Vec<CoachPrompt> {
        self.state
            .lock()
            .map(|s| s.received_prompts.clone())
            .unwrap_or_default()
    }

    /// Returns the most recently received prompt, if any.
    pub fn last_prompt(&self) -> Option<CoachPrompt> {
        self.state
            .lock()
            .ok()
            .and_then(|s| s.received_prompts.last().cloned())
    }

    /// Resets call count, prompt history, and last response.
    pub fn reset_history(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.call_count = 0;
            state.received_prompts.clear();
            state.last_response = None;
        }
    }
}

#[async_trait::async_trait]
impl CoachProvider for MockCoachProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }

    async fn evaluate(&self, prompt: &CoachPrompt) -> Result<CoachRawResponse, CoachError> {
        prompt.validate()?;

        let (delay, result) = {
            let mut state = self.state.lock().map_err(|e| {
                CoachError::provider_unavailable(self.name, format!("Mock mutex poisoned: {e}"))
            })?;

            state.call_count += 1;
            state.received_prompts.push(prompt.clone());

            let result = match &mut state.behavior {
                MockBehavior::Canned(res) => res.clone(),
                MockBehavior::Queue { items, on_empty } => {
                    if let Some(item) = items.pop_front() {
                        item
                    } else {
                        match on_empty {
                            QueueEmptyBehavior::Error => Err(CoachError::provider_unavailable(
                                self.name,
                                "MockCoachProvider FIFO queue exhausted",
                            )),
                            QueueEmptyBehavior::Fallback(fb) => Ok(fb.clone()),
                            QueueEmptyBehavior::RepeatLast => {
                                state.last_response.clone().unwrap_or_else(|| {
                                    Err(CoachError::provider_unavailable(
                                        self.name,
                                        "MockCoachProvider FIFO queue empty and no prior response",
                                    ))
                                })
                            }
                        }
                    }
                }
                MockBehavior::Heuristic => Ok(evaluate_heuristic(prompt)),
                MockBehavior::Custom(func) => func(prompt),
            };

            state.last_response = Some(result.clone());
            (state.simulated_delay, result)
        };

        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }

        result
    }
}

/// Evaluates prompt deterministically based on keyword heuristics.
fn evaluate_heuristic(prompt: &CoachPrompt) -> CoachRawResponse {
    let lower = prompt.user_prompt.to_lowercase();

    // 1. Break state handling
    if lower.contains("break active: true") || lower.contains("in break state") {
        return CoachRawResponse {
            relevance: RelevanceClassification::Relevant,
            confidence: 0.90,
            uncertainty: Some(0.10),
            rationale: "User is in a scheduled focus break; activity is permitted.".to_string(),
            question: None,
            action_candidate: Some(ActionCandidate::None),
            ..Default::default()
        };
    }

    // 2. Distraction keywords (longer domains precede shorter ones to avoid substring collisions like x.com in netflix.com)
    let distraction_keywords = [
        "instagram.com",
        "facebook.com",
        "youtube.com",
        "netflix.com",
        "twitter.com",
        "tiktok.com",
        "reddit.com",
        "twitch.tv",
        "gaming",
        "steam",
        "x.com",
    ];
    for kw in distraction_keywords {
        if lower.contains(kw) {
            return CoachRawResponse {
                relevance: RelevanceClassification::PossiblyUnrelated,
                confidence: 0.88,
                uncertainty: Some(0.12),
                rationale: format!(
                    "Observed activity '{kw}' appears unrelated to the active goal."
                ),
                question: Some("Are you taking an unscheduled break or researching?".to_string()),
                action_candidate: Some(ActionCandidate::Remind),
                ..Default::default()
            };
        }
    }

    // 3. Ambiguous keywords
    let ambiguous_keywords = [
        "tutorial",
        "stackoverflow.com",
        "search",
        "wikipedia.org",
        "docs",
        "guide",
    ];
    for kw in ambiguous_keywords {
        if lower.contains(kw) {
            return CoachRawResponse {
                relevance: RelevanceClassification::PossiblyUnrelated,
                confidence: 0.60,
                uncertainty: Some(0.40),
                rationale: format!(
                    "Observed research/tutorial activity '{kw}' may be related to the goal."
                ),
                question: Some("Is this reference material for your focus task?".to_string()),
                action_candidate: Some(ActionCandidate::AskContext),
                ..Default::default()
            };
        }
    }

    // 4. Default productive keywords or general fallback
    CoachRawResponse {
        relevance: RelevanceClassification::Relevant,
        confidence: 0.92,
        uncertainty: Some(0.08),
        rationale: "Observed desktop activity is aligned with focus commitment.".to_string(),
        question: None,
        action_candidate: Some(ActionCandidate::None),
        ..Default::default()
    }
}
