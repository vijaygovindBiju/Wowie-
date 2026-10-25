use crate::error::AppError;
use crate::state::{ActiveInterventionInfo, AppState};
use chrono::Utc;
use wowie_avatar::model::CompanionEvent;
use wowie_coach::models::context::{CoachContext, UntrustedObservation, UserCorrectionSummary};
use wowie_coach::models::response::RelevanceClassification;
use wowie_core::id::{CommitmentId, GoalId};
use wowie_core::models::{
    Ambition, ClassificationLabel, Commitment, ConsentSnapshot, CorrectionScope, FocusSession,
    Goal, SessionEndReason, SessionOutcome, SessionReview, SessionState, UserCorrection,
};
use wowie_core::state_machine::FocusSessionStateMachine;
use wowie_enforcement::model::EnforcementTarget;
use wowie_enforcement::policy::EnforcementPolicyEngine;
use wowie_storage::repos::{
    ActivityLogRepository, GoalRepository, ReviewRepository, SessionRepository,
};

#[derive(Debug, Clone)]
pub enum UserInterventionChoice {
    Resume,
    TakeBreak,
    Correct {
        scope: CorrectionScope,
        label: ClassificationLabel,
    },
    Snooze {
        minutes: u32,
    },
    AuthorizeBlock,
}

#[derive(Debug, Clone)]
pub struct WowieCoordinator {
    state: AppState,
}

impl WowieCoordinator {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn update_companion_mood(
        &self,
        event: CompanionEvent,
    ) -> wowie_avatar::model::CompanionMood {
        let mood = self.state.companion.handle_event(event);
        if let Ok(guard) = self.state.overlay_bridge.lock() {
            if let Some(ref bridge) = *guard {
                bridge.set_mood(mood);
            }
        }
        mood
    }

    /// Step 1: Create a high-level goal
    pub fn create_goal(&self, title: &str, description: Option<&str>) -> Result<Goal, AppError> {
        let now = Utc::now();
        let ambition = Ambition::new(title, description.map(ToString::to_string), now)?;
        self.state.goal_repo.create_ambition(&ambition)?;
        let goal = Goal::new(
            ambition.id,
            title,
            description.map(ToString::to_string),
            None,
            now,
        )?;
        self.state.goal_repo.create_goal(&goal)?;
        Ok(goal)
    }

    /// Step 2: Create a concrete time-bounded commitment
    pub fn create_commitment(
        &self,
        goal_id: GoalId,
        duration_minutes: u32,
        break_allowance_minutes: u32,
    ) -> Result<Commitment, AppError> {
        let now = Utc::now();
        let commitment = Commitment::new(
            goal_id,
            now,
            duration_minutes * 60,
            break_allowance_minutes * 60,
            None,
            now,
        )?;
        self.state.session_repo.create_commitment(&commitment)?;
        Ok(commitment)
    }

    /// Step 3: Start a focus session with explicit user consent
    pub async fn start_focus_session(
        &self,
        commitment_id: CommitmentId,
    ) -> Result<FocusSession, AppError> {
        if let Some(existing) = self.state.session_repo.get_active_session()? {
            return Err(AppError::General(format!(
                "Active session {} is already in progress. Complete or cancel it before starting another.",
                existing.id
            )));
        }

        let now = Utc::now();
        let settings = self.state.settings.read().await.clone();

        let consent = ConsentSnapshot {
            monitoring_enabled: true,
            window_title_redaction_enabled: settings.redact_window_titles,
            browser_origin_enabled: settings.collect_browser_origins,
            granted_at: now,
        };

        let mut session = FocusSession::new(commitment_id, consent, now);
        FocusSessionStateMachine::transition_start(&mut session, now)?;

        self.state.session_repo.create_session(&session)?;

        // Fetch associated commitment and goal for context
        let commitment = self.state.session_repo.get_commitment(commitment_id)?;
        let goal = if let Some(ref c) = commitment {
            self.state.goal_repo.get_goal(c.goal_id)?
        } else {
            None
        };

        *self.state.active_session.write().await = Some(session.clone());
        *self.state.active_commitment.write().await = commitment;
        *self.state.active_goal.write().await = goal;
        *self.state.consecutive_distractions.write().await = 0;
        *self.state.active_intervention.write().await = None;

        // Start activity observer
        self.state.observer.start().await?;

        // Update companion mood
        self.update_companion_mood(CompanionEvent::FocusStarted);

        // Send desktop notification
        let _ = tokio::process::Command::new("notify-send")
            .args([
                "-a",
                "Wowie",
                "Focus Session Started",
                "Focus session started! Monitoring active desktop activity.",
            ])
            .output()
            .await;

        Ok(session)
    }

    /// Pause the active session
    pub async fn pause_session(&self) -> Result<(), AppError> {
        let now = Utc::now();
        let mut session_guard = self.state.active_session.write().await;
        if let Some(ref mut session) = *session_guard {
            FocusSessionStateMachine::transition_pause(session, now)?;
            self.state.session_repo.update_session(session)?;
            self.state.observer.pause().await;
            self.state.coach_engine.set_idle_state(true, now);
            self.update_companion_mood(CompanionEvent::Idle);
        }
        Ok(())
    }

    /// Resume the active session
    pub async fn resume_session(&self) -> Result<(), AppError> {
        let now = Utc::now();
        let mut session_guard = self.state.active_session.write().await;
        if let Some(ref mut session) = *session_guard {
            FocusSessionStateMachine::transition_resume(
                session,
                session.consent.monitoring_enabled,
                now,
            )?;
            self.state.session_repo.update_session(session)?;
            self.state.observer.resume().await?;
            self.state.coach_engine.set_idle_state(false, now);
            self.state.coach_engine.set_break_state(false, now);
            self.update_companion_mood(CompanionEvent::WorkResumed);
        }
        Ok(())
    }

    /// Start a planned break
    pub async fn start_break(&self) -> Result<(), AppError> {
        let now = Utc::now();
        let mut session_guard = self.state.active_session.write().await;
        if let Some(ref mut session) = *session_guard {
            FocusSessionStateMachine::transition_break(session, now)?;
            self.state.session_repo.update_session(session)?;
            self.state.observer.pause().await;
            self.state.coach_engine.set_break_state(true, now);
            self.update_companion_mood(CompanionEvent::BreakStarted);
        }
        Ok(())
    }

    /// End a planned break and resume focus
    pub async fn end_break(&self) -> Result<(), AppError> {
        let now = Utc::now();
        let mut session_guard = self.state.active_session.write().await;
        if let Some(ref mut session) = *session_guard {
            FocusSessionStateMachine::transition_end_break(session, now)?;
            self.state.session_repo.update_session(session)?;
            self.state.observer.resume().await?;
            self.state.coach_engine.set_break_state(false, now);
            self.update_companion_mood(CompanionEvent::WorkResumed);
        }
        Ok(())
    }

    /// Core observation and evaluation tick.
    /// Runs periodically during an active focus session.
    pub async fn tick(&self) -> Result<Option<ActiveInterventionInfo>, AppError> {
        let now = Utc::now();
        let (session, commitment, goal) = {
            let s = self.state.active_session.read().await.clone();
            let c = self.state.active_commitment.read().await.clone();
            let g = self.state.active_goal.read().await.clone();
            (s, c, g)
        };

        let session = match session {
            Some(s) if s.state.allows_observation() => s,
            _ => return Ok(None),
        };

        // Sweep any expired blocks
        let _ = self.state.enforcement.sweep_expired(now).await;

        // 1. Sample real desktop activity from observer
        let obs = match self
            .state
            .observer
            .sample(session.id, &session.consent, now)
            .await
        {
            Ok(o) => o,
            Err(_e) => {
                // Graceful degradation when sampling temporarily fails
                return Ok(None);
            }
        };

        // Persist observation
        let _ = self.state.activity_repo.record_observation(&obs);
        *self.state.latest_observation.write().await = Some(obs.clone());

        // 2. Build typed untrusted observation for coach engine
        let untrusted_obs = UntrustedObservation::new(
            obs.app_id.clone(),
            obs.window_title.clone(),
            obs.origin.clone(),
            Some(obs.idle_state),
        )?;

        // 3. Build coach context
        let goal_title = goal
            .as_ref()
            .map_or_else(|| "General Focus".to_string(), |g| g.title.clone());
        let next_action = commitment
            .as_ref()
            .map(|c| format!("Work for {} minutes", c.duration_seconds / 60));

        let settings = self.state.settings.read().await.clone();
        let tone = settings.tone_preferences();

        let elapsed_seconds = session.accumulated_focus_seconds as u32
            + if session.state == SessionState::Active {
                (now - session.last_state_change).num_seconds().max(0) as u32
            } else {
                0
            };

        // Load past user corrections for this session and persistent rules
        let mut all_corrections = self
            .state
            .activity_repo
            .get_session_corrections(session.id)
            .unwrap_or_default();
        if let Ok(persistent) = self.state.activity_repo.get_persistent_corrections() {
            all_corrections.extend(persistent);
        }
        let user_corrections: Vec<UserCorrectionSummary> = all_corrections
            .iter()
            .map(UserCorrectionSummary::from)
            .collect();

        // Synchronize with coach engine cooldown tracker
        self.state
            .coach_engine
            .sync_corrections(now, &user_corrections);

        let context = CoachContext::new(
            goal_title,
            next_action,
            session.id,
            elapsed_seconds,
            session.state.is_terminal(),
            untrusted_obs,
            user_corrections,
            tone,
        )?;

        // 4. Evaluate using CoachEngine
        let response = self.state.coach_engine.evaluate(&context, now).await;
        *self.state.latest_coach_response.write().await = Some(response.clone());

        // 5. Evaluate response for intervention
        let mut consecutive = self.state.consecutive_distractions.write().await;
        if response.relevance == RelevanceClassification::PossiblyUnrelated {
            *consecutive += 1;
            if *consecutive >= settings.distraction_threshold_samples {
                // Distraction threshold met! Trigger intervention
                self.update_companion_mood(CompanionEvent::DistractionDetected);

                let rationale = response.rationale.clone();
                let question = response.question.clone();

                // Desktop notification fallback
                let notification_body = format!(
                    "{}\n{}",
                    rationale,
                    question.as_deref().unwrap_or("Ready to refocus?")
                );
                let _ = tokio::process::Command::new("notify-send")
                    .args(["-a", "Wowie", "Focus Check", &notification_body])
                    .output()
                    .await;

                // Check if temporary block proposal should be offered
                let mut proposed_token = None;
                if let Some(ref origin) = obs.origin {
                    let target = EnforcementTarget::Domain(origin.clone());
                    if !EnforcementPolicyEngine::is_essential_tool(&target) {
                        if let Ok(prop) = self.state.enforcement.propose_block(
                            target, 1800, // 30 minutes
                            &rationale, now,
                        ) {
                            proposed_token = Some(prop.token_id);
                        }
                    }
                }

                let info = ActiveInterventionInfo {
                    rationale,
                    question,
                    action_candidate: response.action_candidate,
                    proposed_block_token: proposed_token,
                };

                *self.state.active_intervention.write().await = Some(info.clone());
                return Ok(Some(info));
            }
        } else {
            *consecutive = 0;
            // Clear intervention if user is back on track
            let mut active_int = self.state.active_intervention.write().await;
            if active_int.is_some() {
                *active_int = None;
                self.update_companion_mood(CompanionEvent::WorkResumed);
            }
        }

        Ok(None)
    }

    /// User choice when responding to a coaching intervention.
    pub async fn resolve_intervention(
        &self,
        choice: UserInterventionChoice,
    ) -> Result<(), AppError> {
        let now = Utc::now();
        let intervention = self.state.active_intervention.write().await.take();

        match choice {
            UserInterventionChoice::Resume => {
                *self.state.consecutive_distractions.write().await = 0;
                self.update_companion_mood(CompanionEvent::WorkResumed);
            }
            UserInterventionChoice::TakeBreak => {
                *self.state.consecutive_distractions.write().await = 0;
                self.start_break().await?;
            }
            UserInterventionChoice::Correct { scope, label } => {
                *self.state.consecutive_distractions.write().await = 0;
                self.update_companion_mood(CompanionEvent::WorkResumed);

                if let Some(session) = self.state.active_session.read().await.as_ref() {
                    let latest_obs = self.state.latest_observation.read().await.clone();
                    let (app_id, _window_title, origin) = latest_obs
                        .map_or((None, None, None), |o| (o.app_id, o.window_title, o.origin));

                    let app_id = if app_id.is_none() && origin.is_none() {
                        Some("active_activity".to_string())
                    } else {
                        app_id
                    };

                    let correction = UserCorrection::new(
                        session.id,
                        None,
                        app_id.clone(),
                        origin.clone(),
                        label,
                        scope,
                        Some("User manual classification correction".to_string()),
                        now,
                    )?;

                    let _ = self.state.activity_repo.record_correction(&correction);
                    self.state
                        .coach_engine
                        .record_targeted_correction(now, scope, app_id, origin);
                }
            }
            UserInterventionChoice::Snooze { minutes } => {
                *self.state.consecutive_distractions.write().await = 0;
                self.update_companion_mood(CompanionEvent::SnoozeStarted);
                let _ = self.state.coach_engine.record_snooze(now, minutes * 60);
            }
            UserInterventionChoice::AuthorizeBlock => {
                if let Some(info) = intervention {
                    if let Some(token) = info.proposed_block_token {
                        let _ = self.state.enforcement.confirm_block(token, now).await;
                    }
                }
                *self.state.consecutive_distractions.write().await = 0;
                self.update_companion_mood(CompanionEvent::WorkResumed);
            }
        }

        Ok(())
    }

    /// Complete the focus session and record a review
    pub async fn complete_session(
        &self,
        end_reason: SessionEndReason,
        review_notes: Option<&str>,
        _rating: Option<u8>,
    ) -> Result<SessionReview, AppError> {
        let now = Utc::now();

        // 1. Transition and persist session
        let session = {
            let mut session_guard = self.state.active_session.write().await;
            let mut session = session_guard
                .take()
                .ok_or_else(|| AppError::General("No active session to complete".to_string()))?;

            FocusSessionStateMachine::transition_complete(&mut session, end_reason.clone(), now)?;
            self.state.session_repo.update_session(&session)?;
            session
        };

        // 2. Stop observer
        self.state.observer.stop().await;

        // 3. Clear enforcement blocks
        let _ = self.state.enforcement.stop_all().await;

        // 4. Record session review
        let outcome = match &end_reason {
            SessionEndReason::GoalAchieved | SessionEndReason::TimeExpired => {
                SessionOutcome::CompletedSuccess
            }
            SessionEndReason::UserCancelled { .. } => {
                if session.accumulated_focus_seconds > 0 {
                    SessionOutcome::CompletedPartial
                } else {
                    SessionOutcome::Cancelled
                }
            }
            SessionEndReason::SystemInterrupted { .. } => {
                if session.accumulated_focus_seconds > 0 {
                    SessionOutcome::CompletedPartial
                } else {
                    SessionOutcome::Abandoned
                }
            }
        };

        let review = SessionReview::from_session(
            &session,
            outcome,
            review_notes.map(ToString::to_string),
            None,
            now,
        )?;
        self.state.review_repo.save_review(&review)?;

        // 5. Update companion mood
        self.update_companion_mood(CompanionEvent::GoalCompleted);

        // 6. Notify user
        let _ = tokio::process::Command::new("notify-send")
            .args([
                "-a",
                "Wowie",
                "Session Completed",
                &format!(
                    "Great work! Session finished with {} minutes of focus.",
                    session.accumulated_focus_seconds / 60
                ),
            ])
            .output()
            .await;

        Ok(review)
    }
}
