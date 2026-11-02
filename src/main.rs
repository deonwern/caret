use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{fs, io, net::TcpStream, path::Path, process::Command, time::Duration};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ProjectConfig {
    name: String,
    language: String,
    main_file: String,
    exec_cmd: String,
}

const CONFIG_FILE: &str = "config.json";

fn main() {
    // 1. Check if user config exists. If not, auto-create it with a default template.
    if !Path::new(CONFIG_FILE).exists() {
        init_config_silent();
    }

    let args: Vec<String> = std::env::args().collect();

    // Handle MCP mode or target execution based on arguments
    if args.len() > 1 {
        let command_arg = args[1].as_str();
        if command_arg == "mcp" {
            run_mcp_terminal();
        } else if command_arg == "init" {
            println!("`{}` already exists.", CONFIG_FILE);
        } else {
            debug_target(command_arg);
        }
    } else {
        // No arguments provided: automatically load config and run the first entry!
        match load_default_config_target() {
            Ok(target_name) => {
                println!(
                    "[Info]: No target specified. Automatically running default config entry: '{}'",
                    target_name
                );
                debug_target(&target_name);
            }
            Err(e) => {
                eprintln!("\x1b[31m[Config Error]:\x1b[0m {}", e);
            }
        }
    }
}

/// Automatically creates an initial config file if missing
fn init_config_silent() {
    let default_template = vec![ProjectConfig {
        name: "python".to_string(),
        language: "Python".to_string(),
        main_file: "test.py".to_string(),
        exec_cmd: "python {}".to_string(),
    }];

    let json_data = serde_json::to_string_pretty(&default_template).unwrap();
    let _ = fs::write(CONFIG_FILE, json_data);
}

/// MCP Mode: Queries Ollama directly in terminal for interactive coding assistance
fn run_mcp_terminal() {
    println!("🚀 Starting MCP AI assistant mode...");
    if !check_ollama_dep() {
        eprintln!("\x1b[31m[Error]: Ollama is offline. Cannot run MCP query.\x1b[0m");
        return;
    }

    let prompt =
        "Provide a concise summary of how to structure multi-file project debugging workflows.";
    query_ollama_terminal(prompt);
}

/// Checks if local Ollama server is active on port 11434
fn check_ollama_dep() -> bool {
    TcpStream::connect_timeout(&"127.0.0.1:11434".parse().unwrap(), Duration::from_secs(1)).is_ok()
}

/// Loads the first valid config entry name/main_file to act as default when no args are passed
fn load_default_config_target() -> Result<String, String> {
    let file_content = fs::read_to_string(CONFIG_FILE)
        .map_err(|e| format!("Failed to read `{}`: {}", CONFIG_FILE, e))?;

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

/// Main execution and debugging orchestrator workflow
fn debug_target(target: &str) {
    // 2. Check if config fields are not empty and load configuration
    let config = match load_and_validate_config(target) {
        Ok(cfg) => cfg,
        Err(err_msg) => {
            eprintln!("\x1b[31m[Config Error]:\x1b[0m {}", err_msg);
            return;
        }
    };

    // 3. Check if the required toolchain exists in PATH
    let base_tool = config.exec_cmd.split_whitespace().next().unwrap_or("");
    if !base_tool.is_empty() && !check_toolchain_exists(base_tool) {
        eprintln!(
            "\x1b[31m[Toolchain Error]:\x1b[0m Required tool binary '{}' is missing or not in system PATH.",
            base_tool
        );
        return;
    }

    // 4. Resolve command placeholder `{}` with the main_file path
    let final_command = if config.exec_cmd.contains("{}") {
        config.exec_cmd.replace("{}", &config.main_file)
    } else {
        format!("{} {}", config.exec_cmd, config.main_file)
    };

    // 5. Run the command
    println!("Language: {}", config.language);
    println!("Executing: {}", final_command);

    let output = match exec_command_piped(&final_command) {
        Ok(out) => out,
        Err(e) => {
            eprintln!("[Error]: Failed to spawn process: {}", e);
            return;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // 6. Handle output: if successful (no execution error), print stdout directly
    if output.status.success() {
        print!("{}", stdout);
        if !stderr.is_empty() {
            eprintln!("{}", stderr);
        }
        return;
    }

    // 7. Execution Error occurred: Parse error via regex and send to Ollama / Clipboard
    let full_error_output = format!("{}\n{}", stdout, stderr);
    println!("\n\x1b[1;31m=== Command Execution Failed ===\x1b[0m");

    let parsed_error = parse_error_output(&full_error_output);

    if let Some(ref err_info) = parsed_error {
        print_colored_error_details(err_info);
    } else {
        println!(
            "\x1b[33m[Raw Error Output (Unparsed)]:\x1b[0m\n{}",
            full_error_output
        );
    }

    // Gather file context for prompt payload
    let source_code_content = fs::read_to_string(&config.main_file)
        .unwrap_or_else(|_| "[Could not read main source file content]".to_string());

    let diagnostic_context = format!(
        "Language: {}\nMain File: {}\nExec Command: {}\nError Output:\n{}\nSource Code Content:\n{}",
        config.language, config.main_file, final_command, full_error_output, source_code_content
    );

    // 8. Check if Ollama exists or not
    if check_ollama_dep() {
        println!(
            "\n\x1b[32m[Ollama Detected]\x1b[0m Querying local Ollama server directly in terminal...\n"
        );
        query_ollama_terminal(&diagnostic_context);
    } else {
        println!(
            "\n\x1b[33m[Ollama Offline]\x1b[0m Copying error context and diagnostics to clipboard..."
        );
        copy_details_to_clipboard(&diagnostic_context);
    }
}

/// Validates configuration existence and checks that fields are not empty
fn load_and_validate_config(target: &str) -> Result<ProjectConfig, String> {
    if !Path::new(CONFIG_FILE).exists() {
        return Err(format!("Missing `{}`.", CONFIG_FILE));
    }

    let file_content = fs::read_to_string(CONFIG_FILE)
        .map_err(|e| format!("Failed to read `{}`: {}", CONFIG_FILE, e))?;

    let configs: Vec<ProjectConfig> = serde_json::from_str(&file_content)
        .map_err(|e| format!("Failed to parse config JSON format: {}", e))?;

    for config in configs {
        if config.main_file == target || config.name == target {
            // Check if fields are not empty
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

struct ParsedErrorDetails {
    line_number: String,
    error_message: String,
    total_errors: usize,
    explanation: String,
}

/// Regex error parser identifying line numbers, messages, and cascading subfile issues
fn parse_error_output(output: &str) -> Option<ParsedErrorDetails> {
    let re = Regex::new(r#"(?m)(?:File\s+['"]?(.*?)['"]?,\s+line\s+(\d+)|([a-zA-Z0-9_\-./]+\.[a-zA-Z0-9]+):(\d+)(?::\d+)?):\s*(.*)"#).ok()?;

    let mut matches = Vec::new();
    for cap in re.captures_iter(output) {
        let line = cap.get(2).or(cap.get(4)).map_or("", |m| m.as_str());
        let msg = cap.get(5).map_or("", |m| m.as_str());
        if !line.is_empty() {
            matches.push((line.to_string(), msg.to_string()));
        }
    }

    if matches.is_empty() {
        return None;
    }

    let total_errors = matches.len();
    let (line_number, error_message) = matches[0].clone();

    let explanation = if total_errors > 1 {
        format!(
            "Detected {} total errors. Primary error on line {}. A subfile used by the main file may be causing cascading compilation/runtime exceptions.",
            total_errors, line_number
        )
    } else {
        format!(
            "Single error found localized cleanly at line {}.",
            line_number
        )
    };

    Some(ParsedErrorDetails {
        line_number,
        error_message,
        total_errors,
        explanation,
    })
}

/// Prints error diagnostics in color-formatted terminal style
fn print_colored_error_details(err: &ParsedErrorDetails) {
    println!("\x1b[1;31m[Parsed Diagnostic Summary]\x1b[0m");
    println!("  • \x1b[1mTotal Errors Count:\x1b[0m {}", err.total_errors);
    println!(
        "  • \x1b[1mPrimary Line Number:\x1b[0m line {}",
        err.line_number
    );
    println!(
        "  • \x1b[1mError Message:\x1b[0m      \x1b[31m{}\x1b[0m",
        err.error_message
    );
    println!(
        "  • \x1b[1mRoot Cause Analysis:\x1b[0m \x1b[36m{}\x1b[0m\n",
        err.explanation
    );
}

/// Queries Ollama locally and streams response directly into the terminal
fn query_ollama_terminal(prompt_text: &str) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            println!("[Error]: Failed to build HTTP client for Ollama.");
            return;
        }
    };

    let payload = serde_json::json!({
        "model": "qwen2.5-coder:1.5b",
        "prompt": format!("Analyze this code error context, identify why the error happened (including subfile relations), and provide a fix:\n\n{}", prompt_text),
        "stream": false
    });

    match client
        .post("http://localhost:11434/api/generate")
        .json(&payload)
        .send()
    {
        Ok(response) => {
            if let Ok(json_val) = response.json::<serde_json::Value>() {
                if let Some(answer) = json_val["response"].as_str() {
                    println!(
                        "\x1b[1;32m=== Ollama AI Terminal Analysis ===\x1b[0m\n{}",
                        answer
                    );
                    return;
                }
            }
            println!("[Error]: Received malformed response JSON from Ollama server.");
        }
        Err(_) => {
            println!("[Error]: Failed to communicate with Ollama generation endpoint.");
        }
    }
}

/// Automatically copies details to clipboard, prints fallback details if it fails
fn copy_details_to_clipboard(content: &str) {
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => match clipboard.set_text(content.to_string()) {
            Ok(_) => println!(
                "\x1b[32m✨ Successfully copied error details and file context to clipboard!\x1b[0m"
            ),
            Err(_) => print_clipboard_failure_fallback(content),
        },
        Err(_) => print_clipboard_failure_fallback(content),
    }
}

fn print_clipboard_failure_fallback(content: &str) {
    println!(
        "\x1b[31m[Warning]: Failed to copy contents to system clipboard automatically.\x1b[0m"
    );
    println!(
        "Here are the raw diagnostic details instead:\n\n```\n{}\n```",
        content
    );
}
