#!/usr/bin/env bash
set -euo pipefail

PULSAR_DAILY_DIR="${PULSAR_DAILY_DIR:-$HOME/.local/share/pulsar/daily}"
PULSAR_RAW_RETENTION_DAYS="${PULSAR_RAW_RETENTION_DAYS:-15}"

mkdir -p "$PULSAR_DAILY_DIR"

find "$PULSAR_DAILY_DIR" \
  -maxdepth 1 \
  -type f \
  -name '*.jsonl' \
  -mtime "+$PULSAR_RAW_RETENTION_DAYS" \
  -delete
