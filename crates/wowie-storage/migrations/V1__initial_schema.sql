-- V1 Initial Schema: Wowie Core Domain & Audit Persistence
-- Strict foreign keys, bounding check constraints, and performance indexes.

-- 1. Schema Migrations Audit Table
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL,
    checksum TEXT NOT NULL
);

-- 2. Ambitions Table (broad user intention)
CREATE TABLE IF NOT EXISTS ambitions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL CHECK(length(title) > 0 AND length(title) <= 256),
    description TEXT CHECK(description IS NULL OR length(description) <= 2048),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    archived_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_ambitions_archived_at ON ambitions(archived_at);

-- 3. Goals Table (concrete near-term objective under ambition)
CREATE TABLE IF NOT EXISTS goals (
    id TEXT PRIMARY KEY,
    ambition_id TEXT NOT NULL REFERENCES ambitions(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK(length(title) > 0 AND length(title) <= 256),
    next_action TEXT CHECK(next_action IS NULL OR (length(next_action) > 0 AND length(next_action) <= 512)),
    status TEXT NOT NULL CHECK(status IN ('active', 'paused', 'completed', 'archived')),
    target_date TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    archived_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_goals_ambition_id ON goals(ambition_id);
CREATE INDEX IF NOT EXISTS idx_goals_status ON goals(status);

-- 4. Commitments Table (time-bounded focus schedule for a goal)
CREATE TABLE IF NOT EXISTS commitments (
    id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    planned_start TEXT NOT NULL,
    duration_seconds INTEGER NOT NULL CHECK(duration_seconds > 0 AND duration_seconds <= 86400),
    break_allowance_seconds INTEGER NOT NULL DEFAULT 0 CHECK(break_allowance_seconds >= 0 AND break_allowance_seconds <= duration_seconds),
    monitoring_profile_id TEXT CHECK(monitoring_profile_id IS NULL OR length(monitoring_profile_id) <= 64),
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_commitments_goal_id ON commitments(goal_id);
CREATE INDEX IF NOT EXISTS idx_commitments_planned_start ON commitments(planned_start);

-- 5. Focus Sessions Table (active lifecycle state machine)
CREATE TABLE IF NOT EXISTS focus_sessions (
    id TEXT PRIMARY KEY,
    commitment_id TEXT NOT NULL REFERENCES commitments(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN ('Ready', 'Active', 'Paused', 'Break', 'Completed', 'Interrupted', 'ready', 'active', 'paused', 'break', 'completed', 'interrupted')),
    started_at TEXT,
    ended_at TEXT,
    end_reason TEXT,
    consent_snapshot TEXT NOT NULL DEFAULT '{}',
    accumulated_focus_seconds INTEGER NOT NULL DEFAULT 0,
    accumulated_break_seconds INTEGER NOT NULL DEFAULT 0,
    accumulated_idle_seconds INTEGER NOT NULL DEFAULT 0,
    last_state_change TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_focus_sessions_commitment_id ON focus_sessions(commitment_id);
CREATE INDEX IF NOT EXISTS idx_focus_sessions_state ON focus_sessions(state);
CREATE INDEX IF NOT EXISTS idx_focus_sessions_started_at ON focus_sessions(started_at);
-- Invariant: At most ONE session may be in an active/in-progress state concurrently
CREATE UNIQUE INDEX IF NOT EXISTS idx_focus_sessions_active_singleton 
    ON focus_sessions((1)) 
    WHERE state IN ('Active', 'Paused', 'Break', 'active', 'paused', 'break');

-- 6. Activity Observations Table (raw desktop observations)
CREATE TABLE IF NOT EXISTS activity_observations (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES focus_sessions(id) ON DELETE CASCADE,
    observed_at TEXT NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('platform_app', 'window_title', 'browser_origin', 'manual')),
    app_id TEXT CHECK(app_id IS NULL OR length(app_id) <= 256),
    window_title TEXT CHECK(window_title IS NULL OR length(window_title) <= 256),
    origin TEXT CHECK(origin IS NULL OR length(origin) <= 512),
    idle_state TEXT NOT NULL CHECK(idle_state IN ('active', 'idle', 'unknown')),
    consent_scope_id TEXT CHECK(consent_scope_id IS NULL OR length(consent_scope_id) <= 64),
    expires_at TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_observations_session_id ON activity_observations(session_id);
CREATE INDEX IF NOT EXISTS idx_observations_observed_at ON activity_observations(observed_at);
CREATE INDEX IF NOT EXISTS idx_observations_session_time ON activity_observations(session_id, observed_at);

-- 7. Activity Segments Table (normalized time intervals)
CREATE TABLE IF NOT EXISTS activity_segments (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES focus_sessions(id) ON DELETE CASCADE,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL CHECK(end_time >= start_time),
    app_id TEXT CHECK(app_id IS NULL OR length(app_id) <= 256),
    origin TEXT CHECK(origin IS NULL OR length(origin) <= 512),
    kind TEXT NOT NULL CHECK(kind IN ('active_work', 'active_focus', 'idle', 'break', 'planned_break', 'ambiguous', 'switching', 'distraction', 'unknown')),
    confidence REAL NOT NULL CHECK(confidence >= 0.0 AND confidence <= 1.0),
    evidence_refs TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_segments_session_id ON activity_segments(session_id);
CREATE INDEX IF NOT EXISTS idx_segments_time_range ON activity_segments(session_id, start_time, end_time);

-- 8. Classifications Table (provenance-preserving categorization)
CREATE TABLE IF NOT EXISTS classifications (
    id TEXT PRIMARY KEY,
    segment_id TEXT NOT NULL REFERENCES activity_segments(id) ON DELETE CASCADE,
    label TEXT NOT NULL CHECK(label IN ('relevant', 'possibly_unrelated', 'distracting', 'neutral', 'unknown', 'unrelated')),
    confidence REAL NOT NULL CHECK(confidence >= 0.0 AND confidence <= 1.0),
    basis TEXT NOT NULL CHECK(basis IN ('deterministic_rule', 'ai_suggestion', 'user_correction', 'default_heuristic')),
    classifier_version TEXT NOT NULL CHECK(length(classifier_version) <= 64),
    rationale TEXT CHECK(rationale IS NULL OR length(rationale) <= 1024),
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_classifications_segment_id ON classifications(segment_id);
CREATE INDEX IF NOT EXISTS idx_classifications_basis ON classifications(basis);

-- 9. User Actions & Corrections Table
CREATE TABLE IF NOT EXISTS user_actions (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES focus_sessions(id) ON DELETE CASCADE,
    action_type TEXT NOT NULL CHECK(action_type IN ('resume', 'correct', 'planned_break', 'snooze', 'dismiss', 'apply_block', 'cancel_session')),
    target_segment_id TEXT REFERENCES activity_segments(id) ON DELETE SET NULL,
    target_app_or_origin TEXT CHECK(target_app_or_origin IS NULL OR length(target_app_or_origin) <= 512),
    scope TEXT CHECK(scope IS NULL OR scope IN ('item', 'session', 'persistent_rule', 'current_event')),
    label TEXT CHECK(label IS NULL OR label IN ('relevant', 'possibly_unrelated', 'unrelated', 'unknown')),
    note TEXT CHECK(note IS NULL OR length(note) <= 2048),
    details TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_user_actions_session_id ON user_actions(session_id);
CREATE INDEX IF NOT EXISTS idx_user_actions_created_at ON user_actions(created_at);

-- 10. Session Reviews Table (summary record upon session completion)
CREATE TABLE IF NOT EXISTS session_reviews (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL UNIQUE REFERENCES focus_sessions(id) ON DELETE CASCADE,
    total_duration_seconds INTEGER NOT NULL CHECK(total_duration_seconds >= 0),
    active_focus_seconds INTEGER NOT NULL CHECK(active_focus_seconds >= 0),
    break_seconds INTEGER NOT NULL CHECK(break_seconds >= 0),
    idle_seconds INTEGER NOT NULL CHECK(idle_seconds >= 0),
    interventions_count INTEGER NOT NULL DEFAULT 0 CHECK(interventions_count >= 0),
    corrections_count INTEGER NOT NULL DEFAULT 0 CHECK(corrections_count >= 0),
    outcome TEXT NOT NULL CHECK(outcome IN ('completed_as_planned', 'partially_completed', 'interrupted', 'abandoned', 'completed_success', 'completed_partial', 'cancelled')),
    user_note TEXT CHECK(user_note IS NULL OR length(user_note) <= 2048),
    next_plan TEXT CHECK(next_plan IS NULL OR length(next_plan) <= 1024),
    summary_payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_session_reviews_session_id ON session_reviews(session_id);
CREATE INDEX IF NOT EXISTS idx_session_reviews_created_at ON session_reviews(created_at);

-- 11. Event History Table (immutable audit log)
CREATE TABLE IF NOT EXISTS event_history (
    id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES focus_sessions(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL CHECK(length(event_type) <= 64),
    entity_type TEXT NOT NULL CHECK(length(entity_type) <= 64),
    entity_id TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}',
    schema_version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_event_history_session_id ON event_history(session_id);
CREATE INDEX IF NOT EXISTS idx_event_history_event_type ON event_history(event_type);
CREATE INDEX IF NOT EXISTS idx_event_history_created_at ON event_history(created_at);
CREATE INDEX IF NOT EXISTS idx_event_history_entity ON event_history(entity_type, entity_id);

-- 12. User Rules Table (allow/distraction/exemption pattern rules)
CREATE TABLE IF NOT EXISTS user_rules (
    id TEXT PRIMARY KEY,
    pattern_type TEXT NOT NULL CHECK(pattern_type IN ('app_id', 'window_title_pattern', 'domain_origin', 'keyword')),
    pattern_value TEXT NOT NULL CHECK(length(pattern_value) > 0 AND length(pattern_value) <= 512),
    relevance TEXT NOT NULL CHECK(relevance IN ('relevant', 'distracting', 'context_dependent', 'exempt')),
    goal_id TEXT REFERENCES goals(id) ON DELETE CASCADE,
    is_enabled INTEGER NOT NULL DEFAULT 1 CHECK(is_enabled IN (0, 1)),
    description TEXT CHECK(description IS NULL OR length(description) <= 512),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_user_rules_pattern ON user_rules(pattern_type, pattern_value);
CREATE INDEX IF NOT EXISTS idx_user_rules_goal_id ON user_rules(goal_id);
CREATE INDEX IF NOT EXISTS idx_user_rules_enabled ON user_rules(is_enabled);

-- 13. Settings Table (key-value configuration store)
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY CHECK(length(key) > 0 AND length(key) <= 128),
    value TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK(version > 0),
    updated_at TEXT NOT NULL
);
