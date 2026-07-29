#!/usr/bin/env bash
set -euo pipefail

temporal_cli="${1:?usage: temporal-cli-extension-smoke.sh TEMPORAL_CLI TUI_BINARY VERSION}"
tui_binary="${2:?usage: temporal-cli-extension-smoke.sh TEMPORAL_CLI TUI_BINARY VERSION}"
expected_version="${3:?usage: temporal-cli-extension-smoke.sh TEMPORAL_CLI TUI_BINARY VERSION}"
expected_version="${expected_version#v}"

if [[ ! -x "${temporal_cli}" ]]; then
  echo "Temporal CLI is not executable: ${temporal_cli}" >&2
  exit 1
fi
if [[ ! -x "${tui_binary}" ]]; then
  echo "temporal-tui is not executable: ${tui_binary}" >&2
  exit 1
fi

temporary_dir="$(mktemp -d)"
trap 'rm -rf "${temporary_dir}"' EXIT
extension_dir="${temporary_dir}/bin"
config="${temporary_dir}/config.toml"
env_config="${temporary_dir}/env-config.toml"
export HOME="${temporary_dir}/home"
export XDG_CONFIG_HOME="${temporary_dir}/xdg-config"
mkdir -p "${extension_dir}" "${HOME}" "${XDG_CONFIG_HOME}"
install -m 0755 "${tui_binary}" "${extension_dir}/temporal-tui"

plugin_path="${extension_dir}:${PATH}"
cli_version="$("${temporal_cli}" --version)"
if [[ "${cli_version}" != temporal\ version\ 1.8.1* ]]; then
  echo "expected Temporal CLI 1.8.1, got: ${cli_version}" >&2
  exit 1
fi
actual_version="$(
  PATH="${plugin_path}" "${temporal_cli}" tui --version
)"
if [[ "${actual_version}" != "temporal-tui ${expected_version}" ]]; then
  echo "unexpected extension version: ${actual_version}" >&2
  exit 1
fi

timeout_version="$(
  PATH="${plugin_path}" \
    "${temporal_cli}" tui --command-timeout 5s --version
)"
if [[ "${timeout_version}" != "temporal-tui ${expected_version}" ]]; then
  echo "Temporal CLI command timeout was not accepted: ${timeout_version}" >&2
  exit 1
fi

timeout_config="$(
  PATH="${plugin_path}" \
    "${temporal_cli}" tui --command-timeout 5s \
    --config "${config}" config-path
)"
test "${timeout_config}" = "${config}"

PATH="${plugin_path}" "${temporal_cli}" tui --help |
  grep -q 'terminal dashboard and control plane for Temporal'
PATH="${plugin_path}" "${temporal_cli}" help --all |
  grep -Eq '^  tui[[:space:]]'

actual_config="$(
  PATH="${plugin_path}" \
    "${temporal_cli}" tui --config "${config}" config-path
)"
test "${actual_config}" = "${config}"

# `--profile` after `tui` is the temporal-tui profile selector. Temporal CLI is
# only the dispatcher and its own config-file profile is not inherited.
actual_profile_config="$(
  PATH="${plugin_path}" \
    "${temporal_cli}" tui --profile rubase \
    --config "${config}" config-path
)"
test "${actual_profile_config}" = "${config}"

actual_env_config="$(
  TEMPORAL_TUI_CONFIG="${env_config}" \
    PATH="${plugin_path}" \
    "${temporal_cli}" tui config-path
)"
test "${actual_env_config}" = "${env_config}"

PATH="${plugin_path}" \
  "${temporal_cli}" tui --profile rubase auth whoami --help |
  grep -q 'current signed-in identity and session status'

set +e
PATH="${plugin_path}" \
  "${temporal_cli}" tui --definitely-invalid \
  >"${temporary_dir}/invalid.stdout" \
  2>"${temporary_dir}/invalid.stderr"
exit_code=$?
set -e
if [[ "${exit_code}" -ne 2 ]]; then
  echo "extension exit code was not preserved: ${exit_code}" >&2
  exit 1
fi
grep -q "unexpected argument '--definitely-invalid'" \
  "${temporary_dir}/invalid.stderr"

set +e
PATH="${plugin_path}" \
  "${temporal_cli}" tui --command-timeout invalid --version \
  >"${temporary_dir}/invalid-timeout.stdout" \
  2>"${temporary_dir}/invalid-timeout.stderr"
timeout_exit_code=$?
set -e
if [[ "${timeout_exit_code}" -ne 1 ]]; then
  echo "invalid parent timeout returned ${timeout_exit_code}, expected 1" >&2
  exit 1
fi
test ! -s "${temporary_dir}/invalid-timeout.stdout"
grep -q 'invalid argument "invalid" for "--command-timeout"' \
  "${temporary_dir}/invalid-timeout.stderr"

set +e
PATH="${plugin_path}" \
  "${temporal_cli}" tui --command-timeout 5s \
  --config "${config}" \
  >"${temporary_dir}/interactive-timeout.stdout" \
  2>"${temporary_dir}/interactive-timeout.stderr"
interactive_timeout_exit_code=$?
set -e
if [[ "${interactive_timeout_exit_code}" -ne 1 ]]; then
  echo "interactive timeout returned ${interactive_timeout_exit_code}, expected 1" >&2
  exit 1
fi
test ! -s "${temporary_dir}/interactive-timeout.stdout"
grep -q 'cannot safely interrupt the dashboard' \
  "${temporary_dir}/interactive-timeout.stderr"

echo "Temporal CLI extension smoke test passed for temporal-tui ${expected_version}"
