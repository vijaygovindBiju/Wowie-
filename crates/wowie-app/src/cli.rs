use crate::config::AppSettings;
use crate::coordinator::{UserInterventionChoice, WowieCoordinator};
use crate::error::AppError;
use crate::state::AppState;
use crate::ui::{Screen, UiRenderer};
use chrono::Utc;
use std::path::{Path, PathBuf};
use wowie_coach::models::context::{CoachContext, UntrustedObservation};
use wowie_coach::models::response::RelevanceClassification;
use wowie_coach::models::tone::TonePreferences;
use wowie_core::models::{ClassificationLabel, CorrectionScope, IdleState, SessionEndReason};
use wowie_storage::{GoalRepository, SessionFilter, SessionRepository};

#[derive(Debug)]
pub struct CliRunner {
    pub coordinator: WowieCoordinator,
}

impl CliRunner {
    pub fn new(db_path: Option<PathBuf>, settings: AppSettings) -> Result<Self, AppError> {
        let state = AppState::new_with_db(db_path, settings)?;
        Ok(Self::from_coordinator(WowieCoordinator::new(state)))
    }

    pub fn from_coordinator(coordinator: WowieCoordinator) -> Self {
        Self { coordinator }
    }

    pub fn coordinator(&self) -> &WowieCoordinator {
        &self.coordinator
    }

    /// Run the interactive desktop interface.
    pub async fn run_interactive(&self) -> Result<(), AppError> {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let mut current_screen = Screen::Onboarding;
        println!(
            "{}",
            UiRenderer::render_screen(current_screen, &self.coordinator).await
        );

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            let cmd = line.trim();
            if cmd.is_empty() {
                continue;
            }

            match cmd {
                "1" => current_screen = Screen::Onboarding,
                "2" => current_screen = Screen::Goals,
                "3" => current_screen = Screen::Session,
                "4" => current_screen = Screen::LiveActivity,
                "5" => current_screen = Screen::Intervention,
                "6" => current_screen = Screen::History,
                "7" => current_screen = Screen::Character,
                "8" => current_screen = Screen::Settings,
                "p" | "pause" => {
                    let _ = self.coordinator.pause_session().await;
                }
                "r" | "resume" => {
                    let _ = self.coordinator.resume_session().await;
                }
                "b" | "break" => {
                    let is_break = {
                        let s = self.coordinator.state().active_session.read().await.clone();
                        s.is_some_and(|s| s.state == wowie_core::models::SessionState::Break)
                    };
                    if is_break {
                        let _ = self.coordinator.end_break().await;
                    } else {
                        let _ = self.coordinator.start_break().await;
                    }
                }
                "c" | "complete" => {
                    let _ = self
                        .coordinator
                        .complete_session(
                            SessionEndReason::GoalAchieved,
                            Some("Session completed via UI"),
                            Some(5),
                        )
                        .await;
                }
                "t" | "tick" => {
                    let _ = self.coordinator.tick().await;
                }
                "q" | "quit" | "exit" => {
                    println!("Exiting Wowie companion. Stay focused!");
                    break;
                }
                other => {
                    if other.starts_with("goal ") {
                        let title = other.trim_start_matches("goal ").trim();
                        let _ = self.coordinator.create_goal(title, None);
                        current_screen = Screen::Goals;
                    } else if other == "start" || other.starts_with("start ") {
                        let goals = self
                            .coordinator
                            .state()
                            .goal_repo
                            .list_goals(wowie_storage::GoalFilter::default())
                            .unwrap_or_default();
                        let goal_id = if let Some(g) = goals.first() {
                            g.id
                        } else {
                            self.coordinator.create_goal("Deep Work Session", None)?.id
                        };
                        let commitments = self
                            .coordinator
                            .state()
                            .session_repo
                            .list_commitments_for_goal(goal_id)
                            .unwrap_or_default();
                        let commitment_id = if let Some(c) = commitments.first() {
                            c.id
                        } else {
                            self.coordinator.create_commitment(goal_id, 45, 10)?.id
                        };
                        let _ = self.coordinator.start_focus_session(commitment_id).await;
                        current_screen = Screen::Session;
                    } else if current_screen == Screen::Intervention {
                        match other {
                            "1" | "resume" => {
                                let _ = self
                                    .coordinator
                                    .resolve_intervention(UserInterventionChoice::Resume)
                                    .await;
                            }
                            "2" | "break" => {
                                let _ = self
                                    .coordinator
                                    .resolve_intervention(UserInterventionChoice::TakeBreak)
                                    .await;
                            }
                            "3" | "correct" => {
                                let _ = self
                                    .coordinator
                                    .resolve_intervention(UserInterventionChoice::Correct {
                                        scope: CorrectionScope::Session,
                                        label: ClassificationLabel::Relevant,
                                    })
                                    .await;
                            }
                            "4" | "snooze" => {
                                let _ = self
                                    .coordinator
                                    .resolve_intervention(UserInterventionChoice::Snooze {
                                        minutes: 10,
                                    })
                                    .await;
                            }
                            "5" | "block" => {
                                let _ = self
                                    .coordinator
                                    .resolve_intervention(UserInterventionChoice::AuthorizeBlock)
                                    .await;
                            }
                            _ => {}
                        }
                    }
                }
            }

            println!(
                "{}",
                UiRenderer::render_screen(current_screen, &self.coordinator).await
            );
        }

        Ok(())
    }

    /// Execute the complete real end-to-end workflow on Arch Linux!
    pub async fn run_e2e_workflow(&self) -> Result<(), AppError> {
        println!("=================================================================");
        println!("           WOWIE - END-TO-END WORKFLOW (ARCH LINUX)              ");
        println!("=================================================================");

        // 1. User launches Wowie and creates a goal
        println!("\n[Step 1] Creating Goal: 'Become a software engineer'");
        let goal = self.coordinator.create_goal(
            "Become a software engineer",
            Some("Master systems programming in Rust"),
        )?;
        println!("✓ Goal created with ID: {}", goal.id);

        // 2. User creates a concrete commitment
        println!("\n[Step 2] Creating Commitment: 45 minutes of Rust project work");
        let commitment = self.coordinator.create_commitment(goal.id, 45, 10)?;
        println!(
            "✓ Commitment created: {} mins duration (ID: {})",
            commitment.duration_seconds / 60,
            commitment.id
        );

        // Clean up any stale active session left over from prior runs
        if let Ok(Some(_)) = self.coordinator.state().session_repo.get_active_session() {
            let _ = self
                .coordinator
                .complete_session(
                    SessionEndReason::UserCancelled {
                        reason: Some("Reset for demo run".to_string()),
                    },
                    None,
                    None,
                )
                .await;
        }

        // 3. User starts a focus session
        println!("\n[Step 3] Starting Focus Session with activity monitoring enabled");
        let session = self.coordinator.start_focus_session(commitment.id).await?;
        println!("✓ Focus session started (State: {:?})", session.state);
        println!(
            "✓ Companion reaction: {}",
            self.coordinator.state().companion.companion_name()
        );
        println!("{}", self.coordinator.state().companion.render_ascii_art());

        // 4. Observe real permitted desktop activity from platform (Hyprland / Arch Linux)
        println!("\n[Step 4] Observing real permitted desktop activity on Linux");
        println!(
            "Observer platform detected: {}",
            self.coordinator.state().observer.platform_name()
        );
        let _ = self.coordinator.tick().await?;

        if let Some(obs) = self
            .coordinator
            .state()
            .latest_observation
            .read()
            .await
            .as_ref()
        {
            println!("✓ Live Window Sampled:");
            println!("  - Application: {:?}", obs.app_id);
            println!("  - Window Title: {:?}", obs.window_title);
            println!("  - Browser Origin: {:?}", obs.origin);
            println!("  - Idle State: {:?}", obs.idle_state);
        } else {
            println!("  (No foreground window active, observer returned idle/empty)");
        }

        // 5. Evaluate productive coding activity against active commitment
        println!("\n[Step 5] Evaluating productive coding activity");
        let coding_obs = UntrustedObservation::new(
            Some("code".to_string()),
            Some("src/main.rs - Wowie - Visual Studio Code".to_string()),
            None,
            Some(IdleState::Active),
        )?;
        let t_step5 = Utc::now() + chrono::Duration::seconds(600);
        let coding_ctx = CoachContext::new(
            goal.title.clone(),
            Some("Work on Rust project for 45 minutes".to_string()),
            session.id,
            600,
            false,
            coding_obs,
            vec![],
            TonePreferences::default(),
        )?;
        let coding_resp = self
            .coordinator
            .state()
            .coach_engine
            .evaluate(&coding_ctx, t_step5)
            .await;
        println!("✓ Relevance: {:?}", coding_resp.relevance);
        println!("✓ Confidence: {:.2}", coding_resp.confidence);
        println!("✓ Rationale: {}", coding_resp.rationale);
        assert_eq!(coding_resp.relevance, RelevanceClassification::Relevant);

        // 6. Evaluate off-task distraction (e.g. YouTube entertainment during coding commitment)
        println!(
            "\n[Step 6] Detecting off-task distraction and generating contextual intervention"
        );
        let distracted_obs = UntrustedObservation::new(
            Some("brave-browser".to_string()),
            Some("Epic Gaming Fails Compilation - YouTube - Brave".to_string()),
            Some("youtube.com".to_string()),
            Some(IdleState::Active),
        )?;
        let t_step6 = t_step5 + chrono::Duration::seconds(600);
        let distracted_ctx = CoachContext::new(
            goal.title.clone(),
            Some("Work on Rust project for 45 minutes".to_string()),
            session.id,
            1200,
            false,
            distracted_obs,
            vec![],
            TonePreferences::default(),
        )?;
        let distracted_resp = self
            .coordinator
            .state()
            .coach_engine
            .evaluate(&distracted_ctx, t_step6)
            .await;
        println!("✓ Relevance: {:?}", distracted_resp.relevance);
        println!("✓ Rationale: {}", distracted_resp.rationale);
        println!("✓ Question: {:?}", distracted_resp.question);
        println!("✓ Proposed Action: {:?}", distracted_resp.action_candidate);
        assert_eq!(
            distracted_resp.relevance,
            RelevanceClassification::PossiblyUnrelated
        );

        // 7. Companion enters concerned state and delivers visible intervention
        println!("\n[Step 7] Companion delivers visible intervention alert");
        self.coordinator
            .state()
            .companion
            .handle_event(wowie_avatar::model::CompanionEvent::DistractionDetected);
        println!("{}", self.coordinator.state().companion.render_ascii_art());

        // 8. User choice options (Resume, Break, Correct, Snooze, Block)
        println!("\n[Step 8] User resolves intervention: Correct classification as valid research");
        self.coordinator
            .resolve_intervention(UserInterventionChoice::Correct {
                scope: CorrectionScope::Session,
                label: ClassificationLabel::Relevant,
            })
            .await?;
        println!("✓ User correction recorded in database and cooldown tracker.");
        println!("✓ Companion mood returned to Focused.");
        println!("{}", self.coordinator.state().companion.render_ascii_art());

        // 9. Complete focus session
        println!("\n[Step 9] Completing focus session and recording review");
        let review = self
            .coordinator
            .complete_session(
                SessionEndReason::GoalAchieved,
                Some("Finished core architectural integration of Wowie companion!"),
                Some(5),
            )
            .await?;
        println!("✓ Focus session ended successfully!");
        println!(
            "✓ Session review recorded (Outcome: {:?}, Notes: {:?})",
            review.outcome, review.user_notes
        );
        println!("✓ Companion celebration reaction:");
        println!("{}", self.coordinator.state().companion.render_ascii_art());

        // 10. Review history
        println!("\n[Step 10] Reviewing persisted session history");
        let sessions = self
            .coordinator
            .state()
            .session_repo
            .list_sessions(SessionFilter::default())?;
        println!("✓ Persisted sessions in database: {}", sessions.len());
        for s in sessions {
            println!(
                "  - ID: {} | State: {:?} | Focus: {} mins",
                s.id,
                s.state,
                s.accumulated_focus_seconds / 60
            );
        }

        println!("\n=================================================================");
        println!("✓ Complete End-to-End Workflow successfully verified on Arch Linux!");
        println!("=================================================================\n");

        Ok(())
    }

    /// Import custom 3D model
    pub fn import_character(&self, path: &Path, display_name: &str) -> Result<(), AppError> {
        let profile = self
            .coordinator
            .state()
            .avatar_registry
            .import_character(path, display_name)?;
        println!("Successfully imported character: {}", profile.display_name);
        println!("Format: {:?}", profile.metadata.format);
        println!("License: {:?}", profile.metadata.license_name);
        println!("Expressions: {:?}", profile.available_expressions);
        Ok(())
    }
}
