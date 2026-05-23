# LLDAP Self-Service Portal

Community admin portal for [LLDAP](https://github.com/lldap/lldap): create one-time invite links for new members and reset passwords (with separate LDAP group permissions).

## Features

- **Admin login** via LDAP bind; access gated by configurable group membership
- **Invite links** (one-time): new users set username, email, and password
- **Password reset** for admins in an additional LDAP group
- **Configurable default groups** on signup (e.g. `cn=bambam` for Matrix/Zulip)
- Uses LLDAP GraphQL + `lldap_set_password` (OPAQUE) for user management

## LLDAP setup (one-time)

Create these groups in the LLDAP UI (`ou=groups,dc=bambam,dc=fun`):

| Group | Purpose |
|-------|---------|
| `selfservice_admins` | Create invites, list users |
| `selfservice_password_reset` | Reset other users' passwords |

Add community admins to `selfservice_admins`. Add a subset to `selfservice_password_reset` if they may reset passwords.

Create a **service account** (e.g. `selfservice`) in the `lldap_admin` group for GraphQL mutations. Store its password in sops when deploying.

## Configuration

Copy [`config.example.toml`](config.example.toml) to `/etc/lldap-selfservice/config.toml` and adjust.

| Variable | Description |
|----------|-------------|
| `CONFIG_PATH` | Path to config TOML (default `/etc/lldap-selfservice/config.toml`) |
| `DATABASE_URL` | PostgreSQL URL for app DB |
| `SESSION_SECRET_FILE` | File with 32+ char secret for session cookies |
| `LLDAP_SERVICE_PASSWORD_FILE` | LLDAP service account password |
| `LLDAP_HTTP_URL` | LLDAP HTTP API (default `http://127.0.0.1:17170`) |
| `LLDAP_SET_PASSWORD_BIN` | Path to `lldap_set_password` |
| `PUBLIC_BASE_URL` | Public URL for invite links |
| `LISTEN` | Bind address (default `127.0.0.1:8080`) |
| `STATIC_DIR` | Static assets directory |

## Build

```bash
nix build
# binary: ./result/bin/lldap-selfservice
```

Dev shell:

```bash
nix develop
cargo run
```

## Database

Uses a **separate** PostgreSQL database (`lldap_selfservice`), not LLDAP's internal DB.

```sql
CREATE USER lldap_selfservice;
CREATE DATABASE lldap_selfservice OWNER lldap_selfservice;
```

Migrations run automatically on startup.

## NixOS module

Import the flake module in your server config:

```nix
{
  inputs.lldap-selfservice.url = "path:/path/to/lldap-selfservice";

  services.lldap-selfservice = {
    enable = true;
    package = inputs.lldap-selfservice.packages.${pkgs.system}.default;
    configFile = /etc/lldap-selfservice/config.toml;
    publicBaseUrl = "https://selfservice.bambam.fun";
    sessionSecretFile = config.sops.secrets.lldap_selfservice_session.path;
    servicePasswordFile = config.sops.secrets.lldap_selfservice_service_pass.path;
  };
}
```

### PostgreSQL (add to your host config)

```nix
services.postgresql.ensureDatabases = [ "lldap_selfservice" ];
services.postgresql.ensureUsers = [
  { name = "lldap_selfservice"; ensureDBOwnership = true; }
];
```

### SOPS secrets (add to `secrets.yaml`)

```yaml
lldap_selfservice_session: ...
lldap_selfservice_service_pass: ...
```

### Nginx (example vhost)

```nix
services.nginx.virtualHosts."selfservice.bambam.fun" = {
  forceSSL = true;
  locations."/" = {
    proxyPass = "http://127.0.0.1:8080";
    proxyWebsockets = true;
  };
};
```

## Routes

| Path | Description |
|------|-------------|
| `/admin/login` | Admin LDAP login |
| `/admin` | Invite dashboard |
| `/admin/users` | User search + password reset |
| `/invite/:token` | Public signup form |
| `/api/invites` | Create/list invites (session) |
| `/api/users/:uid/reset-password` | Reset password (extra group) |

## Security notes

- Invite tokens are stored as SHA-256 hashes only
- Session cookies are HMAC-signed and `httpOnly`
- CSRF tokens on all admin POST forms
- Password reset blocked for `lldap_admin` members
- Human admins never receive LLDAP admin JWTs in the browser

## License

MIT
