#!/usr/bin/env bash
set -euo pipefail

archive="${1:?usage: package-smoke.sh ARCHIVE}"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "${temporary_dir}"' EXIT
extract_dir="${temporary_dir}/extract"
prefix="${temporary_dir}/prefix"
config="${temporary_dir}/config.toml"

mkdir -p "${extract_dir}" "${prefix}/bin"
tar -xzf "${archive}" -C "${extract_dir}"
package_root="$(find "${extract_dir}" -mindepth 1 -maxdepth 1 -type d -print -quit)"
if [[ -z "${package_root}" ]]; then
  echo "archive does not contain one package directory" >&2
  exit 1
fi

test -x "${package_root}/temporal-tui"
test -f "${package_root}/man/temporal-tui.1"
test -f "${package_root}/completions/temporal-tui.bash"
test -f "${package_root}/completions/_temporal-tui"
test -f "${package_root}/completions/temporal-tui.fish"
test -f "${package_root}/completions/_temporal-tui.ps1"
test -f "${package_root}/completions/temporal-tui.elv"

install -m 0755 "${package_root}/temporal-tui" "${prefix}/bin/temporal-tui"
PATH="${prefix}/bin:${PATH}" temporal-tui --version
PATH="${prefix}/bin:${PATH}" temporal-tui --help >/dev/null

# Simulate an upgrade from the published schema-1 config without using any
# user-owned location or server.
printf 'schema_version = 1\n' >"${config}"
TEMPORAL_TUI_CONFIG="${config}" \
  PATH="${prefix}/bin:${PATH}" \
  temporal-tui filter list
grep -q '^schema_version = 2$' "${config}"
cmp "${config}.v1.bak" <(printf 'schema_version = 1\n')

rm -f "${prefix}/bin/temporal-tui"
test ! -e "${prefix}/bin/temporal-tui"
# Uninstalling the binary intentionally leaves user configuration recoverable.
test -f "${config}"
test -f "${config}.v1.bak"
