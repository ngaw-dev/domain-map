use std::io::Write;
use std::process::{Command, Stdio};

use crate::generate::Step;
use crate::ui;
use owo_colors::OwoColorize;

/// A step that failed: its description, rendered command and captured output.
#[derive(Debug)]
pub struct Failure {
    pub description: String,
    pub command: String,
    pub output: String,
}

/// Run steps sequentially. Failures do not abort the run; all steps are
/// attempted and a summary of failures is returned for the final report.
pub fn run_steps(steps: &[Step], quiet: bool) -> Vec<Failure> {
    let mut failures = Vec::new();

    for (i, step) in steps.iter().enumerate() {
        // Always show which step is running; quiet mode hides command output.
        println!("{}", ui::step(i + 1, steps.len(), step.icon, &step.description));
        match run_step(step, quiet) {
            Ok(()) => {
                if !quiet {
                    println!(
                        "  {} {}",
                        ui::ICON_CHECK.green(),
                        "done".dimmed()
                    );
                }
            }
            Err(failure) => {
                println!(
                    "  {} {}",
                    ui::ICON_CROSS.red(),
                    "failed, continuing".red()
                );
                failures.push(failure);
            }
        }
    }

    failures
}

fn run_step(step: &Step, quiet: bool) -> std::result::Result<(), Failure> {
    let mut child = Command::new(&step.program)
        .args(&step.args)
        .stdin(Stdio::piped())
        .stdout(if quiet { Stdio::piped() } else { Stdio::inherit() })
        .stderr(if quiet { Stdio::piped() } else { Stdio::inherit() })
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            return Err(Failure {
                description: step.description.clone(),
                command: describe_step(step),
                output: format!("failed to spawn {}: {e}", step.program),
            });
        }
    };

    if let Some(payload) = &step.stdin {
        let mut stdin = child.stdin.take().expect("stdin piped");
        stdin
            .write_all(payload.as_bytes())
            .and_then(|_| stdin.flush())
            .ok();
    }
    drop(child.stdin.take());

    let output = match child.wait_with_output() {
        Ok(out) => out,
        Err(e) => {
            return Err(Failure {
                description: step.description.clone(),
                command: describe_step(step),
                output: format!("failed waiting for {}: {e}", step.program),
            });
        }
    };

    if !output.status.success() {
        let mut captured = String::new();
        if !output.stdout.is_empty() {
            captured.push_str(&format!("stdout:\n{}", String::from_utf8_lossy(&output.stdout)));
        }
        if !output.stderr.is_empty() {
            captured.push_str(&format!("stderr:\n{}", String::from_utf8_lossy(&output.stderr)));
        }
        if captured.is_empty() {
            captured = format!("exited with {}", output.status);
        }
        return Err(Failure {
            description: step.description.clone(),
            command: describe_step(step),
            output: captured,
        });
    }
    Ok(())
}

/// Human-readable rendering of a step for --dry-run and the approval prompt.
pub fn describe_step(step: &Step) -> String {
    let mut line = format!("{} {}", step.program.bright_blue(), shell_join(&step.args));
    if let Some(payload) = &step.stdin {
        line.push_str(&format!(
            " <<'EOF'\n{}{}",
            payload.dimmed(),
            "EOF".bright_blue()
        ));
    }
    line
}

fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+".contains(c)) {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
