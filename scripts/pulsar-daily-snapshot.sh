#!/usr/bin/env bash
set -euo pipefail

PULSAR_BIN="${PULSAR_BIN:-$HOME/.local/bin/pulsar}"
PULSAR_CONFIG="${PULSAR_CONFIG:-$HOME/.config/pulsar/pulsar.toml}"
PULSAR_DAILY_DIR="${PULSAR_DAILY_DIR:-$HOME/.local/share/pulsar/daily}"

mkdir -p "$PULSAR_DAILY_DIR"

OUTPUT_FILE="$PULSAR_DAILY_DIR/$(date +%F).jsonl"
"$PULSAR_BIN" --config "$PULSAR_CONFIG" snapshot --format json >> "$OUTPUT_FILE"
