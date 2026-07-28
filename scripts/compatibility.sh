#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tool_dir="${project_root}/.tools/bin"

# These official CLI releases embed one disposable server from each tested
# Temporal minor line. The installer verifies the upstream SHA-256 manifest.
cli_versions=(1.5.1 1.6.2 1.8.1)
expected_servers=(1.29.1 1.30.2 1.31.2)

for index in "${!cli_versions[@]}"; do
  cli_version="${cli_versions[index]}"
  expected_server="${expected_servers[index]}"
  cli_path="${tool_dir}/temporal-${cli_version}"

  if [[ ! -x "${cli_path}" ]]; then
    TEMPORAL_CLI_VERSION="${cli_version}" \
      TEMPORAL_CLI_OUTPUT="${cli_path}" \
      "${project_root}/scripts/install-temporal-cli.sh"
  fi

  actual_version="$("${cli_path}" --version)"
  if [[ "${actual_version}" != *"Server ${expected_server}"* ]]; then
    echo "expected Temporal Server ${expected_server}, got: ${actual_version}" >&2
    exit 1
  fi

  echo "==> read-only compatibility contract: Temporal Server ${expected_server}"
  TEMPORAL_CLI="${cli_path}" \
    cargo test --locked --test compatibility -- --ignored --nocapture
done
