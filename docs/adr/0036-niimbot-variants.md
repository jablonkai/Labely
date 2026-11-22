# ADR-0036: NIIMBOT variants (Phase 6.2)

- Status: proposed
- Date: 2026-11-20

## Context
Slice 6.2 of ADR-0035 covers the NIIMBOT printers that do more than print black: two-colour
and grayscale rows, the cutter and the half-cut, and the nine models that only the OSS table
knows. The driver sent only 0x84 and 0x85 rows, the cut fields of SetPageSize were fixed at
zero, and those nine models were not in the registry.

Nothing above the driver knows a second colour yet. The document has no colour, the renderer
makes one 1-bit bitmap, and `device.print` has no colour or cut option. No source says which
models have a cutter or print gray levels, and the values of the cut fields are not documented.

## Decision
- **Scope: the driver side.** This slice makes the protocol, the driver, the simulator and the
  registry able to print these jobs. The editor colour, the cut options of the print dialog and
  a gray render path are separate work. They need a schema change and UI on three shells, and
  they need a printer to try them on.
- **Two-colour rows (0x8A).** `encode_rows_two_color` takes a black and a red plane of the same
  size. A blank row, a row of one colour and a row of both each have their own form
  (protocol.md §7.5). A dot set in both planes prints red. Identical rows are merged.
- **Grayscale rows (0x8D).** `encode_rows_gray` takes 16 levels per dot and writes the run
  descriptors of §7.5. The packet has a 2-byte length. `Packet` frames it by its command code.
  Only `Decoder::for_requests` (the simulator) reads the long form, so a host decoder cannot
  stall on a wrong length. A run at a data position above 511 is sent as plain data, because
  the descriptor has 9 bits for the position. No check-line is sent for gray rows: the only
  task with gray rows has none.
- **Which task.** `PrintTask::supports` follows the OSS capability table: two-colour on B1 and
  D110M_V4, grayscale on D110M_V4. These are the tasks whose PrintStart has the colour field.
  `build_pages` takes `Page::Mono`, `TwoColor` or `Gray`, sets the colour field from the pages,
  and refuses a job that mixes kinds or that the task cannot send.
- **The request.** `PrintRequest` gains `planes` (`None`, `Red` or `Gray`) and `half_cut`.
  `pages` stays the 1-bit page, so every driver can still print the job.
  - *Red:* the driver sends two planes when the printer reported a second colour at connect
    (0xB5 byte 10) and the task can. Otherwise the red dots print black.
  - *Gray:* the driver sends the gray maps when the task can. Otherwise it prints `pages`. No
    printer reports gray support, so the caller decides whether to ask for it.
- **Cut and half-cut.** `JobSettings.cut` fills the cut height and cut type of the 9- and
  13-byte SetPageSize. They stay zero by default, and no caller sets them yet.
  `JobSettings.half_cut` sends 0x5C `[01][on]` before PrintStart and waits for 0x6C.
- **Nine models.** A1 Pro, A8, D11_H, D11 Pro, D110_M, D11S, ET10, H1 and Hi-D110 are in the
  registry, after the vendor models. For a shared code (256, 2305) the vendor model stays the
  default, and the BLE name picks the OSS one. The OSS table gives the code, dpi, head, media,
  density and direction. The label sizes follow registry models with the same head, and RFID
  is off (printers.md).
- **Simulator.** It rebuilds the red plane and the gray map of each page, reports a second
  colour when told to, answers the half-cut and records the cut fields.

## Consequences
- None of this has run on hardware. The B21 Pro has one colour, no gray levels and no cutter.
- No user can print in two colours or cut yet: no command sets the new request fields.
  Update (#45): an element `color` of `red` now fills the red plane from `print.submit`;
  the registry marks the B21-C2B with `red`. Cutting is still open. No
  command schema changed, so `API_VERSION` stays 3.
- The feature-matrix rows for two-colour printing and the cutter options stay open. They need
  the document colour, the dialog and a list of cutter models.
- A mixed row is at most 125 bytes wide (1000 dots) and a one-colour row 250 bytes, because the
  0x8A packet has an 8-bit length. A wider row fails the job with a packet error.
- The registry has 66 NIIMBOT models. The label sizes of the nine new ones are inferred.
