use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Core domain error: {0}")]
    Core(#[from] wowie_core::error::CoreError),

    #[error("Storage error: {0}")]
    Storage(#[from] wowie_storage::error::StorageError),

    #[error("Coach engine error: {0}")]
    Coach(#[from] wowie_coach::error::CoachError),

    #[error("Observer error: {0}")]
    Observer(#[from] wowie_observer::error::ObserverError),

    #[error("Avatar error: {0}")]
    Avatar(#[from] wowie_avatar::error::AvatarError),

    #[error("Enforcement error: {0}")]
    Enforcement(#[from] wowie_enforcement::error::EnforcementError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Application configuration or lifecycle error: {0}")]
    General(String),
}
