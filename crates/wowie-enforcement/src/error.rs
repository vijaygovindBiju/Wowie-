use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EnforcementError {
    #[error("Action rejected: target '{0}' is an essential system tool or developer resource and cannot be blocked")]
    EssentialToolProtected(String),

    #[error("Unauthorized: proposal token has expired or is invalid")]
    UnauthorizedToken,

    #[error("No active session or session does not permit enforcement")]
    SessionInactive,

    #[error("Target rule not configured or disabled by user: {0}")]
    RuleNotConfigured(String),

    #[error("Enforcement adapter error ({adapter}): {details}")]
    AdapterFailure {
        adapter: &'static str,
        details: String,
    },

    #[error("Internal enforcement error: {0}")]
    Internal(String),
}
