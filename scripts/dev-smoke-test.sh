#!/usr/bin/env bash
# End-to-end smoke test against local LLDAP + self-service portal.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TIMEOUT="${ROOT}/scripts/with-timeout.sh"
BASE="${BASE_URL:-http://127.0.0.1:8080}"
COOKIE_JAR="$(mktemp)"
TMP="$(mktemp -d)"
trap 'rm -f "$COOKIE_JAR"; rm -rf "$TMP"' EXIT

if [[ ! -x "$TIMEOUT" ]]; then
  TIMEOUT="timeout"
fi

log() { printf '==> %s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

# --- prerequisites ---
log "PostgreSQL"
PGPASSWORD="${PGPASSWORD:-devpostgrespass}" "$TIMEOUT" 10 psql \
  'postgres://lldap_selfservice:devpostgrespass@localhost/lldap_selfservice' -c 'SELECT 1' >/dev/null

log "LLDAP HTTP"
"$TIMEOUT" 10 curl -sf "${LLDAP_URL:-http://127.0.0.1:17170}/auth/simple/login" \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"devadminpass"}' >/dev/null

log "App HTTP"
"$TIMEOUT" 10 curl -sf -o /dev/null "$BASE/"

# --- admin login ---
log "Admin login (testadmin)"
LOGIN_CODE=$("$TIMEOUT" 15 curl -s -o /dev/null -w '%{http_code}' \
  -c "$COOKIE_JAR" -b "$COOKIE_JAR" \
  -X POST "$BASE/admin/login" \
  -d 'username=testadmin&password=testadminpass')
[[ "$LOGIN_CODE" == "303" ]] || fail "login returned HTTP $LOGIN_CODE (expected 303)"

grep -q 'lldap_selfservice_session' "$COOKIE_JAR" || fail "no session cookie after login"

# --- dashboard + csrf ---
log "Dashboard"
"$TIMEOUT" 10 curl -sf -b "$COOKIE_JAR" "$BASE/admin" -o "$TMP/dashboard.html"
CSRF=$(grep -oP 'name="csrf_token" value="\K[^"]+' "$TMP/dashboard.html" | head -1)
[[ -n "$CSRF" ]] || fail "csrf_token not found on dashboard"

# --- create invite ---
log "Create invite"
"$TIMEOUT" 15 curl -sf -b "$COOKIE_JAR" -X POST "$BASE/api/invites" \
  -d "csrf_token=${CSRF}&label=smoke-test" -o "$TMP/invite-created.html"
INVITE_URL=$(grep -oP 'http://127\.0\.0\.1:8080/invite/\K[a-f0-9]+' "$TMP/invite-created.html" | head -1)
[[ -n "$INVITE_URL" ]] || fail "invite URL not found in response"
TOKEN="$INVITE_URL"
log "Invite token: ${TOKEN:0:8}…"

# --- signup via invite ---
TEST_USER="smoke$(date +%s | tail -c 6)"
TEST_PASS='smokepass123'
log "Signup user $TEST_USER"
"$TIMEOUT" 10 curl -sf "$BASE/invite/$TOKEN" -o "$TMP/invite-form.html"
"$TIMEOUT" 30 curl -sf -X POST "$BASE/invite/$TOKEN" \
  -d "uid=${TEST_USER}&email=${TEST_USER}@bambam.fun&password=${TEST_PASS}&password_confirm=${TEST_PASS}" \
  -o "$TMP/signup.html"
grep -qi "$TEST_USER" "$TMP/signup.html" || fail "signup did not show success for $TEST_USER"

# --- verify user in LLDAP ---
log "Verify user in LLDAP"
JWT=$("$TIMEOUT" 10 curl -sf http://127.0.0.1:17170/auth/simple/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"selfservice","password":"devservicepassword"}' | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])")
USER_FOUND=$("$TIMEOUT" 10 curl -sf http://127.0.0.1:17170/api/graphql \
  -H "Authorization: Bearer $JWT" -H 'Content-Type: application/json' \
  -d "{\"query\":\"query(\$id: String!) { user(userId: \$id) { id } }\",\"variables\":{\"id\":\"$TEST_USER\"}}" \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if d.get('data',{}).get('user') else 'no')")
[[ "$USER_FOUND" == "yes" ]] || fail "user $TEST_USER not found in LLDAP"

# --- password reset ---
log "Password reset"
"$TIMEOUT" 10 curl -sf -b "$COOKIE_JAR" "$BASE/admin/users" -o "$TMP/users.html"
CSRF=$(grep -oP 'name="csrf_token" value="\K[^"]+' "$TMP/users.html" | head -1)
NEW_PASS='newsmokepass123'
RESET_CODE=$("$TIMEOUT" 30 curl -s -o /dev/null -w '%{http_code}' -b "$COOKIE_JAR" \
  -X POST "$BASE/api/users/${TEST_USER}/reset-password" \
  -d "csrf_token=${CSRF}&password=${NEW_PASS}&password_confirm=${NEW_PASS}")
[[ "$RESET_CODE" == "303" ]] || fail "password reset returned HTTP $RESET_CODE (expected 303)"

# --- verify new password via LDAP bind (python ldap3 or curl login) ---
log "Verify login with new password"
LLDAP_LOGIN=$("$TIMEOUT" 10 curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:17170/auth/simple/login \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"${TEST_USER}\",\"password\":\"${NEW_PASS}\"}")
[[ "$LLDAP_LOGIN" == "200" ]] || fail "LLDAP login with new password returned HTTP $LLDAP_LOGIN"

log "All smoke tests passed"
