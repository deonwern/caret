use crate::{ai, parser};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::Path,
    process::Command,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub language: String,
    pub main_file: String,
    pub exec_cmd: String,
}

pub const CONFIG_FILE: &str = "config.json";

/// Automatically creates an initial config file if missing
pub fn init_config_silent() {
    let default_template = vec![ProjectConfig {
        name: "python".to_string(),
        language: "Python".to_string(),
        main_file: "test.py".to_string(),
        exec_cmd: "python {}".to_string(),
    }];

    let json_data = serde_json::to_string_pretty(&default_template).unwrap();
    let _ = fs::write(CONFIG_FILE, json_data);
}

/// Loads the first valid config entry name/main_file to act as default when no args are passed
pub fn load_default_config_target() -> Result<String, String> {
    let file_content = fs::read_to_string(CONFIG_FILE)
        .map_err(|e| format!("Failed to read {}: {}", CONFIG_FILE, e))?;

    let configs: Vec<ProjectConfig> = serde_json::from_str(&file_content)
        .map_err(|e| format!("Failed to parse config JSON format: {}", e))?;

    if let Some(first) = configs.first() {
        if !first.name.trim().is_empty() {
            return Ok(first.name.clone());
        } else if !first.main_file.trim().is_empty() {
            return Ok(first.main_file.clone());
        }
    }

    Err("Config file is empty or contains no valid targets.".to_string())
}

/// Main execution and debugging orchestrator workflow with Claude Code UI style
pub fn debug_target(target: &str) {
    let config = match load_and_validate_config(target) {
        Ok(cfg) => cfg,
        Err(err_msg) => {
            eprintln!("\n╭─ Config Error ──────────────────────────────╮");
            eprintln!("│ \x1b[31m✗\x1b[0m {:<43} │", err_msg);
            eprintln!("╰─────────────────────────────────────────────╯\n");
            return;
        }
    };

    let base_tool = config.exec_cmd.split_whitespace().next().unwrap_or("");
    if !base_tool.is_empty() && !check_toolchain_exists(base_tool) {
        eprintln!("\n╭─ Toolchain Error ───────────────────────────╮");
        eprintln!(
            "│ \x1b[31m✗\x1b[0m Binary '{}' missing from system PATH.  │",
            base_tool
        );
        eprintln!("╰─────────────────────────────────────────────╯\n");
        return;
    }

    let final_command = if config.exec_cmd.contains("{}") {
        config.exec_cmd.replace("{}", &config.main_file)
    } else {
        format!("{} {}", config.exec_cmd, config.main_file)
    };

    println!("\n╭─ Execution Target ──────────────────────────╮");
    println!("│ \x1b[36m◆\x1b[0m Language:  {:<31} │", config.language);
    println!("│ \x1b[36m◆\x1b[0m Command:   {:<31} │", final_command);
    println!("╰─────────────────────────────────────────────╯");

    let output = match exec_command_piped(&final_command) {
        Ok(out) => out,
        Err(e) => {
            eprintln!("\x1b[31m[Error]: Failed to spawn process: {}\x1b[0m", e);
            return;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        println!("\n╭─ Output ────────────────────────────────────╮");
        print!("{}", stdout);
        if !stderr.is_empty() {
            eprintln!("{}", stderr);
        }
        println!("╰─────────────────────────────────────────────╯\n");
        return;
    }

    let full_error_output = format!("{}\n{}", stdout, stderr);
    println!("\n\x1b[1;31m╭─ Execution Failed ──────────────────────────╮\x1b[0m");
    println!("\x1b[1;31m│\x1b[0m Status: Exit code nonzero                   \x1b[1;31m│\x1b[0m");
    println!("\x1b[1;31m╰─────────────────────────────────────────────╯\x1b[0m\n");

    let parsed_error = parser::parse_error_output(&full_error_output);

    if let Some(ref err_info) = parsed_error {
        parser::print_colored_error_details(err_info);
    } else {
        println!(
            "\x1b[33m[Raw Error Output (Unparsed)]:\x1b[0m\n{}",
            full_error_output
        );
    }

    let source_code_content = fs::read_to_string(&config.main_file)
        .unwrap_or_else(|_| "[Could not read main source file content]".to_string());

    let diagnostic_context = format!(
        "Language: {}\nMain File: {}\nExec Command: {}\nError Output:\n{}\nSource Code Content:\n{}",
        config.language, config.main_file, final_command, full_error_output, source_code_content
    );

    if ai::check_ollama_dep() {
        println!("\n\x1b[32m◆ Ollama Active\x1b[0m — Querying local AI assistant...\n");
        ai::query_ollama_terminal(&diagnostic_context);
    } else {
        println!("\n\x1b[33m◆ Ollama Offline\x1b[0m — Copying diagnostic context to clipboard...");
        ai::copy_details_to_clipboard(&diagnostic_context);
    }
}

/// Runs a simple interactive TUI menu to select and debug targets from config.json
pub fn run_interactive_ui() {
    loop {
        println!("\n╭─ Interactive Debugger UI ───────────────────╮");
        println!("│ Select a target to run or type 'q' to exit: │");
        println!("╰─────────────────────────────────────────────╯");

        let file_content = match fs::read_to_string(CONFIG_FILE) {
            Ok(content) => content,
            Err(_) => {
                eprintln!("\x1b[31m[Error]: Could not read `{}`\x1b[0m", CONFIG_FILE);
                return;
            }
        };

        let configs: Vec<ProjectConfig> = match serde_json::from_str(&file_content) {
            Ok(cfg) => cfg,
            Err(_) => {
                eprintln!("\x1b[31m[Error]: Failed to parse `{}`\x1b[0m", CONFIG_FILE);
                return;
            }
        };

        if configs.is_empty() {
            println!("No targets found in config.");
            return;
        }

        for (i, cfg) in configs.iter().enumerate() {
            println!(
                "  [{}] {} ({}) -> {}",
                i + 1,
                cfg.name,
                cfg.language,
                cfg.main_file
            );
        }
        println!("  [q] Quit");

        print!("\nEnter choice: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("Failed to read input.");
            continue;
        }

        let choice = input.trim();
        if choice.eq_ignore_ascii_case("q") {
            println!("Exiting UI.");
            break;
        }

        if let Ok(index) = choice.parse::<usize>() {
            if index > 0 && index <= configs.len() {
                let selected = &configs[index - 1];
                println!("\nLaunching target: {}", selected.name);
                debug_target(&selected.name);
            } else {
                println!("\x1b[31m[Error]: Invalid selection index.\x1b[0m");
            }
        } else {
            let matched = configs
                .iter()
                .any(|c| c.name == choice || c.main_file == choice);
            if matched {
                debug_target(choice);
            } else {
                println!("\x1b[31m[Error]: Unknown option or target.\x1b[0m");
            }
        }
    }
}

pub fn load_and_validate_config(target: &str) -> Result<ProjectConfig, String> {
    if !Path::new(CONFIG_FILE).exists() {
        return Err(format!("Missing {}.", CONFIG_FILE));
    }

    let file_content = fs::read_to_string(CONFIG_FILE)
        .map_err(|e| format!("Failed to read `{}`: {}", CONFIG_FILE, e))?;

    let configs: Vec<ProjectConfig> = serde_json::from_str(&file_content)
        .map_err(|e| format!("Failed to parse config JSON format: {}", e))?;

    for config in configs {
        if config.main_file == target || config.name == target {
            if config.name.trim().is_empty()
                || config.language.trim().is_empty()
                || config.main_file.trim().is_empty()
                || config.exec_cmd.trim().is_empty()
            {
                return Err(format!(
                    "Configuration entry for target '{}' contains empty fields.",
                    target
                ));
            }
            return Ok(config);
        }
    }

    Err(format!(
        "Target '{}' not found inside `{}`",
        target, CONFIG_FILE
    ))
}

fn check_toolchain_exists(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn exec_command_piped(cmd: &str) -> io::Result<std::process::Output> {
    #[cfg(target_os = "windows")]
    let output = Command::new("cmd").args(["/C", cmd]).output()?;

    #[cfg(not(target_os = "windows"))]
    let output = Command::new("sh").args(["-c", cmd]).output()?;

    Ok(output)
}
