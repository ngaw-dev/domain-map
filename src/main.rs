




use anyhow::Result;
use inquire::{Confirm, Select, Text};
use ngaw_domain::config::{self, Answers, Server};
use ngaw_domain::ui;
use ngaw_domain::{execute, generate, password};
use owo_colors::OwoColorize;

fn main() -> Result<()> {
    let dry_run = std::env::args().any(|arg| arg == "--dry-run" || arg == "-n");

    println!("{}", ui::banner());
    println!(
        "  {} web server domain setup\n",
        "domain provisioning, interactive".dimmed()
    );

    let answers = prompt()?;
    let db_password = if answers.db_mysql || answers.db_pgsql {
        password::random_password()
    } else {
        String::new()
    };
    let steps = generate::build_steps(&answers, &db_password);

    print_summary(&answers);
    println!("{}", ui::section("The following will be executed"));
    for (i, step) in steps.iter().enumerate() {
        println!("\n{}", ui::step(i + 1, steps.len(), &step.description));
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

    if dry_run {
        println!("\n{}", ui::info("Dry run — nothing executed."));
        return Ok(());
    }

    let go = Confirm::new("Execute setup now?")
        .with_default(false)
        .prompt()?;
    if !go {
        println!("\n{}", ui::info("Aborted — nothing executed."));
        return Ok(());
    }

    execute::run_steps(&steps)?;
    println!(
        "\n{} Site root: {}",
        ui::ICON_GLOBE,
        format!("/var/www/{}", answers.dir()).yellow().bold()
    );
    Ok(())
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
            &format!("/var/www/{}{}", a.dir(), if a.public { "" } else { "" })
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
