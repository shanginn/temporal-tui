# Installation, upgrade, and removal

## Verify a release

Download the target archive and `SHA256SUMS` from the matching GitHub Release:

```sh
shasum -a 256 -c SHA256SUMS
gh attestation verify temporal-tui-v1.0.0-aarch64-apple-darwin.tgz \
  --repo shanginn/temporal-tui
```

Linux can use `sha256sum -c`. Archives contain the binary, license, README,
manpage, and Bash, Zsh, Fish, PowerShell, and Elvish completions.

## Package managers

Homebrew on macOS/Linux:

```sh
brew install --formula \
  https://github.com/shanginn/temporal-tui/releases/latest/download/temporal-tui.rb
```

Scoop on 64-bit Windows:

```powershell
scoop install https://github.com/shanginn/temporal-tui/releases/latest/download/temporal-tui.json
```

Cargo from source:

```sh
cargo install --locked --git https://github.com/shanginn/temporal-tui \
  --tag v1.0.0
```

The manifest includes `cargo-binstall` metadata for release archives.

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

Ensure `~/.local/bin` is in `PATH`, then run `temporal-tui --version`.

## Upgrade and rollback

Replace the binary through the same method. On first config read, v1 migrates
schema 1 to schema 2. It first writes byte-identical `config.toml.v1.bak`, then
atomically replaces the config; both files are `0600` on Unix.

Migration stops without replacement for missing/unknown/newer schema, invalid
legacy data, a conflicting backup, or write failure. Keep the backup until the
upgrade is exercised. A v0.5 binary can use that backup for rollback.

## Uninstall

```sh
brew uninstall temporal-tui
# or
cargo uninstall temporal-tui
```

```powershell
scoop uninstall temporal-tui
```

Manual installs remove the binary, manpage, and completion files. Uninstall
intentionally preserves config, migration backups, and exports. Use
`temporal-tui config-path` before removing them manually.

Release CI smoke-tests clean install, schema upgrade, binary removal, and
configuration preservation on every packaged OS.
