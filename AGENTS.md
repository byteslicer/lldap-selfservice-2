# AGENTS.md

## Cursor Cloud specific instructions

### Overview

This is a Rust (Axum) web application that provides a self-service portal for LLDAP. It depends on PostgreSQL and LLDAP as external services. See `README.md` for full feature list and configuration.

### Build / Lint / Test

- **Build:** `cargo build`
- **Lint:** `cargo clippy` (14 warnings are pre-existing and expected)
- **Test:** `cargo test` (no test files currently exist; passes with 0 tests)

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

The app will listen on `127.0.0.1:8080`. Dev config is at `/workspace/config.toml` (gitignored).

### Dev credentials

| Account | Username | Password | Purpose |
|---------|----------|----------|---------|
| LLDAP admin | `admin` | `devadminpass` | LLDAP admin panel & API |
| Service account | `selfservice` | `devservicepassword` | App's service account for GraphQL API |
| Test admin | `testadmin` | `testadminpass` | Member of `selfservice_admins` + `selfservice_password_reset` |

### Known issues (fixed on integration branch)

These were found during LLDAP v0.6.3 smoke testing and are fixed in `cursor/lldap-integration-test-a716`:

1. ~~**Login panics**~~ — LDAP bind now runs in `spawn_blocking` to avoid tokio runtime panic.
2. ~~**GraphQL schema mismatch**~~ — `addUserToGroup` response uses `{ ok }` (not `{ success }`).
3. ~~**Password reset 500**~~ — `user_is_lldap_admin` GraphQL query used structs requiring fields not returned by LLDAP.

Run the smoke test (all steps use `timeout`):

```bash
./scripts/dev-smoke-test.sh
```

### Secret files

Dev secret files are at `/tmp/lldap-selfservice-secrets/`:
- `session_secret` — session signing key (32+ chars)
- `service_password` — LLDAP service account password
