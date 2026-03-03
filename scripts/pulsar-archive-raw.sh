#!/usr/bin/env bash
set -euo pipefail

PULSAR_DAILY_DIR="${PULSAR_DAILY_DIR:-$HOME/.local/share/pulsar/daily}"
PULSAR_ARCHIVE_DIR="${PULSAR_ARCHIVE_DIR:-$HOME/.local/share/pulsar/archives}"
PULSAR_ARCHIVE_MIN_DAYS="${PULSAR_ARCHIVE_MIN_DAYS:-15}"
PULSAR_ARCHIVE_MAX_DAYS="${PULSAR_ARCHIVE_MAX_DAYS:-60}"

if ! command -v zip >/dev/null 2>&1; then
  echo "Missing required command: zip" >&2
  exit 1
fi

mkdir -p "$PULSAR_DAILY_DIR" "$PULSAR_ARCHIVE_DIR"

mapfile -d '' FILES < <(
  find "$PULSAR_DAILY_DIR" \
    -maxdepth 1 \
    -type f \
    -name '*.jsonl' \
    -mtime "+$PULSAR_ARCHIVE_MIN_DAYS" \
    -mtime "-$PULSAR_ARCHIVE_MAX_DAYS" \
    -print0
)

if [[ "${#FILES[@]}" -eq 0 ]]; then
  exit 0
fi

ARCHIVE_PATH="$PULSAR_ARCHIVE_DIR/pulsar-archive-$(date +%F).zip"
zip -qj "$ARCHIVE_PATH" "${FILES[@]}"
rm -f "${FILES[@]}"

find "$PULSAR_ARCHIVE_DIR" \
  -maxdepth 1 \
  -type f \
  -name 'pulsar-archive-*.zip' \
  -mtime "+$PULSAR_ARCHIVE_MAX_DAYS" \
  -delete
