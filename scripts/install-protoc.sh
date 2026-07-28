#!/usr/bin/env bash
set -euo pipefail

version="35.1"
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
install_dir="${TEMPORAL_PROTOC_OUTPUT:-${project_root}/.tools/protoc-${version}}"

case "$(uname -s):$(uname -m)" in
  Darwin:arm64 | Darwin:aarch64)
    asset="protoc-${version}-osx-aarch_64.zip"
    expected_sha="193289af0470c6a1aada357d4fba0bbf8d78bfaac8b5e42ca30af2ef75583de2"
    ;;
  Darwin:x86_64)
    asset="protoc-${version}-osx-x86_64.zip"
    expected_sha="537d73604a344ded6fc94e98e07e529d4fe3e4a0b09e59905353950fafc2a1f7"
    ;;
  Linux:x86_64)
    asset="protoc-${version}-linux-x86_64.zip"
    expected_sha="6930ebf62bd4ea607b98fff052596c6ee564b9835b4ce172c75a3f53ae9d91b7"
    ;;
  *)
    echo "unsupported protoc platform: $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

binary="${install_dir}/bin/protoc"
reported_version=""
if [[ -x "${binary}" ]]; then
  reported_version="$("${binary}" --version)"
fi

if [[ "${reported_version}" != "libprotoc ${version}" ]]; then
  mkdir -p "${install_dir}"
  archive="${install_dir}/${asset}"
  curl --fail --location --silent --show-error \
    "https://github.com/protocolbuffers/protobuf/releases/download/v${version}/${asset}" \
    --output "${archive}"

  if command -v sha256sum >/dev/null 2>&1; then
    actual_sha="$(sha256sum "${archive}" | awk '{print $1}')"
  else
    actual_sha="$(shasum -a 256 "${archive}" | awk '{print $1}')"
  fi
  if [[ "${actual_sha}" != "${expected_sha}" ]]; then
    echo "checksum mismatch for ${asset}" >&2
    exit 1
  fi

  unzip -o -q "${archive}" -d "${install_dir}"
  chmod 0755 "${binary}"
  reported_version="$("${binary}" --version)"
fi

if [[ "${reported_version}" != "libprotoc ${version}" ]]; then
  echo "unexpected protoc version: ${reported_version}" >&2
  exit 1
fi
if [[ -n "${GITHUB_PATH:-}" ]]; then
  printf '%s\n' "${install_dir}/bin" >>"${GITHUB_PATH}"
fi
printf '%s\n' "${reported_version}"
