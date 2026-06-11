#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

COMPOSE_FILE="docker-compose.deploy.yml"
ENV_FILE=".env"
ASSUME_YES="false"
SKIP_HEALTH="false"
HEALTH_TIMEOUT_SECONDS=60
ACTION="deploy"
CLI_HOST_PORT=""
REMOVE_VOLUMES="false"

timestamp() {
  date +%Y%m%d-%H%M%S
}

usage() {
  cat <<EOF_USAGE
Usage: $0 [deploy|undeploy|status|logs|backup] [options]

Actions:
  deploy          Configure, build, start, and health-check the server (default)
  undeploy        Stop and remove deployed containers, preserving database data
  status          Show Docker Compose service status
  logs            Follow recent app and database logs
  backup          Dump the Postgres database to server/backups/

Deploy options:
  -y, --yes                  Run non-interactively using existing env/defaults
      --env-file PATH        Env file to read/write (default: .env)
      --host-port PORT       Public HTTP port (default: existing env or 18080)
      --health-timeout SECS  Health-check timeout (default: 60)
      --skip-health          Do not wait for /healthz
      --volumes              With undeploy, also delete the Postgres data volume
  -h, --help                 Show this help

Examples:
  ./scripts/deploy.sh
  ./scripts/deploy.sh --yes --host-port 8080
  ./scripts/deploy.sh status
  ./scripts/deploy.sh backup
  ./scripts/deploy.sh undeploy
EOF_USAGE
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

is_tty() {
  [[ -t 0 && -t 1 ]]
}

prompt() {
  local name="$1"
  local label="$2"
  local default="${3:-}"
  local secret="${4:-false}"
  local value=""

  if [[ "$ASSUME_YES" == "true" || ! is_tty ]]; then
    if [[ -z "$default" ]]; then
      echo "Missing required value for $name; provide it in $ENV_FILE or via an option." >&2
      exit 1
    fi
    printf -v "$name" '%s' "$default"
    return
  fi

  if [[ "$secret" == "true" ]]; then
    if [[ -n "$default" ]]; then
      read -r -s -p "$label [keep existing/generated]: " value
    else
      read -r -s -p "$label: " value
    fi
    echo
  else
    if [[ -n "$default" ]]; then
      read -r -p "$label [$default]: " value
    else
      read -r -p "$label: " value
    fi
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

url_encode() {
  python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$1"
}

write_env() {
  local db_user_encoded db_password_encoded db_name_encoded
  db_user_encoded="$(url_encode "$POSTGRES_USER")"
  db_password_encoded="$(url_encode "$POSTGRES_PASSWORD")"
  db_name_encoded="$(url_encode "$POSTGRES_DB")"

  cat > "$ENV_FILE" <<EOF_ENV
HOST_PORT=$HOST_PORT
PORT=18080
POSTGRES_DB=$POSTGRES_DB
POSTGRES_USER=$POSTGRES_USER
POSTGRES_PASSWORD=$POSTGRES_PASSWORD
DATABASE_URL=postgres://$db_user_encoded:$db_password_encoded@postgres:5432/$db_name_encoded?sslmode=disable
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
  docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

parse_args() {
  if [[ $# -gt 0 ]]; then
    case "$1" in
      deploy|undeploy|status|logs|backup)
        ACTION="$1"
        shift
        ;;
    esac
  fi

  while [[ $# -gt 0 ]]; do
    case "$1" in
      -y|--yes)
        ASSUME_YES="true"
        shift
        ;;
      --env-file)
        ENV_FILE="${2:?--env-file requires a path}"
        shift 2
        ;;
      --host-port)
        CLI_HOST_PORT="${2:?--host-port requires a port}"
        shift 2
        ;;
      --health-timeout)
        HEALTH_TIMEOUT_SECONDS="${2:?--health-timeout requires seconds}"
        shift 2
        ;;
      --skip-health)
        SKIP_HEALTH="true"
        shift
        ;;
      --volumes)
        REMOVE_VOLUMES="true"
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        echo "Unknown argument: $1" >&2
        usage >&2
        exit 1
        ;;
    esac
  done
}

confirm_deploy() {
  if [[ "$ASSUME_YES" == "true" || ! is_tty ]]; then
    return
  fi

  echo
  echo "This will write $ENV_FILE and run: docker compose --env-file $ENV_FILE -f $COMPOSE_FILE up -d --build"
  read -r -p "Continue? [y/N]: " confirm
  case "$confirm" in
    y|Y|yes|YES) ;;
    *) echo "Aborted."; exit 0 ;;
  esac
}

wait_for_health() {
  if [[ "$SKIP_HEALTH" == "true" ]]; then
    echo "Skipping health check."
    return
  fi

  echo
  echo "Waiting up to ${HEALTH_TIMEOUT_SECONDS}s for local plaintext health check http://127.0.0.1:$HOST_PORT/healthz ..."
  local deadline=$((SECONDS + HEALTH_TIMEOUT_SECONDS))
  while (( SECONDS < deadline )); do
    if curl -fsS "http://127.0.0.1:$HOST_PORT/healthz" >/dev/null 2>&1; then
      echo "Deploy complete. Health check passed. Configure HTTPS/TLS in your reverse proxy before exposing the API publicly."
      return
    fi
    sleep 2
  done

  echo "Deploy finished, but health check did not pass. Recent app logs:"
  compose logs --tail=80 app
  exit 1
}

configure_env() {
  load_existing

  prompt HOST_PORT "Local HTTP port for TLS reverse proxy" "${CLI_HOST_PORT:-${HOST_PORT:-18080}}"
  PORT=18080
  prompt POSTGRES_DB "Postgres database name" "${POSTGRES_DB:-tasks}"
  prompt POSTGRES_USER "Postgres user" "${POSTGRES_USER:-tasks}"

  if [[ -z "${POSTGRES_PASSWORD:-}" ]]; then
    POSTGRES_PASSWORD="$(random_secret)"
    echo "Generated Postgres password."
  fi

  if [[ -z "${JWT_SECRET:-}" || "${JWT_SECRET:-}" == "replace-with-a-long-random-secret" ]]; then
    JWT_SECRET="$(random_secret)"
    echo "Generated JWT secret."
  fi

  prompt JWT_ISSUER "JWT issuer" "${JWT_ISSUER:-tasks-server}"
  prompt ACCESS_TOKEN_TTL "Access token TTL" "${ACCESS_TOKEN_TTL:-15m}"
  prompt REFRESH_TOKEN_TTL "Refresh token TTL" "${REFRESH_TOKEN_TTL:-720h}"
  prompt WRITE_RATE_LIMIT_PER_MIN "Write rate limit per user/min" "${WRITE_RATE_LIMIT_PER_MIN:-60}"
  prompt MAX_BLOB_BYTES "Max blob bytes" "${MAX_BLOB_BYTES:-1048576}"
  prompt MAX_BATCH_BLOBS "Max batch blobs" "${MAX_BATCH_BLOBS:-100}"
  prompt TOMBSTONE_RETENTION "Tombstone retention" "${TOMBSTONE_RETENTION:-720h}"
}

deploy() {
  echo "Tasks server Docker Compose deploy"
  echo "Working directory: $ROOT_DIR"
  echo "Env file: $ENV_FILE"
  echo

  need_cmd docker
  need_cmd curl
  need_cmd python3
  docker compose version >/dev/null

  configure_env
  confirm_deploy
  write_env
  compose up -d --build
  wait_for_health
}

status() {
  need_cmd docker
  load_existing
  compose ps
}

logs() {
  need_cmd docker
  load_existing
  compose logs --tail=120 -f app postgres
}

backup() {
  need_cmd docker
  load_existing
  mkdir -p backups
  local output="backups/tasks-server-$(timestamp).sql.gz"
  echo "Writing $output"
  compose exec -T postgres pg_dump -U "${POSTGRES_USER:-tasks}" "${POSTGRES_DB:-tasks}" | gzip > "$output"
  echo "Backup complete: $output"
}

undeploy() {
  need_cmd docker
  load_existing

  if [[ "$REMOVE_VOLUMES" == "true" ]]; then
    if [[ "$ASSUME_YES" != "true" && is_tty ]]; then
      echo "WARNING: this will delete the Postgres data volume."
      read -r -p "Delete containers and database volume? [y/N]: " confirm
      case "$confirm" in
        y|Y|yes|YES) ;;
        *) echo "Aborted."; exit 0 ;;
      esac
    fi
    compose down --volumes --remove-orphans
  else
    compose down --remove-orphans
  fi

  echo "Undeploy complete."
}

main() {
  parse_args "$@"
  case "$ACTION" in
    deploy) deploy ;;
    undeploy) undeploy ;;
    status) status ;;
    logs) logs ;;
    backup) backup ;;
  esac
}

main "$@"
