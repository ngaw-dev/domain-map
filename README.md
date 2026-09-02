# ngaw-domain

Interactive CLI that sets up web server vhosts, SSL and database scaffolding for a domain.

## What it does

Prompts for email, server username, domain, subdomain, server (nginx/apache) and options (public docroot, Let's Encrypt, HTTPS, PHP-FPM, logs, deny dotfiles, MySQL/PostgreSQL), then:

- Creates `/var/www/<domain.tld>/<subdomain>[/public]` with a placeholder `index.html`
- Writes and enables the vhost config (`sites-available` + symlink / `a2ensite`)
- Generates a self-signed certificate when HTTPS is enabled without Let's Encrypt
- Saves database SQL to `/var/www/<domain.tld>/<fqn>-mysql.sql` or `<fqn>-pgsql.sql` for manual import
- Saves a ready-to-use `.env` to `/var/www/<domain.tld>/<fqn>.env`
- Finishes with `nginx -t` / `apachectl configtest` and a service restart
- Failures don't abort the run — all steps execute and a summary of failed commands is shown at the end

## Usage

```bash
ngaw-domain                            # interactive prompts, dry run
ngaw-domain -y                         # interactive prompts, execute
ngaw-domain sub.domain.tld             # non-interactive defaults, dry run
ngaw-domain sub.domain.tld user        # non-interactive, dry run
ngaw-domain sub.domain.tld user -y     # non-interactive, execute
```

Defaults for the argument mode: nginx, public docroot, HTTPS (self-signed), PHP-FPM, logs, deny dotfiles, MySQL (no Let's Encrypt, no PostgreSQL).

With `-y` each step prints as it runs, command output is hidden unless a step fails, and at the end you get:

- `✔ Setup complete` with the site root, or a red summary of failed steps with their output
- Manual follow-up commands, e.g.:
  - `sudo mysql -u root -p < /var/www/mango.com/pear10.mango.com-mysql.sql`
  - `sudo psql -U postgres -f /var/www/mango.com/pear10.mango.com-pgsql.sql`

## Build

```bash
cargo build --release
```

Binary: `target/release/ngaw-domain`

## Manual copy to a live server

Build the Linux binary (from macOS, cross-compile via Docker):

```bash
docker run --rm -v "$PWD:/app" -w /app rust:1 \
  cargo build --release --target x86_64-unknown-linux-gnu
```

Copy to the server and install:

```bash
# via docker
docker cp target/x86_64-unknown-linux-gnu/release/ngaw-domain <container>:/usr/local/bin/ngaw-domain

# via scp to a live server
scp target/x86_64-unknown-linux-gnu/release/ngaw-domain user@<server-ip>:/tmp/
ssh user@<server-ip>
sudo mv /tmp/ngaw-domain /usr/local/bin/
sudo chmod +x /usr/local/bin/ngaw-domain
```

Then run on the server (the setup steps use `sudo`, so run as a user with sudo rights):

```bash
ngaw-domain sub.domain.tld $USER -y
```

## Development

```bash
cargo build
cargo test
```
