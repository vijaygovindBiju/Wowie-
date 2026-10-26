use std::path::PathBuf;
use wowie_avatar::CharacterRegistry;
use wowie_overlay::{DesktopOverlay, OverlayConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let data_dir = std::env::var("XDG_DATA_HOME")
        .map_or_else(
            |_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".local/share")
            },
            PathBuf::from,
        )
        .join("wowie");

    let avatar_dir = data_dir.join("avatars");
    let registry = CharacterRegistry::new(avatar_dir)?;

    // Ensure default 3D companion character is seeded if registry is empty
    let active_profile = registry.seed_default_character_if_empty()?;
    println!("Active 3D Companion: {}", active_profile.display_name);

    let Some(model) = registry.load_active_character_model()? else {
        eprintln!("Error: No 3D companion model could be loaded.");
        std::process::exit(1);
    };

    println!(
        "Loaded 3D Character Model '{}' ({} meshes, {} vertices, {} triangles)",
        model.metadata.title,
        model.meshes.len(),
        model.total_vertex_count,
        model.total_triangle_count
    );

    let mut config = OverlayConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--width" => {
                if let Some(val) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                    config.width = val;
                    i += 1;
                }
            }
            "--height" => {
                if let Some(val) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                    config.height = val;
                    i += 1;
                }
            }
            "--fps" => {
                if let Some(val) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                    config.fps_limit = val;
                    i += 1;
                }
            }
            "--prefer-gpu" => {
                config.prefer_gpu = true;
            }
            "--show-hud" => {
                config.show_hud = true;
            }
            "--no-hud" => {
                config.show_hud = false;
            }
            "--roam" => {
                config.free_roam = true;
            }
            "--no-roam" => {
                config.free_roam = false;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: wowie-overlay [--width <u32>] [--height <u32>] [--fps <u32>] [--prefer-gpu] [--show-hud] [--roam|--no-roam]"
                );
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    if !DesktopOverlay::is_display_available() {
        eprintln!("Display server is unavailable (neither WAYLAND_DISPLAY nor DISPLAY is set).");
        eprintln!("Wowie 3D desktop companion overlay cannot start in headless mode.");
        std::process::exit(2);
    }

    println!("Launching Wowie 3D Desktop Overlay on Wayland/Hyprland...");
    println!("Controls:");
    println!("  • [Q] or [Esc]: Quit overlay");
    println!("  • [H]: Toggle hide/show");
    println!("  • [F]: Toggle autonomous screen roaming (float freely)");
    println!("  • [U]: Toggle HUD glass pill");
    println!(
        "  • [M]: Cycle companion state (Idle, Focused, Distraction Alert, Snoozed, Celebration)"
    );
    println!("  • [C]: Cycle active character from registry");
    println!("  • Mouse Drag: Orbit 3D camera | Mouse Wheel: Zoom");

    let overlay = DesktopOverlay::new(config, model, Some(registry), None);
    overlay.run()?;

    Ok(())
}
