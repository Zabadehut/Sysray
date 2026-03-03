#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

DURATION_SECS=30
INTERVAL_SECS=5
OUTPUT_ROOT="${OUTPUT_ROOT:-$ROOT_DIR/.benchmarks}"
PULSAR_BIN="${PULSAR_BIN:-}"
PULSAR_CONFIG="${PULSAR_CONFIG:-$HOME/.config/pulsar/pulsar.toml}"

usage() {
  cat <<'EOF'
Usage: ./scripts/benchmark-overhead-linux.sh [--duration N] [--interval N] [--output-dir DIR]

Benchmarks local monitoring overhead for Pulsar vs classic Linux tools when available.

Options:
  --duration N    Benchmark duration per tool in seconds (default: 30)
  --interval N    Sampling interval in seconds (default: 5)
  --output-dir    Root directory for benchmark artifacts (default: ./.benchmarks)
  -h, --help      Show this help message

Environment:
  PULSAR_BIN      Override Pulsar binary path
  PULSAR_CONFIG   Override Pulsar config path
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --duration)
      DURATION_SECS="${2:-}"
      shift
      ;;
    --interval)
      INTERVAL_SECS="${2:-}"
      shift
      ;;
    --output-dir)
      OUTPUT_ROOT="${2:-}"
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
  shift
done

require_positive_int() {
  local label="$1"
  local value="$2"
  if [[ -z "$value" || ! "$value" =~ ^[0-9]+$ || "$value" -eq 0 ]]; then
    echo "$label must be a positive integer" >&2
    exit 1
  fi
}

require_positive_int "duration" "$DURATION_SECS"
require_positive_int "interval" "$INTERVAL_SECS"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_command awk
require_command getconf
require_command hostname
require_command uname

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This benchmark script is intended for Linux only." >&2
  exit 1
fi

find_pulsar_bin() {
  if [[ -n "$PULSAR_BIN" && -x "$PULSAR_BIN" ]]; then
    printf '%s\n' "$PULSAR_BIN"
    return
  fi

  if [[ -x "$ROOT_DIR/target/debug/pulsar" ]]; then
    printf '%s\n' "$ROOT_DIR/target/debug/pulsar"
    return
  fi

  if [[ -x "$HOME/.local/bin/pulsar" ]]; then
    printf '%s\n' "$HOME/.local/bin/pulsar"
    return
  fi

  cargo build >/dev/null
  printf '%s\n' "$ROOT_DIR/target/debug/pulsar"
}

PULSAR_BIN="$(find_pulsar_bin)"
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="$OUTPUT_ROOT/$RUN_ID"
SAMPLES_DIR="$RUN_DIR/samples"
PULSAR_RECORD_DIR="$RUN_DIR/pulsar-record"
mkdir -p "$SAMPLES_DIR" "$PULSAR_RECORD_DIR"

SUMMARY_CSV="$RUN_DIR/summary.csv"
SUMMARY_MD="$RUN_DIR/summary.md"

printf 'tool,status,elapsed_secs,cpu_pct_avg,rss_kb,vsz_kb,read_bytes,write_bytes,fd_count,output\n' > "$SUMMARY_CSV"

CLK_TCK="$(getconf CLK_TCK)"
PAGE_SIZE="$(getconf PAGESIZE)"
CPU_COUNT="$(getconf _NPROCESSORS_ONLN)"
COUNT=$(( (DURATION_SECS + INTERVAL_SECS - 1) / INTERVAL_SECS ))

snapshot_pid_state() {
  local pid="$1"
  local prefix="$2"

  [[ -d "/proc/$pid" ]] || return 1
  cat "/proc/$pid/stat" > "${prefix}.stat"
  cat "/proc/$pid/io" > "${prefix}.io" 2>/dev/null || :
  cat "/proc/$pid/status" > "${prefix}.status"
  ls "/proc/$pid/fd" > "${prefix}.fd" 2>/dev/null || :
  return 0
}

extract_stat_field() {
  local file="$1"
  local field="$2"
  awk -v field="$field" '
    {
      close_paren = index($0, ")")
      rest = substr($0, close_paren + 2)
      split(rest, values, " ")
      print values[field - 3]
    }
  ' "$file"
}

extract_status_kb() {
  local file="$1"
  local key="$2"
  awk -v key="$key" '$1 == key ":" { print $2; exit }' "$file"
}

extract_io_bytes() {
  local file="$1"
  local key="$2"
  awk -v key="$key" '$1 == key ":" { print $2; exit }' "$file"
}

append_summary() {
  local tool="$1"
  local status="$2"
  local elapsed="$3"
  local cpu_pct="$4"
  local rss_kb="$5"
  local vsz_kb="$6"
  local read_bytes="$7"
  local write_bytes="$8"
  local fd_count="$9"
  local output_path="${10}"

  printf '%s,%s,%s,%s,%s,%s,%s,%s,%s,%s\n' \
    "$tool" "$status" "$elapsed" "$cpu_pct" "$rss_kb" "$vsz_kb" \
    "$read_bytes" "$write_bytes" "$fd_count" "$output_path" >> "$SUMMARY_CSV"
}

measure_command() {
  local tool="$1"
  local stop_mode="$2"
  local command="$3"
  local output_path="$4"
  local pid start_epoch end_epoch elapsed_secs exit_status=0 stop_sent=0
  local start_prefix="$SAMPLES_DIR/$tool.start"
  local last_prefix="$SAMPLES_DIR/$tool.last"

  echo "==> Benchmarking $tool"
  bash -lc "exec $command" >"$RUN_DIR/${tool}.stdout.log" 2>"$RUN_DIR/${tool}.stderr.log" &
  pid=$!
  start_epoch="$(date +%s)"

  sleep 1
  if ! kill -0 "$pid" 2>/dev/null; then
    wait "$pid" || exit_status=$?
    append_summary "$tool" "failed_to_start" "0" "0.00" "0" "0" "0" "0" "0" "$output_path"
    return
  fi

  snapshot_pid_state "$pid" "$start_prefix"
  cat "${start_prefix}.stat" > "${last_prefix}.stat"
  cat "${start_prefix}.status" > "${last_prefix}.status"
  cat "${start_prefix}.fd" > "${last_prefix}.fd" 2>/dev/null || :
  cat "${start_prefix}.io" > "${last_prefix}.io" 2>/dev/null || :

  while kill -0 "$pid" 2>/dev/null; do
    snapshot_pid_state "$pid" "$last_prefix" || :

    if [[ "$stop_mode" == "interrupt_after_duration" ]]; then
      if (( $(date +%s) - start_epoch >= DURATION_SECS )) && [[ "$stop_sent" -eq 0 ]]; then
        kill -INT "$pid" 2>/dev/null || :
        stop_sent=1
      fi
    fi

    sleep 1
  done

  wait "$pid" || exit_status=$?
  end_epoch="$(date +%s)"
  elapsed_secs=$(( end_epoch - start_epoch ))
  if (( elapsed_secs <= 0 )); then
    elapsed_secs=1
  fi

  local start_utime start_stime end_utime end_stime delta_ticks rss_kb vsz_kb
  local start_read start_write end_read end_write delta_read delta_write fd_count cpu_pct

  start_utime="$(extract_stat_field "${start_prefix}.stat" 14)"
  start_stime="$(extract_stat_field "${start_prefix}.stat" 15)"
  end_utime="$(extract_stat_field "${last_prefix}.stat" 14)"
  end_stime="$(extract_stat_field "${last_prefix}.stat" 15)"
  delta_ticks=$(( (end_utime + end_stime) - (start_utime + start_stime) ))

  cpu_pct="$(awk -v ticks="$delta_ticks" -v clk="$CLK_TCK" -v elapsed="$elapsed_secs" 'BEGIN {
    printf "%.2f", (ticks / clk) / elapsed * 100.0
  }')"

  rss_kb="$(extract_status_kb "${last_prefix}.status" "VmRSS")"
  rss_kb="${rss_kb:-0}"
  vsz_kb="$(extract_status_kb "${last_prefix}.status" "VmSize")"
  if [[ -z "$vsz_kb" ]]; then
    vsz_kb="$(awk -v pagesize="$PAGE_SIZE" -v bytes="$(extract_stat_field "${last_prefix}.stat" 23)" 'BEGIN {
      printf "%.0f", bytes / 1024.0
    }')"
  fi
  vsz_kb="${vsz_kb:-0}"

  start_read="$(extract_io_bytes "${start_prefix}.io" "read_bytes")"
  start_read="${start_read:-0}"
  start_write="$(extract_io_bytes "${start_prefix}.io" "write_bytes")"
  start_write="${start_write:-0}"
  end_read="$(extract_io_bytes "${last_prefix}.io" "read_bytes")"
  end_read="${end_read:-$start_read}"
  end_write="$(extract_io_bytes "${last_prefix}.io" "write_bytes")"
  end_write="${end_write:-$start_write}"
  delta_read=$(( end_read - start_read ))
  delta_write=$(( end_write - start_write ))
  fd_count="$(wc -l < "${last_prefix}.fd" 2>/dev/null || printf '0')"

  if [[ "$exit_status" -eq 0 || "$exit_status" -eq 130 || "$exit_status" -eq 143 ]]; then
    append_summary "$tool" "ok" "$elapsed_secs" "$cpu_pct" "$rss_kb" "$vsz_kb" \
      "$delta_read" "$delta_write" "$fd_count" "$output_path"
  else
    append_summary "$tool" "exit_$exit_status" "$elapsed_secs" "$cpu_pct" "$rss_kb" "$vsz_kb" \
      "$delta_read" "$delta_write" "$fd_count" "$output_path"
  fi
}

measure_pulsar() {
  local output_path="$PULSAR_RECORD_DIR"
  local command

  command="\"$PULSAR_BIN\" --config \"$PULSAR_CONFIG\" record --interval ${INTERVAL_SECS}s --output \"$output_path\""
  measure_command "pulsar" "interrupt_after_duration" "$command" "$output_path"
}

measure_nmon() {
  if ! command -v nmon >/dev/null 2>&1; then
    append_summary "nmon" "missing" "0" "0.00" "0" "0" "0" "0" "0" "n/a"
    return
  fi

  local output_path="$RUN_DIR/nmon"
  mkdir -p "$output_path"
  measure_command "nmon" "wait_for_exit" "nmon -f -s $INTERVAL_SECS -c $COUNT -m \"$output_path\"" "$output_path"
}

measure_vmstat() {
  if ! command -v vmstat >/dev/null 2>&1; then
    append_summary "vmstat" "missing" "0" "0.00" "0" "0" "0" "0" "0" "n/a"
    return
  fi

  local output_path="$RUN_DIR/vmstat.txt"
  measure_command "vmstat" "wait_for_exit" "vmstat $INTERVAL_SECS $COUNT > \"$output_path\"" "$output_path"
}

measure_sar() {
  if ! command -v sar >/dev/null 2>&1; then
    append_summary "sar" "missing" "0" "0.00" "0" "0" "0" "0" "0" "n/a"
    return
  fi

  local output_path="$RUN_DIR/sar.txt"
  measure_command "sar" "wait_for_exit" "sar -u -r -d -n DEV $INTERVAL_SECS $COUNT > \"$output_path\"" "$output_path"
}

measure_pulsar
measure_nmon
measure_vmstat
measure_sar

{
  echo "# Benchmark Overhead"
  echo
  echo "- Run ID: \`$RUN_ID\`"
  echo "- Host: \`$(hostname)\`"
  echo "- Kernel: \`$(uname -r)\`"
  echo "- CPU count: \`$CPU_COUNT\`"
  echo "- Duration per tool: \`${DURATION_SECS}s\`"
  echo "- Sampling interval: \`${INTERVAL_SECS}s\`"
  echo
  echo "| Tool | Status | Elapsed (s) | Avg CPU % | RSS KB | VSZ KB | Read bytes | Write bytes | FD count | Output |"
  echo "|---|---:|---:|---:|---:|---:|---:|---:|---:|---|"
  awk -F, 'NR > 1 {
    printf "| %s | %s | %s | %s | %s | %s | %s | %s | %s | `%s` |\n",
      $1, $2, $3, $4, $5, $6, $7, $8, $9, $10
  }' "$SUMMARY_CSV"
  echo
  echo "Raw logs and per-process samples are stored under \`$RUN_DIR\`."
} > "$SUMMARY_MD"

echo "Benchmark summary: $SUMMARY_MD"
cat "$SUMMARY_MD"
