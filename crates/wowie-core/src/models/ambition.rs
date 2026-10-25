use crate::error::CoreError;
use crate::id::AmbitionId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MAX_AMBITION_TITLE_LENGTH: usize = 200;
pub const MAX_AMBITION_DESCRIPTION_LENGTH: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ambition {
    pub id: AmbitionId,
    pub title: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

impl Ambition {
    pub fn new(
        title: impl Into<String>,
        description: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        Self::with_id(AmbitionId::new(), title, description, now)
    }

    pub fn with_id(
        id: AmbitionId,
        title: impl Into<String>,
        description: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        let title = title.into().trim().to_string();
        if title.is_empty() {
            return Err(CoreError::validation(
                "title",
                "Ambition title cannot be empty",
            ));
        }
        if title.chars().count() > MAX_AMBITION_TITLE_LENGTH {
            return Err(CoreError::validation(
                "title",
                format!(
                    "Ambition title exceeds maximum length of {} characters",
                    MAX_AMBITION_TITLE_LENGTH
                ),
            ));
        }

        let description = description.map(|d| d.trim().to_string());
        if let Some(ref d) = description {
            if d.chars().count() > MAX_AMBITION_DESCRIPTION_LENGTH {
                return Err(CoreError::validation(
                    "description",
                    format!(
                        "Ambition description exceeds maximum length of {} characters",
                        MAX_AMBITION_DESCRIPTION_LENGTH
                    ),
                ));
            }
        }

        Ok(Self {
            id,
            title,
            description,
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
    }

    pub fn update(
        &mut self,
        title: Option<String>,
        description: Option<Option<String>>,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        if let Some(t) = title {
            let trimmed = t.trim().to_string();
            if trimmed.is_empty() {
                return Err(CoreError::validation(
                    "title",
                    "Ambition title cannot be empty",
                ));
            }
            if trimmed.chars().count() > MAX_AMBITION_TITLE_LENGTH {
                return Err(CoreError::validation(
                    "title",
                    format!(
                        "Ambition title exceeds maximum length of {} characters",
                        MAX_AMBITION_TITLE_LENGTH
                    ),
                ));
            }
            self.title = trimmed;
        }

        if let Some(d) = description {
            if let Some(desc_val) = d {
                let trimmed_desc = desc_val.trim().to_string();
                if trimmed_desc.chars().count() > MAX_AMBITION_DESCRIPTION_LENGTH {
                    return Err(CoreError::validation(
                        "description",
                        format!(
                            "Ambition description exceeds maximum length of {} characters",
                            MAX_AMBITION_DESCRIPTION_LENGTH
                        ),
                    ));
                }
                self.description = Some(trimmed_desc);
            } else {
                self.description = None;
            }
        }

        self.updated_at = now;
        Ok(())
    }

    pub fn archive(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.archived_at = Some(now);
        self.updated_at = now;
        Ok(())
    }

    pub fn unarchive(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.archived_at = None;
        self.updated_at = now;
        Ok(())
    }

    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ambition_creation_valid() {
        let now = Utc::now();
        let ambition =
            Ambition::new("Learn Rust", Some("Daily study".into()), now).expect("Creation failed");
        assert_eq!(ambition.title, "Learn Rust");
        assert_eq!(ambition.description, Some("Daily study".to_string()));
        assert!(!ambition.is_archived());
    }

    #[test]
    fn test_ambition_bounds() {
        let now = Utc::now();
        // Empty title
        assert!(matches!(
            Ambition::new("", None, now),
            Err(CoreError::ValidationError { field, .. }) if field == "title"
        ));
        // Whitespace only title
        assert!(matches!(
            Ambition::new("   ", None, now),
            Err(CoreError::ValidationError { field, .. }) if field == "title"
        ));
        // Title exceeds max length
        let long_title = "a".repeat(MAX_AMBITION_TITLE_LENGTH + 1);
        assert!(matches!(
            Ambition::new(long_title, None, now),
            Err(CoreError::ValidationError { field, .. }) if field == "title"
        ));
        // Description exceeds max length
        let long_desc = "d".repeat(MAX_AMBITION_DESCRIPTION_LENGTH + 1);
        assert!(matches!(
            Ambition::new("Valid Title", Some(long_desc), now),
            Err(CoreError::ValidationError { field, .. }) if field == "description"
        ));
    }

    #[test]
    fn test_ambition_archive_cycle() {
        let now = Utc::now();
        let mut ambition = Ambition::new("Learn Rust", None, now).unwrap();
        assert!(!ambition.is_archived());

        let later = now + chrono::Duration::hours(1);
        ambition.archive(later).unwrap();
        assert!(ambition.is_archived());
        assert_eq!(ambition.archived_at, Some(later));

        let even_later = later + chrono::Duration::hours(1);
        ambition.unarchive(even_later).unwrap();
        assert!(!ambition.is_archived());
        assert_eq!(ambition.archived_at, None);
    }
}
