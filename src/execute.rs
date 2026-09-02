use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::generate::Step;
use crate::ui;
use owo_colors::OwoColorize;

/// Run steps sequentially, streaming stdin payloads and checking exit codes.
/// On the first failure the run aborts and remaining steps are skipped.
/// When `quiet`, successful steps print nothing; failures show the step
/// and its captured output.
pub fn run_steps(steps: &[Step], quiet: bool) -> Result<()> {
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

fn run_step(step: &Step, quiet: bool) -> Result<()> {
    let mut child = Command::new(&step.program)
        .args(&step.args)
        .stdin(Stdio::piped())
        .stdout(if quiet { Stdio::piped() } else { Stdio::inherit() })
        .stderr(if quiet { Stdio::piped() } else { Stdio::inherit() })
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

    let output = if quiet {
        match child.wait_with_output() {
            Ok(out) => Some(out),
            Err(e) => return Err(anyhow::Error::from(e).context(format!("failed waiting for {}", step.program))),
        }
    } else {
        let status = child
            .wait()
            .with_context(|| format!("failed waiting for {}", step.program))?;
        if !status.success() {
            anyhow::bail!("{} exited with {}", step.program, status);
        }
        return Ok(());
    };

    let output = output.expect("quiet branch always produces output");
    if !output.status.success() {
        println!(
            "\n  {} {}",
            ui::ICON_WRENCH,
            describe_step(step)
        );
        if !output.stdout.is_empty() {
            println!("  stdout:\n{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            println!("  stderr:\n{}", String::from_utf8_lossy(&output.stderr));
        }
        anyhow::bail!("{} exited with {}", step.program, output.status);
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
