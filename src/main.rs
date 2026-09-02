




use anyhow::Result;
use ngaw_domain::config::{self, Answers, Server};
use ngaw_domain::{execute, generate, password};
use inquire::{Confirm, Select, Text};

fn main() -> Result<()> {
    let dry_run = std::env::args().any(|arg| arg == "--dry-run" || arg == "-n");

    println!("ngaw-domain — web server domain setup\n");

    let answers = prompt()?;
    let db_password = if answers.db_mysql || answers.db_pgsql {
        password::random_password()
    } else {
        String::new()
    };
    let steps = generate::build_steps(&answers, &db_password);

    println!("\n=== The following will be executed ===");
    for (i, step) in steps.iter().enumerate() {
        println!("\n--- ({}/{}) {} ---", i + 1, steps.len(), step.description);
        println!("{}", execute::describe_step(step));
    }
    if let Some(env) = generate::env_snippet(&answers, &db_password) {
        println!("\n--- .env (copy into your app) ---\n{env}");
    }

    if dry_run {
        println!("\nDry run — nothing executed.");
        return Ok(());
    }

    let go = Confirm::new("Execute setup now?")
        .with_default(false)
        .prompt()?;
    if !go {
        println!("Aborted — nothing executed.");
        return Ok(());
    }

    execute::run_steps(&steps)?;
    println!("\nDone. Site root: /var/www/{}", answers.dir());
    Ok(())
}

fn prompt() -> Result<Answers> {
    let email = Text::new("Email")
        .with_default("hello@ngaw.xyz")
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
