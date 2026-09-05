use owo_colors::OwoColorize;

pub const ICON_ROCKET: &str = "\u{1f680}";
pub const ICON_GLOBE: &str = "\u{1f310}";
pub const ICON_FOLDER: &str = "\u{1f4c1}";
pub const ICON_CONFIG: &str = "\u{2699}\u{fe0f}";
pub const ICON_LOCK: &str = "\u{1f512}";
pub const ICON_DB: &str = "\u{1f5c3}\u{fe0f}";
pub const ICON_FILE: &str = "\u{1f4c4}";
pub const ICON_CHECK: &str = "\u{2714}";
pub const ICON_CROSS: &str = "\u{2718}";
pub const ICON_ARROW: &str = "\u{276f}";
pub const ICON_WRENCH: &str = "\u{1f527}";
pub const ICON_INFO: &str = "\u{2139}\u{fe0f}";

pub fn banner() -> String {
    format!("{} {}", ICON_ROCKET, "ngaw-domain".bold().cyan())
}

pub fn section(title: &str) -> String {
    format!("\n{} {}", ICON_ARROW.dimmed(), title.bold().green())
}

pub fn step(n: usize, total: usize, icon: &str, desc: &str) -> String {
    format!(
        "{} {} {} {}",
        format!("({}/{})", n, total).dimmed(),
        icon,
        ICON_ARROW.cyan(),
        desc.white()
    )
}

pub fn command(cmd: &str) -> String {
    cmd.dimmed().blue().to_string()
}

pub fn success(msg: &str) -> String {
    format!("{} {}", ICON_CHECK.green(), msg.green())
}

pub fn error(msg: &str) -> String {
    format!("{} {}", ICON_CROSS.red(), msg.red())
}

pub fn info(msg: &str) -> String {
    format!("{} {}", ICON_INFO.cyan(), msg)
}

pub fn label_value(label: &str, value: &str) -> String {
    format!("  {} {}", format!("{label}:").dimmed(), value.yellow())
}
