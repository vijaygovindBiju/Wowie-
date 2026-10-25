pub mod activity_log;
pub mod goal;
pub mod review;
pub mod session;

use crate::error::StorageError;
use chrono::{DateTime, Utc};
use wowie_core::id::{
    AmbitionId, ClassificationId, CommitmentId, CorrectionId, GoalId, ObservationId, ReviewId,
    SegmentId, SessionId,
};
use wowie_core::models::{
    ActivitySegment, Ambition, Classification, Commitment, FocusSession, Goal, GoalStatus,
    Observation, SessionEndReason, SessionReview, SessionState, UserCorrection,
};

/// Filtering criteria for querying goals.
#[derive(Debug, Clone, Default)]
pub struct GoalFilter {
    pub ambition_id: Option<AmbitionId>,
    pub status: Option<GoalStatus>,
    pub include_archived: bool,
}

/// Filtering criteria for querying sessions.
#[derive(Debug, Clone, Default)]
pub struct SessionFilter {
    pub commitment_id: Option<CommitmentId>,
    pub state: Option<SessionState>,
    pub limit: Option<usize>,
}

/// Repository contract for Ambition and Goal persistence.
pub trait GoalRepository: Send + Sync {
    // Ambition operations
    fn create_ambition(&self, ambition: &Ambition) -> Result<(), StorageError>;
    fn get_ambition(&self, id: AmbitionId) -> Result<Option<Ambition>, StorageError>;
    fn list_ambitions(&self, include_archived: bool) -> Result<Vec<Ambition>, StorageError>;
    fn update_ambition(&self, ambition: &Ambition) -> Result<(), StorageError>;
    fn delete_ambition(&self, id: AmbitionId) -> Result<(), StorageError>;

    // Goal operations
    fn create_goal(&self, goal: &Goal) -> Result<(), StorageError>;
    fn get_goal(&self, id: GoalId) -> Result<Option<Goal>, StorageError>;
    fn list_goals(&self, filter: GoalFilter) -> Result<Vec<Goal>, StorageError>;
    fn update_goal(&self, goal: &Goal) -> Result<(), StorageError>;
    fn archive_goal(&self, id: GoalId, now: DateTime<Utc>) -> Result<(), StorageError>;
    fn unarchive_goal(&self, id: GoalId, now: DateTime<Utc>) -> Result<(), StorageError>;
    fn delete_goal(&self, id: GoalId) -> Result<(), StorageError>;
}

/// Repository contract for Focus Commitment and Session persistence.
pub trait SessionRepository: Send + Sync {
    // Commitment operations
    fn create_commitment(&self, commitment: &Commitment) -> Result<(), StorageError>;
    fn get_commitment(&self, id: CommitmentId) -> Result<Option<Commitment>, StorageError>;
    fn list_commitments_for_goal(&self, goal_id: GoalId) -> Result<Vec<Commitment>, StorageError>;

    // Session operations
    fn create_session(&self, session: &FocusSession) -> Result<(), StorageError>;
    fn get_session(&self, id: SessionId) -> Result<Option<FocusSession>, StorageError>;
    fn get_active_session(&self) -> Result<Option<FocusSession>, StorageError>;
    fn list_sessions(&self, filter: SessionFilter) -> Result<Vec<FocusSession>, StorageError>;
    fn update_session(&self, session: &FocusSession) -> Result<(), StorageError>;
    fn update_session_state(
        &self,
        id: SessionId,
        state: SessionState,
        end_reason: Option<&SessionEndReason>,
        ended_at: Option<DateTime<Utc>>,
        last_state_change: DateTime<Utc>,
    ) -> Result<(), StorageError>;
    fn update_session_durations(
        &self,
        id: SessionId,
        focus_seconds: u64,
        break_seconds: u64,
        idle_seconds: u64,
        now: DateTime<Utc>,
    ) -> Result<(), StorageError>;
}

/// Repository contract for Observations, Segments, Classifications, and User Corrections.
pub trait ActivityLogRepository: Send + Sync {
    // Observation operations
    fn record_observation(&self, observation: &Observation) -> Result<(), StorageError>;
    fn record_observations_batch(&self, observations: &[Observation]) -> Result<(), StorageError>;
    fn get_observation(&self, id: ObservationId) -> Result<Option<Observation>, StorageError>;
    fn get_session_observations(
        &self,
        session_id: SessionId,
        limit: Option<usize>,
    ) -> Result<Vec<Observation>, StorageError>;
    fn delete_expired_observations(&self, now: DateTime<Utc>) -> Result<usize, StorageError>;

    // Segment operations
    fn record_segment(&self, segment: &ActivitySegment) -> Result<(), StorageError>;
    fn get_segment(&self, id: SegmentId) -> Result<Option<ActivitySegment>, StorageError>;
    fn get_session_segments(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<ActivitySegment>, StorageError>;

    // Classification operations
    fn record_classification(&self, classification: &Classification) -> Result<(), StorageError>;
    fn get_classification(
        &self,
        id: ClassificationId,
    ) -> Result<Option<Classification>, StorageError>;
    fn get_segment_classifications(
        &self,
        segment_id: SegmentId,
    ) -> Result<Vec<Classification>, StorageError>;

    // User Correction operations
    fn record_correction(&self, correction: &UserCorrection) -> Result<(), StorageError>;
    fn get_correction(&self, id: CorrectionId) -> Result<Option<UserCorrection>, StorageError>;
    fn get_session_corrections(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<UserCorrection>, StorageError>;
    fn get_persistent_corrections(&self) -> Result<Vec<UserCorrection>, StorageError>;
}

/// Repository contract for Post-Session Reviews.
pub trait ReviewRepository: Send + Sync {
    fn save_review(&self, review: &SessionReview) -> Result<(), StorageError>;
    fn get_review(&self, id: ReviewId) -> Result<Option<SessionReview>, StorageError>;
    fn get_review_by_session(
        &self,
        session_id: SessionId,
    ) -> Result<Option<SessionReview>, StorageError>;
    fn list_reviews(&self, limit: usize, offset: usize)
        -> Result<Vec<SessionReview>, StorageError>;
    fn delete_review(&self, id: ReviewId) -> Result<(), StorageError>;
}
