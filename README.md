# ngaw-domain

Interactive CLI that sets up web server vhosts, databases and SSL for a domain — a Rust port of the [amol-co-nz/domain](https://www.amol.co.nz/domain) PHP tool.

## What it does

Prompts for email, domain, subdomain, server (nginx/apache) and options (public docroot, Let's Encrypt, HTTPS, PHP-FPM, logs, deny dotfiles, MySQL/PostgreSQL), then executes the full setup:

- Creates `/var/www/<domain>/<subdomain>[/public]` with a placeholder `index.html`
- Writes and enables the vhost config (`sites-available` + symlink / `a2ensite`)
- Generates a self-signed certificate when HTTPS is enabled without Let's Encrypt
- Runs `nginx -t` / `certbot` / service restarts
- Creates the database, user and password, and prints a ready-to-use `.env`

Every command is displayed for approval before anything runs. Use `--dry-run` (or `-n`) to preview without executing.

## Install

```bash
cargo install --git https://github.com/ngaw/ngaw-domain
```

## Usage

```bash
sudo -E ngaw-domain            # run with sudo rights on the target server
ngaw-domain --dry-run          # preview the commands only
```

> The `chown` step targets `SUDO_USER` (or `USER`) so the docroot ends up owned by the invoking user.

## Development

```bash
cargo build
cargo test
```
