#!/usr/bin/env bash
set -euo pipefail

archive="${1:?usage: installer-smoke.sh ARCHIVE VERSION}"
version="${2:?usage: installer-smoke.sh ARCHIVE VERSION}"
version="${version#v}"
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
archive_directory="$(cd "$(dirname "${archive}")" && pwd)"
archive="${archive_directory}/$(basename "${archive}")"
temporary_dir="$(mktemp -d)"
trap 'rm -rf "${temporary_dir}"' EXIT

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64|Darwin-aarch64)
    target="aarch64-apple-darwin"
    ;;
  Darwin-x86_64|Darwin-amd64)
    target="x86_64-apple-darwin"
    ;;
  Linux-x86_64|Linux-amd64)
    target="x86_64-unknown-linux-gnu"
    ;;
  *)
    echo "unsupported installer smoke-test platform: $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

archive_name="temporal-tui-v${version}-${target}.tgz"
if [[ "$(basename "${archive}")" != "${archive_name}" ]]; then
  echo "archive name does not match the current platform: ${archive}" >&2
  exit 1
fi

sha256() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    sha256sum "$1" | awk '{print $1}'
  fi
}

checksums="${temporary_dir}/SHA256SUMS"
printf '%s  %s\n' "$(sha256 "${archive}")" "${archive_name}" >"${checksums}"
prefix="${temporary_dir}/prefix"

# The public installer must perform a clean install using only the archive and
# operating-system tools, then replace a stale binary on upgrade.
"${project_root}/scripts/install.sh" \
  --version "${version}" \
  --archive "${archive}" \
  --checksums "${checksums}" \
  --prefix "${prefix}"
printf 'stale binary\n' >"${prefix}/bin/temporal-tui"
"${project_root}/scripts/install.sh" \
  --version "v${version}" \
  --archive "${archive}" \
  --checksums "${checksums}" \
  --prefix "${prefix}"

test "$("${prefix}/bin/temporal-tui" --version)" = "temporal-tui ${version}"
"${prefix}/bin/temporal-tui" --help >/dev/null
test -x "${prefix}/bin/temporal-tui"
test -f "${prefix}/share/man/man1/temporal-tui.1"
test -f "${prefix}/share/man/man1/temporal-tui-auth.1"
test -f "${prefix}/share/man/man1/temporal-tui-auth-login.1"
test -f "${prefix}/share/bash-completion/completions/temporal-tui"
test -f "${prefix}/share/zsh/site-functions/_temporal-tui"
test -f "${prefix}/share/fish/vendor_completions.d/temporal-tui.fish"
test -f "${prefix}/share/temporal-tui/completions/_temporal-tui.ps1"
test -f "${prefix}/share/temporal-tui/completions/temporal-tui.elv"
packaged_man_count="$(
  tar -tzf "${archive}" |
    awk '$0 ~ "/man/[^/]+[.]1$" { count += 1 } END { print count + 0 }'
)"
test "${packaged_man_count}" -gt 1
expected_installed_files="$((packaged_man_count + 6))"
test "$(find "${prefix}" -type f | wc -l | tr -d ' ')" = \
  "${expected_installed_files}"

# A changed archive must fail closed before extraction or installation.
bad_checksums="${temporary_dir}/BAD_SHA256SUMS"
printf '%064d  %s\n' 0 "${archive_name}" >"${bad_checksums}"
if "${project_root}/scripts/install.sh" \
  --version "${version}" \
  --archive "${archive}" \
  --checksums "${bad_checksums}" \
  --prefix "${temporary_dir}/bad-prefix" \
  >"${temporary_dir}/bad-checksum.log" 2>&1
then
  echo "installer accepted an invalid SHA-256" >&2
  exit 1
fi
grep -q 'SHA-256 mismatch' "${temporary_dir}/bad-checksum.log"
test ! -e "${temporary_dir}/bad-prefix"

# A checksum-valid archive containing a symlink must also fail closed.
unsafe_extract="${temporary_dir}/unsafe-extract"
mkdir -p "${unsafe_extract}"
tar -xzf "${archive}" -C "${unsafe_extract}"
ln -s temporal-tui \
  "${unsafe_extract}/temporal-tui-v${version}-${target}/linked-binary"
unsafe_archive="${temporary_dir}/unsafe.tgz"
COPYFILE_DISABLE=1 tar -czf "${unsafe_archive}" \
  -C "${unsafe_extract}" "temporal-tui-v${version}-${target}"
unsafe_checksums="${temporary_dir}/UNSAFE_SHA256SUMS"
printf '%s  %s\n' "$(sha256 "${unsafe_archive}")" "${archive_name}" \
  >"${unsafe_checksums}"
if "${project_root}/scripts/install.sh" \
  --version "${version}" \
  --archive "${unsafe_archive}" \
  --checksums "${unsafe_checksums}" \
  --prefix "${temporary_dir}/unsafe-prefix" \
  >"${temporary_dir}/unsafe.log" 2>&1
then
  echo "installer accepted an archive containing a symlink" >&2
  exit 1
fi
grep -q 'archive contains a link or unsupported entry' \
  "${temporary_dir}/unsafe.log"
test ! -e "${temporary_dir}/unsafe-prefix"

echo "standalone installer smoke test passed for ${archive_name}"
