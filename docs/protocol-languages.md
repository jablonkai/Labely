# Other printer languages — clean-room behavioural spec

Status: research draft, written for Labely (MIT/Apache). This file covers the printers that are not
NIIMBOT: generic ESC/POS, Phomemo, Marklife, TSPL and ZPL. It describes observable behaviour in our
own words. It contains no vendor code, no decompiled output and no OSS code (ADR-0001).

**Nothing in this file has been tried on a printer.** Labely's only test printer is a NIIMBOT B21
Pro. Every byte below comes from the sources in §0 and is checked only against Labely's own
simulators. A line marked *unverified* has one source, or sources that disagree.

## 0. Sources and evidence keys

| Key | Source | Licence | Reuse in Labely |
|---|---|---|---|
| `E:pos` | Epson, *ESC/POS Command Reference*, `download4.epson.biz/sec_pubs/pos/reference_en/escpos/` (pages `gs_lv_0`, `dle_eot`) | vendor documentation | facts only |
| `T:tspl` | TSC, *TSPL/TSPL2 Programming Manual* (31-0000001-00, 2025), `fs.tscprinters.com` | vendor documentation | facts only |
| `Z:zpl` | Zebra, *ZPL II Programming Guide*, `docs.zebra.com/us/en/printers/software/zpl-pg/` (pages `^GF`, `^MN`, `~HS`) | vendor documentation | facts only |
| `O:pt` | github.com/vivier/phomemo-tools, README §4 and §5 | GPL-3.0 | facts only; code not read into Labely |
| `O:phy` | github.com/transcriptionstream/phomymo (`ble.js`, `printer.js`, `printers.json`, `constants.js`) | README says MIT, no licence file | facts only |
| `O:d30` | github.com/polskafan/phomemo_d30 | MIT | facts only |
| `O:d30w` | github.com/odensc/phomemo-d30-web-bluetooth | Apache-2.0 | facts only |
| `O:pyp` | github.com/mkuhlmann/pyphomemo | MIT | facts only |
| `O:ml` | github.com/thermal-label/marklife and its docs (`thermal-label.github.io/marklife/protocol/l11`, `/hardware`) | MIT | facts only |
| `O:tp` | github.com/tomLadder/thermoprint, `REVERSE_ENGINEERING.md` | README says MIT, no licence file | facts only |
| `O:lf` | github.com/Finnfj/labelforge, `docs/PROTOCOL.md` | MIT | facts only |

The Epson pages refuse automated readers, so `E:pos` was read from search extracts. The syntax of
`ESC @`, `ESC d`, `GS V` and of the ZPL commands `^XA ^XZ ^PW ^LL ^LH ^PQ ~SD ^FO ^FS` was not
re-read for this spec. It is written from general knowledge of the two languages and is
*unverified* until someone checks the page or a printer.

## 1. Common model

- **One-way streams.** A job is a byte stream. The printer does not confirm a page and does not
  say when it has finished. Labely counts a page as printed once its bytes are sent.
- **No identity.** None of these printers says which model it is. The model comes from the
  Bluetooth name (§7.2) or from the user, as `?model=<id>` on the address.
- **Raster only.** Labely sends the rendered page as a 1-bit bitmap. It never uses the text,
  font or barcode commands of these languages.
- **Bitmap geometry.** As for NIIMBOT (protocol.md §7.1): the page is turned for `left` models,
  padded to a whole number of bytes, and cropped symmetrically to the head width. Rows are sent
  top first, most significant bit left.
- **Status before a job.** Where a status request exists, the driver asks before printing and
  refuses with a printer error if the cover is open or the paper is out. A printer that does not
  answer within 500 ms is taken as ready, and its status fields are unknown.
- **Copies.** TSPL and ZPL carry a copy count. For the others the driver sends the page once per
  copy.
- **Cancel.** A cancelled job stops before the next page. Bytes already sent are printed.

## 2. ESC/POS (generic)

### 2.1 Commands

| Bytes | Name | Use |
|---|---|---|
| `1B 40` | ESC @ | reset to power-on settings, at the start of each page |
| `1D 76 30 m xL xH yL yH d…` | GS v 0 | raster block; `m` = 0; `x` = bytes per row, `y` = rows, both little-endian; then `x·y` bytes; a 1 bit prints (`E:pos`) |
| `1B 64 n` | ESC d | print and feed `n` lines |
| `1D 56 m` | GS V | cut; `m` = 0 full, 1 partial |
| `10 04 n` | DLE EOT | real-time status `n`; the reply is one byte (`E:pos`) |

### 2.2 Page

`ESC @`, the page as blocks of at most 255 rows, `ESC d 3`, then `GS V` if the model cuts.
The block size is Labely's choice: `E:pos` allows more rows on most printers, and 255 is what
the Phomemo sources use for the same command (§3).

### 2.3 Status

- `DLE EOT 4`: bits 5 and 6 set (`0x60`) mean the paper has ended (`E:pos`).
- `DLE EOT 2`: bit 2 set means the cover is open (*unverified*, not re-read).
- A status byte has bits 1 and 4 set and bits 0 and 7 clear (*unverified*). Labely ignores a
  reply without that pattern.

### 2.4 Registry tasks

`raster` (no cut), `raster_cut` (partial cut), `raster_full_cut`.

## 3. Phomemo

Phomemo printers take ESC/POS raster blocks between vendor commands that start with `1F 11`
or `1B 4E`. There are three job layouts. `O:phy` describes two more (M04S, P12) that Labely does
not implement.

### 3.1 M02 family (M02, M02 Pro, T02): task `m02`

Continuous paper. One page (`O:pt` §4):

| Bytes | Meaning |
|---|---|
| `1B 40` | reset |
| `1B 61 01` | justify: 0 left, 1 centre, 2 right. Labely centres, to match the symmetric crop |
| `1F 11 02 04` | sent as `O:pt` shows it. `O:phy` documents `1F 11 02 n` as density 0–15 on the M04S; on the M02 the meaning is *unverified*, so Labely does not vary it |
| `1D 76 30 00 …` | raster blocks of at most 255 rows |
| `1B 64 02`, twice | feed |

`O:pt` ends the job with four more `1F 11 n` commands. `O:phy` identifies them as queries
(battery, power, firmware, serial), so Labely leaves them out.

### 3.2 M110 family (M110, M120, M220): task `m110`

Labels with a media sensor. One page (`O:pt` §5, agreed by `O:phy` and `O:pyp`):

| Bytes | Meaning |
|---|---|
| `1B 4E 0D n` | speed, 1 (slow) to 5. Labely sends 3 |
| `1B 4E 04 n` | density, 1 to 15 |
| `1F 11 n` | media: `0A` labels with gaps, `0B` continuous, `26` labels with black marks |
| `1D 76 30 00 …` | raster blocks of at most 240 rows (`O:pt`) |
| `1F F0 05 00`, `1F F0 03 00` | end of the label. No source explains the bytes |

*Unverified:* `O:pt` puts the M220 in this family; `O:phy` drives it with a plainer ESC/POS path.
Labely follows `O:pt`, which is a printer driver that lists the M220 as supported.

### 3.3 D30: task `d30`

A 12 mm label printer with a 96-dot head; the label runs along the tape, so the page is turned
(`left`). One page (`O:phy`, `O:d30w`; the raster header agrees with `O:d30`):

| Bytes | Meaning |
|---|---|
| `1F 11 n` | media: `0A` gap, `0B` continuous |
| `1B 40` | reset |
| `1D 76 30 00 …` | the whole label as one raster block |
| `1B 64 00` | end of the label |

`O:d30` sends a longer preamble of `1F 11 n` commands instead of the media command. They match
the queries of §3.4, so Labely leaves them out. `O:phy` groups the D35, Q30 and others with the
D30 without a capture of their own; they are not in the registry.

### 3.4 Status (`O:phy`)

Requests are `1F 11 n`: `11` paper, `12` cover (also `08` battery, `07` firmware, `09` serial,
not used). Replies are `1A <kind> <value…>`:

- `1A 06 n`: paper; `88` is out of paper.
- `1A 05 n`: cover; `98` open, `99` closed.

The battery reply (`1A 04 n`) has two readings in the sources, so Labely reports no battery.

## 4. Marklife

Marklife printers use two vendor dialects (`O:ml`, `O:tp`). Labely implements the first.

### 4.1 L11 (P12, P15): task `l11`

One page (`O:ml`, agreed by `O:tp`):

| Bytes | Meaning |
|---|---|
| `1F 70 02 n` | density 1 to 3 |
| `00` × 15 | wake the printer |
| `10 FF F1 02` | start of job |
| `1D 76 30 00 …` | the whole page as one raster block |
| `1D 0C` or `1B 4A 64` | feed to the next gap, or feed 100 dots on continuous tape |
| `10 FF F1 45` | end of job |

The printer answers `AA` when the job is done (`O:tp` also saw `4F 4B`). Labely waits up to 5 s
for `AA` after each page and goes on without it, because a printer that stays silent has usually
printed. No status request of this dialect is confirmed, so the status fields are unknown.

- The head has 96 dots (12 mm): measured for the P12, assumed for the P15 (`O:ml`). `O:tp` says
  384, which contradicts it. The resolution is taken as 203 dpi (*unverified*).
- Marklife P12 and Phomemo P12 are different printers with different byte streams.

### 4.2 Compressed dialect (P50, P50S): not implemented

`O:lf` documents it from a P50S: `1F`-prefixed commands around a zlib-compressed raster
(`1F 10 wH wL hH hL len`, window of 1024 bytes), ending when the printer answers `4F 4B`. It needs
a deflate encoder in the protocol crate and is left for later.

## 5. TSPL

Commands are text lines ending in CR LF (`T:tspl`). One label:

```
SIZE <w> mm,<h> mm
GAP <g> mm,0 mm          (or: BLINE <g> mm,0 mm)
DIRECTION 0
REFERENCE 0,0
DENSITY <0–15>
CLS
BITMAP 0,0,<bytes per row>,<rows>,0,<data>
PRINT 1,<copies>
```

- `SIZE` is the bitmap size converted with the model's resolution, to two decimals.
- `GAP 0 mm,0 mm` means continuous paper. For labels Labely sends a 2 mm gap or mark: the job
  carries the media type but no gap size. The printer measures the real gap itself.
- `BITMAP` mode 0 overwrites. The data is binary and follows the fifth comma. A **0 bit prints**
  (*unverified*: `T:tspl` shows only an example; `O:phy` inverts its data for TSPL). The padding
  bits are 1.
- `PRINT m,n` prints `m` sets of `n` copies.
- Status: `1B 21 3F` returns one byte: `01` head open, `02` paper jam, `04` out of paper, `08`
  out of ribbon, `10` paused, `20` printing, `80` other error; values add up (`T:tspl` p. 122).

Registry task: `bitmap`.

## 6. ZPL II

One label (`Z:zpl`; command syntax other than `^GF`, `^MN` and `~HS` is *unverified*, §0):

```
~SD<00–30>
^XA
^MN<Y|M|N>
^PW<width dots>
^LL<length dots>
^LH0,0
^PQ<copies>
^FO0,0^GFA,<bytes>,<bytes>,<bytes per row>,<hex>^FS
^XZ
```

- `~SD` sets the darkness. It is sent before the format because it acts at once.
- `^MN`: `Y` gap or notch sensing, `M` mark sensing, `N` continuous.
- `^GFA`: ASCII hex, two digits per byte; the first two numbers are the byte count of the data
  and of the whole graphic, which are equal here. A 1 bit prints (the guide says `00` is white).
  Labely does not use the compressed forms.
- Status: `~HS` returns three lines, each between STX (`02`) and ETX (`03`). Fields are separated
  by commas. Line 1, field 2 is "paper out" and field 3 "paused". Line 2, field 3 is "head up" and
  field 4 "ribbon out". `1` means yes.

Registry task: `gfa`.

## 7. Transports

### 7.1 TCP

Network label printers take the job on a raw socket, port 9100 by default (`T:tspl`, `NET PORT`).
Address: `tcp:<host>[:<port>]?model=<id>`.

### 7.2 Bluetooth LE

- **Phomemo:** service `FF00`, writes to `FF02`, notifications on `FF03` (`O:phy`, `O:pyp`,
  `O:d30w`). Sources send chunks of 128 bytes about 20 ms apart.
- **Marklife:** service `FF00`, writes to `FF02` without response, replies on `FF01`; `FF03`
  notifies flow-control credits (`01 nn`) and the MTU (`02 lo hi`) (`O:tp`, `O:lf`). `O:ml`
  printed L11 jobs without waiting for credits, and so does Labely.
- Labely writes to `FF02` and reads the notifications of every other characteristic of the
  service as one stream. Chunks are the negotiated write size with a 10 ms pause.
- Names: Phomemo printers advertise their model name (`M02`, `M110`, `D30`). Marklife names are
  the model, a separator and an id (`P12_2950_BLE`).
- Generic ESC/POS and TSPL printers often expose a serial-like service (`18F0` with `2AF1`, or
  the Microchip `49535343-…` service). Labely uses one when it has a characteristic that both
  notifies and takes writes without response.

### 7.3 Serial

`usb:<port>?model=<id>` opens a serial port (protocol.md §3.2) and sends the model's language.
USB printer-class devices (the usual USB mode of these printers) and Bluetooth Classic are not
supported.

## 8. Models

The registry files are `crates/printers/models/{phomemo,marklife,generic}.json`.

| Id | Driver / task | dpi | Head dots | Media | Source of head and dpi |
|---|---|---|---|---|---|
| `phomemo.m02` | phomemo / m02 | 203 | 384 | continuous | `O:pt`, `O:phy` |
| `phomemo.m02_pro` | phomemo / m02 | 300 | 624 | continuous | `O:phy` (78 bytes per row) |
| `phomemo.t02` | phomemo / m02 | 203 | 384 | continuous | `O:phy` |
| `phomemo.m110` | phomemo / m110 | 203 | 384 | gap, mark, continuous | `O:phy` |
| `phomemo.m120` | phomemo / m110 | 203 | 384 | gap, mark, continuous | `O:phy` |
| `phomemo.m220` | phomemo / m110 | 203 | 576 | gap, mark, continuous | `O:phy` |
| `phomemo.d30` | phomemo / d30 | 203 | 96 | gap, continuous | `O:d30`, `O:phy` |
| `marklife.p12` | marklife / l11 | 203 | 96 | continuous, gap | `O:ml` (head measured; dpi assumed) |
| `marklife.p15` | marklife / l11 | 203 | 96 | continuous, gap | `O:ml` (assumed) |
| `generic.escpos_58` | escpos / raster | 203 | 384 | continuous | common 58 mm receipt class |
| `generic.escpos_80` | escpos / raster_cut | 203 | 576 | continuous | common 80 mm receipt class |
| `generic.tspl_203` | tspl / bitmap | 203 | 832 | gap, mark, continuous | common 4-inch desktop class |
| `generic.tspl_300` | tspl / bitmap | 300 | 1248 | gap, mark, continuous | common 4-inch desktop class |
| `generic.zpl_203` | zpl / gfa | 203 | 832 | gap, mark, continuous | common 4-inch desktop class |
| `generic.zpl_300` | zpl / gfa | 300 | 1280 | gap, mark, continuous | common 4-inch desktop class |

- The generic entries describe a class of printer, not one product. A printer with another head
  width needs its own entry in a user registry file.
- The maximum label lengths in the registry are Labely's limits, not printer facts.
- Left out because the sources disagree or have no capture: Phomemo M02S (203 or 300 dpi), M03,
  M04S, M200, D35, Q30, P12; Marklife P11, P7, P50, P50S.

## 9. Open questions

- Everything: none of it has run on hardware.
- TSPL bitmap polarity and `DIRECTION` for a given printer.
- Whether the M02 accepts a density in `1F 11 02 n`.
- Phomemo battery reply, and a Marklife status request.
- Whether Phomemo printers need the 128-byte chunks and 20 ms pause the sources use.
