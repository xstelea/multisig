#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"

cleanup() {
  echo ""
  echo "Shutting down..."
  kill 0 2>/dev/null
  wait 2>/dev/null
}
trap cleanup SIGINT SIGTERM EXIT

echo "Starting PostgreSQL..."
docker compose -f "$ROOT/docker-compose.yml" up -d

echo "Waiting for PostgreSQL to be healthy..."
until docker compose -f "$ROOT/docker-compose.yml" ps postgres --format json | grep -q '"healthy"'; do
  sleep 1
done
echo "PostgreSQL is ready."

echo ""
echo "Starting development servers..."
echo "  [server]  http://localhost:3001  (cargo run)"
echo "  [app]     http://localhost:3000  (vite dev)"
echo ""

# Backend
(cd "$ROOT/multisig-server" && cargo run 2>&1 | sed 's/^/[server]  /') &

# Frontend (override PORT so Nitro doesn't inherit the backend's PORT=3001)
(cd "$ROOT/multisig-app" && PORT=3000 pnpm run dev 2>&1 | sed 's/^/[app]     /') &

wait
