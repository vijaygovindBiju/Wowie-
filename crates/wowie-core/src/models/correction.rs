use crate::error::CoreError;
use crate::id::{ClassificationId, CorrectionId, SegmentId, SessionId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationLabel {
    Relevant,
    PossiblyUnrelated,
    Unrelated,
    Unknown,
}

impl fmt::Display for ClassificationLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Relevant => write!(f, "relevant"),
            Self::PossiblyUnrelated => write!(f, "possibly_unrelated"),
            Self::Unrelated => write!(f, "unrelated"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationBasis {
    DeterministicRule,
    AiSuggestion,
    UserCorrection,
}

impl fmt::Display for ClassificationBasis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeterministicRule => write!(f, "deterministic_rule"),
            Self::AiSuggestion => write!(f, "ai_suggestion"),
            Self::UserCorrection => write!(f, "user_correction"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Classification {
    pub id: ClassificationId,
    pub segment_id: SegmentId,
    pub label: ClassificationLabel,
    pub confidence: f32,
    pub basis: ClassificationBasis,
    pub classifier_version: String,
    pub created_at: DateTime<Utc>,
}

impl Classification {
    pub fn new(
        segment_id: SegmentId,
        label: ClassificationLabel,
        confidence: f32,
        basis: ClassificationBasis,
        classifier_version: impl Into<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        Self::with_id(
            ClassificationId::new(),
            segment_id,
            label,
            confidence,
            basis,
            classifier_version,
            now,
        )
    }

    pub fn with_id(
        id: ClassificationId,
        segment_id: SegmentId,
        label: ClassificationLabel,
        confidence: f32,
        basis: ClassificationBasis,
        classifier_version: impl Into<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(CoreError::validation(
                "confidence",
                "Classification confidence must be between 0.0 and 1.0",
            ));
        }
        let classifier_version = classifier_version.into().trim().to_string();
        if classifier_version.is_empty() {
            return Err(CoreError::validation(
                "classifier_version",
                "Classifier version cannot be empty",
            ));
        }

        Ok(Self {
            id,
            segment_id,
            label,
            confidence,
            basis,
            classifier_version,
            created_at: now,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionScope {
    CurrentEvent,
    Session,
    PersistentRule,
}

impl fmt::Display for CorrectionScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentEvent => write!(f, "current_event"),
            Self::Session => write!(f, "session"),
            Self::PersistentRule => write!(f, "persistent_rule"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCorrection {
    pub id: CorrectionId,
    pub session_id: SessionId,
    pub target_segment_id: Option<SegmentId>,
    pub app_id: Option<String>,
    pub origin: Option<String>,
    pub label: ClassificationLabel,
    pub scope: CorrectionScope,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl UserCorrection {
    pub fn new(
        session_id: SessionId,
        target_segment_id: Option<SegmentId>,
        app_id: Option<String>,
        origin: Option<String>,
        label: ClassificationLabel,
        scope: CorrectionScope,
        note: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        Self::with_id(
            CorrectionId::new(),
            session_id,
            target_segment_id,
            app_id,
            origin,
            label,
            scope,
            note,
            now,
        )
    }

    pub fn with_id(
        id: CorrectionId,
        session_id: SessionId,
        target_segment_id: Option<SegmentId>,
        app_id: Option<String>,
        origin: Option<String>,
        label: ClassificationLabel,
        scope: CorrectionScope,
        note: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        if target_segment_id.is_none() && app_id.is_none() && origin.is_none() {
            return Err(CoreError::validation(
                "target",
                "User correction must specify at least one of target_segment_id, app_id, or origin",
            ));
        }
        if label == ClassificationLabel::Unknown {
            return Err(CoreError::validation(
                "label",
                "User correction label must specify an explicit relevance state (cannot be Unknown)",
            ));
        }

        Ok(Self {
            id,
            session_id,
            target_segment_id,
            app_id,
            origin,
            label,
            scope,
            note,
            created_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classification_validation() {
        let now = Utc::now();
        let segment_id = SegmentId::new();

        // Valid classification
        let c = Classification::new(
            segment_id,
            ClassificationLabel::Relevant,
            0.95,
            ClassificationBasis::DeterministicRule,
            "rule-engine-v1",
            now,
        )
        .unwrap();
        assert_eq!(c.confidence, 0.95);

        // Invalid confidence
        assert!(Classification::new(
            segment_id,
            ClassificationLabel::Relevant,
            1.1,
            ClassificationBasis::DeterministicRule,
            "v1",
            now,
        )
        .is_err());

        // Empty version
        assert!(Classification::new(
            segment_id,
            ClassificationLabel::Relevant,
            0.5,
            ClassificationBasis::DeterministicRule,
            "",
            now,
        )
        .is_err());
    }

    #[test]
    fn test_user_correction_bounds() {
        let now = Utc::now();
        let session_id = SessionId::new();

        // Valid correction
        let corr = UserCorrection::new(
            session_id,
            None,
            Some("spotify".into()),
            None,
            ClassificationLabel::Relevant,
            CorrectionScope::Session,
            Some("Focus music".into()),
            now,
        )
        .unwrap();
        assert_eq!(corr.label, ClassificationLabel::Relevant);

        // Missing all targets
        assert!(UserCorrection::new(
            session_id,
            None,
            None,
            None,
            ClassificationLabel::Relevant,
            CorrectionScope::Session,
            None,
            now,
        )
        .is_err());

        // Label is Unknown
        assert!(UserCorrection::new(
            session_id,
            Some(SegmentId::new()),
            None,
            None,
            ClassificationLabel::Unknown,
            CorrectionScope::CurrentEvent,
            None,
            now,
        )
        .is_err());
    }
}
