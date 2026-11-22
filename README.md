# Labely

Free, open-source label printing for NIIMBOT and other thermal label printers.
Works fully offline, with no accounts, ads or telemetry.

One Rust engine drives every surface: a desktop editor (studio), a command-line tool, an MCP
server for AI agents, and Android and iOS apps.

## Features

- Label editor with text, barcodes, QR and other 2D codes, images, shapes and lines.
- Data merge from CSV and spreadsheets (`.csv`, `.tsv`, `.xlsx`, `.xls`, `.ods`), serial
  numbers, and scan-to-print record lookup.
- Print queue, multiple printers, copies, density, offset, mirror and 180° rotation.
- 16-level grayscale (B1 Pro, B2 Pro, B21 Pro) and red/black two-colour printing.
- 50 starter templates by category, listed by fit to the loaded roll.
- Export to PDF and PNG. Opens NiimBlue JSON labels and a subset of ZPL.
- An open JSON document format (`.labely`), with undo and redo for every change.
- English and Hungarian UI.

## Printers

- **NIIMBOT** (B1, B21, B21 Pro, B3S, D11, D110 and more) over Bluetooth LE and USB.
- **Other vendors:** Phomemo, Marklife, generic ESC/POS, TSPL and ZPL printers, also over TCP.

See [docs/printers.md](docs/printers.md) and [docs/protocol-languages.md](docs/protocol-languages.md).
`labely models` lists every known model.

## Build from source

The toolchain is pinned in `rust-toolchain.toml`; [rustup](https://rustup.rs) installs it.

```sh
cargo run -p labely-studio --release    # desktop editor
cargo install --path crates/cli         # the `labely` command-line tool
```

Mobile apps: [mobile/android/README.md](mobile/android/README.md) and
[mobile/ios/README.md](mobile/ios/README.md).

## Command line

```sh
labely devices                                   # find printers (Bluetooth and USB)
labely status  -p ble:<id>                       # battery, lid, loaded labels
labely render  examples/hello-b21pro.labely -o hello.png
labely print   examples/hello-b21pro.labely -p ble:<id> --copies 2
labely print   label.labely -p usb:/dev/cu.usbmodem1 --data names.csv --records 1-10
labely export  label.labely -o labels.pdf --data names.csv
labely run     label.labely --cmd <command> --params '{…}' --save
labely commands                                  # every command with its JSON schema
```

Printer addresses are `ble:<id>`, `usb:<port>`, `tcp:<host>[:<port>]`, `sim:<model>` (simulator)
or `fake:<dir>` (writes PNGs). Add `?model=<id>` for printers that are not NIIMBOT.

## AI agents (MCP)

Labely exposes its whole command registry over MCP. Add it to an MCP client such as Claude Code,
Claude Desktop or Cursor:

```json
{ "mcpServers": { "labely": { "command": "labely", "args": ["mcp"] } } }
```

When the studio is running, the agent's changes appear live in it; otherwise the server runs
headless. Agents can print only when you allow it. See [AGENTS.md](AGENTS.md) and
[docs/agents/](docs/agents/usage.md).

## Development

```sh
cargo xtask ci    # fmt, layer check, i18n check, clippy and tests
```

- [docs/architecture.md](docs/architecture.md): crates and design.
- [docs/adr/](docs/adr/): architecture decisions.
- [docs/roadmap.md](docs/roadmap.md): phases and plans.
- [docs/releasing.md](docs/releasing.md): how a tag releases.

Labely is a clean-room implementation and is not affiliated with NIIMBOT or any other printer
vendor.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
