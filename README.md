# ngaw-domain

Interactive CLI that sets up web server vhosts, SSL and database scaffolding for a domain.

## What it does

Prompts for email, server username, domain, subdomain, server (nginx/apache) and options (public docroot, Let's Encrypt, HTTPS, PHP-FPM, logs, deny dotfiles, MySQL/PostgreSQL), then:

- Creates `/var/www/<domain.tld>/<subdomain>[/public]` with a placeholder `index.html`
- Writes and enables the vhost config (`sites-available` + symlink / `a2ensite`)
- Generates a self-signed certificate when HTTPS is enabled without Let's Encrypt
- Saves database SQL to `/var/www/<domain.tld>/mysql-<fqn>.sql` or `pgsql-<fqn>.sql` for manual import
- Saves a ready-to-use `.env` to `/var/www/<domain.tld>/env-<fqn>`
- Writes the nginx vhost with `access_log ... json_logs;` (JSON-formatted access logs) when logging is enabled
- Appends Fluent Bit `[INPUT]`/`[OUTPUT]` blocks shipping the site's access/error logs to OpenObserve, then restarts fluent-bit
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

## Environment variables (required)

Before running, the tool reads its OpenObserve log-shipping credentials from the environment. It aborts at startup if these are missing:

```bash
export OPENOBSERVE_USER="root@example.com"   # OpenObserve HTTP ingest user
export OPENOBSERVE_PASS="..."                # OpenObserve HTTP ingest password
```

Optional overrides:

```bash
export OPENOBSERVE_HOST="139.84.204.42"      # default: 139.84.204.42
export OPENOBSERVE_PORT="5080"               # default: 5080
```

These are substituted into the Fluent Bit `HTTP_User` / `HTTP_Passwd` fields — no credentials are ever hardcoded in the binary.

## Log shipping (Fluent Bit → OpenObserve)

When nginx logging is enabled, the tool appends to `/etc/fluent-bit/fluent-bit.conf` two `[INPUT]` tails and two `[OUTPUT]` http blocks per site, then runs `systemctl restart fluent-bit`. Streams auto-create in OpenObserve on first ingest.

Naming rules (enforced by the generator):

| Item | Pattern | Example |
|---|---|---|
| Fluent Bit tag | `<site>.access` / `<site>.error` | `s3_api.access` |
| OpenObserve stream | `<site>` / `<site>_error` | `s3_api` / `s3_api_error` |
| Tail DB | `/var/lib/fluent-bit/<site>-access.db` / `-error.db` | `s3_api-access.db` |

where `<site>` is the subdomain with `.`/`-` sanitized to `_` (`s3-api` → `s3_api`).

Server prerequisites (assumed already in place on your servers, matching e.g. `monitor.amolw.xyz`):

- nginx: a `log_format json_logs` defined in `nginx.conf` (used as the vhost access-log format)
- Fluent Bit: a `json` and an `nginx_error` parser available (parsers config)

## Build

```bash
cargo build --release
```

Binary: `target/release/ngaw-domain`

## Install from Git

On any machine with Rust:

```bash
cargo install --git https://github.com/ngaw-dev/domain-map.git
```

Installs the `ngaw-domain` binary to `~/.cargo/bin` (make sure it's on your `PATH`).

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
