use crate::config::AppSettings;
use crate::error::AppError;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use wowie_avatar::companion::CompanionRuntime;
use wowie_avatar::registry::CharacterRegistry;
use wowie_coach::engine::{CoachEngine, CoachEngineConfig};
use wowie_coach::models::action::ActionCandidate;
use wowie_coach::models::response::CoachResponse;
use wowie_coach::provider::mock::MockCoachProvider;
use wowie_coach::provider::openai::{OpenAiCompatibleClient, OpenAiConfig};
use wowie_core::models::{Commitment, FocusSession, Goal, Observation};
use wowie_enforcement::adapter::linux::LinuxEnforcementAdapter;
use wowie_enforcement::manager::EnforcementManager;
use wowie_enforcement::policy::EnforcementPolicyEngine;
use wowie_observer::model::ObserverConfig;
use wowie_observer::observer::ActivityObserver;
use wowie_observer::platform::detect_platform_observer;
use wowie_storage::connection::DbPool;
use wowie_storage::repos::activity_log::SqliteActivityLogRepository;
use wowie_storage::repos::goal::SqliteGoalRepository;
use wowie_storage::repos::review::SqliteReviewRepository;
use wowie_storage::repos::session::SqliteSessionRepository;
use wowie_storage::{GoalRepository, SessionRepository};

#[derive(Debug, Clone)]
pub struct ActiveInterventionInfo {
    pub rationale: String,
    pub question: Option<String>,
    pub action_candidate: Option<ActionCandidate>,
    pub proposed_block_token: Option<uuid::Uuid>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub goal_repo: SqliteGoalRepository,
    pub session_repo: SqliteSessionRepository,
    pub activity_repo: SqliteActivityLogRepository,
    pub review_repo: SqliteReviewRepository,
    pub coach_engine: Arc<CoachEngine>,
    pub observer: ActivityObserver,
    pub avatar_registry: CharacterRegistry,
    pub companion: CompanionRuntime,
    pub enforcement: EnforcementManager,
    pub settings: Arc<RwLock<AppSettings>>,
    pub active_session: Arc<RwLock<Option<FocusSession>>>,
    pub active_commitment: Arc<RwLock<Option<Commitment>>>,
    pub active_goal: Arc<RwLock<Option<Goal>>>,
    pub latest_observation: Arc<RwLock<Option<Observation>>>,
    pub latest_coach_response: Arc<RwLock<Option<CoachResponse>>>,
    pub active_intervention: Arc<RwLock<Option<ActiveInterventionInfo>>>,
    pub consecutive_distractions: Arc<RwLock<u32>>,
    pub overlay_bridge: Arc<std::sync::Mutex<Option<wowie_overlay::OverlayBridge>>>,
}

impl AppState {
    pub fn new_with_db(db_path: Option<PathBuf>, settings: AppSettings) -> Result<Self, AppError> {
        let pool = match db_path {
            Some(path) => {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                DbPool::open_file(&path).map_err(AppError::Storage)?
            }
            None => DbPool::open_in_memory().map_err(AppError::Storage)?,
        };

        // Run migrations
        pool.with_conn(|conn| {
            use wowie_storage::Migrator;
            wowie_storage::SqliteMigrator::new().initialize_and_migrate(conn)
        })
        .map_err(AppError::Storage)?;

        let goal_repo = SqliteGoalRepository::new(pool.clone());
        let session_repo = SqliteSessionRepository::new(pool.clone());
        let activity_repo = SqliteActivityLogRepository::new(pool.clone());
        let review_repo = SqliteReviewRepository::new(pool.clone());

        // Platform observer (detects Hyprland on Linux / Arch)
        let platform = detect_platform_observer();
        let observer_config = ObserverConfig {
            sampling_interval_ms: settings.sampling_interval_seconds * 1000,
            ..Default::default()
        };
        let observer = ActivityObserver::new(Arc::from(platform), observer_config);

        // Coach engine: configure provider (OpenAI/Ollama or heuristic fallback)
        let coach_builder = CoachEngine::builder().config(CoachEngineConfig::default());
        let coach_builder = if let Some(ref endpoint) = settings.ai_endpoint {
            let config = OpenAiConfig::new(
                endpoint,
                settings
                    .ai_model
                    .clone()
                    .unwrap_or_else(|| "llama3".to_string()),
            );
            let client = OpenAiCompatibleClient::new(config).map_err(AppError::Coach)?;
            coach_builder.provider(Arc::new(client))
        } else {
            // Default production / offline provider: deterministic heuristic
            coach_builder.provider(Arc::new(MockCoachProvider::heuristic()))
        };
        let coach_engine = Arc::new(coach_builder.build().map_err(AppError::Coach)?);

        // Character avatar registry
        let avatar_dir = std::env::var("XDG_DATA_HOME")
            .map_or_else(
                |_| {
                    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                    PathBuf::from(home).join(".local/share")
                },
                PathBuf::from,
            )
            .join("wowie/avatars");
        let avatar_registry =
            CharacterRegistry::new(avatar_dir).unwrap_or_else(|_| CharacterRegistry::in_memory());
        let _ = avatar_registry.seed_default_character_if_empty();

        let active_char = avatar_registry.get_active_character();
        let companion = CompanionRuntime::new(active_char);

        // Enforcement subsystem
        let enforcement_policy = Arc::new(EnforcementPolicyEngine::new());
        let enforcement_adapter = Arc::new(LinuxEnforcementAdapter::default());
        let enforcement = EnforcementManager::new(enforcement_policy, enforcement_adapter);

        // Restore any in-progress active session on restart
        let active_session = session_repo.get_active_session().unwrap_or(None);
        let active_commitment = if let Some(ref s) = active_session {
            session_repo.get_commitment(s.commitment_id).unwrap_or(None)
        } else {
            None
        };
        let active_goal = if let Some(ref c) = active_commitment {
            goal_repo.get_goal(c.goal_id).unwrap_or(None)
        } else {
            None
        };

        if let Some(ref s) = active_session {
            if s.state == wowie_core::models::SessionState::Active {
                companion.handle_event(wowie_avatar::model::CompanionEvent::FocusStarted);
            } else if s.state == wowie_core::models::SessionState::Break {
                companion.handle_event(wowie_avatar::model::CompanionEvent::BreakStarted);
            }
        }

        Ok(Self {
            pool,
            goal_repo,
            session_repo,
            activity_repo,
            review_repo,
            coach_engine,
            observer,
            avatar_registry,
            companion,
            enforcement,
            settings: Arc::new(RwLock::new(settings)),
            active_session: Arc::new(RwLock::new(active_session)),
            active_commitment: Arc::new(RwLock::new(active_commitment)),
            active_goal: Arc::new(RwLock::new(active_goal)),
            latest_observation: Arc::new(RwLock::new(None)),
            latest_coach_response: Arc::new(RwLock::new(None)),
            active_intervention: Arc::new(RwLock::new(None)),
            consecutive_distractions: Arc::new(RwLock::new(0)),
            overlay_bridge: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    pub fn in_memory(settings: AppSettings) -> Result<Self, AppError> {
        Self::new_with_db(None, settings)
    }
}
