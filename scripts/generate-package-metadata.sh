#!/usr/bin/env bash
set -euo pipefail

version="${1:?usage: generate-package-metadata.sh VERSION CHECKSUMS [OUTPUT_DIR]}"
checksums_input="${2:?usage: generate-package-metadata.sh VERSION CHECKSUMS [OUTPUT_DIR]}"
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
checksums="$(cd "$(dirname "${checksums_input}")" && pwd)/$(basename "${checksums_input}")"
output_dir="${3:-${project_root}/dist}"
repository="https://github.com/shanginn/temporal-tui"
cd "${project_root}"

checksum_for() {
  local filename="$1"
  local checksum
  checksum="$(awk -v filename="${filename}" '$2 == filename { print $1 }' "${checksums}")"
  if [[ ! "${checksum}" =~ ^[0-9a-f]{64}$ ]]; then
    echo "missing SHA-256 for ${filename}" >&2
    exit 1
  fi
  printf '%s' "${checksum}"
}

linux_archive="temporal-tui-v${version}-x86_64-unknown-linux-gnu.tgz"
macos_arm_archive="temporal-tui-v${version}-aarch64-apple-darwin.tgz"
macos_intel_archive="temporal-tui-v${version}-x86_64-apple-darwin.tgz"
windows_archive="temporal-tui-v${version}-x86_64-pc-windows-msvc.zip"

linux_sha="$(checksum_for "${linux_archive}")"
macos_arm_sha="$(checksum_for "${macos_arm_archive}")"
macos_intel_sha="$(checksum_for "${macos_intel_archive}")"
windows_sha="$(checksum_for "${windows_archive}")"
mkdir -p "${output_dir}"

sed \
  -e "s|@VERSION@|${version}|g" \
  -e "s|@REPOSITORY@|${repository}|g" \
  -e "s|@LINUX_ARCHIVE@|${linux_archive}|g" \
  -e "s|@LINUX_SHA@|${linux_sha}|g" \
  -e "s|@MACOS_ARM_ARCHIVE@|${macos_arm_archive}|g" \
  -e "s|@MACOS_ARM_SHA@|${macos_arm_sha}|g" \
  -e "s|@MACOS_INTEL_ARCHIVE@|${macos_intel_archive}|g" \
  -e "s|@MACOS_INTEL_SHA@|${macos_intel_sha}|g" \
  packaging/homebrew/temporal-tui.rb.in \
  >"${output_dir}/temporal-tui.rb"

sed \
  -e "s|@VERSION@|${version}|g" \
  -e "s|@REPOSITORY@|${repository}|g" \
  -e "s|@WINDOWS_ARCHIVE@|${windows_archive}|g" \
  -e "s|@WINDOWS_SHA@|${windows_sha}|g" \
  packaging/scoop/temporal-tui.json.in \
  >"${output_dir}/temporal-tui.json"
