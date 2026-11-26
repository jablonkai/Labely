---
name: hw-test
description: Test Labely against the user's real NIIMBOT B21 Pro label printer over Bluetooth LE — scan and connect, read status and the RFID paper roll, print one small test label, and verify the physical result with the user — or fall back to the protocol simulator (sim:) and the PNG-writing fake printer (fake:) when the device is not available. Use this whenever a change touches printing, BLE, transports, NIIMBOT protocol, device commands, print queue, RFID paper detection or print offsets and should be confirmed on hardware, or whenever the user asks for a device, hardware, real-printer or end-to-end print test. Triggers include "test it on the real printer", "try a print on the B21 Pro", "does it work on the device", "check the RFID paper", and Hungarian "teszteld a valódi nyomtatón", "próbáld ki a készüléken", "nyomtass egy tesztcímkét", "nézd meg a B21 Pro-n", "működik a valódi eszközön?", "olvasd ki a papírt". Every real print consumes a label, so each one needs the user's go-ahead.
---

# Hardware test on the B21 Pro

The simulator only proves the bytes Labely sends match what Labely expects. The real B21 Pro
over BLE has found bugs the simulator could not (a BLE panic among them), so device-facing
changes are worth one real check. But a real print uses up a label from the user's roll, and
the user has to look at the paper to tell whether it came out right. So: read-only steps
freely, prints one at a time and only with a yes, and the user judges the output.

## 0. Pick the route

- **CLI** (default for a test): `labely` is not installed globally, so run it from the repo:
  `cargo run -q -p labely-cli -- <subcommand>` (`ble` and `serial` are default features).
  Each invocation is its own session; `print` and `status` connect by themselves. The CLI
  session is not restricted, so `labely print` needs no `--allow-print`.
- **MCP** when you need several device commands in one session (e.g. `paper.detect`, queue
  inspection): `cargo run -q -p labely-cli -- mcp --headless --allow-print`. Without
  `--allow-print`, `print.*` and `device.calibrate` return `forbidden`. If the studio is
  running, `labely mcp` bridges to it instead, and printing then needs "Allow agents to print"
  ticked in the studio.
- Device commands (kind `device`) are refused inside atomic batches, so `labely run --script`
  cannot connect-and-print; use the dedicated subcommands or MCP `command_run`.

Filter cargo output (`2>&1 | tail -30` or grep) — the build log is noise.

## 1. Get the address

Never hard-code or invent an address. In order:

1. The user's memory for this project (the B21 Pro test-device note) or local config such as
   the studio's `studio.json` (`print_offsets` is keyed by address).
2. Scan: `cargo run -q -p labely-cli -- devices --timeout-ms 8000`. Pick the B21 Pro entry;
   addresses look like `ble:<peripheral id>`. On macOS the id is a CoreBluetooth UUID, stable
   per Mac.
3. Otherwise ask the user. If the printer is not found, ask them to switch it on, close the
   lid and make sure no phone app holds the BLE connection; then rescan once.

Do not paste the address into commits, docs, issues or test files.

## 2. Read-only checks (no confirmation needed)

```sh
cargo run -q -p labely-cli -- status -p <address>
```

`device.status` returns `battery_percent`, `lid_closed`, `paper_present` and `paper`
(RFID: `barcode`, `serial`, `total`, `used`, `media`). Report these. Stop and tell the user if
the lid is open, there is no paper, or the battery is very low — a print would fail or waste a
label.

RFID paper (ADR-0041): via MCP, `command_run {id: "device.connect", params: {address}}` then
`command_run {id: "paper.detect", params: {address}}` returns `rfid`, `barcode`, `media` and
the matching library paper, if any. `paper.detect` never changes the document; only the studio
auto-applies paper, and only with its setting on. The bundled barcode table
(`crates/core/papers/builtin.json`, `rfid`) holds only rolls read on owned devices, so an
unknown barcode is expected, not a bug — note it for the user.

For a lower-level look without Labely's driver (raw `PrinterInfo` replies, never prints):

```sh
cargo run -q -p labely-printers --features ble --example niimbot_probe -- <address>
```

## 3. Print one test label (confirm first)

Use a small, known label: `examples/hello-b21pro.labely` (50 x 30 mm, B21 Pro, border, text
with accents). Or a label built for the change under test; render it first so you know what
to expect:

```sh
cargo run -q -p labely-cli -- check examples/hello-b21pro.labely
cargo run -q -p labely-cli -- render examples/hello-b21pro.labely -o /tmp/labely-hw.png --dpi 203
```

View the PNG with Read. Then ask, every time, before any real print:

> I'll print **1 label** of `<file>` on the B21 Pro (`<address>`), density `<n>`. It uses one
> label from the roll (`<used>/<total>` used so far). OK?

Only after a yes:

```sh
cargo run -q -p labely-cli -- print examples/hello-b21pro.labely -p <address> --copies 1
```

Keep it to one copy and one record (`--count`, `--records`, `--copies` > 1 only if that is what
the test is about and the user agreed to the number). A "yes" covers that one print; the next
print, a retry, or `device.calibrate` (it feeds labels) needs a new yes.

Print offsets saved in the studio are a studio preference and are not applied by the CLI or
by headless MCP. If the user calibrated an offset for this printer, pass it explicitly
(`--offset-x` / `--offset-y` in mm) or say that the print is without it.

## 4. Verify

- Command side: the print waits for the job by default; check the envelope is `ok` and the job
  is `done`. Run `status` again: `paper.used` should have gone up by the labels printed.
- Paper side: ask the user to compare the label with the rendered PNG — position (left/right,
  top/bottom shift), cut-off edges, density, accents, barcode scannability. You cannot see the
  paper; do not claim it printed correctly from the envelope alone.
- Record what was tested, the firmware/paper reported, and the result in your report.

On an error, read the envelope `code`, `message` and `hint` before retrying. One retry after
fixing the cause, with a fresh confirmation; no loops.

## 5. Fallback: no device

When the printer is unavailable (not found, flat battery, user away), test the same path on
the virtual printers and say clearly that no real hardware was involved:

- `sim:niimbot.b21_pro` — the protocol simulator: real NIIMBOT encoding, decoded back,
  answers status. `cargo run -q -p labely-cli -- print <file> -p sim:niimbot.b21_pro` and
  `status -p sim:niimbot.b21_pro`.
- `fake:<dir>` — writes each job's pages as PNGs to `<dir>/job-NNNN/` (default model B21 Pro;
  `?model=<id>`, `?delay_ms=<n>` to watch the queue). View those PNGs with Read.
- Automated: `cargo test -p labely-printers`, `cargo test -p labely-ffi` (native BLE path on
  the simulator), `cargo test -p labely-mcp`.

Simulator output needs no confirmation; it consumes nothing.
