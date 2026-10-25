use crate::coordinator::WowieCoordinator;
use crate::state::AppState;
use std::fmt::Write as _;
use wowie_storage::{
    GoalFilter, GoalRepository, ReviewRepository, SessionFilter, SessionRepository,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Onboarding,
    Goals,
    Session,
    LiveActivity,
    Intervention,
    History,
    Character,
    Settings,
}

impl Screen {
    pub const ALL: [Self; 8] = [
        Self::Onboarding,
        Self::Goals,
        Self::Session,
        Self::LiveActivity,
        Self::Intervention,
        Self::History,
        Self::Character,
        Self::Settings,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Self::Onboarding => "1. Onboarding",
            Self::Goals => "2. Goals",
            Self::Session => "3. Session",
            Self::LiveActivity => "4. Live Activity",
            Self::Intervention => "5. Intervention",
            Self::History => "6. History",
            Self::Character => "7. Character",
            Self::Settings => "8. Settings",
        }
    }
}

#[derive(Debug)]
pub struct UiRenderer;

impl UiRenderer {
    pub async fn render_screen(screen: Screen, coordinator: &WowieCoordinator) -> String {
        let state = coordinator.state();
        let mut out = String::new();

        // 1. Header with Tab Bar
        let _ = writeln!(
            out,
            "╔════════════════════════════════════════════════════════════════════════════╗"
        );
        let _ = writeln!(
            out,
            "║                         WOWIE - AI DESKTOP COMPANION                       ║"
        );
        let _ = writeln!(
            out,
            "╠════════════════════════════════════════════════════════════════════════════╣"
        );

        let mut nav = String::from("║ ");
        for (i, s) in Screen::ALL.iter().enumerate() {
            if *s == screen {
                let _ = write!(nav, "[* {}] ", s.title());
            } else {
                let _ = write!(nav, " {}  ", i + 1);
            }
        }
        nav.push('║');
        let _ = writeln!(out, "{nav}");
        let _ = writeln!(
            out,
            "╠════════════════════════════════════════════════════════════════════════════╣"
        );

        // 2. Screen Content
        match screen {
            Screen::Onboarding => {
                Self::render_onboarding(&mut out);
            }
            Screen::Goals => {
                Self::render_goals(&mut out, state);
            }
            Screen::Session => {
                Self::render_session(&mut out, state).await;
            }
            Screen::LiveActivity => {
                Self::render_live_activity(&mut out, state).await;
            }
            Screen::Intervention => {
                Self::render_intervention(&mut out, state).await;
            }
            Screen::History => {
                Self::render_history(&mut out, state);
            }
            Screen::Character => {
                Self::render_character(&mut out, state);
            }
            Screen::Settings => {
                Self::render_settings(&mut out, state).await;
            }
        }

        // 3. Footer
        let _ = writeln!(
            out,
            "╠════════════════════════════════════════════════════════════════════════════╣"
        );
        let _ = writeln!(
            out,
            "║ [1-8] Switch Screen | [Space] Toggle Pause | [Esc] Back | [Q] Quit        ║"
        );
        let _ = writeln!(
            out,
            "╚════════════════════════════════════════════════════════════════════════════╝"
        );

        out
    }

    fn render_onboarding(out: &mut String) {
        let _ = writeln!(
            out,
            "║ SCREEN 1: FIRST-RUN ONBOARDING                                            ║"
        );
        let _ = writeln!(
            out,
            "║                                                                            ║"
        );
        let _ = writeln!(
            out,
            "║ Welcome to Wowie, your goal-aware AI desktop companion for Linux!          ║"
        );
        let _ = writeln!(
            out,
            "║                                                                            ║"
        );
        let _ = writeln!(
            out,
            "║ Key Privacy & Operating Guarantees:                                        ║"
        );
        let _ = writeln!(
            out,
            "║  • Zero continuous screenshots or audio capture.                           ║"
        );
        let _ = writeln!(
            out,
            "║  • Permission-based desktop activity observation only while focusing.     ║"
        );
        let _ = writeln!(
            out,
            "║  • Zero OS execution authority for AI. All actions require your consent.   ║"
        );
        let _ = writeln!(
            out,
            "║  • Reversible, user-controlled distraction blocks that automatically expire║"
        );
        let _ = writeln!(
            out,
            "║  • Customizable 3D companion characters (VRM 1.0 & glTF 2.0).              ║"
        );
        let _ = writeln!(
            out,
            "║                                                                            ║"
        );
        let _ = writeln!(
            out,
            "║ Switch to [2. Goals] to set your first ambition and start focusing!        ║"
        );
    }

    fn render_goals(out: &mut String, state: &AppState) {
        let _ = writeln!(
            out,
            "║ SCREEN 2: GOAL & COMMITMENT MANAGEMENT                                    ║"
        );
        let goals = state
            .goal_repo
            .list_goals(GoalFilter::default())
            .unwrap_or_default();
        if goals.is_empty() {
            let _ = writeln!(
                out,
                "║ No active goals found. Use 'wowie goal create' or press 'A' to add goal.  ║"
            );
        } else {
            for g in goals {
                let _ = writeln!(out, "║ • Goal: {:<35} (Status: {:?})║", g.title, g.status);
                let commitments = state
                    .session_repo
                    .list_commitments_for_goal(g.id)
                    .unwrap_or_default();
                for c in commitments {
                    let _ = writeln!(
                        out,
                        "║    └─ Commitment: {} mins (ID: {})║",
                        c.duration_seconds / 60,
                        c.id
                    );
                }
            }
        }
    }

    async fn render_session(out: &mut String, state: &AppState) {
        let _ = writeln!(
            out,
            "║ SCREEN 3: FOCUS SESSION CONTROLS                                          ║"
        );
        let session = state.active_session.read().await.clone();
        match session {
            Some(s) => {
                let _ = writeln!(
                    out,
                    "║ Session State: {:<58} ║",
                    format!("{:?}", s.state).to_uppercase()
                );
                let _ = writeln!(
                    out,
                    "║ Monitoring:    {:<58} ║",
                    if s.consent.monitoring_enabled {
                        "ENABLED (Active)"
                    } else {
                        "DISABLED"
                    }
                );
                let now = chrono::Utc::now();
                let focus_elapsed = s.accumulated_focus_seconds
                    + if s.state == wowie_core::models::SessionState::Active {
                        (now - s.last_state_change).num_seconds().max(0) as u64
                    } else {
                        0
                    };
                let break_elapsed = s.accumulated_break_seconds
                    + if s.state == wowie_core::models::SessionState::Break {
                        (now - s.last_state_change).num_seconds().max(0) as u64
                    } else {
                        0
                    };
                let _ = writeln!(
                    out,
                    "║ Focus Elapsed: {:<58} ║",
                    format!(
                        "{focus_elapsed} seconds ({}m {}s)",
                        focus_elapsed / 60,
                        focus_elapsed % 60
                    )
                );
                let _ = writeln!(
                    out,
                    "║ Break Elapsed: {:<58} ║",
                    format!(
                        "{break_elapsed} seconds ({}m {}s)",
                        break_elapsed / 60,
                        break_elapsed % 60
                    )
                );
                let _ = writeln!(
                    out,
                    "║ Controls: [P] Pause | [R] Resume | [B] Take Break | [C] Complete Session  ║"
                );
            }
            None => {
                let _ = writeln!(out, "║ No active focus session. Start one from Goals or run 'wowie session start'.║");
            }
        }
    }

    async fn render_live_activity(out: &mut String, state: &AppState) {
        let _ = writeln!(
            out,
            "║ SCREEN 4: CURRENT ACTIVITY & COACHING STATUS                              ║"
        );
        let obs = state.latest_observation.read().await.clone();
        let coach_resp = state.latest_coach_response.read().await.clone();
        let observer_state = state.observer.current_state().await;

        let _ = writeln!(
            out,
            "║ Observer Platform: {:<54} ║",
            state.observer.platform_name()
        );
        let _ = writeln!(
            out,
            "║ Observer State:    {:<54} ║",
            format!("{:?}", observer_state)
        );

        if let Some(o) = obs {
            let _ = writeln!(
                out,
                "║ Active App:        {:<54} ║",
                o.app_id.unwrap_or_else(|| "none".to_string())
            );
            let _ = writeln!(
                out,
                "║ Window Title:      {:<54} ║",
                o.window_title.unwrap_or_else(|| "none".to_string())
            );
            let _ = writeln!(
                out,
                "║ Browser Origin:    {:<54} ║",
                o.origin.unwrap_or_else(|| "none".to_string())
            );
            let _ = writeln!(
                out,
                "║ Idle State:        {:<54} ║",
                format!("{:?}", o.idle_state)
            );
        } else {
            let _ = writeln!(
                out,
                "║ No desktop activity sampled yet.                                          ║"
            );
        }

        if let Some(cr) = coach_resp {
            let _ = writeln!(
                out,
                "║ Relevance:         {:<54} ║",
                format!("{:?} (conf: {:.2})", cr.relevance, cr.confidence)
            );
            let _ = writeln!(out, "║ Coach Rationale:   {:<54} ║", cr.rationale);
            if let Some(action) = cr.action_candidate {
                let _ = writeln!(
                    out,
                    "║ Proposed Action:   {:<54} ║",
                    format!("{:?}", action)
                );
            }
        }
    }

    async fn render_intervention(out: &mut String, state: &AppState) {
        let _ = writeln!(
            out,
            "║ SCREEN 5: COACHING INTERVENTION (ACTIVE)                                  ║"
        );
        let intervention = state.active_intervention.read().await.clone();
        let ascii_art = state.companion.render_ascii_art();

        for line in ascii_art.lines() {
            if !line.trim().is_empty() {
                let _ = writeln!(out, "║ {:<74} ║", line);
            }
        }

        let _ = writeln!(
            out,
            "║                                                                            ║"
        );
        if let Some(info) = intervention {
            let _ = writeln!(out, "║ Assessment: {:<61} ║", info.rationale);
            if let Some(q) = info.question {
                let _ = writeln!(out, "║ Coach Asks: {:<61} ║", q);
            }
            let _ = writeln!(
                out,
                "║                                                                            ║"
            );
            let _ = writeln!(
                out,
                "║ Choices:                                                                   ║"
            );
            let _ = writeln!(
                out,
                "║  [1] Resume Work         (Acknowledge and refocus)                         ║"
            );
            let _ = writeln!(
                out,
                "║  [2] Take a Break        (Switch to planned rest)                          ║"
            );
            let _ = writeln!(
                out,
                "║  [3] Correct Assessment  (Mark this activity as relevant research)         ║"
            );
            let _ = writeln!(
                out,
                "║  [4] Snooze (10m)        (Suppress coaching reminders)                     ║"
            );
            if info.proposed_block_token.is_some() {
                let _ = writeln!(out, "║  [5] Temporarily Block   (Explicitly authorize 30m distraction block)      ║");
            }
        } else {
            let _ = writeln!(
                out,
                "║ No active intervention. You are currently focused and in the flow!         ║"
            );
            let _ = writeln!(
                out,
                "║                                                                            ║"
            );
        }
    }

    fn render_history(out: &mut String, state: &AppState) {
        let _ = writeln!(
            out,
            "║ SCREEN 6: ACTIVITY & SESSION HISTORY                                      ║"
        );
        let sessions = state
            .session_repo
            .list_sessions(SessionFilter::default())
            .unwrap_or_default();
        if sessions.is_empty() {
            let _ = writeln!(
                out,
                "║ No completed sessions recorded in database.                               ║"
            );
        } else {
            for s in sessions.iter().take(5) {
                let review = state
                    .review_repo
                    .get_review_by_session(s.id)
                    .unwrap_or(None);
                let _ = writeln!(
                    out,
                    "║ • Session: {} | Focus: {}m | Break: {}m | Outcome: {:<12} ║",
                    &s.id.to_string()[..8],
                    s.accumulated_focus_seconds / 60,
                    s.accumulated_break_seconds / 60,
                    review
                        .map_or_else(|| "In-progress".to_string(), |r| format!("{:?}", r.outcome))
                );
            }
        }
    }

    fn render_character(out: &mut String, state: &AppState) {
        let _ = writeln!(
            out,
            "║ SCREEN 7: 3D CHARACTER MANAGEMENT                                         ║"
        );
        let active = state.avatar_registry.get_active_character();
        if let Some(p) = active {
            let _ = writeln!(out, "║ Active Companion:  {:<54} ║", p.display_name);
            let _ = writeln!(
                out,
                "║ Format:            {:<54} ║",
                format!("{:?}", p.metadata.format)
            );
            let _ = writeln!(out, "║ Title:             {:<54} ║", p.metadata.title);
            let _ = writeln!(
                out,
                "║ Author:            {:<54} ║",
                p.metadata.author.unwrap_or_else(|| "Unknown".to_string())
            );
            let _ = writeln!(
                out,
                "║ License:           {:<54} ║",
                p.metadata
                    .license_name
                    .unwrap_or_else(|| "Standard".to_string())
            );
            let _ = writeln!(
                out,
                "║ Expressions:       {:<54} ║",
                format!("{} expressions supported", p.available_expressions.len())
            );
        } else {
            let _ = writeln!(
                out,
                "║ Active Companion:  Wowie (Built-in SVG/ASCII companion)                   ║"
            );
            let _ = writeln!(
                out,
                "║ Import custom VRM: Use 'wowie character import <path.vrm>'                ║"
            );
        }

        let _ = writeln!(
            out,
            "║                                                                            ║"
        );
        let _ = writeln!(
            out,
            "║ Companion Live Reaction Preview:                                           ║"
        );
        let ascii_art = state.companion.render_ascii_art();
        for line in ascii_art.lines() {
            if !line.trim().is_empty() {
                let _ = writeln!(out, "║ {:<74} ║", line);
            }
        }
    }

    async fn render_settings(out: &mut String, state: &AppState) {
        let _ = writeln!(
            out,
            "║ SCREEN 8: AI, PRIVACY & ENFORCEMENT SETTINGS                              ║"
        );
        let s = state.settings.read().await.clone();
        let _ = writeln!(
            out,
            "║ AI Provider:       {:<54} ║",
            s.ai_endpoint
                .unwrap_or_else(|| "Local Offline Heuristic".to_string())
        );
        let _ = writeln!(out, "║ Persona Name:      {:<54} ║", s.persona_name);
        let _ = writeln!(
            out,
            "║ Tone Directness:   {:<54} ║",
            format!("{:.1}", s.directness)
        );
        let _ = writeln!(
            out,
            "║ Tone Warmth:       {:<54} ║",
            format!("{:.1}", s.warmth)
        );
        let _ = writeln!(
            out,
            "║ Window Redaction:  {:<54} ║",
            if s.redact_window_titles {
                "ON (Titles obscured)"
            } else {
                "OFF"
            }
        );
        let _ = writeln!(
            out,
            "║ Browser Origins:   {:<54} ║",
            if s.collect_browser_origins {
                "ON (Domains observed)"
            } else {
                "OFF"
            }
        );
        let _ = writeln!(
            out,
            "║ Distraction Alert: {:<54} ║",
            format!(
                "After {} consecutive off-task samples",
                s.distraction_threshold_samples
            )
        );
    }
}
