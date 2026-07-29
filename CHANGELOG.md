# Changelog

## 1.2.0 — 2026-07-29

- Make `temporal tui` the preferred launch with Temporal CLI 1.8.1 or newer.
  Temporal CLI discovers the installed `temporal-tui` executable through its
  official `temporal-NAME` `PATH` extension convention.
- Route the complete dashboard, profile, filter, and protected self-hosted
  `auth login`/`whoami`/`logout` UX through the extension, with all TUI flags
  placed after `tui`.
- Keep `temporal-tui` fully supported as a standalone executable. Releases do
  not bundle or require Temporal CLI, and Windows discovery uses the packaged
  `temporal-tui.exe`.
- Document extension discovery through `temporal help --all` and recovery for
  version, `PATH`, argument-order, duplicate-installation, and Windows issues.
- Accept Temporal CLI's forwarded `--command-timeout` for the read-only local
  `config-path` command while rejecting unsafe forced interruption of the
  dashboard, authentication, credential storage, config loading/migration, or
  config mutations. Keep Temporal CLI config/env-file profiles separate from
  TUI profiles while documenting inherited process variables such as
  `TEMPORAL_PROFILE`.
- Lower the Linux release baseline to glibc 2.35, statically link the Windows
  MSVC runtime, and run Windows ZIP package smoke tests before release tags.

## 1.1.0 — 2026-07-29

- Add native `auth login`, `auth whoami`, and `auth logout` for protected
  self-hosted Temporal deployments. The TUI connects directly to Temporal gRPC
  without a wrapper, plugin, or Temporal CLI runtime.
- Mask interactive passwords, support stdin automation without a password
  argument, keep short-lived access tokens in memory, and store only rotating
  refresh credentials in the operating-system credential manager.
- Require HTTPS, reject redirects and cross-origin token endpoints, restrict
  the hidden insecure mode to loopback, and revoke refresh access before local
  logout deletion.
- Bind stored refresh credentials to profile/origin/token endpoint/username and
  coordinate one-time rotation across concurrent processes.
- Migrate schema-1 and schema-2 configs atomically to schema 3 with
  byte-identical private backups.
- Cover exchange, refresh rotation, authenticated gRPC, and revocation with an
  isolated acceptance test; no positive production login is part of the test.

## 1.0.2 — 2026-07-28

- Add a checksum-verifying standalone installer for macOS ARM64/Intel and
  Linux x86_64. It installs the prebuilt release without Homebrew, Rust, Xcode,
  or a compiler.
- Exercise clean install, in-place upgrade, support assets, checksum failure,
  and unsafe-archive rejection on every supported Unix release runner.

## 1.0.1 — 2026-07-28

- Publish the official `shanginn/homebrew-temporal-tui` tap and replace the
  direct formula URL, which current Homebrew no longer accepts.
- Retain the complete v1.0 runtime, compatibility, packaging, security, and
  provenance contract unchanged.

## 1.0.0 — 2026-07-28

- Capability negotiation with independent unavailable, restricted, and
  transient-unknown degradation.
- Read-only compatibility contracts for Temporal Server 1.29, 1.30, and 1.31.
- Atomic schema-1 to schema-2 migration with byte-identical private backup.
- Linux, macOS ARM/Intel, and Windows archives with completions, manpage,
  package metadata, checksums, CycloneDX SBOM, and provenance.
- Cross-platform CI, live/compatibility contracts, RustSec, license, and source
  policy.
- Connection/header hardening, terminal escape/bidi sanitization, and
  production documentation.

Earlier `0.x` milestones are summarized in [ROADMAP.md](ROADMAP.md).
