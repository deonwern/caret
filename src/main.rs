mod ai;
mod cli;
mod parser;

use std::path::Path;

const CONFIG_FILE: &str = "config.json";

fn main() {
    // 1. Check if user config exists. If not, auto-create it with a default template.
    if !Path::new(CONFIG_FILE).exists() {
        cli::init_config_silent();
    }

    let args: Vec<String> = std::env::args().collect();

    // Handle MCP mode or target execution based on arguments
    if args.len() > 1 {
        let command_arg = args[1].as_str();
        if command_arg == "mcp" {
            ai::run_mcp_terminal();
        } else if command_arg == "init" {
            println!("`{}` already exists.", CONFIG_FILE);
        } else {
            cli::debug_target(command_arg);
        }
    } else {
        // No arguments provided: automatically load config and run the first entry!
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
