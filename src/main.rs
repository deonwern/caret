mod ai;
mod cli;
mod language;
mod parser;

use std::path::Path;

const CONFIG_FILE: &str = "config.json";

fn main() {
    if !Path::new(CONFIG_FILE).exists() {
        cli::init_config_silent();
    }

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        let command_arg = args[1].as_str();
        match command_arg {
            "mcp" => ai::run_mcp_terminal(),
            "init" => println!("`{}` already exists.", CONFIG_FILE),
            "--ui" | "ui" => cli::run_interactive_ui(),
            _ => cli::debug_target(command_arg),
        }
    } else {
        match cli::load_default_config_target() {
            Ok(target_name) => {
                println!(
                    "[Info]: No target specified. Automatically running default config entry: '{}'",
                    target_name
                );
                cli::debug_target(&target_name);
            }
            Err(e) => {
                eprintln!("\x1b[31m[Config Error]:\x1b[0m {}", e);
            }
        }
    }
}
