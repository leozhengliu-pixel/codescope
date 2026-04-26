#!/usr/bin/env bash
set -euo pipefail

repo_root=${1:-$(pwd)}
if [[ ! -d "$repo_root" ]]; then
  echo "repo root not found: $repo_root" >&2
  exit 1
fi

api_pid=""
runtime_dir=""
api_log=""
worker_log=""

cleanup() {
  if [[ -n "$api_pid" ]] && kill -0 "$api_pid" 2>/dev/null; then
    kill "$api_pid" 2>/dev/null || true
    wait "$api_pid" 2>/dev/null || true
  fi
  if [[ -n "$runtime_dir" ]] && [[ -d "$runtime_dir" ]]; then
    rm -rf "$runtime_dir"
  fi
}
trap cleanup EXIT INT TERM

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "required command missing: $1" >&2
    exit 1
  fi
}

require_command cargo
require_command python3

redact_json_file() {
  local file_path=$1
  python3 - "$file_path" <<'PY'
import json
import sys

REDACT_KEYS = {"secret", "session_secret"}

def redact(value):
    if isinstance(value, dict):
        return {
            key: ("<redacted>" if key in REDACT_KEYS else redact(item))
            for key, item in value.items()
        }
    if isinstance(value, list):
        return [redact(item) for item in value]
    return value

path = sys.argv[1]
with open(path, 'r', encoding='utf-8') as fh:
    body = fh.read()

try:
    data = json.loads(body)
except json.JSONDecodeError:
    print(body)
else:
    print(json.dumps(redact(data), indent=2))
PY
}

json_assert() {
  local file_path=$1
  local check_name=$2
  python3 - "$file_path" "$check_name" <<'PY'
import json
import os
import sys

REDACT_KEYS = {"secret", "session_secret"}


def redact(value):
    if isinstance(value, dict):
        return {
            key: ("<redacted>" if key in REDACT_KEYS else redact(item))
            for key, item in value.items()
        }
    if isinstance(value, list):
        return [redact(item) for item in value]
    return value


path, check_name = sys.argv[1], sys.argv[2]
with open(path, 'r', encoding='utf-8') as fh:
    data = json.load(fh)

thread_id = os.environ.get("JSON_ASSERT_THREAD_ID", "")
webhook_id = os.environ.get("JSON_ASSERT_WEBHOOK_ID", "")
run_id = os.environ.get("JSON_ASSERT_RUN_ID", "")

checks = {
    "health_ok": lambda: data.get("status") == "ok" and data.get("service") == "sourcebot-api",
    "ready_file_ok": lambda: data.get("status") == "ok" and data.get("service") == "sourcebot-api" and data.get("metadata_backend") == "file" and data.get("database") is None,
    "bootstrap_complete": lambda: data.get("bootstrap_required") is False,
    "login_has_session": lambda: data.get("user_id") == "local_user_bootstrap_admin" and bool(data.get("session_id")) and bool(data.get("session_secret")),
    "auth_me_admin": lambda: data.get("user_id") == "local_user_bootstrap_admin" and data.get("email") == "admin@example.com",
    "connections_github": lambda: isinstance(data, list) and len(data) == 1 and data[0].get("id") == "conn_github" and data[0].get("kind") == "github",
    "search_healthz": lambda: data.get("repo_id") == "repo_sourcebot_rewrite" and any(item.get("repo_id") == "repo_sourcebot_rewrite" and "healthz" in item.get("line", "") for item in data.get("results", [])),
    "ask_completion": lambda: data.get("provider") == "stub-citations" and bool(data.get("thread_id")) and "healthz" in data.get("answer", "").lower() and len(data.get("citations", [])) >= 1,
    "threads_contains_thread": lambda: isinstance(data, list) and any(item.get("id") == thread_id for item in data),
    "thread_detail_matches_scope": lambda: data.get("id") == thread_id and len(data.get("messages", [])) >= 2 and data.get("repo_scope") == ["repo_sourcebot_rewrite"],
    "create_webhook_secret": lambda: data.get("organization_id") == "org_acme" and data.get("connection_id") == "conn_github" and data.get("repository_id") == "repo_demo_docs" and bool(data.get("secret")),
    "intake_accepted": lambda: data.get("webhook_id") == webhook_id and data.get("accepted") is True,
    "runs_before_contains_queued": lambda: isinstance(data, list) and any(item.get("webhook_id") == webhook_id and item.get("status") == "queued" for item in data),
    "run_before_queued": lambda: data.get("id") == run_id and data.get("status") == "queued",
    "runs_after_contains_completed": lambda: isinstance(data, list) and any(item.get("id") == run_id and item.get("status") == "completed" for item in data),
    "run_after_completed": lambda: data.get("id") == run_id and data.get("status") == "completed",
    "organization_state_completed": lambda: any(item.get("id") == run_id and item.get("status") == "completed" for item in data.get("review_agent_runs", [])),
}

if check_name not in checks:
    raise SystemExit(f"unknown json assertion: {check_name}")

if not checks[check_name]():
    raise SystemExit(
        f"json assertion failed: {check_name}\nbody={json.dumps(redact(data), indent=2)}"
    )
PY
}

http_json() {
  local method=$1
  local url=$2
  local expected_status=$3
  local output_file=$4
  local auth_header=${5:-}
  local payload_file=${6:-}
  local auth_header_file=""

  if [[ -n "$auth_header" ]]; then
    auth_header_file="$runtime_dir/http_auth_header.txt"
    printf '%s' "$auth_header" >"$auth_header_file"
  fi

  local status
  status=$(HTTP_JSON_AUTH_HEADER_FILE="$auth_header_file" HTTP_JSON_PAYLOAD_PATH="$payload_file" \
    python3 - "$method" "$url" "$output_file" <<'PY'
import os
import sys
import urllib.error
import urllib.request

method, url, output_path = sys.argv[1:4]
auth_header_file = os.environ.get("HTTP_JSON_AUTH_HEADER_FILE", "")
payload_path = os.environ.get("HTTP_JSON_PAYLOAD_PATH", "")
headers = {"content-type": "application/json"}
if auth_header_file:
    with open(auth_header_file, 'r', encoding='utf-8') as fh:
        auth_header = fh.read()
    if auth_header:
        headers["authorization"] = auth_header
payload = None
if payload_path:
    with open(payload_path, 'rb') as fh:
        payload = fh.read()
request = urllib.request.Request(url, data=payload, headers=headers, method=method)
try:
    with urllib.request.urlopen(request) as response:
        body = response.read()
        status = response.getcode()
except urllib.error.HTTPError as error:
    body = error.read()
    status = error.code
with open(output_path, 'wb') as fh:
    fh.write(body)
print(status)
PY
)
  if [[ "$status" != "$expected_status" ]]; then
    echo "unexpected HTTP status for $method $url: got $status expected $expected_status" >&2
    if [[ -f "$output_file" ]]; then
      printf 'response body (redacted):\n%s\n' "$(redact_json_file "$output_file")" >&2
    fi
    exit 1
  fi
}

http_get_ok() {
  local url=$1
  local output_file=$2
  python3 - "$url" "$output_file" <<'PY'
import sys
import urllib.request
url, output_path = sys.argv[1:3]
with urllib.request.urlopen(url) as response:
    body = response.read()
with open(output_path, 'wb') as fh:
    fh.write(body)
PY
}

runtime_dir=$(mktemp -d)
api_log="$runtime_dir/api.log"
worker_log="$runtime_dir/worker.log"

organization_state_path="$runtime_dir/organizations.json"
bootstrap_state_path="$runtime_dir/bootstrap-state.json"
local_sessions_path="$runtime_dir/local-sessions.json"

python3 - "$organization_state_path" <<'PY'
import json
import sys
path = sys.argv[1]
state = {
    "organizations": [
        {"id": "org_acme", "slug": "acme", "name": "Acme"}
    ],
    "connections": [
        {"id": "conn_github", "name": "GitHub Smoke", "kind": "github"}
    ],
    "memberships": [
        {
            "organization_id": "org_acme",
            "user_id": "local_user_bootstrap_admin",
            "role": "admin",
            "joined_at": "2026-04-21T00:00:00Z"
        }
    ],
    "accounts": [
        {
            "id": "local_user_bootstrap_admin",
            "email": "admin@example.com",
            "name": "Local Bootstrap Admin",
            "created_at": "2026-04-20T23:55:00Z"
        }
    ],
    "repo_permissions": [
        {
            "organization_id": "org_acme",
            "repository_id": "repo_sourcebot_rewrite",
            "synced_at": "2026-04-21T00:06:00Z"
        }
    ],
    "review_webhooks": [],
    "review_webhook_delivery_attempts": [],
    "review_agent_runs": []
}
with open(path, 'w', encoding='utf-8') as fh:
    json.dump(state, fh)
PY
printf '{"sessions":[]}' >"$local_sessions_path"

pushd "$repo_root" >/dev/null
cargo build -q -p sourcebot-api -p sourcebot-worker
api_bin="$repo_root/target/debug/sourcebot-api"
worker_bin="$repo_root/target/debug/sourcebot-worker"
popd >/dev/null

port=$(python3 - <<'PY'
import socket
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    print(sock.getsockname()[1])
PY
)
base_url="http://127.0.0.1:$port"

(
  cd "$repo_root"
  SOURCEBOT_BIND_ADDR="127.0.0.1:$port" \
  SOURCEBOT_DATA_DIR="$runtime_dir" \
  SOURCEBOT_LLM_PROVIDER=stub-citations \
  SOURCEBOT_LLM_MODEL=task84-smoke \
  "$api_bin"
) >"$api_log" 2>&1 &
api_pid=$!

for _ in $(seq 1 120); do
  if python3 - "$base_url/healthz" <<'PY' >/dev/null 2>&1
import sys
import urllib.request
urllib.request.urlopen(sys.argv[1]).read()
PY
  then
    break
  fi
  if ! kill -0 "$api_pid" 2>/dev/null; then
    echo "sourcebot-api exited before becoming healthy" >&2
    if [[ -f "$api_log" ]]; then
      printf 'api log:\n%s\n' "$(<"$api_log")" >&2
    fi
    exit 1
  fi
  sleep 1
done
if ! python3 - "$base_url/healthz" <<'PY' >/dev/null 2>&1
import sys
import urllib.request
urllib.request.urlopen(sys.argv[1]).read()
PY
then
  echo "sourcebot-api did not become healthy" >&2
  if [[ -f "$api_log" ]]; then
    printf 'api log:\n%s\n' "$(<"$api_log")" >&2
  fi
  exit 1
fi

health_response="$runtime_dir/health.json"
ready_response="$runtime_dir/ready.json"
bootstrap_response="$runtime_dir/bootstrap.json"
login_response="$runtime_dir/login.json"
auth_me_response="$runtime_dir/auth_me.json"
connections_response="$runtime_dir/connections.json"
search_response="$runtime_dir/search.json"
ask_response="$runtime_dir/ask.json"
threads_response="$runtime_dir/threads.json"
thread_response="$runtime_dir/thread.json"
create_webhook_response="$runtime_dir/create_webhook.json"
intake_response="$runtime_dir/intake.json"
runs_before_response="$runtime_dir/runs_before.json"
run_before_response="$runtime_dir/run_before.json"
runs_after_response="$runtime_dir/runs_after.json"
run_after_response="$runtime_dir/run_after.json"

http_get_ok "$base_url/healthz" "$health_response"
json_assert "$health_response" health_ok
http_get_ok "$base_url/readyz" "$ready_response"
json_assert "$ready_response" ready_file_ok

bootstrap_request="$runtime_dir/bootstrap_request.json"
login_request="$runtime_dir/login_request.json"
ask_request="$runtime_dir/ask_request.json"
create_webhook_request="$runtime_dir/create_webhook_request.json"
intake_request="$runtime_dir/intake_request.json"

python3 - "$bootstrap_request" "$login_request" "$ask_request" "$create_webhook_request" "$intake_request" <<'PY'
import json
import sys
bootstrap_path, login_path, ask_path, webhook_path, intake_path = sys.argv[1:6]
with open(bootstrap_path, 'w', encoding='utf-8') as fh:
    json.dump({"email": "admin@example.com", "name": "Admin User", "password": "hunter2"}, fh)
with open(login_path, 'w', encoding='utf-8') as fh:
    json.dump({"email": "admin@example.com", "password": "hunter2"}, fh)
with open(ask_path, 'w', encoding='utf-8') as fh:
    json.dump({
        "prompt": "Where is healthz implemented?",
        "repo_scope": ["repo_sourcebot_rewrite"]
    }, fh)
with open(webhook_path, 'w', encoding='utf-8') as fh:
    json.dump({
        "organization_id": "org_acme",
        "connection_id": "conn_github",
        "repository_id": "repo_demo_docs",
        "events": ["pull_request"]
    }, fh)
with open(intake_path, 'w', encoding='utf-8') as fh:
    json.dump({
        "event_type": "pull_request",
        "connection_id": "conn_github",
        "repository_id": "repo_demo_docs",
        "review_id": "review_task84_smoke",
        "external_event_id": "event_task84_smoke"
    }, fh)
PY

http_json POST "$base_url/api/v1/auth/bootstrap" 201 "$bootstrap_response" "" "$bootstrap_request"
json_assert "$bootstrap_response" bootstrap_complete

http_json POST "$base_url/api/v1/auth/login" 201 "$login_response" "" "$login_request"
json_assert "$login_response" login_has_session
auth_header=$(python3 - "$login_response" <<'PY'
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    data = json.load(fh)
print(f"Bearer {data['session_id']}:{data['session_secret']}")
PY
)

echo "[auth] bootstrap and login ok"
http_json GET "$base_url/api/v1/auth/me" 200 "$auth_me_response" "$auth_header"
json_assert "$auth_me_response" auth_me_admin
echo "[auth] auth/me ok"

http_json GET "$base_url/api/v1/auth/connections" 200 "$connections_response" "$auth_header"
json_assert "$connections_response" connections_github
echo "[integrations] auth/connections ok"

http_json GET "$base_url/api/v1/search?q=healthz&repo_id=repo_sourcebot_rewrite" 200 "$search_response" "$auth_header"
json_assert "$search_response" search_healthz
echo "[search] search ok"

http_json POST "$base_url/api/v1/ask/completions" 200 "$ask_response" "$auth_header" "$ask_request"
json_assert "$ask_response" ask_completion
thread_id=$(python3 - "$ask_response" <<'PY'
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    data = json.load(fh)
print(data['thread_id'])
PY
)
http_json GET "$base_url/api/v1/ask/threads" 200 "$threads_response" "$auth_header"
JSON_ASSERT_THREAD_ID="$thread_id" json_assert "$threads_response" threads_contains_thread
http_json GET "$base_url/api/v1/ask/threads/$thread_id" 200 "$thread_response" "$auth_header"
JSON_ASSERT_THREAD_ID="$thread_id" json_assert "$thread_response" thread_detail_matches_scope
echo "[ask] completion and thread readback ok"

http_json POST "$base_url/api/v1/auth/review-webhooks" 201 "$create_webhook_response" "$auth_header" "$create_webhook_request"
json_assert "$create_webhook_response" create_webhook_secret
webhook_auth_header=$(python3 - "$create_webhook_response" <<'PY'
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    data = json.load(fh)
print(f"Bearer {data['id']}:{data['secret']}")
PY
)
webhook_id=$(python3 - "$create_webhook_response" <<'PY'
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    data = json.load(fh)
print(data['id'])
PY
)

http_json POST "$base_url/api/v1/review-webhooks/$webhook_id/events" 202 "$intake_response" "$webhook_auth_header" "$intake_request"
JSON_ASSERT_WEBHOOK_ID="$webhook_id" json_assert "$intake_response" intake_accepted
http_json GET "$base_url/api/v1/auth/review-agent-runs" 200 "$runs_before_response" "$auth_header"
JSON_ASSERT_WEBHOOK_ID="$webhook_id" json_assert "$runs_before_response" runs_before_contains_queued
run_id=$(python3 - "$runs_before_response" <<'PY'
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    data = json.load(fh)
for item in data:
    if item['status'] == 'queued':
        print(item['id'])
        break
else:
    raise SystemExit('no queued run found')
PY
)
http_json GET "$base_url/api/v1/auth/review-agent-runs/$run_id" 200 "$run_before_response" "$auth_header"
JSON_ASSERT_RUN_ID="$run_id" json_assert "$run_before_response" run_before_queued
echo "[review-agent] queued run visible before worker"

set +e
(
  cd "$repo_root"
  SOURCEBOT_DATA_DIR="$runtime_dir" \
  SOURCEBOT_STUB_REVIEW_AGENT_RUN_EXECUTION_OUTCOME=completed \
  "$worker_bin"
) >"$worker_log" 2>&1
worker_status=$?
set -e
if [[ $worker_status -ne 0 ]]; then
  echo "sourcebot-worker exited with status $worker_status" >&2
  if [[ -f "$worker_log" ]]; then
    printf 'worker log:\n%s\n' "$(<"$worker_log")" >&2
  fi
  exit "$worker_status"
fi

http_json GET "$base_url/api/v1/auth/review-agent-runs" 200 "$runs_after_response" "$auth_header"
JSON_ASSERT_RUN_ID="$run_id" json_assert "$runs_after_response" runs_after_contains_completed
http_json GET "$base_url/api/v1/auth/review-agent-runs/$run_id" 200 "$run_after_response" "$auth_header"
JSON_ASSERT_RUN_ID="$run_id" json_assert "$run_after_response" run_after_completed
JSON_ASSERT_RUN_ID="$run_id" json_assert "$organization_state_path" organization_state_completed
echo "[review-agent] worker completed queued run"

echo "SMOKE MATRIX PASS: auth integrations search ask review-agent"
