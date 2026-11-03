use regex::Regex;

pub struct ParsedErrorDetails {
    pub line_number: String,
    pub error_message: String,
    pub total_errors: usize,
    pub explanation: String,
}

/// Regex error parser identifying line numbers, messages, and cascading subfile issues
pub fn parse_error_output(output: &str) -> Option<ParsedErrorDetails> {
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
pub fn print_colored_error_details(err: &ParsedErrorDetails) {
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
