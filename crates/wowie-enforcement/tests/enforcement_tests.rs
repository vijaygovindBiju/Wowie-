use chrono::{Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;
use wowie_enforcement::adapter::linux::LinuxEnforcementAdapter;
use wowie_enforcement::adapter::mock::MockEnforcementAdapter;
use wowie_enforcement::adapter::EnforcementAdapter;
use wowie_enforcement::error::EnforcementError;
use wowie_enforcement::manager::EnforcementManager;
use wowie_enforcement::model::EnforcementTarget;
use wowie_enforcement::policy::EnforcementPolicyEngine;

#[test]
fn test_essential_tools_protection_invariant() {
    let policy = EnforcementPolicyEngine::new();
    let now = Utc::now();

    // 1. Essential domains cannot be blocked
    let github_target = EnforcementTarget::Domain("github.com".to_string());
    let err = policy
        .propose_block(github_target, 1800, "distraction", now)
        .expect_err("Proposing block on github.com must fail");
    assert!(matches!(err, EnforcementError::EssentialToolProtected(_)));

    let crates_target = EnforcementTarget::Domain("crates.io".to_string());
    assert!(policy
        .propose_block(crates_target, 1800, "distraction", now)
        .is_err());

    // 2. Essential applications cannot be blocked
    let rustc_target = EnforcementTarget::AppClass("rustc".to_string());
    assert!(policy
        .propose_block(rustc_target, 1800, "distraction", now)
        .is_err());

    let code_target = EnforcementTarget::AppClass("code".to_string());
    assert!(policy
        .propose_block(code_target, 1800, "distraction", now)
        .is_err());

    // 3. Distracting targets CAN be proposed
    let yt_target = EnforcementTarget::Domain("youtube.com".to_string());
    assert!(policy
        .propose_block(yt_target, 1800, "distraction", now)
        .is_ok());
}

#[tokio::test]
async fn test_explicit_authorization_and_token_single_use() {
    let policy = Arc::new(EnforcementPolicyEngine::new());
    let adapter = Arc::new(MockEnforcementAdapter::new());
    let manager = EnforcementManager::new(policy, adapter.clone());

    let now = Utc::now();
    let yt_target = EnforcementTarget::Domain("youtube.com".to_string());

    // Propose block (AI suggestion / repeated distraction)
    let proposal = manager
        .propose_block(yt_target.clone(), 900, "Off-task video watching", now)
        .expect("Proposal succeeds");

    // Before user authorization: adapter must NOT have applied block
    assert_eq!(adapter.block_count(), 0);
    assert!(!adapter.is_blocked(&yt_target));

    // Confirm block with single-use token
    let block = manager
        .confirm_block(proposal.token_id, now)
        .await
        .expect("Explicit confirmation succeeds");

    assert_eq!(block.target, yt_target);
    assert_eq!(adapter.block_count(), 1);
    assert!(adapter.is_blocked(&yt_target));

    // Re-using the same token must be rejected!
    let reauth_err = manager.confirm_block(proposal.token_id, now).await.err();
    assert_eq!(reauth_err, Some(EnforcementError::UnauthorizedToken));

    // Invalid token must be rejected
    let fake_token = Uuid::new_v4();
    let fake_err = manager.confirm_block(fake_token, now).await.err();
    assert_eq!(fake_err, Some(EnforcementError::UnauthorizedToken));
}

#[tokio::test]
async fn test_expiration_and_automatic_cleanup() {
    let policy = Arc::new(EnforcementPolicyEngine::new());
    let adapter = Arc::new(MockEnforcementAdapter::new());
    let manager = EnforcementManager::new(policy, adapter.clone());

    let now = Utc::now();
    let target = EnforcementTarget::Domain("reddit.com".to_string());

    // Propose and confirm a 300 second block
    let proposal = manager
        .propose_block(target.clone(), 300, "Social media break", now)
        .unwrap();
    manager.confirm_block(proposal.token_id, now).await.unwrap();

    assert!(adapter.is_blocked(&target));

    // Sweep at +100s: still active
    let expired_mid = manager
        .sweep_expired(now + Duration::seconds(100))
        .await
        .unwrap();
    assert!(expired_mid.is_empty());
    assert!(adapter.is_blocked(&target));

    // Sweep at +350s: expired!
    let expired_late = manager
        .sweep_expired(now + Duration::seconds(350))
        .await
        .unwrap();
    assert_eq!(expired_late.len(), 1);
    assert_eq!(expired_late[0].target, target);
    assert!(!adapter.is_blocked(&target));
    assert_eq!(adapter.block_count(), 0);
}

#[tokio::test]
async fn test_manual_user_stop_control() {
    let policy = Arc::new(EnforcementPolicyEngine::new());
    let adapter = Arc::new(MockEnforcementAdapter::new());
    let manager = EnforcementManager::new(policy, adapter.clone());

    let now = Utc::now();
    let target = EnforcementTarget::Domain("twitch.tv".to_string());

    let proposal = manager
        .propose_block(target.clone(), 600, "Gaming stream", now)
        .unwrap();
    manager.confirm_block(proposal.token_id, now).await.unwrap();

    assert!(adapter.is_blocked(&target));

    // User emergency stop
    manager.stop_all().await.expect("Stop all succeeds");
    assert!(!adapter.is_blocked(&target));
    assert_eq!(adapter.block_count(), 0);
    assert!(manager.list_active_blocks().await.is_empty());
}

#[tokio::test]
async fn test_linux_adapter_file_management_and_drop_cleanup() {
    let temp_file = std::env::temp_dir().join(format!("wowie_test_block_{}.txt", Uuid::new_v4()));
    let target = EnforcementTarget::Domain("distraction.example.com".to_string());

    {
        let adapter = LinuxEnforcementAdapter::new(temp_file.clone());
        adapter.apply_block(&target).await.unwrap();

        assert!(temp_file.exists());
        let content = std::fs::read_to_string(&temp_file).unwrap();
        assert!(content.contains("127.0.0.1 distraction.example.com"));

        // Dropping adapter cleans up file
    }

    assert!(!temp_file.exists(), "Block file must be cleaned up on drop");
}

#[tokio::test]
async fn test_hosts_preservation_and_demarcated_blocks() {
    let hosts_file = std::env::temp_dir().join(format!("mock_etc_hosts_{}", Uuid::new_v4()));
    let original_content = "127.0.0.1 localhost\n::1 localhost\n192.168.1.100 devbox.local\n";
    std::fs::write(&hosts_file, original_content).unwrap();

    let target = EnforcementTarget::Domain("procrastination.com".to_string());

    {
        let adapter = LinuxEnforcementAdapter::new(hosts_file.clone());
        assert!(adapter.is_shared_system_file());

        adapter.apply_block(&target).await.unwrap();

        // Target file exists and has backup
        assert!(hosts_file.exists());
        assert!(adapter.backup_path().exists());

        let content = std::fs::read_to_string(&hosts_file).unwrap();
        // Original entries MUST still be present
        assert!(content.contains("127.0.0.1 localhost"));
        assert!(content.contains("192.168.1.100 devbox.local"));
        // Demarcated section must be present
        assert!(content.contains("BEGIN WOWIE TEMPORARY DISTRACTION BLOCKLIST"));
        assert!(content.contains("127.0.0.1 procrastination.com"));
        assert!(content.contains("::1 procrastination.com"));
        assert!(content.contains("END WOWIE TEMPORARY DISTRACTION BLOCKLIST"));

        // Removing block cleans up demarcated section
        adapter.remove_block(&target).await.unwrap();
        let cleaned = std::fs::read_to_string(&hosts_file).unwrap();
        assert!(!cleaned.contains("procrastination.com"));
        assert!(cleaned.contains("127.0.0.1 localhost"));
        assert!(cleaned.contains("192.168.1.100 devbox.local"));
    }

    // On drop, shared file MUST NOT be deleted
    assert!(hosts_file.exists());
    let final_content = std::fs::read_to_string(&hosts_file).unwrap();
    assert_eq!(final_content.trim(), original_content.trim());

    let _ = std::fs::remove_file(&hosts_file);
}

#[tokio::test]
async fn test_hosts_crash_recovery_and_repeated_starts() {
    let hosts_file = std::env::temp_dir().join(format!("mock_crash_hosts_{}", Uuid::new_v4()));
    let original_content = "127.0.0.1 localhost\n# User comment\n10.0.0.1 db.lan\n";
    // Simulate stale crash state with orphaned Wowie section
    let crashed_state = format!(
        "{original_content}\n# --- BEGIN WOWIE TEMPORARY DISTRACTION BLOCKLIST ---\n127.0.0.1 old-distraction.com\n# --- END WOWIE TEMPORARY DISTRACTION BLOCKLIST ---\n"
    );
    std::fs::write(&hosts_file, crashed_state).unwrap();

    // App restarts after crash
    let adapter = LinuxEnforcementAdapter::new(hosts_file.clone());
    assert!(adapter.is_shared_system_file());

    // File MUST NOT be deleted, and stale block section MUST be cleanly purged!
    assert!(hosts_file.exists());
    let content_after_recovery = std::fs::read_to_string(&hosts_file).unwrap();
    assert!(!content_after_recovery.contains("old-distraction.com"));
    assert!(content_after_recovery.contains("127.0.0.1 localhost"));
    assert!(content_after_recovery.contains("10.0.0.1 db.lan"));

    // Repeated starts must be idempotent
    let adapter2 = LinuxEnforcementAdapter::new(hosts_file.clone());
    assert!(hosts_file.exists());
    let content2 = std::fs::read_to_string(&hosts_file).unwrap();
    assert_eq!(content2.trim(), original_content.trim());

    // Test user manual recovery from backup
    std::fs::write(adapter2.backup_path(), original_content).unwrap();
    adapter2.restore_from_backup().unwrap();
    let restored = std::fs::read_to_string(&hosts_file).unwrap();
    assert_eq!(restored.trim(), original_content.trim());

    let _ = std::fs::remove_file(&hosts_file);
    let _ = std::fs::remove_file(adapter2.backup_path());
}

#[tokio::test]
async fn test_empty_or_comment_only_hosts_file_is_never_deleted() {
    let hosts_file = std::env::temp_dir().join(format!("hosts_{}", Uuid::new_v4()));
    // Create an empty hosts file
    std::fs::write(&hosts_file, "# Only a comment\n").unwrap();

    {
        let adapter = LinuxEnforcementAdapter::new(hosts_file.clone());
        assert!(adapter.is_shared_system_file());
        assert!(
            hosts_file.exists(),
            "Empty hosts file must not be deleted on startup"
        );

        let target = EnforcementTarget::Domain("distraction.org".to_string());
        adapter.apply_block(&target).await.unwrap();
        assert!(hosts_file.exists());
    }

    assert!(
        hosts_file.exists(),
        "Empty hosts file must not be deleted on drop"
    );
    let content = std::fs::read_to_string(&hosts_file).unwrap();
    assert_eq!(content.trim(), "# Only a comment");

    let _ = std::fs::remove_file(&hosts_file);
}
