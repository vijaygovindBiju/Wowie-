use std::path::PathBuf;
use wowie_app::cli::CliRunner;
use wowie_app::config::AppSettings;
use wowie_storage::{GoalRepository, SessionRepository};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map_or("run", String::as_str);

    let config_path = AppSettings::default_path();
    let settings = AppSettings::load_or_default(&config_path);

    let data_dir = std::env::var("XDG_DATA_HOME")
        .map_or_else(
            |_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".local/share")
            },
            PathBuf::from,
        )
        .join("wowie");
    let db_path = data_dir.join("wowie.db");

    let runner = CliRunner::new(Some(db_path), settings)?;

    match command {
        "demo" => {
            runner.run_e2e_workflow().await?;
        }
        "status" => {
            let coordinator = runner.coordinator();
            let state = coordinator.state();
            let session = state.active_session.read().await.clone();
            println!("Wowie Desktop Companion Status:");
            println!("  Active Session: {:?}", session.map(|s| s.id));
            println!("  Companion Mood: {:?}", state.companion.current_mood());
            println!("  Observer Platform: {}", state.observer.platform_name());
        }
        "character" => {
            let sub = args.get(2).map_or("list", String::as_str);
            if sub == "import" {
                if let Some(file_str) = args.get(3) {
                    let name = args.get(4).map_or("Custom Character", String::as_str);
                    runner.import_character(std::path::Path::new(file_str), name)?;
                } else {
                    eprintln!("Usage: wowie character import <path.vrm> [display_name]");
                }
            } else if sub == "select" {
                if let Some(id_str) = args.get(3) {
                    if let Ok(id) = id_str.parse() {
                        let registry = runner.coordinator().state().avatar_registry.clone();
                        registry.set_active_character(id)?;
                        if let Some(char) = registry.get_character(id) {
                            println!(
                                "✓ Set active 3D character to: {} ({})",
                                char.display_name, char.id
                            );
                        }
                    } else {
                        eprintln!("Invalid character ID format");
                    }
                } else {
                    eprintln!("Usage: wowie character select <id>");
                }
            } else if sub == "3d-info" {
                let registry = runner.coordinator().state().avatar_registry.clone();
                let model = registry.load_active_character_model()?;
                if let Some(m) = model {
                    println!("Active 3D Character Model:");
                    println!("  Title: {}", m.metadata.title);
                    println!("  Author: {:?}", m.metadata.author);
                    println!("  Format: {:?}", m.metadata.format);
                    println!("  Meshes: {}", m.meshes.len());
                    println!("  Total Vertices: {}", m.total_vertex_count);
                    println!("  Total Triangles: {}", m.total_triangle_count);
                    println!("  Bounding Box Extent: {:.2}", m.bounds.max_extent);
                    println!("  Expressions: {:?}", m.available_expressions);
                } else {
                    println!("No active 3D character model found.");
                }
            } else {
                let chars = runner
                    .coordinator()
                    .state()
                    .avatar_registry
                    .list_characters();
                println!("Imported Characters ({}):", chars.len());
                for c in chars {
                    println!(
                        "  • {} (Format: {:?}, ID: {})",
                        c.display_name, c.metadata.format, c.id
                    );
                }
            }
        }
        "overlay" => {
            let registry = runner.coordinator().state().avatar_registry.clone();
            let active_profile = registry.seed_default_character_if_empty()?;
            println!("Active 3D Companion: {}", active_profile.display_name);

            let model = match registry.load_active_character_model() {
                Ok(Some(m)) if !m.is_empty() => m,
                Ok(_) | Err(_) => {
                    eprintln!("Warning: Active 3D model could not be loaded or is empty. Falling back to default WowieBot 3D model.");
                    let sample_bytes = wowie_avatar::create_sample_vrm1_bytes();
                    wowie_avatar::validator::ModelValidator::default()
                        .load_3d_model(&sample_bytes)?
                }
            };

            println!(
                "Loaded 3D Character Model '{}' ({} meshes, {} vertices, {} triangles)",
                model.metadata.title,
                model.meshes.len(),
                model.total_vertex_count,
                model.total_triangle_count
            );

            if !wowie_overlay::DesktopOverlay::is_display_available() {
                eprintln!("Warning: No Wayland/X11 display server detected. Falling back to terminal companion.");
                println!("\n=======================================================");
                println!("           WOWIE TERMINAL COMPANION FALLBACK           ");
                println!("=======================================================");
                let current_mood = runner.coordinator().state().companion.current_mood();
                println!("Status: Active (Mood: {:?})", current_mood);
                println!(
                    "{}",
                    runner.coordinator().state().companion.render_ascii_art()
                );
                println!("Run 'wowie' for the full interactive companion terminal interface.");
                return Ok(());
            }

            println!("Starting 3D Companion Overlay... (Press [Q] or [Esc] to close)");
            let (bridge, bridge_rx) = wowie_overlay::OverlayBridge::new();
            let initial_mood = runner.coordinator().state().companion.current_mood();
            bridge.set_mood(initial_mood);
            *runner.coordinator().state().overlay_bridge.lock().unwrap() = Some(bridge);

            let config = wowie_overlay::OverlayConfig::default();
            let overlay =
                wowie_overlay::DesktopOverlay::new(config, model, Some(registry), Some(bridge_rx));
            overlay.run()?;
        }
        "goal" => {
            let sub = args.get(2).map_or("list", String::as_str);
            if sub == "create" {
                let title = args.get(3).map_or("Master Rust", String::as_str);
                let desc = args.get(4).map(String::as_str);
                let goal = runner.coordinator().create_goal(title, desc)?;
                println!("✓ Created Goal: {} (ID: {})", goal.title, goal.id);
                let commitment = runner.coordinator().create_commitment(goal.id, 45, 10)?;
                println!(
                    "✓ Created Default Commitment: {} mins (ID: {})",
                    commitment.duration_seconds / 60,
                    commitment.id
                );
            } else {
                let goals = runner
                    .coordinator()
                    .state()
                    .goal_repo
                    .list_goals(wowie_storage::GoalFilter::default())?;
                println!("Goals ({}):", goals.len());
                for g in goals {
                    println!("  • {} (Status: {:?}, ID: {})", g.title, g.status, g.id);
                    let commitments = runner
                        .coordinator()
                        .state()
                        .session_repo
                        .list_commitments_for_goal(g.id)?;
                    for c in commitments {
                        println!(
                            "     └─ Commitment: {} mins (ID: {})",
                            c.duration_seconds / 60,
                            c.id
                        );
                    }
                }
            }
        }
        "session" => {
            let sub = args.get(2).map_or("status", String::as_str);
            match sub {
                "start" => {
                    let goals = runner
                        .coordinator()
                        .state()
                        .goal_repo
                        .list_goals(wowie_storage::GoalFilter::default())?;
                    let goal_id = if let Some(g) = goals.first() {
                        g.id
                    } else {
                        let g = runner
                            .coordinator()
                            .create_goal("Deep Work Session", None)?;
                        g.id
                    };
                    let commitments = runner
                        .coordinator()
                        .state()
                        .session_repo
                        .list_commitments_for_goal(goal_id)?;
                    let commitment_id = if let Some(c) = commitments.first() {
                        c.id
                    } else {
                        let c = runner.coordinator().create_commitment(goal_id, 45, 10)?;
                        c.id
                    };
                    let session = runner
                        .coordinator()
                        .start_focus_session(commitment_id)
                        .await?;
                    println!("✓ Focus session started! (ID: {})", session.id);
                }
                "pause" => {
                    runner.coordinator().pause_session().await?;
                    println!("✓ Focus session paused.");
                }
                "resume" => {
                    runner.coordinator().resume_session().await?;
                    println!("✓ Focus session resumed.");
                }
                "break" => {
                    runner.coordinator().start_break().await?;
                    println!("✓ Break started.");
                }
                "complete" => {
                    let notes = args.get(3).map(String::as_str);
                    let review = runner
                        .coordinator()
                        .complete_session(
                            wowie_core::models::SessionEndReason::GoalAchieved,
                            notes,
                            Some(5),
                        )
                        .await?;
                    println!("✓ Focus session completed! Outcome: {:?}", review.outcome);
                }
                _ => {
                    let state = runner.coordinator().state();
                    let session = state.active_session.read().await.clone();
                    println!("Session Status: {:?}", session.map(|s| s.state));
                }
            }
        }
        "tick" => {
            if let Some(int) = runner.coordinator().tick().await? {
                println!("! Active Intervention: {}", int.rationale);
            } else {
                println!("✓ Tick evaluated. Activity on track or session idle.");
            }
        }
        _ => {
            // Default: run interactive companion interface
            runner.run_interactive().await?;
        }
    }

    Ok(())
}
