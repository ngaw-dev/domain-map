use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::generate::Step;
use crate::ui;
use owo_colors::OwoColorize;

/// Run steps sequentially, streaming stdin payloads and checking exit codes.
/// On the first failure the run aborts and remaining steps are skipped.
pub fn run_steps(steps: &[Step]) -> Result<()> {
    for (i, step) in steps.iter().enumerate() {
        println!("{}", ui::step(i + 1, steps.len(), &step.description));
        match run_step(step) {
            Ok(()) => println!(
                "  {} {}",
                ui::ICON_CHECK.green(),
                "done".dimmed()
            ),
            Err(e) => {
                println!(
                    "\n{}",
                    ui::error(&format!("step failed: {}", step.description))
                );
                return Err(e.context(format!("step failed: {}", step.description)));
            }
        }
    }
    Ok(())
}

fn run_step(step: &Step) -> Result<()> {
    let mut child = Command::new(&step.program)
        .args(&step.args)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn {}", step.program))?;

    if let Some(payload) = &step.stdin {
        let mut stdin = child.stdin.take().expect("stdin piped");
        stdin
            .write_all(payload.as_bytes())
            .and_then(|_| stdin.flush())
            .with_context(|| format!("failed writing stdin to {}", step.program))?;
    }
    drop(child.stdin.take());

    let status = child
        .wait()
        .with_context(|| format!("failed waiting for {}", step.program))?;
    if !status.success() {
        anyhow::bail!("{} exited with {}", step.program, status);
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
