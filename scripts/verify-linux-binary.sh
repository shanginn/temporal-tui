#!/usr/bin/env bash
set -euo pipefail

binary="${1:?usage: verify-linux-binary.sh BINARY}"
baseline="${TEMPORAL_TUI_MAX_GLIBC:-2.35}"

if [[ ! -x "${binary}" ]]; then
  echo "Linux binary is not executable: ${binary}" >&2
  exit 1
fi
if ! command -v readelf >/dev/null 2>&1; then
  echo "readelf is required to verify the Linux release baseline" >&2
  exit 1
fi

maximum="$(
  readelf --version-info "${binary}" |
    sed -n 's/.*Name: GLIBC_\([0-9][0-9.]*\).*/\1/p' |
    sort -Vu |
    tail -n 1
)"
if [[ -z "${maximum}" ]]; then
  echo "no GLIBC symbol versions found in ${binary}" >&2
  exit 1
fi

newest="$(printf '%s\n%s\n' "${baseline}" "${maximum}" | sort -V | tail -n 1)"
if [[ "${newest}" != "${baseline}" ]]; then
  echo "Linux binary requires GLIBC_${maximum}; maximum is GLIBC_${baseline}" >&2
  exit 1
fi

echo "Linux binary GLIBC baseline verified: maximum GLIBC_${maximum}"
