# Changelog

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
