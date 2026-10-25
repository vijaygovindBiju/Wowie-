use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            pub fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            pub fn into_uuid(self) -> Uuid {
                self.0
            }

            pub fn parse_str(s: &str) -> Result<Self, CoreError> {
                Uuid::parse_str(s)
                    .map(Self)
                    .map_err(|e| CoreError::InvalidId(format!("{}: {}", stringify!($name), e)))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl FromStr for $name {
            type Err = CoreError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse_str(s)
            }
        }
    };
}

define_id!(
    AmbitionId,
    "Strongly typed unique identifier for an Ambition."
);
define_id!(GoalId, "Strongly typed unique identifier for a Goal.");
define_id!(
    CommitmentId,
    "Strongly typed unique identifier for a Focus Commitment."
);
define_id!(
    SessionId,
    "Strongly typed unique identifier for a Focus Session."
);
define_id!(
    ObservationId,
    "Strongly typed unique identifier for an Activity Observation."
);
define_id!(
    SegmentId,
    "Strongly typed unique identifier for an Activity Segment."
);
define_id!(
    ClassificationId,
    "Strongly typed unique identifier for a Classification record."
);
define_id!(
    CorrectionId,
    "Strongly typed unique identifier for a User Correction."
);
define_id!(
    ReviewId,
    "Strongly typed unique identifier for a Session Review."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_generation_unique() {
        let id1 = GoalId::new();
        let id2 = GoalId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_id_serde_roundtrip() {
        let id = SessionId::new();
        let json = serde_json::to_string(&id).expect("Serialization failed");
        let deserialized: SessionId = serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_id_parse_valid_and_invalid() {
        let uuid = Uuid::new_v4();
        let parsed = AmbitionId::parse_str(&uuid.to_string()).expect("Should parse valid UUID");
        assert_eq!(parsed.into_uuid(), uuid);

        let err = AmbitionId::parse_str("not-a-uuid");
        assert!(matches!(err, Err(CoreError::InvalidId(_))));
    }

    #[test]
    fn test_id_from_str_trait() {
        let uuid_str = "67e55044-10b1-426f-9247-bb4e0e5c829f";
        let parsed: GoalId = uuid_str.parse().expect("FromStr failed");
        assert_eq!(parsed.to_string(), uuid_str);
    }
}
