#!/bin/sh
set -eu

repository="https://github.com/shanginn/temporal-tui"
requested_version=""
install_prefix="${TEMPORAL_TUI_INSTALL_PREFIX:-}"
local_archive=""
local_checksums=""
temporary_dir=""

usage() {
  cat <<'EOF'
Install a prebuilt temporal-tui binary without Homebrew, Rust, or Xcode.

Usage:
  install.sh [--version VERSION] [--prefix DIRECTORY]
  install.sh --version VERSION --archive FILE --checksums FILE [--prefix DIRECTORY]

Options:
  --version VERSION    Install an exact release; defaults to the latest release.
  --prefix DIRECTORY  Install below this prefix; defaults to $HOME/.local.
  --archive FILE       Use a local release archive instead of downloading it.
  --checksums FILE     Verify a local archive with this release SHA256SUMS file.
  -h, --help           Show this help.
EOF
}

fail() {
  printf 'temporal-tui installer: %s\n' "$*" >&2
  exit 1
}

cleanup() {
  if [ -n "${temporary_dir}" ] && [ -d "${temporary_dir}" ]; then
    rm -rf "${temporary_dir}"
  fi
}

require_value() {
  option="$1"
  value="${2:-}"
  [ -n "${value}" ] || fail "${option} requires a value"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      require_value "$1" "${2:-}"
      requested_version="$2"
      shift 2
      ;;
    --prefix)
      require_value "$1" "${2:-}"
      install_prefix="$2"
      shift 2
      ;;
    --archive)
      require_value "$1" "${2:-}"
      local_archive="$2"
      shift 2
      ;;
    --checksums)
      require_value "$1" "${2:-}"
      local_checksums="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "unknown argument: $1"
      ;;
  esac
done

if [ -n "${local_archive}" ] || [ -n "${local_checksums}" ]; then
  [ -n "${local_archive}" ] && [ -n "${local_checksums}" ] ||
    fail "--archive and --checksums must be used together"
  [ -n "${requested_version}" ] ||
    fail "--version is required when installing from local files"
  [ -f "${local_archive}" ] || fail "archive not found: ${local_archive}"
  [ -f "${local_checksums}" ] || fail "checksum file not found: ${local_checksums}"
fi

if [ -z "${install_prefix}" ]; then
  [ -n "${HOME:-}" ] || fail "HOME is unset; pass --prefix DIRECTORY"
  install_prefix="${HOME}/.local"
fi

case "$(uname -s)" in
  Darwin)
    case "$(uname -m)" in
      arm64|aarch64) target="aarch64-apple-darwin" ;;
      x86_64|amd64) target="x86_64-apple-darwin" ;;
      *) fail "unsupported macOS architecture: $(uname -m)" ;;
    esac
    ;;
  Linux)
    case "$(uname -m)" in
      x86_64|amd64) target="x86_64-unknown-linux-gnu" ;;
      *) fail "unsupported Linux architecture: $(uname -m)" ;;
    esac
    ;;
  *)
    fail "unsupported operating system: $(uname -s)"
    ;;
esac

if [ -z "${requested_version}" ]; then
  command -v curl >/dev/null 2>&1 || fail "curl is required"
  latest_url="$(
    curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
      --output /dev/null --write-out '%{url_effective}' \
      "${repository}/releases/latest"
  )"
  requested_version="${latest_url##*/}"
fi

requested_version="${requested_version#v}"
printf '%s\n' "${requested_version}" |
  grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$' ||
  fail "invalid release version: ${requested_version}"

archive_name="temporal-tui-v${requested_version}-${target}.tgz"
package_root="temporal-tui-v${requested_version}-${target}"
temporary_dir="$(mktemp -d "${TMPDIR:-/tmp}/temporal-tui-install.XXXXXX")" ||
  fail "could not create a temporary directory"
trap cleanup 0 HUP INT TERM

if [ -n "${local_archive}" ]; then
  archive_path="${local_archive}"
  checksums_path="${local_checksums}"
else
  command -v curl >/dev/null 2>&1 || fail "curl is required"
  release_base="${repository}/releases/download/v${requested_version}"
  archive_path="${temporary_dir}/${archive_name}"
  checksums_path="${temporary_dir}/SHA256SUMS"
  curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
    --output "${checksums_path}" "${release_base}/SHA256SUMS"
  curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
    --output "${archive_path}" "${release_base}/${archive_name}"
fi

expected_sha="$(
  awk -v archive="${archive_name}" '
    $2 == archive {
      print $1
      matches += 1
    }
    END {
      if (matches != 1) {
        exit 1
      }
    }
  ' "${checksums_path}" 2>/dev/null || true
)"
printf '%s\n' "${expected_sha}" | grep -Eq '^[0-9a-fA-F]{64}$' ||
  fail "SHA256SUMS has no unique SHA-256 for ${archive_name}"

if command -v shasum >/dev/null 2>&1; then
  actual_sha="$(shasum -a 256 "${archive_path}" | awk '{print $1}')"
elif command -v sha256sum >/dev/null 2>&1; then
  actual_sha="$(sha256sum "${archive_path}" | awk '{print $1}')"
else
  fail "shasum or sha256sum is required"
fi
[ "${actual_sha}" = "${expected_sha}" ] ||
  fail "SHA-256 mismatch for ${archive_name}"

command -v tar >/dev/null 2>&1 || fail "tar is required"
tar -tzf "${archive_path}" >"${temporary_dir}/archive-files"
awk -v root="${package_root}" '
  BEGIN {
    entries = 0
  }
  {
    entry = $0
    entries += 1
    if (substr(entry, 1, 1) == "/") {
      exit 1
    }
    if (entry != root && entry != root "/" && index(entry, root "/") != 1) {
      exit 1
    }
    parts = split(entry, component, "/")
    for (i = 1; i <= parts; i += 1) {
      if (component[i] == "..") {
        exit 1
      }
    }
  }
  END {
    if (entries == 0) {
      exit 1
    }
  }
' "${temporary_dir}/archive-files" ||
  fail "archive contains an unsafe path"
tar -tvzf "${archive_path}" |
  awk 'substr($1, 1, 1) != "-" && substr($1, 1, 1) != "d" { exit 1 }' ||
  fail "archive contains a link or unsupported entry"
tar -xzf "${archive_path}" -C "${temporary_dir}"

package_path="${temporary_dir}/${package_root}"
[ -x "${package_path}/temporal-tui" ] || fail "archive binary is missing"
[ -f "${package_path}/man/temporal-tui.1" ] || fail "archive manpage is missing"
[ -d "${package_path}/completions" ] || fail "archive completions are missing"
[ "$("${package_path}/temporal-tui" --version)" = "temporal-tui ${requested_version}" ] ||
  fail "archive binary version does not match v${requested_version}"

mkdir -p \
  "${install_prefix}/bin" \
  "${install_prefix}/share/man/man1" \
  "${install_prefix}/share/bash-completion/completions" \
  "${install_prefix}/share/zsh/site-functions" \
  "${install_prefix}/share/fish/vendor_completions.d" \
  "${install_prefix}/share/temporal-tui/completions"
install -m 0755 "${package_path}/temporal-tui" "${install_prefix}/bin/temporal-tui"
install -m 0644 \
  "${package_path}/man/temporal-tui.1" \
  "${install_prefix}/share/man/man1/temporal-tui.1"
install -m 0644 \
  "${package_path}/completions/temporal-tui.bash" \
  "${install_prefix}/share/bash-completion/completions/temporal-tui"
install -m 0644 \
  "${package_path}/completions/_temporal-tui" \
  "${install_prefix}/share/zsh/site-functions/_temporal-tui"
install -m 0644 \
  "${package_path}/completions/temporal-tui.fish" \
  "${install_prefix}/share/fish/vendor_completions.d/temporal-tui.fish"
install -m 0644 \
  "${package_path}/completions/_temporal-tui.ps1" \
  "${package_path}/completions/temporal-tui.elv" \
  "${install_prefix}/share/temporal-tui/completions/"

[ "$("${install_prefix}/bin/temporal-tui" --version)" = "temporal-tui ${requested_version}" ] ||
  fail "installed binary failed its version check"

printf 'Installed temporal-tui %s to %s/bin/temporal-tui\n' \
  "${requested_version}" "${install_prefix}"
case ":${PATH:-}:" in
  *":${install_prefix}/bin:"*) ;;
  *)
    printf 'Add %s/bin to PATH to run temporal-tui globally.\n' "${install_prefix}"
    ;;
esac
