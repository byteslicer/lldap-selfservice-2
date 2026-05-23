# AGENTS.md

## Cursor Cloud specific instructions

### Overview

This is a Rust (Axum) web application that provides a self-service portal for LLDAP. It depends on PostgreSQL and LLDAP as external services. See `README.md` for full feature list and configuration.

### Shell commands: always use timeouts

Commands that talk to PostgreSQL, LLDAP, HTTP, or `sudo` can hang on **interactive password prompts**. Wrap them with `timeout` so a stuck command exits with code **124** instead of blocking forever.

- Helper: `scripts/with-timeout.sh [seconds] command …` (default **30s**)
- Or prefix directly: `timeout 10s …`

**PostgreSQL** — do not use a bare `psql postgres://lldap_selfservice@localhost/…` URL; TCP auth requires a password and will prompt interactively. Use one of:

```bash
# As postgres superuser (no password prompt)
timeout 10s sudo -u postgres psql -d lldap_selfservice -c '\dt'

# As app user (password in URL or PGPASSWORD; dev password: devpostgrespass)
PGPASSWORD=devpostgrespass timeout 10s \
  psql 'postgres://lldap_selfservice:devpostgrespass@localhost/lldap_selfservice' -c '\dt'
```

**LLDAP / HTTP checks:**

```bash
timeout 5s curl -sf http://127.0.0.1:17170/auth/simple/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"devadminpass"}'
```

**LLDAP server** — avoid bare `sudo … lldap run` in a foreground shell (may wait for a sudo password). Prefer env vars on the binary if already installed, or run via tmux after confirming `sudo -n true` works:

```bash
timeout 5s sudo -n true && echo ok || echo 'sudo needs password — use tmux + passwordless sudo or run lldap without sudo'
```

If `timeout` exits **124**, the command hung (often waiting for input).

### Build / Lint / Test

- **Build:** `timeout 300s cargo build`
- **Lint:** `timeout 300s cargo clippy` (14 warnings are pre-existing and expected)
- **Test:** `timeout 120s cargo test` (no test files currently exist; passes with 0 tests)

### Running the application

The app requires three services to be running:

1. **PostgreSQL** with a `lldap_selfservice` database
2. **LLDAP** (lightweight LDAP server) on ports 3890 (LDAP) and 17170 (HTTP/GraphQL)
3. The app itself via `cargo run`

**Start PostgreSQL:**
```
sudo pg_ctlcluster 16 main start
```

**Start LLDAP** (installed at `/opt/lldap/amd64-lldap/`):
```
sudo LLDAP_LDAP_USER_PASS=devadminpass LLDAP_JWT_SECRET=devjwtsecretmustbe32charslong12 \
  /opt/lldap/amd64-lldap/lldap run --config-file /opt/lldap/lldap_config.toml
```

**Start the app:**
```
CONFIG_PATH=/workspace/config.toml \
SESSION_SECRET_FILE=/tmp/lldap-selfservice-secrets/session_secret \
LLDAP_SERVICE_PASSWORD_FILE=/tmp/lldap-selfservice-secrets/service_password \
RUST_LOG=info cargo run
```

The app will listen on `127.0.0.1:8080`. Dev `config.toml` is gitignored; database URL should include the dev password: `postgres://lldap_selfservice:devpostgrespass@localhost/lldap_selfservice`.

### Dev credentials

| Account | Username | Password | Purpose |
|---------|----------|----------|---------|
| LLDAP admin | `admin` | `devadminpass` | LLDAP admin panel & API |
| Service account | `selfservice` | `devservicepassword` | App's service account for GraphQL API |
| Test admin | `testadmin` | `testadminpass` | Member of `selfservice_admins` + `selfservice_password_reset` |
| PostgreSQL | `lldap_selfservice` | `devpostgrespass` | App database (TCP/scram auth on localhost) |

### Known pre-existing code issues

1. **Login panics:** The `ldap3` crate's sync API (`LdapConn`) is called inside the tokio async runtime, causing a "Cannot start a runtime from within a runtime" panic on login attempts. This is a code bug, not an environment issue.
2. **GraphQL schema mismatch:** The code uses `{ success }` in the `addUserToGroup` mutation response, but LLDAP v0.6.3 uses `{ ok }`. This causes 400 errors during user signup's group assignment step.
3. **`lldap_set_password` flags:** Fixed in main (`--base-url`, `--token`, `--username`). Dev binary: `/opt/lldap/amd64-lldap/lldap_set_password`.

### Secret files

Dev secret files are at `/tmp/lldap-selfservice-secrets/`:
- `session_secret` — session signing key (32+ chars)
- `service_password` — LLDAP service account password
