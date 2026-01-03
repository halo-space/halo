#!/usr/bin/env bash
set -euo pipefail

# Simple benchmark script (wrk by default), runs all targets sequentially.
# Start three services first:
# - halo-rest example: cargo run -p halo-rest --example hello --release   (port 8080)
# - axum example:      cargo run -p halo-rest --example axum --release   (port 8081)
# - gin example:       go run gin_server.go                              (port 8082)
# Then run: ./bench.sh
# Tunables: DURATION=20s CONCURRENCY=64 THREADS=4 ./bench.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="${SCRIPT_DIR}/bench_out"
mkdir -p "${OUT_DIR}"

URL_REST="${URL_REST:-http://127.0.0.1:8080/ai/v1/v1/api/square}"
URL_AXUM="${URL_AXUM:-http://127.0.0.1:8081/ai/v1/v1/api/square}"
URL_GIN="${URL_GIN:-http://127.0.0.1:8082/ai/v1/v1/api/square}"

# Use TARGETS to choose targets (comma separated, default: rest,axum,gin)
TARGETS_DEFAULT="rest,axum,gin"
TARGETS="${TARGETS:-$TARGETS_DEFAULT}"

DURATION="${DURATION:-15s}"
CONCURRENCY="${CONCURRENCY:-32}"
THREADS="${THREADS:-4}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing command: $1" >&2
    exit 1
  fi
}

require_cmd wrk

run_wrk() {
  local name="$1"
  local url="$2"
  local ts
  ts="$(date +%Y%m%d_%H%M%S)"
  local out="${OUT_DIR}/${name}_${ts}.txt"

  echo "==> ${name} ${url} (t=${THREADS} c=${CONCURRENCY} d=${DURATION}) --latency"
  wrk -t"${THREADS}" -c"${CONCURRENCY}" -d"${DURATION}" --latency "${url}" | tee "${out}"
}

for target in ${TARGETS//,/ }; do
  case "$target" in
    rest) run_wrk "rest" "${URL_REST}" ;;
    axum) run_wrk "axum" "${URL_AXUM}" ;;
    gin)  run_wrk "gin"  "${URL_GIN}" ;;
    *)
      echo "unknown target: $target (supported: rest, axum, gin)" >&2
      exit 1
      ;;
  esac
done

echo "Results written to ${OUT_DIR}/*.txt"

