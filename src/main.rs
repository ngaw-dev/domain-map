use anyhow::{bail, Result};
use inquire::{Confirm, Select, Text};
use ngaw_domain::config::{self, Answers, Server};
use ngaw_domain::ui;
use ngaw_domain::{execute, generate, password};
use owo_colors::OwoColorize;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let execute_now = args.iter().any(|a| a == "-y" || a == "--yes");
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();

    println!("{}", ui::banner());
    println!(
        "  {} web server domain setup\n",
        "domain provisioning, interactive".dimmed()
    );

    let answers = match positional.as_slice() {
        [] => prompt()?,
        [domain] => from_args(domain, None)?,
        [domain, user, ..] => from_args(domain, Some(user))?,
    };

    let db_password = if answers.db_mysql || answers.db_pgsql {
        password::random_password()
    } else {
        String::new()
    };
    let steps = generate::build_steps(&answers, &db_password);

    print_summary(&answers);
    if !execute_now {
        println!("{}", ui::section("The following will be executed"));
        for (i, step) in steps.iter().enumerate() {
            println!("\n{}", ui::step(i + 1, steps.len(), step.icon, &step.description));
            println!("  {}", ui::command(&execute::describe_step(step)));
        }
        if let Some(env) = generate::env_snippet(&answers, &db_password) {
            println!(
                "\n{} {}",
                ui::ICON_DB,
                ".env (copy into your app)".bold()
            );
            println!("{}", env.dimmed());
        }
        println!(
            "\n{}",
            ui::info("Dry run — nothing executed. Re-run with -y to apply.")
        );
        return Ok(());
    }

    execute::run_steps(&steps, true)?;
    println!(
        "\n{} Setup complete. Site root: {}",
        ui::ICON_CHECK,
        format!("/var/www/{}", answers.dir()).yellow().bold()
    );
    print_followups(&answers);
    Ok(())
}

/// Commands the user must run manually, e.g. importing the generated SQL.
fn print_followups(a: &Answers) {
    let fqn = a.fqn();
    let domain_folder = a.domain.split('.').next().unwrap_or(&a.domain);
    let mut cmds: Vec<(&str, String)> = Vec::new();
    if a.db_mysql {
        cmds.push((
            "Import MySQL SQL",
            format!("sudo mysql -u root -p < /var/www/{domain_folder}/{fqn}-mysql.sql"),
        ));
    }
    if a.db_pgsql {
        cmds.push((
            "Import PostgreSQL SQL",
            format!("sudo -u postgres psql -f /var/www/{domain_folder}/{fqn}-pgsql.sql"),
        ));
    }
    if cmds.is_empty() {
        return;
    }
    println!("\n{}", ui::section("Run these manually"));
    for (label, cmd) in cmds {
        println!("  {} {}", label, cmd.bright_blue());
    }
}

/// Non-interactive mode: `ngaw-domain sub.domain.tld [username]`.
/// A 2+ label argument is `sub.domain.tld`; a single-label TLD-only value is
/// invalid. Everything else falls back to defaults (matching prompt defaults).
fn from_args(domain: &str, user: Option<&str>) -> Result<Answers> {
    let domain = domain.trim().to_lowercase();
    if !matches!(config::validate_domain(&domain), inquire::validator::Validation::Valid) {
        bail!("invalid domain: {domain} (expected sub.domain.tld or domain.tld)");
    }

    // `a.b.c` -> subdomain `a`, domain `b.c`; `a.b` -> bare domain (implicit www).
    let (subdomain, domain) = match domain.split_once('.') {
        Some((first, rest)) if rest.contains('.') => (first.to_string(), rest.to_string()),
        Some((_, _)) => (String::new(), domain),
        None => unreachable!("validated domain always has a dot"),
    };

    Ok(Answers {
        email: "hello@ngaw.xyz".into(),
        server_user: user
            .map(str::to_string)
            .unwrap_or_else(whoami_default),
        domain,
        subdomain,
        server: Server::Nginx,
        public: true,
        letsencrypt: false,
        nginx_https: true,
        nginx_php: true,
        nginx_logs: true,
        nginx_deny: true,
        db_mysql: true,
        db_pgsql: false,
    })
}

fn print_summary(a: &Answers) {
    println!("{}", ui::section("Plan"));
    println!("{}", ui::label_value("Domain", &a.fqn()));
    println!(
        "{}",
        ui::label_value("Server", if a.server == Server::Nginx { "nginx" } else { "apache" })
    );
    println!(
        "{}",
        ui::label_value(
            "Docroot",
            &format!("/var/www/{}", a.dir())
        )
    );
    println!(
        "{}",
        ui::label_value("SSL", if a.letsencrypt { "Let's Encrypt" } else if a.nginx_https { "self-signed" } else { "none" })
    );
    let mut dbs: Vec<&str> = Vec::new();
    if a.db_mysql {
        dbs.push("MySQL");
    }
    if a.db_pgsql {
        dbs.push("PostgreSQL");
    }
    let dbs_joined = dbs.join(", ");
    println!(
        "{}",
        ui::label_value("Databases", if dbs.is_empty() { "none" } else { &dbs_joined })
    );
}

fn whoami_default() -> String {
    std::env::var("SUDO_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "www-data".into())
}

fn prompt() -> Result<Answers> {
    let email = Text::new("Email")
        .with_default("hello@ngaw.xyz")
        .prompt()?;

    let server_user = Text::new("Server username (for file ownership)")
        .with_default(whoami_default().as_str())
        .prompt()?;

    let domain = Text::new("Domain")
        .with_placeholder("example.com")
        .with_validator(|v: &str| Ok(config::validate_domain(v.trim())))
        .prompt()?;
    let domain = domain.trim().to_lowercase();

    let subdomain = Text::new("Subdomain")
        .with_placeholder("www (optional)")
        .prompt()?;
    let subdomain = subdomain.trim().to_string();

    let server = Select::new("Server", vec!["apache", "nginx"])
        .with_starting_cursor(1)
        .prompt()?;
    let server = if server == "nginx" { Server::Nginx } else { Server::Apache };

    let public = Confirm::new("Use public directory?")
        .with_default(true)
        .prompt()?;
    let letsencrypt = Confirm::new("Let's Encrypt cert?")
        .with_default(false)
        .prompt()?;

    let (mut nginx_https, mut nginx_php, mut nginx_logs, mut nginx_deny) =
        (false, false, false, false);
    if server == Server::Nginx {
        nginx_https = Confirm::new("HTTPS (SSL + redirect)?").with_default(true).prompt()?;
        nginx_php = Confirm::new("PHP (PHP-FPM)?").with_default(true).prompt()?;
        nginx_logs = Confirm::new("Access / error logs?").with_default(true).prompt()?;
        nginx_deny = Confirm::new("Deny dotfiles?").with_default(true).prompt()?;
    }

    let db_mysql = Confirm::new("MySQL database?").with_default(true).prompt()?;
    let db_pgsql = Confirm::new("PostgreSQL database?").with_default(false).prompt()?;

    Ok(Answers {
        email,
        server_user,
        domain,
        subdomain,
        server,
        public,
        letsencrypt,
        nginx_https,
        nginx_php,
        nginx_logs,
        nginx_deny,
        db_mysql,
        db_pgsql,
    })
}
