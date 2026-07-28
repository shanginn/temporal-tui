#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="${1:-$(rustc -vV | awk '/^host:/ { print $2 }')}"
output_dir="${2:-${project_root}/dist}"
binary_path="${3:-${project_root}/target/release/temporal-tui}"
version="$(
  awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version = / {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  ' "${project_root}/Cargo.toml"
)"

if [[ -z "${version}" ]]; then
  echo "could not read package version" >&2
  exit 1
fi
if [[ ! -x "${binary_path}" ]]; then
  echo "release binary is missing or not executable: ${binary_path}" >&2
  exit 1
fi
if [[ "$("${binary_path}" --version)" != "temporal-tui ${version}" ]]; then
  echo "release binary version does not match Cargo.toml" >&2
  exit 1
fi

package_name="temporal-tui-v${version}-${target}"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "${temporary_dir}"' EXIT
package_root="${temporary_dir}/${package_name}"

mkdir -p \
  "${package_root}/completions" \
  "${package_root}/man"
install -m 0755 "${binary_path}" "${package_root}/temporal-tui"
install -m 0644 "${project_root}/README.md" "${package_root}/README.md"
install -m 0644 "${project_root}/LICENSE" "${package_root}/LICENSE"
install -m 0644 "${project_root}"/assets/man/*.1 "${package_root}/man/"
install -m 0644 "${project_root}"/assets/completions/* "${package_root}/completions/"

mkdir -p "${output_dir}"
archive="${output_dir}/${package_name}.tgz"
tar -czf "${archive}" -C "${temporary_dir}" "${package_name}"
echo "${archive}"
