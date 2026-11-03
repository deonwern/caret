use std::{
    fs,
    io::Write,
    net::TcpStream,
    process::{Command, Stdio},
    time::Duration,
};

/// Checks if local Ollama server is active on port 11434
pub fn check_ollama_dep() -> bool {
    TcpStream::connect_timeout(&"127.0.0.1:11434".parse().unwrap(), Duration::from_secs(1)).is_ok()
}

/// MCP Mode: Queries Ollama directly in terminal for interactive coding assistance
pub fn run_mcp_terminal() {
    println!("🚀 Starting MCP AI assistant mode...");
    if !check_ollama_dep() {
        eprintln!("\x1b[31m[Error]: Ollama is offline. Cannot run MCP query.\x1b[0m");
        return;
    }

    let prompt =
        "Provide a concise summary of how to structure multi-file project debugging workflows.";
    query_ollama_terminal(prompt);
}

/// Queries Ollama locally and streams response directly into the terminal
pub fn query_ollama_terminal(prompt_text: &str) {
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
                    println!("\n╭─ Ollama AI Terminal Analysis ───────────────╮");
                    render_with_mdcat_or_save(answer);
                    println!("╰─────────────────────────────────────────────╯\n");
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

/// Renders markdown content via mdcat if available, otherwise saves to file
fn render_with_mdcat_or_save(content: &str) {
    if check_command_exists("mdcat") {
        if let Ok(mut child) = Command::new("mdcat").stdin(Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(content.as_bytes());
            }
            let _ = child.wait();
            return;
        }
    }

    // Fallback if mdcat is missing or fails
    println!("\x1b[33m[Notice]: mdcat is not available to render text in terminal.\x1b[0m");
    let filename = "ai_analysis.md";
    match fs::write(filename, content) {
        Ok(_) => println!("\x1b[32m◆ Saved output to file: {}\x1b[0m", filename),
        Err(e) => eprintln!(
            "\x1b[31m[Error]: Failed to write output to file: {}\x1b[0m",
            e
        ),
    }
}

fn check_command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Automatically copies details to clipboard, prints fallback details if it fails
pub fn copy_details_to_clipboard(content: &str) {
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
