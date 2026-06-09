#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

COMPOSE_FILE="docker-compose.deploy.yml"
ENV_FILE=".env"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

prompt() {
  local name="$1"
  local label="$2"
  local default="${3:-}"
  local value=""

  if [[ -n "$default" ]]; then
    read -r -p "$label [$default]: " value
  else
    read -r -p "$label: " value
  fi
  value="${value:-$default}"
  printf -v "$name" '%s' "$value"
}

load_existing() {
  if [[ -f "$ENV_FILE" ]]; then
    set -a
    # shellcheck disable=SC1090
    . "$ENV_FILE"
    set +a
  fi
}

random_secret() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -hex 32
  else
    head -c 32 /dev/urandom | base64 | tr -d '\n'
  fi
}

write_env() {
  cat > "$ENV_FILE" <<EOF_ENV
HOST_PORT=$HOST_PORT
PORT=8080
POSTGRES_DB=$POSTGRES_DB
POSTGRES_USER=$POSTGRES_USER
POSTGRES_PASSWORD=$POSTGRES_PASSWORD
DATABASE_URL=postgres://$POSTGRES_USER:$POSTGRES_PASSWORD@postgres:5432/$POSTGRES_DB?sslmode=disable
JWT_SECRET=$JWT_SECRET
JWT_ISSUER=$JWT_ISSUER
ACCESS_TOKEN_TTL=$ACCESS_TOKEN_TTL
REFRESH_TOKEN_TTL=$REFRESH_TOKEN_TTL
WRITE_RATE_LIMIT_PER_MIN=$WRITE_RATE_LIMIT_PER_MIN
MAX_BLOB_BYTES=$MAX_BLOB_BYTES
MAX_BATCH_BLOBS=$MAX_BATCH_BLOBS
TOMBSTONE_RETENTION=$TOMBSTONE_RETENTION
EOF_ENV
  chmod 600 "$ENV_FILE"
}

compose() {
  docker compose -f "$COMPOSE_FILE" "$@"
}

main() {
  echo "Tasks server interactive Docker Compose deploy"
  echo "Working directory: $ROOT_DIR"
  echo

  need_cmd docker
  need_cmd curl
  docker compose version >/dev/null

  load_existing

  prompt HOST_PORT "Public HTTP port" "${HOST_PORT:-${PORT:-8080}}"
  PORT=8080
  prompt POSTGRES_DB "Postgres database name" "${POSTGRES_DB:-tasks}"
  prompt POSTGRES_USER "Postgres user" "${POSTGRES_USER:-tasks}"

  POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-}"
  if [[ -z "$POSTGRES_PASSWORD" ]]; then
    POSTGRES_PASSWORD="$(random_secret)"
    echo "Generated Postgres password."
  fi

  JWT_SECRET="${JWT_SECRET:-}"
  if [[ -z "$JWT_SECRET" || "$JWT_SECRET" == "replace-with-a-long-random-secret" ]]; then
    JWT_SECRET="$(random_secret)"
    echo "Generated JWT signing key."
  fi

  JWT_ISSUER="${JWT_ISSUER:-tasks-server}"
  ACCESS_TOKEN_TTL="${ACCESS_TOKEN_TTL:-15m}"
  REFRESH_TOKEN_TTL="${REFRESH_TOKEN_TTL:-720h}"
  WRITE_RATE_LIMIT_PER_MIN="${WRITE_RATE_LIMIT_PER_MIN:-60}"
  MAX_BLOB_BYTES="${MAX_BLOB_BYTES:-1048576}"
  MAX_BATCH_BLOBS="${MAX_BATCH_BLOBS:-100}"
  TOMBSTONE_RETENTION="${TOMBSTONE_RETENTION:-720h}"

  echo
  echo "This will write $ENV_FILE and run: docker compose -f $COMPOSE_FILE up -d --build"
  read -r -p "Continue? [y/N]: " confirm
  case "$confirm" in
    y|Y|yes|YES) ;;
    *) echo "Aborted."; exit 0 ;;
  esac

  write_env
  compose up -d --build

  echo
  echo "Waiting for app to respond on http://localhost:$HOST_PORT/healthz ..."
  for _ in $(seq 1 30); do
    if command -v curl >/dev/null 2>&1 && curl -fsS "http://localhost:$HOST_PORT/healthz" >/dev/null; then
      echo "Deploy complete. Health check passed."
      exit 0
    fi
    sleep 2
  done

  echo "Deploy finished, but health check did not pass yet. Recent logs:"
  compose logs --tail=80 app
  exit 1
}

main "$@"
