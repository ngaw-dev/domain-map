use crate::config::{Answers, Server};

/// One shell command to run, with an optional stdin payload (heredoc body).
#[derive(Debug)]
pub struct Step {
    pub description: String,
    pub program: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
}

pub fn build_steps(a: &Answers, db_password: &str) -> Vec<Step> {
    let fqn = a.fqn();
    let dir = a.dir();

    let mut steps = Vec::new();

    // Create document root and placeholder index.html.
    let root = format!("/var/www/{dir}");
    let index_body = format!("<h1>{fqn}</h1>\n");
    steps.push(Step {
        description: format!("Create document root {root}"),
        program: "sudo".into(),
        args: vec!["mkdir".into(), "-p".into(), root.clone()],
        stdin: None,
    });
    steps.push(Step {
        description: "Write placeholder index.html".into(),
        program: "sudo".into(),
        args: vec![
            "tee".into(),
            format!("{}/index.html", root.trim_end_matches('/')),
        ],
        stdin: Some(index_body),
    });

    // Vhost config + enable site.
    match a.server {
        Server::Nginx => steps.extend(nginx_steps(a, &fqn)),
        Server::Apache => steps.extend(apache_steps(a, &fqn)),
    }

    // Permissions.
    steps.push(Step {
        description: "Grant ownership to $USER:www-data".into(),
        program: "sudo".into(),
        args: vec![
            "chown".into(),
            "-R".into(),
            format!("{}:www-data", whoami_arg()),
            format!("/var/www/{}", a.domain),
        ],
        stdin: None,
    });

    // Let's Encrypt, after the site is live.
    if a.letsencrypt {
        let flag = if a.server == Server::Nginx {
            "--nginx"
        } else {
            "--apache"
        };
        steps.push(Step {
            description: "Obtain Let's Encrypt certificate".into(),
            program: "sudo".into(),
            args: vec![
                "certbot".into(),
                flag.into(),
                "-d".into(),
                fqn.clone(),
            ],
            stdin: None,
        });
    }

    // Databases (one shared password for SQL and the .env snippet).
    let password = db_password;
    let dbname = a.dbname();
    let username = a.username();
    if a.db_mysql {
        let sql = format!(
            "CREATE DATABASE {dbname};\n\
             CREATE USER '{username}'@'localhost' IDENTIFIED BY '{password}';\n\
             GRANT ALL PRIVILEGES ON {dbname}.* TO '{username}'@'localhost';\n\
             FLUSH PRIVILEGES;\n"
        );
        steps.push(Step {
            description: "Create MySQL database and user".into(),
            program: "sudo".into(),
            args: vec!["mysql".into()],
            stdin: Some(sql),
        });
    }
    if a.db_pgsql {
        let sql = format!(
            "CREATE DATABASE {dbname};\n\
             CREATE USER {username} WITH PASSWORD '{password}';\n\
             GRANT ALL PRIVILEGES ON DATABASE {dbname} TO {username};\n"
        );
        steps.push(Step {
            description: "Create PostgreSQL database and user".into(),
            program: "sudo".into(),
            args: vec![
                "-u".into(),
                "postgres".into(),
                "psql".into(),
                "-v".into(),
                "ON_ERROR_STOP=1".into(),
            ],
            stdin: Some(sql),
        });
        steps.push(Step {
            description: "Grant schema privileges".into(),
            program: "sudo".into(),
            args: vec![
                "-u".into(),
                "postgres".into(),
                "psql".into(),
                "-d".into(),
                dbname,
                "-c".into(),
                format!("GRANT ALL ON SCHEMA public TO {username};"),
            ],
            stdin: None,
        });
    }

    steps
}

fn nginx_steps(a: &Answers, fqn: &str) -> Vec<Step> {
    let dir = a.dir();
    let server_names = if a.is_implicit_www() {
        format!("{fqn} www.{fqn}")
    } else {
        fqn.to_string()
    };

    let mut conf = String::new();
    if a.nginx_https {
        conf.push_str("server {\n");
        conf.push_str("    listen 80;\n");
        conf.push_str("    listen [::]:80;\n");
        conf.push_str(&format!("    server_name {server_names};\n"));
        conf.push_str("    return 301 https://$host$request_uri;\n");
        conf.push_str("}\n\n");
    }
    conf.push_str("server {\n");
    if a.nginx_https {
        conf.push_str("    listen 443 ssl;\n    listen [::]:443 ssl;\n\n");
    } else {
        conf.push_str("    listen 80;\n    listen [::]:80;\n\n");
    }
    conf.push_str(&format!("    server_name {server_names};\n\n"));
    if a.nginx_https && !a.letsencrypt {
        conf.push_str(&format!(
            "    ssl_certificate /etc/ssl/certs/{fqn}-selfsigned.crt;\n"
        ));
        conf.push_str(&format!(
            "    ssl_certificate_key /etc/ssl/private/{fqn}-selfsigned.key;\n\n"
        ));
    }
    conf.push_str(&format!("    root /var/www/{dir};\n"));
    conf.push_str(&format!(
        "    index {}index.html index.htm;\n\n",
        if a.nginx_php { "index.php " } else { "" }
    ));
    conf.push_str("    location / {\n");
    conf.push_str(if a.nginx_php {
        "        try_files $uri $uri/ /index.php?$query_string;\n"
    } else {
        "        try_files $uri $uri/ =404;\n"
    });
    conf.push_str("    }\n");
    if a.nginx_php {
        conf.push_str("\n    location ~ \\.php$ {\n");
        conf.push_str("        include snippets/fastcgi-php.conf;\n");
        conf.push_str("        fastcgi_pass unix:/run/php/php8.3-fpm.sock;\n");
        conf.push_str("        fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\n");
        conf.push_str("        include fastcgi_params;\n");
        conf.push_str("    }\n");
    }
    if a.nginx_deny {
        conf.push_str("\n    location ~ /\\.ht {\n");
        conf.push_str("        deny all;\n");
        conf.push_str("    }\n");
    }
    if a.nginx_logs {
        conf.push_str(&format!(
            "\n    access_log /var/log/nginx/{fqn}.access.log;\n"
        ));
        conf.push_str(&format!(
            "    error_log /var/log/nginx/{fqn}.error.log;\n"
        ));
    }
    conf.push_str("}\n");

    let mut steps = vec![
        Step {
            description: "Write nginx vhost config".into(),
            program: "sudo".into(),
            args: vec![
                "tee".into(),
                format!("/etc/nginx/sites-available/{fqn}.conf"),
            ],
            stdin: Some(conf),
        },
        Step {
            description: "Enable site".into(),
            program: "sudo".into(),
            args: vec![
                "ln".into(),
                "-s".into(),
                format!("/etc/nginx/sites-available/{fqn}.conf"),
                "/etc/nginx/sites-enabled/".into(),
            ],
            stdin: None,
        },
    ];

    if a.nginx_https && !a.letsencrypt {
        steps.push(Step {
            description: "Generate self-signed certificate".into(),
            program: "sudo".into(),
            args: vec![
                "openssl".into(),
                "req".into(),
                "-x509".into(),
                "-nodes".into(),
                "-days".into(),
                "365".into(),
                "-newkey".into(),
                "rsa:2048".into(),
                "-keyout".into(),
                format!("/etc/ssl/private/{fqn}-selfsigned.key"),
                "-out".into(),
                format!("/etc/ssl/certs/{fqn}-selfsigned.crt"),
                "-subj".into(),
                format!(
                    "/C=NZ/ST=Wellington/L=NGAW/O=NGAW/OU=Unit/CN={fqn}/emailAddress={}",
                    a.email
                ),
            ],
            stdin: None,
        });
    }

    steps.push(Step {
        description: "Test nginx configuration".into(),
        program: "sudo".into(),
        args: vec!["nginx".into(), "-t".into()],
        stdin: None,
    });
    steps.push(Step {
        description: "Restart nginx".into(),
        program: "sudo".into(),
        args: vec!["systemctl".into(), "restart".into(), "nginx".into()],
        stdin: None,
    });

    steps
}

fn apache_steps(a: &Answers, fqn: &str) -> Vec<Step> {
    let dir = a.dir();
    let mut conf = String::new();
    conf.push_str("<VirtualHost *:80>\n");
    conf.push_str(&format!("    ServerAdmin {}\n", a.email));
    conf.push_str(&format!("    ServerName {fqn}\n"));
    if a.is_implicit_www() {
        conf.push_str(&format!("    ServerAlias www.{fqn}\n"));
    }
    conf.push_str(&format!("    DocumentRoot /var/www/{dir}\n"));
    conf.push_str("</VirtualHost>\n");

    vec![
        Step {
            description: "Write apache vhost config".into(),
            program: "sudo".into(),
            args: vec![
                "tee".into(),
                format!("/etc/apache2/sites-available/{fqn}.conf"),
            ],
            stdin: Some(conf),
        },
        Step {
            description: "Enable site".into(),
            program: "sudo".into(),
            args: vec!["a2ensite".into(), format!("{fqn}.conf")],
            stdin: None,
        },
        Step {
            description: "Reload apache".into(),
            program: "sudo".into(),
            args: vec!["service".into(), "apache2".into(), "reload".into()],
            stdin: None,
        },
        Step {
            description: "Restart apache".into(),
            program: "sudo".into(),
            args: vec!["service".into(), "apache2".into(), "restart".into()],
            stdin: None,
        },
    ]
}

pub fn env_snippet(a: &Answers, password: &str) -> Option<String> {
    if !a.db_mysql && !a.db_pgsql {
        return None;
    }
    let pg_only = a.db_pgsql && !a.db_mysql;
    let (connection, host, port) = if pg_only {
        ("pgsql", "localhost", "5432")
    } else {
        ("mysql", "127.0.0.1", "3306")
    };
    Some(format!(
        "DB_CONNECTION=\"{connection}\"\n\
         DB_HOST=\"{host}\"\n\
         DB_PORT=\"{port}\"\n\
         DB_DATABASE=\"{}\"\n\
         DB_USERNAME=\"{}\"\n\
         DB_PASSWORD=\"{password}\"\n",
        a.dbname(),
        a.username()
    ))
}

fn whoami_arg() -> String {
    std::env::var("SUDO_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "$USER".into())
}
