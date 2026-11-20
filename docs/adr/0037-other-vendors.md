# ADR-0037: Other vendors (Phase 6.3)

- Status: proposed
- Date: 2026-11-20

## Context
Slice 6.3 of ADR-0035 adds the printers that are not NIIMBOT: Phomemo, Marklife, generic ESC/POS,
TSPL and ZPL. The repository had no fact about any of them. Every driver path in the hub built a
`NiimbotDriver`, and the `driver` field of a registry entry was never read.

These printers differ from NIIMBOT in three ways. A job is a one-way byte stream that the printer
does not confirm. The printer does not say which model it is. And the common transports are a TCP
socket, a USB printer-class device and, on Bluetooth LE, a service with separate write and notify
characteristics, none of which Labely had.

Labely's only test printer is a B21 Pro.

## Decision
- **Spec first.** `docs/protocol-languages.md` holds the behaviour of the five languages with a
  source for every byte (ADR-0001). Vendor manuals are the source for ESC/POS, TSPL and ZPL. Open
  source projects are the source for Phomemo and Marklife, as facts only; two of them are GPL and
  none of their code is in Labely.
- **Encoders in `labely-protocol`.** One module per language: `escpos`, `phomemo`, `marklife`,
  `tspl`, `zpl`. Each builds the bytes of one page from a `RasterView` and has a decoder for the
  bytes it writes. The decoders exist for the simulator and the tests. `Raster` is the owned
  bitmap they return.
- **Raster only.** Every language gets the rendered 1-bit page. Labely does not use printer
  fonts or native barcodes, so the print matches the preview on every printer.
- **One stream driver.** `StreamDriver` in `labely-printers` serves all five. `Language::of`
  picks the language from the `driver` and `task` of the model. The hub now chooses the driver
  from the model: a stream language gets `StreamDriver`, everything else `NiimbotDriver`.
- **The model comes from outside.** A `ble:` printer is recognised by its name. `ble:`, `usb:`
  and `tcp:` addresses take `?model=<id>`. Without a model the printer is taken for a NIIMBOT, as
  before.
- **Status before the job.** The driver asks for the cover and paper state before printing and
  refuses a job the printer cannot print. A printer that does not answer in 500 ms is taken as
  ready. This replaces the per-page confirmation that these printers do not have.
- **Density per language.** The request's density is clamped to the model's range: 1 to 15 on
  the Phomemo M110 family, 1 to 3 on Marklife, 0 to 15 for TSPL, 0 to 30 for ZPL. ESC/POS, the
  Phomemo M02 family and the D30 have no confirmed density command and a range of one value.
- **TCP transport.** `labely-transport::tcp` is a raw socket on `std::net`, port 9100 by default.
  It needs no new dependency and no feature flag.
- **Split GATT characteristics.** `gatt::select_link` falls back to the `FF00` service of
  Phomemo and Marklife: writes go to `FF02`, and every notifying characteristic of the service
  is read as one stream. The btleplug transport uses it.
- **Simulator.** `StreamSim` answers status requests, sends the Marklife end-of-job byte and
  decodes the received bytes back into pages. It works inside the write call, so a test reads a
  complete result. `sim:<model id>` uses it for stream models. One test prints two copies on
  every model that is not NIIMBOT and compares the pages with the head-fitted label.
- **Registry.** `phomemo.json` (7 models), `marklife.json` (2) and `generic.json` (6). The
  generic entries are classes of printer, such as "ZPL 203 dpi". A model is listed only when its
  head width and job layout have a source.

## Consequences
- None of this has run on hardware. The spec marks what has one source or conflicting sources.
- The mobile shells cannot connect to Phomemo or Marklife printers yet. Their GATT code
  (`Gatt.kt`, `Gatt.swift`) still wants one characteristic for both directions, and they cannot
  pass a model for a printer with an unknown name.
- No user interface names a model for a `usb:` or `tcp:` printer. The address with `?model=` works
  from the CLI and MCP.
- USB printer-class devices and Bluetooth Classic are not supported. Most generic ESC/POS, TSPL
  and ZPL printers on USB are printer-class, so for them TCP is the working route.
- Marklife P50 and P50S are not supported. Their dialect needs a deflate encoder.
- A page counts as printed when its bytes are sent. A jam after that is not reported.
- No command schema changed, so `API_VERSION` stays 3. `device.models` lists 15 more models.
- A TSPL job assumes a 2 mm gap, because the job carries no gap size.
