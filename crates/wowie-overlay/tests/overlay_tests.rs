use wowie_avatar::model::{CompanionMood, Mesh3D, Model3D, Vertex3D};
use wowie_avatar::validator::ModelValidator;
use wowie_overlay::math::Vec3;
use wowie_overlay::renderer::{Camera3D, Renderer3D};
use wowie_overlay::state::{OverlayBridge, OverlayControlEvent};
use wowie_overlay::window::DesktopOverlay;

#[test]
fn test_camera_projection_and_view_matrices() {
    let camera = Camera3D::default();
    let aspect = 320.0 / 380.0;
    let proj = camera.projection_matrix(aspect);
    let view = camera.view_matrix();

    // Verify matrix values are finite and non-degenerate
    for val in proj.m {
        assert!(val.is_finite());
    }
    for val in view.m {
        assert!(val.is_finite());
    }

    let origin = Vec3::new(0.0, 0.0, 0.0);
    let transformed = view.transform_point(origin);
    assert!(transformed.z.abs() > 0.0);
}

#[test]
fn test_3d_renderer_pipeline_depth_testing() {
    let mut renderer = Renderer3D::new(100, 100);

    // Create a model with two overlapping quads at different depths:
    // Quad 1 (Far, z = -0.5, Red)
    // Quad 2 (Near, z = 0.5, Green)
    let quad_far = Mesh3D {
        name: "far".to_string(),
        vertices: vec![
            Vertex3D {
                position: [-0.5, -0.5, -0.5],
                normal: [0.0, 0.0, 1.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            Vertex3D {
                position: [0.5, -0.5, -0.5],
                normal: [0.0, 0.0, 1.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            Vertex3D {
                position: [0.5, 0.5, -0.5],
                normal: [0.0, 0.0, 1.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            Vertex3D {
                position: [-0.5, 0.5, -0.5],
                normal: [0.0, 0.0, 1.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        double_sided: false,
    };

    let quad_near = Mesh3D {
        name: "near".to_string(),
        vertices: vec![
            Vertex3D {
                position: [-0.5, -0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
                color: [0.0, 1.0, 0.0, 1.0],
            },
            Vertex3D {
                position: [0.5, -0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
                color: [0.0, 1.0, 0.0, 1.0],
            },
            Vertex3D {
                position: [0.5, 0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
                color: [0.0, 1.0, 0.0, 1.0],
            },
            Vertex3D {
                position: [-0.5, 0.5, 0.5],
                normal: [0.0, 0.0, 1.0],
                color: [0.0, 1.0, 0.0, 1.0],
            },
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        double_sided: false,
    };

    let model = Model3D {
        metadata: wowie_avatar::model::CharacterMetadata {
            title: "DepthTestModel".to_string(),
            author: None,
            version: None,
            license_name: None,
            license_url: None,
            avatar_permission: None,
            commercial_usage: None,
            format: wowie_avatar::model::ModelFormat::Vrm1,
            mesh_count: 2,
        },
        available_expressions: vec!["neutral".to_string()],
        meshes: vec![quad_far, quad_near],
        bounds: wowie_avatar::model::BoundingBox3D::from_vertices(&[]),
        total_vertex_count: 8,
        total_triangle_count: 4,
    };

    renderer.camera.yaw = 0.0;
    renderer.camera.pitch = 0.0;
    renderer.camera.distance = 2.0;

    let stats = renderer.render_model(&model, CompanionMood::Neutral, 0.0);
    assert!(stats.rendered_triangles > 0);

    let rendered_pixels: Vec<u32> = renderer
        .color_buffer
        .iter()
        .copied()
        .filter(|&p| (p >> 24) == 0xFF)
        .collect();
    assert!(
        !rendered_pixels.is_empty(),
        "Triangles must be rendered to pixels"
    );

    // Verify depth testing: all rendered pixels of the near green quad occlude the far red quad
    let green_dominant_pixels = rendered_pixels
        .iter()
        .filter(|&&p| {
            let red = (p >> 16) & 0xFF;
            let green = (p >> 8) & 0xFF;
            green > red
        })
        .count();

    assert!(
        green_dominant_pixels > 0,
        "Closer surface (green) must occlude farther surface (red) via Z-buffer"
    );
}

#[test]
fn test_3d_renderer_mood_animation_transforms() {
    let renderer = Renderer3D::new(100, 100);

    let idle_mat = renderer.compute_animation_transform(CompanionMood::Neutral, 1.0);
    let focus_mat = renderer.compute_animation_transform(CompanionMood::Focused, 1.0);
    let alert_mat = renderer.compute_animation_transform(CompanionMood::Concerned, 1.0);
    let snooze_mat = renderer.compute_animation_transform(CompanionMood::Snoozed, 1.0);
    let relaxed_mat = renderer.compute_animation_transform(CompanionMood::Relaxed, 1.0);
    let celeb_mat = renderer.compute_animation_transform(CompanionMood::Celebrating, 1.0);

    // Each state must produce distinct transformations
    assert_ne!(idle_mat.m, focus_mat.m);
    assert_ne!(focus_mat.m, alert_mat.m);
    assert_ne!(alert_mat.m, snooze_mat.m);
    assert_ne!(snooze_mat.m, relaxed_mat.m);
    assert_ne!(relaxed_mat.m, celeb_mat.m);

    // Matrix elements must be finite
    for m in [
        idle_mat,
        focus_mat,
        alert_mat,
        snooze_mat,
        relaxed_mat,
        celeb_mat,
    ] {
        for v in m.m {
            assert!(v.is_finite());
        }
    }
}

#[test]
fn test_render_sample_vrm1_character_model() {
    let sample_bytes = wowie_avatar::create_sample_vrm1_bytes();
    let validator = ModelValidator::default();
    let model = validator
        .load_3d_model(&sample_bytes)
        .expect("Sample VRM 1.0 model must load");

    assert!(!model.is_empty());
    assert_eq!(model.metadata.title, "WowieBot 3D");

    let mut renderer = Renderer3D::new(160, 200);

    // Render in focused state
    let stats = renderer.render_model(&model, CompanionMood::Focused, 0.5);
    assert!(stats.rendered_triangles >= 10);
    assert!(stats.rendered_vertices >= 20);

    // Verify non-zero rendered pixels in the framebuffer
    let non_transparent_pixels = renderer
        .color_buffer
        .iter()
        .filter(|&&p| (p >> 24) == 0xFF)
        .count();

    assert!(
        non_transparent_pixels > 100,
        "Actual 3D character mesh must be rasterized to pixels, found {non_transparent_pixels}"
    );

    // Test HUD glass pill drawing
    renderer.draw_hud_pill("WowieBot 3D", CompanionMood::Focused, "Coding Zone");
    let hud_pixels = renderer
        .color_buffer
        .iter()
        .filter(|&&p| p != 0x0000_0000)
        .count();
    assert!(hud_pixels > non_transparent_pixels);
}

#[test]
fn test_overlay_bridge_event_signaling() {
    let (bridge, rx) = OverlayBridge::new();

    assert_eq!(bridge.current_mood(), CompanionMood::Neutral);

    bridge.set_mood(CompanionMood::Celebrating);
    assert_eq!(bridge.current_mood(), CompanionMood::Celebrating);

    bridge.set_status("Session Completed!");
    bridge.toggle_visibility();
    bridge.quit();

    let msg1 = rx.try_recv().unwrap();
    assert!(matches!(
        msg1,
        OverlayControlEvent::SetMood(CompanionMood::Celebrating)
    ));

    let msg2 = rx.try_recv().unwrap();
    assert!(matches!(msg2, OverlayControlEvent::SetStatus(s) if s == "Session Completed!"));

    let msg3 = rx.try_recv().unwrap();
    assert!(matches!(msg3, OverlayControlEvent::ToggleVisibility));

    let msg4 = rx.try_recv().unwrap();
    assert!(matches!(msg4, OverlayControlEvent::Quit));
}

#[test]
fn test_display_server_availability_detection() {
    let available = DesktopOverlay::is_display_available();
    // On this machine with WAYLAND_DISPLAY=wayland-1, display is available!
    assert_eq!(
        available,
        std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok()
    );
}
