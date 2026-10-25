use std::io::{self, BufReader, BufWriter};
use wowie_observer::native_messaging::{
    default_origin_cache_path, read_native_message, sanitize_origin, write_cached_origin,
    write_native_message, BrowserMessage, HostResponse,
};

fn print_help() {
    eprintln!(
        "wowie-browser-host {}\n\
         Native Messaging Host for the Wowie Focus Companion browser extension.\n\n\
         USAGE:\n    \
             wowie-browser-host [OPTIONS]\n\n\
         OPTIONS:\n    \
             -h, --help       Print help information\n    \
             -V, --version    Print version information\n    \
             --ping           Verify host binary execution and origin cache\n\n\
         NOTE:\n    \
             This binary communicates with WebExtensions over stdin/stdout using\n    \
             the 4-byte native-endian length-prefixed JSON protocol.\n    \
             Direct interactive execution in a terminal is not recommended without piping.",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("wowie-browser-host {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--ping" => {
                let cache_path = default_origin_cache_path();
                eprintln!(
                    "wowie-browser-host is operational. Cache path: {}",
                    cache_path.display()
                );
                std::process::exit(0);
            }
            _ => {
                // Browsers may pass parent window or extension ID as arguments
                eprintln!("[wowie-browser-host] launched with args: {:?}", &args[1..]);
            }
        }
    }

    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    let cache_path = default_origin_cache_path();

    eprintln!(
        "[wowie-browser-host] Listening for native messaging frames on stdin (cache: {})",
        cache_path.display()
    );

    loop {
        match read_native_message(&mut reader) {
            Ok(Some(raw_bytes)) => {
                let response = match serde_json::from_slice::<BrowserMessage>(&raw_bytes) {
                    Ok(msg) => match sanitize_origin(&msg.origin) {
                        Ok(clean_origin) => {
                            if let Err(e) = write_cached_origin(
                                &cache_path,
                                &clean_origin,
                                msg.is_active_window,
                                msg.title.as_deref(),
                            ) {
                                eprintln!("[wowie-browser-host] Failed to write origin cache: {e}");
                            } else {
                                eprintln!(
                                    "[wowie-browser-host] Updated origin: {} (active: {})",
                                    clean_origin, msg.is_active_window
                                );
                            }

                            HostResponse {
                                status: "ok".to_string(),
                                acknowledged_origin: Some(clean_origin),
                                error: None,
                            }
                        }
                        Err(e) => {
                            eprintln!("[wowie-browser-host] Origin validation rejected: {e}");
                            HostResponse {
                                status: "error".to_string(),
                                acknowledged_origin: None,
                                error: Some(e.to_string()),
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("[wowie-browser-host] JSON parse error: {e}");
                        HostResponse {
                            status: "error".to_string(),
                            acknowledged_origin: None,
                            error: Some(format!("Invalid JSON: {e}")),
                        }
                    }
                };

                let resp_bytes = match serde_json::to_vec(&response) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("[wowie-browser-host] Response serialization error: {e}");
                        continue;
                    }
                };

                if let Err(e) = write_native_message(&mut writer, &resp_bytes) {
                    eprintln!("[wowie-browser-host] Failed writing response to browser: {e}");
                    break;
                }
            }
            Ok(None) => {
                eprintln!("[wowie-browser-host] Browser closed pipe (clean EOF). Exiting.");
                break;
            }
            Err(e) => {
                eprintln!("[wowie-browser-host] Fatal framing error: {e}");
                break;
            }
        }
    }
}
