#!/usr/bin/env bash
set -euo pipefail

binary="${1:-target/debug/sysray}"
binary_dir="$(cd "$(dirname "${binary}")" && pwd)"
binary="${binary_dir}/$(basename "${binary}")"
platform="$(uname -s)"
managed_home=1
root=""
if [[ -n "${SYSRAY_VALIDATION_HOME:-}" ]]; then
  managed_home=0
  export HOME="${SYSRAY_VALIDATION_HOME}"
  mkdir -p "${HOME}"
else
  root="$(mktemp -d)"
  export HOME="${root}/home"
  mkdir -p "${HOME}"
fi

cleanup() {
  local installed_binary="${install_path:-}"
  if [[ -n "${installed_binary}" && -x "${installed_binary}" ]]; then
    "${installed_binary}" schedule uninstall >/dev/null 2>&1 || true
    "${installed_binary}" service uninstall >/dev/null 2>&1 || true
  else
    "${binary}" schedule uninstall >/dev/null 2>&1 || true
    "${binary}" service uninstall >/dev/null 2>&1 || true
  fi

  if [[ ${managed_home} -eq 1 ]]; then
    rm -rf "${root}"
  fi
}

trap cleanup EXIT

case "${platform}" in
  Linux)
    install_path="${HOME}/.local/bin/sysray"
    config_path="${HOME}/.config/sysray/sysray.toml"
    service_runner="${HOME}/.local/share/sysray/sysray-service.sh"
    service_artifact="${HOME}/.config/systemd/user/sysray.service"
    schedule_runner_dir="${HOME}/.local/share/sysray/schedule"
    schedule_runners=(
      "${schedule_runner_dir}/snapshot.sh"
      "${schedule_runner_dir}/prune.sh"
      "${schedule_runner_dir}/archive.sh"
    )
    schedule_artifacts=(
      "${HOME}/.config/systemd/user/sysray-snapshot.service"
      "${HOME}/.config/systemd/user/sysray-snapshot.timer"
      "${HOME}/.config/systemd/user/sysray-prune.service"
      "${HOME}/.config/systemd/user/sysray-prune.timer"
      "${HOME}/.config/systemd/user/sysray-archive.service"
      "${HOME}/.config/systemd/user/sysray-archive.timer"
    )
    status_is_optional=1
    if [[ "${SYSRAY_REQUIRE_LIVE_STATUS:-0}" == "1" ]]; then
      status_is_optional=0
    fi
    ;;
  Darwin)
    install_path="${HOME}/.local/bin/sysray"
    config_path="${HOME}/Library/Application Support/Sysray/sysray.toml"
    service_runner="${HOME}/Library/Application Support/Sysray/sysray-service.sh"
    service_artifact="${HOME}/Library/LaunchAgents/com.zabadehut.sysray.plist"
    schedule_runner_dir="${HOME}/Library/Application Support/Sysray/schedule"
    schedule_runners=(
      "${schedule_runner_dir}/snapshot.sh"
      "${schedule_runner_dir}/prune.sh"
      "${schedule_runner_dir}/archive.sh"
    )
    schedule_artifacts=(
      "${HOME}/Library/LaunchAgents/com.zabadehut.sysray.snapshot.plist"
      "${HOME}/Library/LaunchAgents/com.zabadehut.sysray.prune.plist"
      "${HOME}/Library/LaunchAgents/com.zabadehut.sysray.archive.plist"
    )
    status_is_optional=1
    ;;
  *)
    echo "unsupported platform: ${platform}" >&2
    exit 1
    ;;
esac

install_output=""
install_status=0
install_output="$("${binary}" install --no-path 2>&1)" || install_status=$?

if [[ ${install_status} -ne 0 && ${status_is_optional} -eq 0 ]]; then
  echo "${install_output}" >&2
  exit "${install_status}"
fi

for path in "${install_path}" "${config_path}" "${service_runner}" "${service_artifact}"; do
  if [[ ! -e "${path}" ]]; then
    echo "missing expected install artifact: ${path}" >&2
    echo "${install_output}" >&2
    exit 1
  fi
done

for path in "${schedule_runners[@]}" "${schedule_artifacts[@]}"; do
  if [[ ! -e "${path}" ]]; then
    echo "missing expected install artifact: ${path}" >&2
    echo "${install_output}" >&2
    exit 1
  fi
done

grep -F "\"${install_path}\"" "${service_runner}" >/dev/null
grep -F "${service_runner}" "${service_artifact}" >/dev/null

for runner in "${schedule_runners[@]}"; do
  grep -F "\"${install_path}\"" "${runner}" >/dev/null
done

case "${platform}" in
  Linux)
    grep -F "${schedule_runner_dir}/snapshot.sh" "${HOME}/.config/systemd/user/sysray-snapshot.service" >/dev/null
    grep -F "${schedule_runner_dir}/prune.sh" "${HOME}/.config/systemd/user/sysray-prune.service" >/dev/null
    grep -F "${schedule_runner_dir}/archive.sh" "${HOME}/.config/systemd/user/sysray-archive.service" >/dev/null
    ;;
  Darwin)
    grep -F "${schedule_runner_dir}/snapshot.sh" "${HOME}/Library/LaunchAgents/com.zabadehut.sysray.snapshot.plist" >/dev/null
    grep -F "${schedule_runner_dir}/prune.sh" "${HOME}/Library/LaunchAgents/com.zabadehut.sysray.prune.plist" >/dev/null
    grep -F "${schedule_runner_dir}/archive.sh" "${HOME}/Library/LaunchAgents/com.zabadehut.sysray.archive.plist" >/dev/null
    ;;
esac

if [[ ${install_status} -eq 0 ]]; then
  "${install_path}" service status >/dev/null 2>&1 || [[ ${status_is_optional} -eq 1 ]]
  "${install_path}" schedule status >/dev/null 2>&1 || [[ ${status_is_optional} -eq 1 ]]
fi

"${install_path}" uninstall --keep-path --purge-data >/dev/null 2>&1 || [[ ${status_is_optional} -eq 1 ]]

for path in "${install_path}" "${service_runner}" "${service_artifact}" "${schedule_runners[@]}" "${schedule_artifacts[@]}" "${config_path}"; do
  if [[ -e "${path}" ]]; then
    echo "install artifact should have been removed: ${path}" >&2
    exit 1
  fi
done
