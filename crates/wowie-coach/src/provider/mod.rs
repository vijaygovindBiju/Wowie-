//! Pluggable LLM provider abstraction and client implementations.

pub mod mock;
pub mod openai;
pub mod trait_def;

pub use mock::{CustomEvaluator, MockBehavior, MockCoachProvider, QueueEmptyBehavior};
pub use openai::{OpenAiCompatibleClient, OpenAiConfig, ResponseFormatMode};
pub use trait_def::CoachProvider;
