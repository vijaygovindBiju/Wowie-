//! Domain models for coaching context, tone, actions, and responses.

pub mod action;
pub mod context;
pub mod response;
pub mod tone;

pub use action::{ActionCandidate, ALLOWED_ACTIONS};
pub use context::{
    CoachContext, SessionCommitmentContext, UntrustedObservation, UserCorrectionSummary,
    MAX_BREAK_ALLOWANCE_SECONDS, MAX_CORRECTION_NOTE_LENGTH, MAX_SESSION_DURATION_SECONDS,
    MIN_SESSION_DURATION_SECONDS,
};
pub use response::{
    CoachRawResponse, CoachResponse, ProviderProvenance, RelevanceClassification,
    MAX_PROVENANCE_IDENTIFIER_LENGTH, MAX_QUESTION_LENGTH, MAX_RATIONALE_LENGTH,
};
pub use tone::{
    PersonalityMode, ToneLevel, TonePreferences, DEFAULT_COACH_NAME, MAX_COACH_NAME_LENGTH,
    MAX_CUSTOM_INSTRUCTIONS_LENGTH,
};
