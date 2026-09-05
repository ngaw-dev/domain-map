use ngaw_domain::config::{Answers, Server};

fn answers() -> Answers {
    Answers {
        email: "hello@ngaw.xyz".into(),
        server_user: "deploy".into(),
        domain: "example.com".into(),
        subdomain: String::new(),
        server: Server::Nginx,
        public: true,
        letsencrypt: false,
        nginx_https: true,
        nginx_php: true,
        nginx_logs: true,
        nginx_deny: true,
        docker: false,
        docker_host_port: 0,
        db_mysql: true,
        db_pgsql: false,
    }
}

fn docker_answers() -> Answers {
    Answers {
        subdomain: "glitchtip".into(),
        nginx_php: false,
        nginx_deny: false,
        docker: true,
        docker_host_port: 8091,
        db_mysql: false,
        db_pgsql: false,
        ..answers()
    }
}

#[test]
fn fqn_bare_domain_when_no_subdomain() {
    assert_eq!(answers().fqn(), "example.com");
}

#[test]
fn fqn_includes_subdomain_when_given() {
    let mut a = answers();
    a.subdomain = "app".into();
    assert_eq!(a.fqn(), "app.example.com");
}

#[test]
fn dir_uses_public_suffix_and_www() {
    assert_eq!(answers().dir(), "example.com/www/public");
}

#[test]
fn dir_without_public_uses_bare_root() {
    let mut a = answers();
    a.public = false;
    assert_eq!(a.dir(), "example.com/www");
}

#[test]
fn dir_with_explicit_subdomain() {
    let mut a = answers();
    a.subdomain = "app".into();
    a.public = false;
    assert_eq!(a.dir(), "example.com/app");
}

#[test]
fn db_identifiers_replace_dots_and_dashes() {
    let mut a = answers();
    a.subdomain = "my-app".into();
    assert_eq!(a.dbname(), "my_app_example_com");
    assert_eq!(a.username(), "my_app_example_com_user");
}

#[test]
fn validate_domain_accepts_valid() {
    assert!(
        ngaw_domain::config::validate_domain("example.com")
            == inquire::validator::Validation::Valid
    );
    assert!(
        ngaw_domain::config::validate_domain("sub.example.co.nz")
            == inquire::validator::Validation::Valid
    );
}

#[test]
fn validate_domain_rejects_invalid() {
    assert_ne!(
        ngaw_domain::config::validate_domain("example"),
        inquire::validator::Validation::Valid
    );
    assert_ne!(
        ngaw_domain::config::validate_domain("exa mple.com"),
        inquire::validator::Validation::Valid
    );
    assert_ne!(
        ngaw_domain::config::validate_domain("-bad.com"),
        inquire::validator::Validation::Valid
    );
    assert_ne!(
        ngaw_domain::config::validate_domain(""),
        inquire::validator::Validation::Valid
    );
}

#[test]
fn nginx_conf_contains_expected_blocks() {
    let steps = ngaw_domain::generate::build_steps(&answers(), "pw");
    let conf_step = steps
        .iter()
        .find(|s| {
            s.args.first().map(String::as_str) == Some("tee")
                && s.args
                    .iter()
                    .any(|a| a.contains("/etc/nginx/sites-available/"))
        })
        .expect("vhost step");
    let conf = conf_step.stdin.as_deref().unwrap();
    assert!(conf.contains("listen 443 ssl;"));
    assert!(conf.contains("server_name example.com www.example.com;"));
    assert!(conf.contains("return 301 https://$host$request_uri;"));
    assert!(conf.contains("fastcgi_pass unix:/run/php/php8.3-fpm.sock;"));
    assert!(conf.contains("deny all;"));
    assert!(conf.contains("access_log /var/log/nginx/example.com.access.log json_logs;"));
}

#[test]
fn nginx_docker_conf_proxies_to_host_port() {
    let a = docker_answers();
    let steps = ngaw_domain::generate::build_steps(&a, "pw");
    let conf = steps
        .iter()
        .find(|s| {
            s.args
                .iter()
                .any(|a| a.contains("/etc/nginx/sites-available/"))
        })
        .expect("vhost step")
        .stdin
        .as_deref()
        .unwrap();
    assert!(conf.contains("server_name glitchtip.example.com;"));
    assert!(conf.contains("return 301 https://$host$request_uri;"));
    assert!(conf.contains("listen 443 ssl;"));
    assert!(conf.contains("proxy_pass http://127.0.0.1:8091;"));
    assert!(conf.contains("proxy_set_header Host $host;"));
    assert!(conf.contains("proxy_set_header X-Real-IP $remote_addr;"));
    assert!(conf.contains("proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;"));
    assert!(conf.contains("proxy_set_header X-Forwarded-Proto $scheme;"));
    assert!(conf.contains("access_log /var/log/nginx/glitchtip.example.com.access.log json_logs;"));
    assert!(!conf.contains("root /var/www"));
    assert!(!conf.contains("try_files"));
    assert!(!conf.contains("fastcgi"));
    assert!(!conf.contains("deny all;"));
}

#[test]
fn docker_mode_skips_docroot_and_db_steps() {
    let steps = ngaw_domain::generate::build_steps(&docker_answers(), "pw");
    assert!(!steps.iter().any(|s| s.program == "mkdir"));
    assert!(!steps.iter().any(|s| s.program == "chown"));
    assert!(
        !steps
            .iter()
            .any(|s| s.stdin.is_some() && s.description.contains("index.html"))
    );
    assert!(!steps.iter().any(|s| s.description.contains("SQL")));
    assert!(!steps.iter().any(|s| s.description.contains(".env")));
}

#[test]
fn apache_conf_matches_php_output() {
    let mut a = answers();
    a.server = Server::Apache;
    a.nginx_https = false;
    let steps = ngaw_domain::generate::build_steps(&a, "pw");
    let conf = steps
        .iter()
        .find(|s| {
            s.args.first().map(String::as_str) == Some("tee")
                && s.args
                    .iter()
                    .any(|a| a.contains("/etc/apache2/sites-available/"))
        })
        .expect("vhost step")
        .stdin
        .as_deref()
        .unwrap();
    assert!(conf.contains("<VirtualHost *:80>"));
    assert!(conf.contains("ServerAdmin hello@ngaw.xyz"));
    assert!(conf.contains("ServerName example.com"));
    assert!(conf.contains("ServerAlias www.example.com"));
    assert!(conf.contains("DocumentRoot /var/www/example.com/www/public"));
}

#[test]
fn mysql_step_uses_shared_password() {
    let steps = ngaw_domain::generate::build_steps(&answers(), "S3cret!pw");
    let sql = steps
        .iter()
        .find(|s| s.args.iter().any(|a| a.ends_with("mysql-example.com.sql")))
        .expect("mysql step")
        .stdin
        .as_deref()
        .unwrap();
    assert!(sql.contains("CREATE DATABASE example_com;"));
    assert!(sql.contains("IDENTIFIED BY 'S3cret!pw'"));
}

#[test]
fn env_snippet_matches_db_choice() {
    let a = answers();
    let env = ngaw_domain::generate::env_snippet(&a, "pw").unwrap();
    assert!(env.contains("DB_CONNECTION=\"mysql\""));
    assert!(env.contains("DB_PORT=\"3306\""));

    let mut pg = a;
    pg.db_mysql = false;
    pg.db_pgsql = true;
    let env = ngaw_domain::generate::env_snippet(&pg, "pw").unwrap();
    assert!(env.contains("DB_CONNECTION=\"pgsql\""));
    assert!(env.contains("DB_PORT=\"5432\""));
}

#[test]
fn env_snippet_none_without_db() {
    let mut a = answers();
    a.db_mysql = false;
    assert!(ngaw_domain::generate::env_snippet(&a, "pw").is_none());
}

#[test]
fn password_shape_matches_php_generator() {
    for _ in 0..100 {
        let pw = ngaw_domain::password::random_password();
        assert_eq!(pw.len(), 16);
        assert!(pw.chars().next().unwrap().is_ascii_alphabetic());
        assert!(pw.chars().filter(|c| c.is_ascii_digit()).count() >= 2);
        assert!(pw.chars().filter(|c| !c.is_ascii_alphanumeric()).count() >= 2);
    }
}

fn obs() -> ngaw_domain::generate::OpenObserve {
    ngaw_domain::generate::OpenObserve {
        host: "139.84.204.42".into(),
        port: "5080".into(),
        user: "root@example.com".into(),
        passwd: "secret".into(),
    }
}

#[test]
fn fluentbit_steps_follow_naming_rules() {
    let mut a = answers();
    a.subdomain = "s3-api".into();
    let steps = ngaw_domain::generate::build_steps_with_obs(&a, "pw", Some(obs()));
    let conf = steps
        .iter()
        .find(|s| {
            s.args
                .iter()
                .any(|arg| arg == "/etc/fluent-bit/fluent-bit.conf")
        })
        .expect("fluent-bit step")
        .stdin
        .as_deref()
        .unwrap();
    assert!(
        conf.contains("Tag               s3_api.access;")
            || conf.contains("Tag               s3_api.access\n")
    );
    assert!(conf.contains("Tag               s3_api.access\n"));
    assert!(conf.contains("Tag               s3_api.error\n"));
    assert!(conf.contains("Match             s3_api.access\n"));
    assert!(conf.contains("Match             s3_api.error\n"));
    assert!(conf.contains("URI               /api/default/s3_api/_json\n"));
    assert!(conf.contains("URI               /api/default/s3_api_error/_json\n"));
    assert!(conf.contains("DB                /var/lib/fluent-bit/s3_api-access.db\n"));
    assert!(conf.contains("DB                /var/lib/fluent-bit/s3_api-error.db\n"));
    assert!(conf.contains("Parser            json\n"));
    assert!(conf.contains("Parser            nginx_error\n"));
    assert!(conf.contains("Host              139.84.204.42\n"));
    assert!(conf.contains("Port              5080\n"));
    assert!(conf.contains("HTTP_User         root@example.com\n"));
    assert!(conf.contains("HTTP_Passwd       secret\n"));
    assert!(
        steps
            .iter()
            .any(|s| s.description.contains("Restart Fluent Bit"))
    );
}

#[test]
fn fluentbit_steps_use_env_host_port_overrides() {
    let mut o = obs();
    o.host = "10.0.0.5".into();
    o.port = "5081".into();
    let steps = ngaw_domain::generate::build_steps_with_obs(&answers(), "pw", Some(o));
    let conf = steps
        .iter()
        .find(|s| {
            s.args
                .iter()
                .any(|arg| arg == "/etc/fluent-bit/fluent-bit.conf")
        })
        .expect("fluent-bit step")
        .stdin
        .as_deref()
        .unwrap();
    assert!(conf.contains("Host              10.0.0.5\n"));
    assert!(conf.contains("Port              5081\n"));
}

#[test]
fn fluentbit_steps_skipped_without_obs() {
    let steps = ngaw_domain::generate::build_steps_with_obs(&answers(), "pw", None);
    assert!(!steps.iter().any(|s| {
        s.args
            .iter()
            .any(|arg| arg == "/etc/fluent-bit/fluent-bit.conf")
    }));
}

#[test]
fn fluentbit_steps_skipped_without_logs() {
    let mut a = answers();
    a.nginx_logs = false;
    let steps = ngaw_domain::generate::build_steps_with_obs(&a, "pw", Some(obs()));
    assert!(!steps.iter().any(|s| {
        s.args
            .iter()
            .any(|arg| arg == "/etc/fluent-bit/fluent-bit.conf")
    }));
}
