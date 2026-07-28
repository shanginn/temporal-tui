# Security policy

## Supported versions

Security fixes are released for the latest `1.x` version. Older `0.x` builds
are unsupported and should be upgraded.

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability. Use
[GitHub private vulnerability reporting](https://github.com/shanginn/temporal-tui/security/advisories/new)
and include the affected version and OS, required Temporal configuration, a
minimal reproduction, expected impact, and whether credentials, payloads,
terminal state, or mutation targeting are involved.

Reports are acknowledged as soon as practical. A fix, advisory, and disclosure
timeline are coordinated privately before publication.

## Release security

Release archives include SHA-256 checksums, a CycloneDX SBOM, and GitHub build
provenance. CI checks RustSec advisories, dependency licenses, and dependency
sources. See [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).
