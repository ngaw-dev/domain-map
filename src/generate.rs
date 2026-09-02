use crate::config::{Answers, Server};

/// One shell command to run, with an optional stdin payload (heredoc body).
#[derive(Debug)]
pub struct Step {
    pub description: String,
    pub icon: &'static str,
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
        icon: "📁",
        program: "mkdir".into(),
        args: vec!["-p".into(), root.clone()],
        stdin: None,
    });
    steps.push(Step {
        description: "Write placeholder index.html".into(),
        icon: "📄",
        program: "tee".into(),
        args: vec![
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
        description: format!("Grant ownership to {}:www-data", a.server_user),
        icon: "👤",
        program: "chown".into(),
        args: vec![
            "-R".into(),
            format!("{}:www-data", a.server_user),
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
            icon: "🔒",
            program: "certbot".into(),
            args: vec![
                flag.into(),
                "-d".into(),
                fqn.clone(),
            ],
            stdin: None,
        });
    }

    // Databases: save SQL to the domain folder for the user to import.
    let password = db_password;
    let dbname = a.dbname();
    let username = a.username();
    let domain_folder = a.domain.split('.').next().unwrap_or(&a.domain);
    let has_files = a.db_mysql || a.db_pgsql || env_snippet(a, db_password).is_some();
    if has_files {
        steps.push(Step {
            description: format!("Create folder /var/www/{domain_folder}"),
            icon: "📁",
            program: "mkdir".into(),
            args: vec!["-p".into(), format!("/var/www/{domain_folder}")],
            stdin: None,
        });
    }
    if a.db_mysql {
        let sql = format!(
            "CREATE DATABASE {dbname};\n\
             CREATE USER '{username}'@'localhost' IDENTIFIED BY '{password}';\n\
             GRANT ALL PRIVILEGES ON {dbname}.* TO '{username}'@'localhost';\n\
             FLUSH PRIVILEGES;\n"
        );
        steps.push(Step {
            description: format!("Save MySQL SQL to /var/www/{domain_folder}/{fqn}-mysql.sql"),
            icon: "🗃️",
            program: "tee".into(),
            args: vec![format!("/var/www/{domain_folder}/{fqn}-mysql.sql")],
            stdin: Some(sql),
        });
    }
    if a.db_pgsql {
        let sql = format!(
            "CREATE DATABASE {dbname};\n\
             CREATE USER {username} WITH PASSWORD '{password}';\n\
             GRANT ALL PRIVILEGES ON DATABASE {dbname} TO {username};\n\
             GRANT ALL ON SCHEMA public TO {username};\n"
        );
        steps.push(Step {
            description: format!("Save PostgreSQL SQL to /var/www/{domain_folder}/{fqn}-pgsql.sql"),
            icon: "🗃️",
            program: "tee".into(),
            args: vec![format!("/var/www/{domain_folder}/{fqn}-pgsql.sql")],
            stdin: Some(sql),
        });
    }

    // Save .env as <fqn>.env in the domain folder, e.g.
    // apple.mango.com -> /var/www/mango/apple.mango.com.env
    if let Some(env) = env_snippet(a, db_password) {
        let env_path = format!("/var/www/{domain_folder}/{fqn}.env");
        steps.push(Step {
            description: format!("Save .env to {env_path}"),
            icon: "📝",
            program: "tee".into(),
            args: vec![env_path],
            stdin: Some(env),
        });
    }

    // Finish: validate and restart the web server so everything is live.
    match a.server {
        Server::Nginx => {
            steps.push(Step {
                description: "Test nginx configuration".into(),
                icon: "🧪",
                program: "nginx".into(),
                args: vec!["-t".into()],
                stdin: None,
            });
            steps.push(Step {
                description: "Restart nginx".into(),
                icon: "🔄",
                program: "service".into(),
                args: vec!["nginx".into(), "restart".into()],
                stdin: None,
            });
        }
        Server::Apache => {
            steps.push(Step {
                description: "Test apache configuration".into(),
                icon: "🧪",
                program: "apachectl".into(),
                args: vec!["configtest".into()],
                stdin: None,
            });
            steps.push(Step {
                description: "Restart apache".into(),
                icon: "🔄",
                program: "service".into(),
                args: vec!["apache2".into(), "restart".into()],
                stdin: None,
            });
        }
    }

    // Wrap every step in sudo (we run as a normal user); runuser steps are
    // already root-only and take the postgres user explicitly.
    for step in &mut steps {
        if step.program != "runuser" {
            let mut args = std::mem::take(&mut step.args);
            args.insert(0, step.program.clone());
            step.program = "sudo".into();
            step.args = args;
        }
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
            icon: "⚙️",
            program: "tee".into(),
            args: vec![
                format!("/etc/nginx/sites-available/{fqn}.conf"),
            ],
            stdin: Some(conf),
        },
        Step {
            description: "Enable site".into(),
            icon: "🔗",
            program: "ln".into(),
            args: vec![
                "-sfn".into(),
                format!("/etc/nginx/sites-available/{fqn}.conf"),
                "/etc/nginx/sites-enabled/".into(),
            ],
            stdin: None,
        },
    ];

    if a.nginx_https && !a.letsencrypt {
        steps.push(Step {
            description: "Generate self-signed certificate".into(),
            icon: "🔑",
            program: "openssl".into(),
            args: vec![
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
            icon: "⚙️",
            program: "tee".into(),
            args: vec![
                format!("/etc/apache2/sites-available/{fqn}.conf"),
            ],
            stdin: Some(conf),
        },
        Step {
            description: "Enable site".into(),
            icon: "🔗",
            program: "a2ensite".into(),
            args: vec![format!("{fqn}.conf")],
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
