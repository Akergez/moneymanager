# Money Manager

Expenses, income and transfers across accounts in different currencies, in one
adaptive window: a table and a sidebar on a desktop, a list and a bottom bar
on a phone. Written in Rust and drawn with [GPUI](https://www.gpui.rs/) through
[GPUI Kit](https://gpui-kit.com).

The ledger is a CRDT document (RDX, from [tresse](https://gitlab.com/ragusseven/tresse)).
It is kept inside the application and can sync with any S3-compatible storage,
end-to-end encrypted, merging what several devices changed without conflicts.

This is the graphical successor of the terminal version (0.4 and earlier). It
reads and writes **exactly the same ledger format**, so the two — and every
device already syncing a ledger — keep working against the same storage.

## What it does

- **Transactions** — the records of the current account: id, amount, category
  and date (and the comment where there is room). One switch turns the screen
  from expenses to income. Sortable by column; a row opens the record.
- **Categories** — expense and income categories, each with a colour and an
  icon.
- **Charts** — one screen with all three: a pie by category (for the month or
  for all time), bars by month, and the running total through the chosen month.
- **Accounts** — chosen at the top of the window, with the balance beside the
  name. Each has a currency and an opening balance.
- **Transfers** — between two accounts, with the two amounts entered
  separately and the exchange rate derived from them. A transfer shows as an
  expense on one account and an income on the other; deleting either side
  deletes both.
- **Sync** — on demand, with S3-compatible storage.

## First run

With no ledger on the device the application asks which there should be:

- **Create a new ledger** — asks for the currency of the first account and
  starts empty. Sync storage can be added later in Settings.
- **Connect to sync storage** — brings in a ledger that already syncs. The
  storage is given either as one **config string** (`tresse1:…`, what another
  device copies out of its Settings, or what the terminal version prints with
  `money_manager config --export`) or field by field: endpoint, region, bucket,
  access key ID, secret access key, prefix and the optional encryption key.

Nothing is recorded until it has worked: a mistyped key leaves the
installation as it was found, and the question is asked again.

## Where things are kept

Everything is private to the application. Nothing is read from or written to
your documents or to shared storage, so no storage permission is ever asked
for — on a phone the Android manifest declares the network permission only,
and the flatpak has no filesystem access at all.

| What | Where |
|------|-------|
| The ledger: sealed chunks and `staging.rdx` | `$XDG_DATA_HOME/app.akergez.MoneyManager/ledger/` |
| Settings: `settings.json` | `$XDG_CONFIG_HOME/app.akergez.MoneyManager/` |

On Android both are under the application's internal data directory.

`settings.json` takes the place of the terminal version's
`money_manager.toml`, under the same names: `source` (this installation's CRDT
stamp source), `remote` (the sync storage), `default_currency` and
`last_account`. It also holds what only an interface has, such as the theme.
**None of it is synced.** The sync storage's keys are in this file in the
clear, as they were in the `.toml`.

### Category colours and icons

A category's name, colour and icon are all part of its record in the ledger
and sync with it. The colour and the icon are two optional fields after the
name: a record without them — every one the terminal version wrote — is an
unstyled category, drawn in a colour derived from its name, the same on every
device; and a client that knows nothing of them still finds the name where it
always was. A record is rewritten whole, so a client that rewrites a category
without these fields removes its colour and icon.

Styles that an earlier build kept in `settings.json` (`category_styles`) are
moved into the ledger the first time it is opened.

## The ledger format

Unchanged from the terminal version; `crates/money-core/src/store.rs` describes
it in full. In short:

- The document is a tuple of six collections: categories, expenses, income
  categories, income, accounts, transfers. A record is a tuple whose first
  child is its hex id.
- Every write is stamped (source + Lamport time); a newer write replaces the
  record, and a deletion is a tombstone. This is what makes two devices'
  changes merge.
- Local writes accumulate in `staging.rdx`. A sync seals them into one
  immutable, content-addressed chunk and exchanges the missing chunks with the
  storage both ways.

A device joining a ledger syncs **first** and only then makes sure the default
account exists: made before the first sync, its fresh stamp would win over the
synced account's name, currency and opening balance. `store.rs` has a test
that shows both orders.

## Keyboard

| Key | Action |
|-----|--------|
| `Ctrl+1` / `Ctrl+2` / `Ctrl+3` | Transactions / Categories / Charts |
| `Ctrl+M` | Switch between expenses and income |
| `Ctrl+N` | New record (a new category on the Categories screen) |
| `Ctrl+E` | Edit the selected record |
| `Ctrl+T` | New transfer |
| `Ctrl+S` | Sync |
| `Ctrl+,` | Settings |
| `Ctrl+Q` | Quit |

## Building

A recent stable Rust (the version is pinned in `rust-toolchain.toml`) and,
on Linux, the development packages for Wayland/X11, Vulkan and fonts:

```sh
# Debian / Ubuntu
sudo apt install pkg-config libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libvulkan-dev libfontconfig-dev libfreetype-dev

# Fedora
sudo dnf install wayland-devel libxkbcommon-devel libxkbcommon-x11-devel \
  libxcb-devel vulkan-loader-devel fontconfig-devel freetype-devel
```

```sh
cargo run -p money-manager             # debug
cargo build --release -p money-manager # optimised, quick to rebuild
cargo build --profile dist -p money-manager  # what ships: thin LTO, slow
cargo test --workspace
```

`MONEY_MANAGER_DEMO=1 cargo run -p money-manager` skips the first-run question
and fills an *empty* installation with sample records. Point `XDG_DATA_HOME`
and `XDG_CONFIG_HOME` somewhere disposable to try it without touching your own
ledger.

## Packages

| Target | How | Result |
|--------|-----|--------|
| Linux tarball | `build-aux/package-tarball.sh` | `dist/money-manager-<v>-linux-<arch>.tar.gz` |
| Flatpak | `build-aux/publish-flatpak.sh` | `.flatpak-repo`; with `--site`, a signed repository and `.flatpakref` |
| Windows (cross-built on Linux) | `build-aux/build-windows.sh x86_64` | `dist/money-manager-<v>-windows-x86_64.zip` |
| Android | `build-aux/build-android.sh` | `dist/money-manager-<v>-android-aarch64.apk` |
| macOS | `build-aux/package-macos.sh` | `dist/money-manager-<v>-macos-<arch>.dmg` |

Each script's header says what it needs. `android/README.md` covers the
Android build in detail.

## Tests

- `cargo test --workspace` — the ledger (format, merge, sync against an
  in-memory remote) and everything in the application that needs no window.
- `build-aux/ui-tests.sh --headless` — the scenarios in
  `crates/money-manager/tests/ui/`, played into the real application under a
  headless sway. Each is a short script of keys, clicks and `expect:` checks
  against the application's state; see `crates/money-manager/src/script.rs`.

## Continuous integration

`.github/workflows/ci.yml` tests every push, builds every package as a
workflow artifact, and publishes only from a version tag (`v1.2.3`) whose
number every crate carries — `build-aux/check-version.sh` enforces it. The
version lives once, in `[workspace.package]` of `Cargo.toml`.

A release needs these repository secrets:

| Secret | For |
|--------|-----|
| `ANDROID_KEYSTORE_B64`, `ANDROID_KEYSTORE_PASSWORD` | Signing the Android package |
| `FLATPAK_GPG_ID`, `FLATPAK_GPG_KEY_B64`, `FLATPAK_GPG_PASSPHRASE` | Signing the flatpak repository |

### Signing the Android package

Android installs an update only over a package signed with the same key, so
every release is signed with one key, made once:

```sh
keytool -genkeypair -storetype PKCS12 -keystore release.p12 -alias money-manager \
  -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 release.p12   # → ANDROID_KEYSTORE_B64
```

Keep the keystore somewhere safe and out of the repository: losing it means
nobody can update an installed copy.

## What the terminal version had that this does not

The command-line subcommands are gone with the terminal interface: `sync` is
the Sync command, `keygen` and `config --export/--import` are in Settings, and
`import-csv` and the one-time migration from the single-blob format have no
counterpart. To bring an existing ledger over, connect to its sync storage; a
ledger that was never synced has to be synced from the terminal version first.

## License

GPL-3.0; see [LICENSE](LICENSE). The bundled Inter font is under the SIL Open
Font License 1.1 (`crates/money-manager/assets/fonts/Inter-LICENSE.txt`). The
icons are [Lucide](https://lucide.dev), ISC.
