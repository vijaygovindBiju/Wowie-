use serde_json::json;
use std::fs;
use wowie_avatar::companion::CompanionRuntime;
use wowie_avatar::error::AvatarError;
use wowie_avatar::model::{CompanionEvent, CompanionMood, ModelFormat};
use wowie_avatar::registry::CharacterRegistry;
use wowie_avatar::validator::ModelValidator;

fn build_mock_glb(json_payload: &serde_json::Value) -> Vec<u8> {
    let json_bytes = serde_json::to_vec(json_payload).unwrap();
    let json_padding = (4 - (json_bytes.len() % 4)) % 4;
    let padded_json_len = json_bytes.len() + json_padding;

    let total_len = 12 + 8 + padded_json_len;

    let mut glb = Vec::with_capacity(total_len);
    // Header
    glb.extend_from_slice(b"glTF"); // magic
    glb.extend_from_slice(&2u32.to_le_bytes()); // version
    glb.extend_from_slice(&(total_len as u32).to_le_bytes()); // length

    // Chunk 0 (JSON)
    glb.extend_from_slice(&(padded_json_len as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json_bytes);
    glb.extend(std::iter::repeat(b' ').take(json_padding));

    glb
}

#[test]
fn test_vrm1_validation_and_metadata_extraction() {
    let json = json!({
        "asset": { "version": "2.0" },
        "meshes": [{ "primitives": [] }],
        "extensions": {
            "VRMC_vrm": {
                "specVersion": "1.0",
                "meta": {
                    "name": "RoboCoach",
                    "authors": ["Wowie Team"],
                    "version": "1.0.0",
                    "licenseUrl": "https://vrm.dev/licenses/1.0/",
                    "avatarPermission": "onlyAuthor",
                    "commercialUsage": "personalNonCommercial"
                },
                "expressions": {
                    "preset": { "happy": {}, "sad": {}, "relaxed": {} },
                    "custom": { "winking": {}, "celebrating": {} }
                }
            }
        }
    });

    let glb = build_mock_glb(&json);
    let validator = ModelValidator::default();
    let (metadata, expressions) = validator
        .validate_and_extract(&glb)
        .expect("Valid VRM 1.0 should pass validation");

    assert_eq!(metadata.title, "RoboCoach");
    assert_eq!(metadata.author.as_deref(), Some("Wowie Team"));
    assert_eq!(metadata.format, ModelFormat::Vrm1);
    assert_eq!(metadata.mesh_count, 1);
    assert!(expressions.contains(&"happy".to_string()));
    assert!(expressions.contains(&"neutral".to_string()));
    assert!(expressions.contains(&"winking".to_string()));
    assert!(expressions.contains(&"celebrating".to_string()));
}

#[test]
fn test_security_rejection_of_external_uris() {
    let json = json!({
        "asset": { "version": "2.0" },
        "buffers": [
            { "uri": "https://attacker.example.com/payload.bin", "byteLength": 1024 }
        ]
    });

    let glb = build_mock_glb(&json);
    let validator = ModelValidator::default();
    let res = validator.validate_and_extract(&glb);

    match res {
        Err(AvatarError::SecurityViolation(msg)) => {
            assert!(msg.contains("External URI references"));
        }
        other => panic!("Expected SecurityViolation, got {:?}", other),
    }
}

#[test]
fn test_security_rejection_of_path_traversal() {
    let json = json!({
        "asset": { "version": "2.0" },
        "images": [
            { "uri": "../../etc/passwd" }
        ]
    });

    let glb = build_mock_glb(&json);
    let validator = ModelValidator::default();
    let res = validator.validate_and_extract(&glb);

    match res {
        Err(AvatarError::SecurityViolation(msg)) => {
            assert!(msg.contains("prohibited"));
        }
        other => panic!("Expected SecurityViolation, got {:?}", other),
    }
}

#[test]
fn test_security_rejection_of_file_too_large() {
    let validator = ModelValidator::new(1024); // 1KB limit
    let huge_bytes = vec![0u8; 2048];

    let res = validator.validate_and_extract(&huge_bytes);
    match res {
        Err(AvatarError::FileTooLarge {
            actual_bytes,
            limit_bytes,
        }) => {
            assert_eq!(actual_bytes, 2048);
            assert_eq!(limit_bytes, 1024);
        }
        other => panic!("Expected FileTooLarge, got {:?}", other),
    }
}

#[test]
fn test_invalid_magic_bytes_rejection() {
    let invalid_bytes = b"RIFF1234WAVEfmt ";
    let validator = ModelValidator::default();
    let res = validator.validate_and_extract(invalid_bytes);

    assert!(matches!(res, Err(AvatarError::InvalidMagic(_))));
}

#[test]
fn test_character_registry_import_select_and_removal() {
    let temp_dir = std::env::temp_dir().join(format!("wowie_reg_test_{}", uuid::Uuid::new_v4()));
    let registry = CharacterRegistry::new(temp_dir.clone()).expect("Registry creates dir");

    let vrm_json = json!({
        "asset": { "version": "2.0" },
        "extensions": {
            "VRMC_vrm": {
                "specVersion": "1.0",
                "meta": { "name": "Aoi", "authors": ["Artist"] }
            }
        }
    });

    let glb_bytes = build_mock_glb(&vrm_json);
    let model_file = temp_dir.join("sample.vrm");
    fs::write(&model_file, glb_bytes).unwrap();

    // 1. Import
    let profile = registry
        .import_character(&model_file, "My Companion Aoi")
        .expect("Import succeeds");

    assert_eq!(profile.display_name, "My Companion Aoi");
    assert_eq!(registry.list_characters().len(), 1);

    // 2. Active character is automatically set
    let active = registry.get_active_character().expect("Active is set");
    assert_eq!(active.id, profile.id);

    // 3. Remove character
    registry
        .remove_character(profile.id)
        .expect("Removes cleanly");
    assert_eq!(registry.list_characters().len(), 0);
    assert!(registry.get_active_character().is_none());

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_companion_runtime_reaction_mapping() {
    let runtime = CompanionRuntime::default();

    assert_eq!(runtime.current_mood(), CompanionMood::Neutral);
    assert_eq!(
        runtime.handle_event(CompanionEvent::FocusStarted),
        CompanionMood::Focused
    );
    assert_eq!(
        runtime.handle_event(CompanionEvent::DistractionDetected),
        CompanionMood::Concerned
    );
    assert_eq!(
        runtime.handle_event(CompanionEvent::BreakStarted),
        CompanionMood::Relaxed
    );
    assert_eq!(
        runtime.handle_event(CompanionEvent::GoalCompleted),
        CompanionMood::Celebrating
    );

    // ASCII presentation should render without panic
    let art = runtime.render_ascii_art();
    assert!(!art.is_empty());
    assert!(art.contains("Goal Achieved!"));
}

#[test]
fn test_load_sample_vrm1_geometry_and_validation() {
    let sample_bytes = wowie_avatar::create_sample_vrm1_bytes();
    let validator = ModelValidator::default();

    // 1. VRM 1.0 metadata validation
    let (metadata, expressions) = validator
        .validate_and_extract(&sample_bytes)
        .expect("Sample VRM 1.0 must validate");
    assert_eq!(metadata.title, "WowieBot 3D");
    assert_eq!(metadata.author.as_deref(), Some("Wowie Studio"));
    assert_eq!(metadata.format, ModelFormat::Vrm1);
    assert!(expressions.contains(&"neutral".to_string()));
    assert!(expressions.contains(&"happy".to_string()));

    // 2. Real 3D geometry extraction
    let model = validator
        .load_3d_model(&sample_bytes)
        .expect("Sample model 3D geometry must load");
    assert!(!model.is_empty());
    assert_eq!(model.meshes.len(), 1);
    let mesh = &model.meshes[0];
    assert!(mesh.vertices.len() > 100, "Must have > 100 3D vertices");
    assert!(
        mesh.indices.len() > 100,
        "Must have > 100 3D triangle indices"
    );
    assert_eq!(model.total_vertex_count, mesh.vertices.len());
    assert_eq!(model.total_triangle_count, mesh.indices.len() / 3);

    // Verify bounds and normalization
    assert!(model.bounds.max_extent > 0.0);
    for v in &mesh.vertices {
        assert!(v.normal[0].is_finite());
        assert!(v.normal[1].is_finite());
        assert!(v.normal[2].is_finite());
        assert!(v.position[0].abs() <= 2.5);
        assert!(v.position[1].abs() <= 2.5);
        assert!(v.position[2].abs() <= 2.5);
    }
}

#[test]
fn test_load_3d_model_rejects_empty_or_corrupt_buffers() {
    let validator = ModelValidator::default();

    // 1. Valid GLB header/JSON but no meshes should fail loading 3D model
    let json = json!({
        "asset": { "version": "2.0" }
    });
    let glb = build_mock_glb(&json);
    let err = validator
        .load_3d_model(&glb)
        .expect_err("Must reject model without meshes");
    assert!(matches!(err, AvatarError::MalformedGlb(_)));

    // 2. Corrupt buffer
    let corrupt_bytes = b"glTF\x02\0\0\0\x20\0\0\0JSON\0\0\0\0";
    assert!(validator.load_3d_model(corrupt_bytes).is_err());
}

#[test]
fn test_companion_runtime_snooze_mapping() {
    let runtime = CompanionRuntime::default();
    assert_eq!(
        runtime.handle_event(CompanionEvent::SnoozeStarted),
        CompanionMood::Snoozed
    );
    let art = runtime.render_ascii_art();
    assert!(art.contains("Snoozed"));
    assert!(art.contains("zZz"));
}
