# Installation, upgrade, and removal

## Standalone installer (no Xcode or Homebrew)

On macOS ARM64/Intel or Linux x86_64, install the latest prebuilt release:

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/shanginn/temporal-tui/releases/latest/download/temporal-tui-installer.sh \
  -o temporal-tui-installer.sh
sh temporal-tui-installer.sh
rm temporal-tui-installer.sh
```

The installer verifies the downloaded archive against the release
`SHA256SUMS`, validates its paths and entry types, checks the binary version,
and installs below `~/.local`. It does not use Homebrew, Rust, Cargo, Xcode,
the Xcode Command Line Tools, a compiler, or `sudo`, and it does not modify
shell startup files.

Install a specific version or a different prefix:

```sh
sh temporal-tui-installer.sh --version 1.2.0
sh temporal-tui-installer.sh --prefix /opt/temporal-tui
```

The default binary path is `~/.local/bin/temporal-tui`. Add
`~/.local/bin` to `PATH` if it is not already present.

## Temporal CLI extension

With Temporal CLI 1.8.1 or newer, the preferred command is `temporal tui`.
Temporal CLI's executable-extension convention maps `temporal-NAME` on `PATH`
to `temporal NAME`, so the installed `temporal-tui` binary is discovered
without a plugin registration step:

```sh
temporal help --all
temporal tui --help
temporal tui --profile production
```

`temporal help --all` should list `tui`. Put TUI flags and subcommands after
`tui`; `temporal tui --profile production` is correct. The installer does not
bundle Temporal CLI, and the standalone `temporal-tui --profile production`
command remains supported when Temporal CLI is absent or older.

Temporal CLI acts only as the extension dispatcher. Its config and env-file
profiles are not imported into `temporal-tui`. Ordinary process environment
variables are inherited, so `TEMPORAL_PROFILE` selects a TUI connection
profile, not a Temporal CLI config profile. Prefer explicit flags after `tui`
when the same shell environment is used for both tools. The host CLI's
`--command-timeout` is accepted only for the read-only local `config-path`
command. It is rejected for the dashboard, authentication, credential storage,
config loading/migration, and config mutations because a forced kill could
interrupt those operations unsafely. Use TUI-native flags such as `--no-color`
instead of Temporal CLI global output/config flags.

On Windows the executable is `temporal-tui.exe`. When it is on `PATH`,
Temporal CLI discovers it as `temporal.exe tui`; no renamed copy or additional
shim is required.

The Linux x86_64 archive targets glibc 2.35 or newer. The Windows executable
statically links the MSVC C runtime and does not require a separate Visual C++
Redistributable install.

## Verify a release

Download the target archive and `SHA256SUMS` from the matching GitHub Release:

```sh
grep '  temporal-tui-v1.2.0-aarch64-apple-darwin.tgz$' SHA256SUMS |
  shasum -a 256 -c -
gh attestation verify temporal-tui-v1.2.0-aarch64-apple-darwin.tgz \
  --repo shanginn/temporal-tui
```

Linux can pipe the same filtered checksum line to `sha256sum -c -`. Archives
contain the binary, license, README, manpages, and Bash, Zsh, Fish, PowerShell,
and Elvish completions.

## Package managers

Homebrew on macOS (ARM64 or Intel) and Linux x86_64:

```sh
brew install shanginn/temporal-tui/temporal-tui
```

The fully qualified name trusts only this formula and automatically adds the
[`shanginn/homebrew-temporal-tui`](https://github.com/shanginn/homebrew-temporal-tui)
tap.

The formula also installs a prebuilt archive, but Homebrew runs its own
toolchain preflight first. On a macOS/Xcode combination that Homebrew does not
accept, use the standalone installer; the `temporal-tui` executable does not
depend on Xcode.

Scoop on 64-bit Windows:

```powershell
scoop install https://github.com/shanginn/temporal-tui/releases/latest/download/temporal-tui.json
```

Cargo from source:

```sh
cargo install --locked --git https://github.com/shanginn/temporal-tui \
  --tag v1.2.0
```

The manifest includes `cargo-binstall` metadata for release archives.

## Protected self-hosted login

The prebuilt binary contains the complete login client. With Temporal CLI
1.8.1+, use the preferred extension syntax:

```sh
temporal tui --profile rubase auth login \
  --url https://temporal.example.com \
  --username admin
temporal tui --profile rubase
temporal tui --profile rubase auth whoami
temporal tui --profile rubase auth logout
```

The interactive password prompt is masked. Non-interactive installations can
pipe a password to `auth login --password-stdin`; there is no password flag.
Use the same commands through standalone `temporal-tui` when Temporal CLI is
not installed. Neither form needs a separate `temporal-auth` wrapper/plugin.

## Manual Unix install

After extraction:

```sh
install -m 0755 temporal-tui ~/.local/bin/temporal-tui
install -m 0644 man/temporal-tui.1 ~/.local/share/man/man1/temporal-tui.1
install -m 0644 completions/temporal-tui.bash \
  ~/.local/share/bash-completion/completions/temporal-tui
install -m 0644 completions/_temporal-tui \
  ~/.local/share/zsh/site-functions/_temporal-tui
install -m 0644 completions/temporal-tui.fish \
  ~/.config/fish/completions/temporal-tui.fish
```

Ensure `~/.local/bin` is in `PATH`, then run `temporal tui --version` with
Temporal CLI 1.8.1+, or `temporal-tui --version` standalone.

## Upgrade and rollback

Replace the binary through the same method. On first config read, v1.2 migrates
schema 1 or schema 2 to schema 3. It first writes a byte-identical
`config.toml.v1.bak` or `config.toml.v2.bak`, then atomically replaces the
config; both files are `0600` on Unix.

Migration stops without replacement for missing/unknown/newer schema, invalid
legacy data, a conflicting backup, or write failure. Keep the backup until the
upgrade is exercised. Restore the matching backup before rolling back to a
binary that predates schema 3.

## Uninstall

```sh
brew uninstall temporal-tui
# or
cargo uninstall temporal-tui
```

```powershell
scoop uninstall temporal-tui
```

Manual installs remove the binary, manpages, and completion files. Uninstall
intentionally preserves config, migration backups, exports, and saved
credentials. Run `temporal tui --profile NAME auth logout` before uninstalling
if a protected self-hosted session should be revoked and removed; standalone
`temporal-tui --profile NAME auth logout` is equivalent. Use
`temporal tui config-path` (or standalone `temporal-tui config-path`) before
removing remaining files manually.

Release CI smoke-tests clean install, schema upgrade, binary removal, and
configuration preservation on every packaged OS.
