use wowie_avatar::model::{BoundingBox3D, CharacterMetadata, CompanionMood, Model3D, ModelFormat};
use wowie_overlay::{
    is_gpu_acceleration_available, query_gpu_device_info, select_rendering_backend, Camera3D,
    GpuAccelerationPipeline, GraphicsBackend, OverlayConfig, Renderer3D,
};

#[test]
fn test_gpu_device_info_query_edge_cases() {
    let info = query_gpu_device_info();
    assert!(
        !info.device_name.is_empty(),
        "device_name must not be empty"
    );
    assert!(
        !info.api_version.is_empty(),
        "api_version must not be empty"
    );
    assert!(
        !info.driver_info.is_empty(),
        "driver_info must not be empty"
    );

    let is_avail = is_gpu_acceleration_available();
    assert_eq!(
        info.is_available, is_avail,
        "is_available in device info must match is_gpu_acceleration_available()"
    );

    if is_avail {
        assert_eq!(info.api_version, "Vulkan 1.3");
        assert_eq!(info.driver_info, "Hardware Accelerated");
    } else {
        assert_eq!(info.api_version, "N/A");
        assert_eq!(info.driver_info, "Pure Rust CPU Rasterizer");
        assert_eq!(info.device_name, "Software Rasterizer Fallback");
    }
}

#[test]
fn test_select_rendering_backend_matrix() {
    // When prefer_gpu is false, backend MUST be Software regardless of hardware
    assert_eq!(
        select_rendering_backend(false),
        GraphicsBackend::Software,
        "prefer_gpu=false must always select Software"
    );

    // When prefer_gpu is true, backend depends strictly on is_gpu_acceleration_available()
    let is_avail = is_gpu_acceleration_available();
    let selected = select_rendering_backend(true);
    if is_avail {
        assert_eq!(selected, GraphicsBackend::VulkanHardware);
    } else {
        assert_eq!(selected, GraphicsBackend::Software);
    }
}

#[test]
fn test_graphics_backend_serde_roundtrip_and_unknown_variants() {
    // 1. Serialization format must match snake_case
    let sw_json = serde_json::to_string(&GraphicsBackend::Software).unwrap();
    assert_eq!(sw_json, "\"software\"");

    let vk_json = serde_json::to_string(&GraphicsBackend::VulkanHardware).unwrap();
    assert_eq!(vk_json, "\"vulkan_hardware\"");

    // 2. Deserialization of valid strings
    let de_sw: GraphicsBackend = serde_json::from_str("\"software\"").unwrap();
    assert_eq!(de_sw, GraphicsBackend::Software);

    let de_vk: GraphicsBackend = serde_json::from_str("\"vulkan_hardware\"").unwrap();
    assert_eq!(de_vk, GraphicsBackend::VulkanHardware);

    // 3. Deserialization of invalid variants must return Err, zero panic
    let invalid_variants = [
        "\"opengl\"",
        "\"directx\"",
        "\"metal\"",
        "\"SOFTWARE\"",
        "\"VulkanHardware\"",
        "\"\"",
        "123",
        "true",
        "null",
        "{}",
    ];

    for raw in invalid_variants {
        let res: Result<GraphicsBackend, _> = serde_json::from_str(raw);
        assert!(
            res.is_err(),
            "Invalid variant {raw} should fail deserialization cleanly"
        );
    }
}

#[test]
fn test_gpu_pipeline_zero_and_extreme_viewport_resizing() {
    // Construct with extreme initial dimensions
    let mut pipeline = GpuAccelerationPipeline::with_backend(GraphicsBackend::VulkanHardware, 0, 0);
    assert_eq!(pipeline.width, 0);
    assert_eq!(pipeline.height, 0);
    assert!(pipeline.is_hardware_accelerated());

    // Resize to (0, 0)
    pipeline.resize(0, 0);
    assert_eq!(pipeline.width, 0);
    assert_eq!(pipeline.height, 0);

    // Resize to u32::MAX
    pipeline.resize(u32::MAX, u32::MAX);
    assert_eq!(pipeline.width, u32::MAX);
    assert_eq!(pipeline.height, u32::MAX);

    // Asymmetric zero dimensions
    pipeline.resize(0, 1920);
    assert_eq!(pipeline.width, 0);
    assert_eq!(pipeline.height, 1920);

    pipeline.resize(1080, 0);
    assert_eq!(pipeline.width, 1080);
    assert_eq!(pipeline.height, 0);

    // High frequency resizing oscillation (stress test)
    for i in 0..1000 {
        let w = if i % 2 == 0 { 0 } else { 3840 };
        let h = if i % 2 == 0 { 0 } else { 2160 };
        pipeline.resize(w, h);
        assert_eq!(pipeline.width, w);
        assert_eq!(pipeline.height, h);
    }
}

#[test]
fn test_overlay_config_serde_adversarial_inputs() {
    // 1. Missing prefer_gpu defaults cleanly to false
    let json_missing_prefer = r#"{
        "width": 320,
        "height": 380,
        "fps_limit": 30,
        "always_on_top": true,
        "enable_hyprland_auto_rules": true,
        "show_hud": true
    }"#;
    let config: OverlayConfig = serde_json::from_str(json_missing_prefer).unwrap();
    assert!(!config.prefer_gpu);
    assert_eq!(config.width, 320);

    // 2. Explicit true and false
    let json_true = r#"{
        "width": 100,
        "height": 100,
        "fps_limit": 60,
        "always_on_top": false,
        "enable_hyprland_auto_rules": false,
        "show_hud": false,
        "prefer_gpu": true
    }"#;
    let config_true: OverlayConfig = serde_json::from_str(json_true).unwrap();
    assert!(config_true.prefer_gpu);

    // 3. Type violations for prefer_gpu must fail cleanly without panics
    let bad_prefer_gpus = [
        r#"{"width":320,"height":380,"fps_limit":30,"always_on_top":true,"enable_hyprland_auto_rules":true,"show_hud":true,"prefer_gpu":"true"}"#,
        r#"{"width":320,"height":380,"fps_limit":30,"always_on_top":true,"enable_hyprland_auto_rules":true,"show_hud":true,"prefer_gpu":1}"#,
        r#"{"width":320,"height":380,"fps_limit":30,"always_on_top":true,"enable_hyprland_auto_rules":true,"show_hud":true,"prefer_gpu":null}"#,
        r#"{"width":320,"height":380,"fps_limit":30,"always_on_top":true,"enable_hyprland_auto_rules":true,"show_hud":true,"prefer_gpu":[true]}"#,
        r#"{"width":320,"height":380,"fps_limit":30,"always_on_top":true,"enable_hyprland_auto_rules":true,"show_hud":true,"prefer_gpu":{"val":true}}"#,
    ];

    for bad_json in bad_prefer_gpus {
        let res: Result<OverlayConfig, _> = serde_json::from_str(bad_json);
        assert!(res.is_err(), "Bad JSON {bad_json} must fail to deserialize");
    }

    // 4. Missing required fields (e.g. width or height) must fail cleanly
    let missing_required = [
        r#"{"prefer_gpu": true}"#,
        "{}",
        r#"{"height": 380, "fps_limit": 30}"#,
    ];
    for bad_json in missing_required {
        let res: Result<OverlayConfig, _> = serde_json::from_str(bad_json);
        assert!(res.is_err(), "Missing required fields {bad_json} must fail");
    }

    // 5. Extreme boundary values for numeric fields
    let extreme_json = r#"{
        "width": 0,
        "height": 0,
        "fps_limit": 0,
        "always_on_top": false,
        "enable_hyprland_auto_rules": false,
        "show_hud": false,
        "prefer_gpu": true
    }"#;
    let extreme_config: OverlayConfig = serde_json::from_str(extreme_json).unwrap();
    assert_eq!(extreme_config.width, 0);
    assert_eq!(extreme_config.height, 0);
    assert_eq!(extreme_config.fps_limit, 0);
    assert!(extreme_config.prefer_gpu);
}

#[test]
fn test_renderer_zero_sized_viewport_and_projection() {
    let mut renderer = Renderer3D::new(0, 0);
    assert_eq!(renderer.width, 0);
    assert_eq!(renderer.height, 0);
    assert_eq!(renderer.color_buffer.len(), 0);
    assert_eq!(renderer.depth_buffer.len(), 0);

    let empty_model = Model3D {
        metadata: CharacterMetadata {
            title: "Empty".to_string(),
            author: None,
            version: None,
            license_name: None,
            license_url: None,
            avatar_permission: None,
            commercial_usage: None,
            format: ModelFormat::Vrm1,
            mesh_count: 0,
        },
        available_expressions: vec![],
        meshes: vec![],
        bounds: BoundingBox3D::from_vertices(&[]),
        total_vertex_count: 0,
        total_triangle_count: 0,
    };

    let stats = renderer.render_model(&empty_model, CompanionMood::Neutral, 0.0);
    assert_eq!(stats.rendered_triangles, 0);
    assert_eq!(stats.rendered_vertices, 0);

    // Resize to zero width, non-zero height
    renderer.resize(0, 400);
    let stats2 = renderer.render_model(&empty_model, CompanionMood::Focused, 1.0);
    assert_eq!(stats2.rendered_triangles, 0);

    // Resize to non-zero width, zero height (potential divide-by-zero aspect ratio danger)
    renderer.resize(400, 0);
    let stats3 = renderer.render_model(&empty_model, CompanionMood::Concerned, 2.0);
    assert_eq!(stats3.rendered_triangles, 0);

    // Camera projection with 0.0 aspect ratio must not panic
    let camera = Camera3D::default();
    let proj_zero = camera.projection_matrix(0.0);
    assert!(!proj_zero.m[0].is_nan());
}
