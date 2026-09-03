use inquire::validator::{ErrorMessage, Validation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Server {
    Nginx,
    Apache,
}

#[derive(Debug)]
pub struct Answers {
    pub email: String,
    pub server_user: String,
    pub domain: String,
    pub subdomain: String,
    pub server: Server,
    pub public: bool,
    pub letsencrypt: bool,
    pub nginx_https: bool,
    pub nginx_php: bool,
    pub nginx_logs: bool,
    pub nginx_deny: bool,
    pub docker: bool,
    pub docker_host_port: u16,
    pub db_mysql: bool,
    pub db_pgsql: bool,
}

impl Answers {
    /// Fully-qualified name: `sub.domain` when a subdomain is given, else the
    /// bare domain (implicit `www` handled via server_name alias).
    pub fn fqn(&self) -> String {
        if self.subdomain.len() > 1 {
            format!("{}.{}", self.subdomain, self.domain)
        } else {
            self.domain.clone()
        }
    }

    /// Implicit `www` when no explicit subdomain was provided.
    pub fn is_implicit_www(&self) -> bool {
        self.subdomain.len() <= 1
    }

    /// Document root relative to /var/www, e.g. `example.com/www/public`.
    pub fn dir(&self) -> String {
        let subdomain = if self.is_implicit_www() {
            "www"
        } else {
            &self.subdomain
        };
        let suffix = if self.public { "/public" } else { "" };
        format!("{}/{}{}", self.domain, subdomain, suffix)
    }

    /// Database name derived from the fqn (`.` and `-` -> `_`).
    pub fn dbname(&self) -> String {
        self.fqn().replace(['.', '-'], "_")
    }

    pub fn username(&self) -> String {
        format!("{}_user", self.dbname())
    }
}

/// Same domain regex as the original PHP tool.
pub fn validate_domain(domain: &str) -> Validation {
    let ok = domain
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
        && valid_domain_labels(domain);
    if ok {
        Validation::Valid
    } else {
        Validation::Invalid(ErrorMessage::Custom(
            "Enter a domain with a TLD, e.g. example.com".into(),
        ))
    }
}

/// Port must be a number in 1..=65535.
pub fn validate_port(port: &str) -> Validation {
    match port.parse::<u16>() {
        Ok(p) if p > 0 => Validation::Valid,
        _ => Validation::Invalid(ErrorMessage::Custom(
            "Enter a port between 1 and 65535".into(),
        )),
    }
}

fn valid_domain_labels(domain: &str) -> bool {
    if domain.is_empty() {
        return false;
    }
    let labels: Vec<&str> = domain.trim_matches('.').split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}
