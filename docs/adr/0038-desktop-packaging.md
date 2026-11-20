# ADR-0038: Desktop packaging (Phase 6.4)

- Status: proposed
- Date: 2026-11-21

## Context
Slice 6.4 of ADR-0035 makes the desktop programs installable. Until now the only way to get the
`labely` CLI or the studio was `cargo build`. The repository had no icon, no bundle metadata and
no release profile.

The two programs do not depend on each other. `labely mcp` finds a running studio through
`mcp-connection.json` in the data directory and never starts it, and both embed their fonts,
locales, templates and papers. So each can ship on its own.

The tag-driven release workflow is slice 6.5. The development machine is a Mac without Docker and
without a Developer ID certificate.

## Decision
- **CLI: dist.** `dist-workspace.toml` packages `labely-cli` with dist (cargo-dist) 0.34: one
  archive per target with both licences and a sha256, plus a shell and a PowerShell installer.
  The targets are macOS and Linux on arm64 and x86_64, and Windows on x86_64. `labely-studio` and
  `labely-ffi` opt out with `[package.metadata.dist] dist = false`.
- **Studio: cargo-packager.** `cargo xtask package [formats]` builds the studio and runs
  cargo-packager 0.11: `.app` and `.dmg` on macOS, MSI (WiX) on Windows, AppImage and deb on
  Linux. One tool covers the four formats and macOS signing. cargo-bundle does not sign, and
  cargo-deb plus cargo-wix plus linuxdeploy would be three configurations.
- **One config file.** `crates/studio/packaging/packager.json` holds what does not change:
  the name "Labely", the identifier `org.labely.studio`, icons, the macOS plist keys and the deb
  dependencies. The xtask adds the version, the directories and the signing identity, and passes
  the result with `--config`. The xtask tests check that every file the config names exists.
- **One profile.** `[profile.dist]` (release with thin LTO) builds both programs.
- **Signing.** macOS signs with the hardened runtime when `LABELY_SIGNING_IDENTITY` is set, and
  notarises when the `APPLE_*` variables of cargo-packager are set too. Without them the build
  still produces an unsigned package, so anyone can build one. The app needs no entitlement:
  it is not sandboxed, and Bluetooth needs only the usage text in `Info.plist`.
- **Icon.** `packaging/icons/labely.svg` is the Labely mark of the mobile launcher icon.
  `generate.sh` renders the PNG, `.icns` and `.ico` files, which are committed.
- **Studio changes.** A release build on Windows is a GUI program (no console window). The window
  has the app id `labely-studio` and, on Windows and Linux, the icon.
- **No release workflow yet.** dist has `ci = []`. `.github/workflows/package.yml` only builds
  the packages on the three desktop systems, on demand and when packaging files change, and
  keeps them as workflow artefacts. Slice 6.5 turns this into the release.

## Consequences
- Only the macOS side ran: the CLI archive for arm64, and the studio as `.app` and `.dmg` with
  an ad-hoc signature, which starts. The MSI, the AppImage, the deb, the other CLI targets and
  the two installers have not been built anywhere. `package.yml` has not run either, because
  GitHub Actions was stopped for the account when this landed.
- Nothing is signed with a Developer ID or notarised yet, and the MSI is not signed. Slice 6.5
  needs the certificates as secrets.
- The installers download from the `repository` of `Cargo.toml`, `github.com/labely/labely`.
  The code is at `github.com/jablonkai/Labely` today. One of the two has to change before the
  first release.
- The deb dependencies come from the crates (D-Bus for BlueZ, libudev, xkbcommon, Wayland or
  X11, Vulkan or GL), not from a test on a Debian system.
- `labely-studio --help` prints nothing on Windows in a release build.
- `.labely` files are not associated with the studio. On macOS a double-clicked file arrives as
  an Apple event, which the studio does not handle.
- The studio has no updater. An update is a new download.
- dist needs `hosting = "github"` while `ci` is empty; without it `dist plan` panics.
