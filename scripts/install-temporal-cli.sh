#!/usr/bin/env bash
set -euo pipefail

version="${TEMPORAL_CLI_VERSION:-1.8.1}"
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tool_dir="${project_root}/.tools/bin"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "${temporary_dir}"' EXIT

case "$(uname -s)" in
  Darwin) platform="darwin" ;;
  Linux) platform="linux" ;;
  *)
    echo "unsupported platform: $(uname -s)" >&2
    exit 1
    ;;
esac

case "$(uname -m)" in
  arm64 | aarch64) architecture="arm64" ;;
  x86_64 | amd64) architecture="amd64" ;;
  *)
    echo "unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

archive="temporal_cli_${version}_${platform}_${architecture}.tar.gz"
release_url="https://github.com/temporalio/cli/releases/download/v${version}"

curl --fail --location --silent --show-error \
  "${release_url}/${archive}" \
  --output "${temporary_dir}/${archive}"
curl --fail --location --silent --show-error \
  "${release_url}/checksums.txt" \
  --output "${temporary_dir}/checksums.txt"

expected_checksum="$(
  awk -v archive="${archive}" '$2 == archive { print $1 }' "${temporary_dir}/checksums.txt"
)"
if [[ -z "${expected_checksum}" ]]; then
  echo "release checksum for ${archive} was not found" >&2
  exit 1
fi

actual_checksum="$(shasum -a 256 "${temporary_dir}/${archive}" | awk '{ print $1 }')"
if [[ "${actual_checksum}" != "${expected_checksum}" ]]; then
  echo "checksum mismatch for ${archive}" >&2
  exit 1
fi

tar -xzf "${temporary_dir}/${archive}" -C "${temporary_dir}"
mkdir -p "${tool_dir}"
install -m 0755 "${temporary_dir}/temporal" "${tool_dir}/temporal"
"${tool_dir}/temporal" --version
