# NIIMBOT printer protocol — clean-room behavioural spec

Status: research draft, written for Labely (MIT/Apache). This file describes observable behaviour in
our own words. It contains no vendor code, no decompiled output and no OSS code.

## 0. Sources and evidence keys

| Key | Source | Version / SHA | Licence | Reuse in Labely |
|---|---|---|---|---|
| `V:jc` | `/Applications/NIIMBOT.app/Contents/Frameworks/libjcPrinterSdk.dylib`, arm64 slice (thinned with `lipo`), studied with `nm`/`objdump`. Universal sha256 `6494605f…1ecd` | NIIMBOT desktop 4.2.7 | proprietary | facts only |
| `V:dev` | `App.framework/Resources/flutter_assets/assets/devices.json` (one JSON line, cited by key) and `series.json` | 4.2.7 | proprietary | facts only |
| `O:nbl` | github.com/MultiMote/niimbluelib | `35110a71` | MIT | allowed with attribution; we do not copy it |
| `O:nb` | github.com/MultiMote/niimblue | `937200ff` | MIT | as above |
| `O:np` | github.com/kjy00302/niimprint | `f4d66533` | MIT | as above |
| `O:npa` | github.com/AndBondStyle/niimprint (fork) | `35c41a70` | MIT | as above |

All four OSS projects are MIT, so they are licence-compatible with MIT/Apache. None is GPL. We still write
our own code from this spec. Vendor addresses are virtual addresses in the arm64 slice.
REA's decompiler providers were unavailable, so vendor evidence comes from symbol names, immediates and
instruction patterns.

Tags: **Confirmed** (vendor and OSS agree), **Vendor-only**, **OSS-only**, **Conflict** (explained inline).

---

## 1. Packet framing

Each packet, in either direction:

| Offset | Size | Field |
|---|---|---|
| 0 | 2 | Header `0x55 0x55` |
| 2 | 1 | Command code |
| 3 | 1 | Payload length N (0–255) |
| 4 | N | Payload |
| 4+N | 1 | Checksum: XOR of the command byte, the length byte and every payload byte |
| 5+N | 2 | Tail `0xAA 0xAA` |

- Header, tail and the 8-bit length: **Confirmed** [E: V:jc `cPrinterV4::packageCommand` @0xd14e0 writes 0x5555 @0xd153c and 0xAAAA @0xd1520, and checks length ≤0xFF @0xd154c] [E: O:nbl src/packets/packet.ts:11-12,80-124] [E: O:np niimprint/niimbotpacket.py].
- XOR checksum: **OSS-only**. The vendor checksum loop was not traced instruction by instruction [E: O:nbl packet.ts:72-78].
- **Connect exception:** the Connect request (0xC1) is sent with one extra byte, `0x03`, in front of the header. **Confirmed** [E: V:jc `cPrinterV4::jcSetConnect` stores 0x03 and inserts it at the start of the buffer @0xd1b4c-0xd1b5c] [E: O:nbl packet.ts:116-121].
- **16-bit length variant:** the grayscale row command 0x8D uses a 2-byte big-endian length, and its checksum covers both length bytes. **OSS-only** [E: O:nbl packet.ts:178-230; commands.ts:255-258]. The vendor has a 4-argument `cPrinterV6::packageCommand(…,int,int)` overload that may be the long form (unverified). Labely frames 0x8D this way and decodes it only on the printer side (ADR-0036).
- **Firmware variant:** firmware commands (0x91, 0x92, 0x9B, 0x9C, 0x90, 0x9A, 0x9D, 0x9E) carry a 2-byte chunk number before the length and a 4-byte big-endian CRC32 instead of the XOR byte. **OSS-only** [E: O:nbl packet.ts:281-396]. Labely should not implement firmware upgrade.
- Receive side: one transport read can hold several packets, and one packet can span several reads. Split the stream on header, length and tail [E: O:nbl packet_parser.ts:35-117] [E: O:npa printer.py `_recv`].
- Multi-byte integers are big-endian everywhere. **Confirmed** [E: V:jc `packageOneColorData` @0x1bebc8 stores the row number high byte first] [E: O:nbl utils.ts `u16ToBytes`].

### Request/response pairing

The response code is usually the request code plus 1 (Connect, PageStart, Print/Page end) or plus 0x10
(the setter commands 0x21–0x2B). The table below is authoritative. Two generic responses can replace any
expected reply:

- `0xDB` PrintError: payload byte 0 is an error code (section 13).
- `0x00` NotSupported.

[E: O:nbl client/abstract_client.ts:172-185] [E: O:npa printer.py `_transceive` handles 219 and 0]

The vendor sends each request with an expected response code (the 5th argument of `jcSendCommand`)
[E: V:jc `cPrinter_New::jcSendCommand` call sites].

---

## 2. Command table

"Vendor fn" names the libjcPrinterSdk method in which the pair was found by an immediate scan (TX code
stored as a command half-word, RX code passed to `jcSendCommand`). One-way means no response is expected.

| TX | RX | Name (ours) | Request payload | Response payload / meaning | Evidence | Tag |
|---|---|---|---|---|---|---|
| C1 | C2 | Connect | `01` (with the 0x03 pre-byte) | 1 byte: 0 = disconnected, 1 = connected (old), 2 = connected (protocol v1), 3 = connected, query 0xA5 for the version; 90 = firmware error | V:jc `jcSetConnect` (V2–V6, cPrinter_New) @0xd1afc/0xd1c28; O:nbl enumerations.ts:99-105, protocol.ts:319-333 | Confirmed |
| A5 | B5 | PrinterStatusData / hardware info | `01` | ≥13 bytes. Byte 10 is non-zero when the printer supports colour. Bytes 11 and 12 give `n = b11*100 + b12`: 204–299 → protocol v3, 300–301 → v4, ≥302 → v5 | V:jc `cPrinter_New::getPrinterHardwareInfo` (checks RX 0xB5); O:nbl packet_parser.ts:141-162 | Confirmed (cmd); OSS-only (thresholds) |
| 40 | 41–4F | PrinterInfo | 1 byte: info type | The reply code is 0x40 + type. Types: 1 density, 2 speed, 3 label type, 6 language, 7 auto-off, **8 model id (u16)**, 9 firmware version, 10 battery, 11 serial number, 12 hardware version, 13 BT MAC (reversed bytes), 14 print mode, 15 area | V:jc `cPrinter_Old::jcGetUSBPrinterBasicInfo` TX 0x40; O:nbl enumerations.ts:5-20, commands.ts:103-115, packet_parser.ts:164-173 | Confirmed (cmd); OSS-only (types) |
| AF | BF | Printer capabilities (TLV) | `01` | Repeated `[type u8][len u8][value]`. Types 0x01–0x24 cover DPI, printhead width, density range, supported label types, print protocol and others | V:jc `cPrinterV5/V6::getPrinterParam`; O:nbl enumerations.ts:184-257, packet_parser.ts:421-584 | Confirmed (cmd); OSS-only (TLV ids) |
| DC | DD/DE/DF/D9 | Heartbeat | 1 byte type: 1 = advanced-1 → DD, 2 = basic → DE, 3 = printer info → DF, 4 = advanced-2 → D9 | Section 9 | V:jc `jcSetPrinterHighLevelInfo` (all classes) expects DF or D9; `cPrinter_New::isSupportRfid` expects DE; O:nbl enumerations.ts:76-81, commands.ts:183 | Confirmed |
| 1A | 1B | Label RFID info | `01` | Section 10 | V:jc `getRfidInfo`/`getRfidParameter` (all classes); O:nbl packet_parser.ts:175-210; O:npa printer.py `get_rfid` | Confirmed |
| 1C | 1D | Ribbon RFID info (thermal-transfer models) | `01` | Same layout as 0x1B | V:jc `cPrinterV5/V6::getRfidInfo` sends both 1A and 1C | Confirmed |
| 59 | 69 | Paper info (gap/label size measured by the printer) | `01` | 18 bytes: gap px u16, total height px u16, paper type u8, gap mm×10 u16, total mm×10 u16, width px u16, width mm×10 u16, direction u8, tail px u16, tail mm×10 u16 | V:jc `cPrinterV5/V6::getPaperParam`; O:nbl packet_parser.ts:383-418 | Confirmed (cmd); OSS-only (layout) |
| 21 | 31 | Set density | 1 byte | `01` = OK | V:jc `setPrintDensity`; O:nbl packet_generator.ts:109 | Confirmed |
| 22 | 32 | Set print speed | 1 byte | bool | V:jc `setPrintSpeed` | Confirmed |
| 23 | 33 | Set label type | 1 byte (section 8) | bool | V:jc `setPrintLabelType`; O:nbl packet_generator.ts:113 | Confirmed |
| 24 | 34 | Motor drive / feed | 1 byte | bool | V:jc `jcSetMotorDrive` | Vendor-only |
| 25 | 35 | Auto paper delivery | 1 byte | bool | V:jc `jcSetAutoPaperDelivery` | Vendor-only |
| 26 | 36 | Set language | 1 byte | bool | V:jc `setPrinterLanguageType` | Confirmed |
| 27 | 37 | Set auto-off time | 1 byte (1–4; OSS names them 15, 30, 45, 60 min). Read back with PrinterInfo type 7 | bool | V:jc `setPrinterAutoShutDownTime`; O:nbl enumerations.ts:84-93 | Confirmed |
| 28 | 38 | Printer reset (settings) | `01` | bool | V:jc `setPrinterReset` | Confirmed |
| 2C | 3C | Set print mode (thermal / thermal-transfer) | 1 byte | bool | V:jc `setPrintMode` (B32, P1, M1 classes) | Vendor-only |
| 2D | 3D | Set label material | 3 bytes | bool | V:jc `Cb32Printer::setPrintMaterial`; O:nbl commands.ts:74 | Confirmed |
| 8E | 8F | Label positioning calibration | 1 byte: label type (section 8). UNCERTAIN: the byte's meaning is inferred from the vendor name `setPrintPaper`; Labely sends the label type and waits up to 20 s for any 8F reply | — | V:jc `setPrintPaper`; O:nbl commands.ts:13 | Confirmed |
| 58 | 68 | Sound settings | 3 bytes `[1 set / 2 get][1 BT-connect / 2 power][on]` | 3 bytes, byte 2 = state | O:nbl packet_generator.ts:97-103 | OSS-only |
| 01 | 02 | **PrintStart** | Length depends on the protocol class (section 5) | `01` = OK | V:jc `jcSetPrintAllTime`; O:nbl packet_generator.ts:213-236 | Confirmed |
| 03 | 04 | PageStart | `01` | bool | V:jc `jcSetPagePrint` @0xd28a4/0xd2968; O:nbl packet_generator.ts:242 | Confirmed |
| 13 | 14 | **SetPageSize** | 2/4/6/13 bytes (section 5) | bool. A 0xDB reply means no page was started | V:jc `jcSetPageHigh` (2 B), `jcSetPageHighWidth`, `jcSetPageInfo` (6 B in V3–V5, 45 B in V6); O:nbl packet_generator.ts:117-193 | Confirmed (see Conflict in 5.3) |
| 15 | 16 | SetQuantity (copies) | u16 | bool | V:jc `jcSetPrintCount` (V2 and older model classes only); O:nbl packet_generator.ts:195 | Confirmed |
| 20 | 30 | PrintClear / allow print clear | `01` | bool | O:nbl D110PrintTask.ts:22; O:npa `allow_print_clear` | OSS-only |
| 84 | — | Empty rows (one-way) | `[row u16][count u8]` | — | V:jc `sendPrintData` selects 0x84 @0xdc054; O:nbl packet_generator.ts:250 | Confirmed |
| 85 | — | Bitmap row (one-way) | `[row u16][c0][c1][c2][repeat u8][row bytes]` | — | V:jc @0xdc04c and `packageOneColorData`; O:nbl packet_generator.ts:254-263 | Confirmed |
| 83 | — | Indexed row (≤6 black px, one-way) | `[row u16][c0][c1][c2][repeat u8][pixel index u16 …]` | — | O:nbl packet_generator.ts:274-291 | OSS-only (see 7.4) |
| 8A | — | Colour row wrapper (one-way) | `[mode][inner cmd 84/85][inner payload]` or mixed mask format | — | V:jc `sendPrintData` @0xdc03c; O:nbl packet_generator.ts:265-318 | Confirmed |
| 8D | — | Grayscale row (16-level, 16-bit length) | `[0x84 empty / 0x86 data][row u16][repeat][…]` | — | V:jc `cPrinterV6::sendPrintData` and `package16GrayData` use 0x84/0x86; O:nbl packet_generator.ts:335-423 | Confirmed |
| 86 | D3 | Row check-line | `[row u16][01]` | D3 (sometimes preceded by C6) | V:jc `jcAskAfterPause` TX 86 len 3 → D3; O:nbl packet_generator.ts:328 | Confirmed |
| E3 | E4 | PageEnd | `01` | bool (false = busy, retry) | V:jc `jcSetPageTerminal` @0xdedbc/0xdee98 | Confirmed |
| A3 | B3 | Print status | `01` | ≥4 bytes: page u16, print progress %, feed progress %. A 10-byte reply carries an error code at byte 6 | V:jc `getPrinterPageStatus` (V2–V6); O:nbl packet_parser.ts:119-134 | Confirmed |
| E0 (unsolicited) | — | Page index notification | — | u16 printed-page counter (old D11) | O:nbl protocol.ts:112-130 | OSS-only |
| F3 | F4 | PrintEnd | `01` | bool (false = still printing, poll again) | V:jc `jcSetPrintTerminal` @0xdf59c/0xdf66c | Confirmed |
| DA | D0 | Cancel print | `01` | bool | V:jc `jcSetCanclePrint` | Confirmed |
| A6 | B6 | Pause / resume | 1 byte | bool | V:jc `pausePrint`, `resumePrint` | Confirmed |
| A4 | B4 | Get BLE max data length | `01` | Max bytes per BLE write | V:jc `cPrinter_New::getBleMaxDataLen` | Vendor-only |
| 54 | 64 | RFID success times / print counter | `01` | — | V:jc `getPrinterTimes`; O:nbl commands.ts:32 | Confirmed |
| 5A | 6A | Paper measurement / test page | `01` | — | V:jc `paperMeasurement` (V5/V6); O:nbl `PrintTestPage` 0x5A | Confirmed (code); meaning differs by source |
| 80 | 81 | Print test page | `01` | — | V:jc `jcSetPrintTestPage` | Vendor-only |
| 5C | 6C | Half-cut get/set | 2 bytes; set is `[01][on]` | — | V:jc `cPrinterV6::setHalfCut/getHalfCut`; O:nbl packet_generator.ts:604 | Confirmed |
| 0E / 0F | 0E / 0F | Tube settings / tube type and width | 3–5 bytes | — | V:jc `cPrinterV6::setTubeInfo`; O:nbl packet_generator.ts:599 | Confirmed |
| 07 | 08 | Printer config-2 (time get/set) | 2 or 8 bytes | — | V:jc `cPrinterV5/V6::setPrinterTime` len 8; O:nbl packet_generator.ts:608 | Confirmed |
| 09 | 0A | Key function get/set | 2–3 bytes | — | V:jc `get/setKeyFunction` | Confirmed |
| 0D | 0D | Print quality | 2 bytes | — | V:jc `getPrintQuality` | Confirmed |
| 89 | 88 | Compression / decompressed result | 2 bytes | — | V:jc `cPrinterV6::getPrinterDecompressedResult`; O:nbl commands.ts:69 | Confirmed (code); usage unknown |
| A0/A1/A2 | B0/B1/B2 | Wi-Fi SSID, password, config | variable | — | V:jc `jcSetWifiSsid/Password`, `getWifiConfiguration` | Vendor-only |
| FE | FF | Disconnect | `01` | — | V:jc `jcSetDiscannnect` | Vendor-only |
| 0B | 0C | Anti-counterfeit key | 1 byte | — | V:jc `cPrinter_New::getAntiCounterfeitKey`; O:nbl commands.ts:45 | Confirmed. **Out of scope:** Labely must not use it |
| F5, 9B, 9C, 91, 92 | F6, 9A, 9D, 9E | Firmware upgrade | — | — | V:jc `jcSetUpdateFirmware`; O:nbl packet_generator.ts:565-597 | Confirmed. Not to be implemented |

Other vendor-only codes seen with unknown semantics: 29/39 (set hardware version), 2A/3A (set SN), 2B/3B
(sensor limits), 50/60-61 (photocell), 52/62-63 (tag id), 1E/1F (empty-row count on P1, reverse feed),
5D (positioning-calibration result), 17 (local template history), 11/12 (time format, templates).
These are factory or advanced commands. Do not send them.

---

## 3. Transports

### 3.1 BLE (GATT)

- Vendor strings list three service/characteristic pairs, in this order [E: V:jc arm64 slice string offsets 0x200b15–0x200bae]:
  1. Service `E7810A71-73AE-499D-8C15-FAA9AEF0C3F2`, characteristic `BEF8D6C9-9C21-4C9E-B632-BD58C1009F9F`. This is the standard NIIMBOT pair.
  2. Service `0000E0FF-3C17-D293-8E48-14FE2E4DA212`, characteristic `FFE1`.
  3. Service `49535343-FE7D-4AE5-8FA9-9FAFD205E455` (Microchip/ISSC transparent UART), characteristic `49535343-8841-43F4-A8D4-ECBE34729BB3`.
  Pairing by list position is an inference.
- OSS filters on service 1. It then picks the first characteristic that has both **notify** and **write-without-response**, uses it for both directions, and filters scans by device-name prefix (the first letters of known model names). **Confirmed** for service 1 [E: O:nbl client/bluetooth_impl.ts:32-34,145-163].
- Writes are write-without-response, with a 10 ms gap between packets by default [E: O:nbl abstract_client.ts:40, bluetooth_impl.ts:180-185]. **OSS-only.**
- MTU/chunking: the vendor can ask the printer for its maximum BLE data length (0xA4 → 0xB4) [E: V:jc `cPrinter_New::getBleMaxDataLen`]. OSS writes one whole packet per GATT write, and 1-bit rows fit in about 80 bytes. **Recommendation:** negotiate MTU, keep each packet ≤ MTU−3, and do not split a packet across writes unless needed. Splitting is UNCERTAIN.
- The vendor also uses CoreBluetooth L2CAP callbacks and classic Bluetooth serial (`BTSerialPortCommunication`) [E: V:jc symbols]. niimprint (fork) prints to B21/B1 over **classic RFCOMM channel 1** [E: O:npa printer.py BluetoothTransport]. The README notes these models expose two BT addresses (classic and BLE).

### 3.2 USB (CDC-ACM serial)

- Vendor: macOS serial device found via IOKit (`IOCalloutDevice`), opened at **230400 baud, 8 data bits, no parity, 1 stop bit, no flow control** [E: V:jc `cPrinter_Old::initUSBPrinterConnect` @0x15dde4-0x15ddf8 → `serial::Serial`].
- OSS opens 115200 baud [E: O:npa printer.py:78] [E: O:nbl client/serial_impl.ts:39].
- **Conflict (baud)**. For a USB CDC-ACM device the baud setting is normally ignored, so both work in practice. Labely should use 230400 to match the vendor.
- Framing is the same as BLE. The vendor has a separate USB connect variant (`jcUSBSetConnect`, response 0x32 seen in the scan), meaning unknown.

### 3.3 Wi-Fi

TCP socket (`WifiCommunication`) on Wi-Fi models only (B4 Pro, K3_W, MP3K_W: `isSupportWifi=true`)
[E: V:jc symbols; V:dev key `isSupportWifi`]. The B21 Pro has no Wi-Fi. Port and discovery: UNCERTAIN.

---

## 4. Connection handshake

1. Open the transport and subscribe to notifications.
2. Send **Connect** (0x03 pre-byte + C1 `01`). The vendor retries up to 5 times [E: V:jc `jcSetConnect` loop counter compared with 5 @0xd1b84]. Read the C2 result:
   - 2 → protocol v1;
   - 3 → send **0xA5** and derive v3/v4/v5 from bytes 11–12 (table above). The vendor checks the result for 3 before the next query [E: V:jc `cPrinter_New::getPrinterProtocolVersion` compares 0xC2, then 3, @0x148ea4-0x148f24];
   - 1 or no reply → old/legacy (v0).
   **Confirmed.**
3. **Model id:** PrinterInfo type 8 → u16 (a 1-byte reply means `b<<8`). Look it up in the model table (printers.md) [E: O:nbl packet_parser.ts:164-173].
4. Optional: serial (11), firmware (9), hardware (12), battery (10), heartbeat type 3 (printhead width, resolution class, RFID support), capabilities 0xAF (protocol ≥4) [E: O:nbl abstract_client.ts:263-303].
5. RFID: 0x1A (label), 0x59 (paper info), 0x1C (ribbon).
6. Start a heartbeat about every 2 s (type 1; type 4 for protocol ≥3). Disconnect after 5 failures in a row [E: O:nbl abstract_client.ts:41,69; protocol.ts:362-376]. The vendor runs a `cPrinterControl::HeartBeatThread`. Interval: UNCERTAIN.

---

## 5. Print job sequence

### 5.1 Generic state machine

```
IDLE --(SetLabelType, SetDensity [, tube/half-cut])--> CONFIGURED
CONFIGURED --PrintStart(01)--> JOB
JOB --[PageStart(03)] SetPageSize(13) [SetQuantity(15)] rows… PageEnd(E3)--> PAGE_SENT
PAGE_SENT --(more pages)--> JOB
PAGE_SENT --poll PrintStatus(A3) until page == total--> DONE_PRINTING
DONE_PRINTING --PrintEnd(F3) until true--> IDLE
any --Cancel(DA)--> IDLE ; any --0xDB error--> ERROR (send F3/DA, then report)
```

### 5.2 Protocol classes (print-task variants)

Vendor classes `cPrinterV2`…`cPrinterV6`, plus per-model classes (`Cb21Printer`, `Cb21C2BPrinter`, `cB21V2`,
`cB21V3`, `cB1V3`, `cB1V4`, `Cb32Printer`, `Cp1Printer`, `CP1SPrinter`, `cN1V2`, `cK1V4`, …) are built by
`SimpleFactory::createPrinter(int)`, a 27-entry switch [E: V:jc @0x96f1c]. The PrintStart payload length
differs per class [E: V:jc `jcSetPrintAllTime`: V2 1 B, V3 2 B @0xb5340, V4 7 B @0xd2190, V5 8 B @0xec13c, V6 9 B @0x10d730].

| OSS task [O:nbl print_tasks/] | PrintStart payload | Page sequence | Completion | Vendor equivalent | Models (OSS mapping, print_tasks/index.ts:77-85) |
|---|---|---|---|---|---|
| D11_V1 | 1 B `01` | Clear(20), PageStart, Size 2 B (rows), Qty, rows, PageEnd | Unsolicited E0 page index | old/V2 | D11, D11S |
| B21_V1 | 1 B | per copy: PageStart, Size 4 B (rows, cols), rows with check-lines, PageEnd | Poll F3 until true | Cb21Printer | B21 |
| B21_L2B | 1 B | per copy: PageStart retried, Size 4 B, rows, PageEnd retried until true | PageEnd result | Cb21Printer variant | B21_L2B |
| D110 | 1 B | Clear, PageStart, Size 4 B, Qty(15), rows, PageEnd | Poll A3 | V2 | B21S, B21S_C2B, D110, D11 v1/v2 |
| B1 | 7 B `[total u16][00 00 00 00][colour]` | PageStart, Size 6 B (rows, cols, copies), rows, PageEnd | Poll A3 | **V4** (7 B, Confirmed @0xd214c-0xd2190) | B1, D110_M, B21_C2B, M2_H, N1, D101 |
| D110M_V4 | 9 B `[total u16][00 00 00 00][colour][speed/quality][flag]` | (one-way A3), Size 13 B, rows, PageEnd | Poll A3; PrintEnd, then a one-way heartbeat | **V6** (9 B, Confirmed @0x10d6dc-0x10d730) | **B21_PRO**, B1_PRO, D11_H, D110_M v4, C1, EP1C; fallback for any protocol ≥4 |
| H1S | 2 B `[total u16]` | PageStart, Size 9 B (rows, cols, copies, cut height, cut type), rows, PageEnd | Poll A3 | V3 (2 B) | H1S |
| (none) | 8 B | ? | ? | V5 | UNCERTAIN |

Labely implements all seven OSS tasks (ADR-0035). Only D110M_V4 has run on hardware (B21 Pro); the other six are checked against the simulator.

Two-colour pages can be sent by the B1 and D110M_V4 tasks, and grayscale pages by D110M_V4: these are the tasks whose PrintStart has the colour field [E: O:nbl print_tasks/index.ts:48-61 `printTasksCapabilities`]. **OSS-only.** Labely follows this (ADR-0036).

- PrintStart byte layout: **Confirmed** for 7 B and 9 B. The vendor writes the total-page count high byte first, then four zero bytes, then one class field (V4), or three class fields (V6) [E: V:jc addresses above] [E: O:nbl packet_generator.ts:229-236].
- Colour field values: 0 single, 1 two-colour, 2 alt, 3 grayscale [E: O:nbl enumerations.ts:157-162]. **OSS-only.**

### 5.3 SetPageSize (0x13) variants

| Len | Layout | Use |
|---|---|---|
| 2 | rows u16 | old D11 |
| 4 | rows u16, cols u16 | B21 V1, D110 |
| 6 | rows u16, cols u16, copies u16 | B1-class. Vendor V3/V4/V5 `jcSetPageInfo` sends 6 bytes, each field rendered as a 4-hex-digit (u16 BE) group [E: V:jc @0xb65e8, 0xd33ac, 0xed37c] |
| 9 | + cut height u16, cut type u8 | H1S |
| 13 | + cut height u16, cut type u8, 00, send-all u8, part height u16 | D110M_V4 / B21 Pro (OSS) |
| 45 | 13 bytes + 32-byte "local template id" | Vendor V6 `jcSetPageInfo(int×7)` [E: V:jc @0x10ec38-0x10ec90] |

- "rows" is the page length in printer rows (the feed direction). "cols" is the row width in pixels.
- **Conflict (V6 length):** OSS sends 13 bytes and leaves the 32-byte id off. The vendor sends 45 bytes. OSS reports that 13 bytes works on the B21 Pro [E: O:nbl packet_generator.ts:164-193].
- Field order rows, cols, copies: **OSS-only**. The vendor confirms the length and the u16 encoding only.

### 5.4 Completion and status polling

- Poll A3 about every 300 ms. Byte layout: page (u16), print % (u8), feed % (u8). The job is done when page equals the declared total. Time out after about 5 s with no progress [E: O:nbl AbstractPrintTask.ts:50-61; protocol.ts:154-170]. Polling A3: **Confirmed** [E: V:jc `getPrinterPageStatus`].
- Then send F3 and repeat while the reply is `00` [E: O:npa printer.py print_image; O:nbl protocol.ts:189-203].
- Errors arrive as 0xDB at any time. Abort on them.

---

## 7. Image encoding

### 7.1 Geometry and orientation

- 1 bit per pixel, 1 = black (burn). Each row is packed MSB first: bit 7 of byte 0 is the leftmost dot. **Confirmed** [E: V:jc `packageOneColorData` starts the bit cursor at 7 and decrements it @0x1bf090/0x1bf160] [E: O:nbl image_encoder.ts:162-186].
- Row width is padded up to a multiple of 8 [E: O:nbl image_encoder.ts:129-130].
- A row runs across the printhead. Rows are numbered from 0 in feed order.
- `printDirection`: models with devices.json `printDirection` 0 (B-series, "top") take the image as designed, with one image row per printer row. Models with 90/270 (D11, D110, B18, H1S, P1, C1: "left") need the label rotated 90° clockwise first, so the label's long axis runs along the feed [E: V:dev key `printDirection`; O:nbl printer_models.ts `printDirection`, image_encoder.ts:34,52-57]. **Confirmed** (the mapping from 0 to "top" and from 90/270 to "left" holds for every model listed in both sources).
- Width limit: cols ≤ printhead pixels (B21 Pro 591 per OSS; devices.json `maxPrintWidth` 50 mm at 300 dpi gives about 590). Content is centred on the head (`printMethodCode` 2 = centre; 1 = left; 3 = right) [E: V:dev keys `printMethodCode`, `printMethodName`].

### 7.2 Row commands

- **0x84 empty rows:** `[row u16][count u8]` means "the next `count` rows are blank". Use it for blank runs (count ≤255).
- **0x85 bitmap row:** `[row u16][c0][c1][c2][repeat u8][packed bytes]`. `repeat` (≥1) prints the same row again on the following rows, so identical neighbouring rows are merged. **Confirmed** [E: V:jc `packageOneColorData` @0x1bebc8-0x1bebe4 (row BE), then three counter bytes and one more byte; O:nbl packet_generator.ts:254-263, image_encoder.ts:202-221].
- **Row check-line (0x86):** after every 200 rows (100 for grayscale), send `[row u16][01]` and wait for D3. Used by the B21_V1/B21_L2B tasks [E: O:nbl image_encoder.ts:225-238; V:jc `jcAskAfterPause` 86→D3].

### 7.3 The three "count" bytes

- OSS: the row is split into three equal byte chunks of `floor(printheadPx/8/3)` bytes. Each count is the number of black pixels in that chunk ("split" mode). If the row is wider than 3 chunks, or in "total" mode, the bytes are `[0][low][high]` of the total count. niimprint sends all zeros and reports that it prints fine on the B21 [E: O:nbl utils.ts:98-151; O:npa printer.py `_encode_image` comment; O:np printencoder.py counts per 4 bytes].
- Vendor `packageOneColorData` groups by **pixel index**: 0–191, 192–383 and ≥384 [E: V:jc compares the index with 0xBF and 0x17F @0x1bf0b8-0x1bf0d0]. It is not yet clear whether the vendor counts black pixels or non-white pixels (the polarity in that loop is unclear).
- **Conflict:** for a 591-px head (B21 Pro) both rules give 24-byte (192-px) groups, so they agree. For 384-px heads OSS uses 128-px groups and the vendor routine uses 192-px groups. To verify on hardware.

### 7.4 Indexed rows (0x83)

OSS sends rows with ≤6 black pixels as a list of u16 pixel indexes. It warns that some printers power off if
more than 6 are sent [E: O:nbl packet_generator.ts:274-291, 442].

**Conflict:** the vendor V3–V6 `sendPrintData` chooses only between 0x8A, 0x85 and 0x84 [E: V:jc @0xdc03c-0xdc054]. 0x83 does not appear there.

**Recommendation for Labely:** do not send 0x83. Use 0x85 for every non-empty row (OSS `noIndexPacket`).

### 7.5 Colour and grayscale (not used on B21 Pro)

- 0x8A wraps a single-colour row (mode 1 red / 2 black) or carries a mixed mask plus colour-bit stream.
- 0x8D carries 4-bit grayscale rows with a run-length descriptor scheme.
[E: O:nbl packet_generator.ts:265-318, 335-423; V:jc `packageTwoColorData`, `package16GrayData`]

**Two-colour rows (0x8A).** The first byte is the mode: 0 empty, 1 red, 2 black, 3 mixed [E: O:nbl enumerations.ts:168-173]. The rows carry no count bytes. **OSS-only.**

| Row | Payload |
|---|---|
| Blank rows | `[00][84][row u16][count u8]` |
| One colour | `[01 red / 02 black][85][row u16][repeat u8][row bytes]` |
| Both colours | `[03][row u16][repeat u8][mask bytes][colour bytes]` |

- The mask has one bit per dot, set for every dot that prints (black or red). It is as long as a row.
- One colour byte follows for each **non-zero** mask byte, in order. A set bit means the dot is red.
- [E: O:nbl packet_generator.ts:265-272, 293-318, 527-553]

**Grayscale rows (0x8D).** The packet has the 16-bit length of section 1. A level is 4 bits: 0 white, 15 black. The width must be even. **OSS-only.**

| Row | Payload |
|---|---|
| Blank rows | `[84][row u16][count u8][00]` |
| Data row | `[86][row u16][repeat u8][n u8][n run bytes][data bytes]` |

- The data holds two levels per byte, high nibble first.
- A run descriptor is 2 bytes: `(dots + 2) | bit 8 of the data position`, then the low byte of the position. The data byte at that position holds two equal dots, and the printer repeats its level for `dots` more dots, which are left out of the data. `dots` is even and at most 252.
- `n` counts descriptor bytes, not runs. A run shorter than 8 dots is sent as data.
- OSS sends no check-line with grayscale rows. The vendor interval of 100 rows (section 7.2) is not used by any OSS task.
- [E: O:nbl packet_generator.ts:335-423, 481-520]

**Cut and half-cut.** The cut height and cut type of the 9- and 13-byte SetPageSize are zero in every OSS task, and their values are not documented. UNCERTAIN. The half-cut is switched with 0x5C `[01][on]`, answered with 0x6C [E: O:nbl packet_generator.ts:603-605, commands.ts:240].

Labely implements the row formats, the cut fields and the half-cut switch in the protocol crate and the driver (ADR-0036). None has run on hardware.

The vendor V6 also has a compressed image path (`processOneColorCompressImageData`, `sendCompressedPrintData`, capability `ImageCompressSupport`). Wire format: UNCERTAIN.

---

## 8. Density, label type, speed

- **Density 0x21:** 1 byte. Ranges: 1–3 for D11/D110/B18/D101/H1S, 1–5 for most B-series including the B21 Pro (default 3), 1–15 for B32/Z401/A63/K4, 6–15 for third-party DZ models [E: V:dev keys `solubilitySetStart/End/Default`; O:nbl printer_models.ts `densityMin/Max`]. **Confirmed.**
- **Label type 0x23:** 1 = with gaps (default), 2 = black mark, 3 = continuous, 4 = perforated, 5 = transparent, 6 = PVC tag, 10 = black-mark gap, 11 = heat-shrink tube [E: O:nbl enumerations.ts:62-73]. devices.json `paperType` uses the same numbering (B21 Pro `1,2,3,5`; B18 `1,5,10,11,3`; P1 `6`) [E: V:dev keys `paperType`, `consumables[].childProperties[].code`]. **Confirmed.**
- Speed 0x22: 1 byte. The OSS V4 task uses 0/1 in the PrintStart speed byte instead [E: O:nbl AbstractPrintTask.ts:31].
- devices.json `consumables[].childProperties[].blindZone` gives the unprintable margins in mm (B21 Pro gap labels `0.5|1.0|0.5|0.5`). Order: UNCERTAIN (probably top|bottom|left|right) [E: V:dev B21_Pro].
- devices.json `errorPrintStrategy` is `LOW_DENSITY_2` for the B21 Pro and `FORBID` for others. Meaning: UNCERTAIN (perhaps the app's behaviour when the RFID label is missing). Labely ignores it.

## 9. Heartbeat / status variants

| Reply | Request type | Len | Field meanings (0-based payload index) | Evidence |
|---|---|---|---|---|
| DD advanced-1 | 1 | 10 | [8] lid (0 = closed), [9] battery | O:nbl packet_parser.ts:235-279; O:npa printer.py heartbeat |
| DD advanced-1 | 1 | 13 | [9] lid, [10] battery, [11] paper (0 = present), [12] RFID read OK (≠0) | same |
| DD advanced-1 | 1 | 19 | [15] lid, [16] battery, [17] paper, [18] RFID | same |
| DD advanced-1 | 1 | 20 | [18] paper, [19] RFID | same |
| D9 advanced-2 | 4 | ≥9 | [2] battery, [3] temperature, [4] lid, [5] paper, [6] label RFID OK, [7] ribbon RFID OK, [8] ribbon present; optional Wi-Fi RSSI u16, lighting error, voltage state | O:nbl packet_parser.ts:281-315 |
| DF printer-info | 3 | 10 | hw version (2 B: major, minor/100), fw version (2 B), printhead width u16, resolution class (2 = 203 dpi, 3 = 300 dpi), printhead alignment, RFID support, RFID write support | O:nbl packet_parser.ts:212-233 |
| DE basic | 2 | ? | used by the vendor to probe RFID support | V:jc `cPrinter_New::isSupportRfid` |

- Battery values ≤4 are levels (×25 %). Larger values are already percent.
- For model ids 512, 513, 514, 2304, 1792, 3584, 5120, 2560, 3840, 4352, 272, 273, 274 the lid bit is inverted [E: O:nbl packet_parser.ts:268-272].
- The vendor accepts DF or D9 for its heartbeat query [E: V:jc `jcSetPrinterHighLevelInfo`].
- Layout details: **OSS-only.**

## 10. RFID label detection (0x1A → 0x1B)

- A 1-byte payload (0x00) means no tag or no RFID.
- Otherwise, in order:
  - UUID: 8 bytes (hex);
  - barcode: 1-byte length + ASCII;
  - serial: 1-byte length + ASCII;
  - total length u16 (label count or length units);
  - used length u16;
  - consumable type u8 (section 8 numbering);
  - optionally capacity u16;
  - optionally a second 8-byte UUID.
- **Confirmed (command); OSS-only (layout)**, with two OSS projects agreeing [E: O:nbl packet_parser.ts:175-210; O:npa printer.py `get_rfid`; V:jc `getRfidInfo`].
- The barcode can be used to look up label size offline (Labely's own catalogue; no vendor cloud).
- devices.json `rfidType` (0–3) is a per-model RFID class: 0 none, 1 label, 2/3 variants on P1/C1/B32/M2 [E: V:dev]. Exact meaning: UNCERTAIN.
- The ribbon tag on thermal-transfer models (M2/M3/B32) is read with 0x1C → 0x1D in the same layout.

## 11. Per-model and protocol-version notes

- **Third-party engines:** devices.json `interfaceType` gives the command family. 1 = NIIMBOT (this protocol), 2 = Zicox (B3), 3 = DZ/"Daozhen" (B11, B50, B50W, S1, S3, T6, T7, T8, JC-M90), 4 = SNBC (T2, T2S). Types 2–4 do **not** use the 0x55 0x55 framing [E: V:dev keys `interfaceType`, `interfaceName`; V:jc symbols `PrinterName_DZ_B`, `PrinterName_DZ_T`, `PrinterName_SNBC_T`]. niimbluelib still lists some of these (B11, B50, T8, S1…) with model ids ≥51457 [E: O:nbl printer_models.ts:206,448,910]. **Conflict:** do not assume they work with this protocol.
- **D11 firmware versions:** devices.json `rfidNotSupportVersions` lists firmware where RFID is unsupported (D11, D41, D61, Hi-NB-D11, B21-C2B, B21S) [E: V:dev].
- **B21 Pro, observed (firmware 3.12, Labely probe `niimbot_probe`):** PrinterInfo replies:
  density `02`, label type `01`, auto-off `03`, model id `03 11`, firmware `03 0C` (3.12),
  battery `02` (level, ×25 %), serial ASCII `HC12345678`, hardware `03 01` (3.01), BT MAC
  6 bytes, area `00 00`. Speed (2), language (6) and print mode (14) answer with cmd `00`
  (not supported). Heartbeat type 3 (DF) gets no reply.
- **B21_PRO firmware quirks (OSS):** the first packet after PrintStart is not answered over BLE, so OSS sends a one-way A3 first. The first packet after PrintEnd is dropped, so OSS sends a one-way heartbeat after F3 [E: O:nbl D110MV4PrintTask.ts:42-46,67-75].

## 12. B21 Pro: exact path to print one label

Model id 785 (0x0311), 300 dpi, 591-dot head (OSS) / 50 mm max width (vendor). Protocol ≥4 → V6-class job.
Evidence as cited in the sections above.

1. **Transport.** BLE: scan for name prefix "B" (the exact advertised name is to be checked on the device). Connect GATT, find service `E7810A71-…` and characteristic `BEF8D6C9-…`, enable notify, write without response. USB-C: CDC serial at 230400 8N1.
2. **Connect.** Send `03 55 55 C1 01 01 C1 AA AA`, wait for C2.
   - If the result is 3: send A5 `01`, read B5, compute the protocol version (expected ≥4).
3. **Identify.** Send 40 `08` and expect `0x0311`. Optionally send DC `03` for head width/resolution and AF for capabilities.
4. **Check media.** Send 1A `01` and read the RFID (label type, remaining count). Send DC `01`/`04` (heartbeat) for lid, paper and battery.
5. **Configure.** Send 23 `01` (gap labels; 2 black mark, 3 continuous, 5 transparent), then 21 `03` (density 1–5). Wait for 33/31 `01` each.
6. **Start the job.** Send 01 with 9 bytes `[00 01 = 1 page][00 00 00 00][00 = single colour][speed/quality][00 = flag]`. Wait for 02 `01`.
7. **Per label.**
   1. Send A3 `01` as one-way (do not wait for the reply; it is the quirk workaround).
   2. Send 13 with 13 bytes: rows = label length in dots (30 mm ≈ 354), cols = image width in dots (≤ 591, padded to a multiple of 8), copies = N, then zeros. Wait for 14.
   3. Send every row as 0x84 (blank runs) or 0x85 (repeat-merged bitmap rows). Rows are one-way; leave about 10 ms between BLE writes.
   4. Send E3 `01` and wait for E4. If the reply is `00`, retry.
8. **Wait.** Poll A3 every 300 ms until page == total pages. Stop on DB (error).
9. **Finish.** Send F3 `01` until F4 `01`, then a one-way DC heartbeat.

Unverified points in this path: the 9-byte start byte 7 (speed) and byte 8 (flag), 13-byte versus vendor
45-byte page info, whether PageStart (03) is required (vendor V6 implements it; OSS omits it), and the count
bytes in 0x85.

## 13. Error codes (0xDB payload byte 0)

01 cover open · 02 no paper · 03 low battery · 04 battery fault · 05 user cancel · 06 data error · 07 overheat ·
08 paper-out fault · 09 busy · 0A no printhead · 0B temperature low · 0C printhead loose · 0D no ribbon ·
0E wrong ribbon · 0F used ribbon · 10 wrong paper · 11 set paper failed · 12 set print mode failed ·
13 set density failed · 14 RFID write failed · 15 set margin failed · 16 communication error · 17 disconnected ·
18 canvas parameter error · 19 rotation parameter error · 1A JSON parameter error · 1B abnormal paper output ·
1C check paper · 1D RFID tag not written · 1E density not supported · 1F print mode not supported ·
20 label material error · 21 label material not supported · 22 RFID write not supported · 32 illegal page ·
33 illegal ribbon page · 34 receive-data timeout · 35 non-dedicated ribbon

[E: O:nbl enumerations.ts:111-151] **OSS-only.** The vendor has `processError`/`processErrorWithCode`, but its table was not extracted.

The C2 connect result 90 (0x5A) means firmware errors [E: O:nbl enumerations.ts:104].

---

## Open questions / to verify on the B21 Pro

1. BLE advertised name format and which GATT pair the B21 Pro exposes (E7810A71/BEF8D6C9 expected; E0FF/FFE1 and ISSC 49535343 also exist in the vendor SDK).
2. Negotiated MTU and the 0xA4 → 0xB4 max-data-length value. Does a packet larger than the MTU need splitting?
3. The C2 connect result and the A5 bytes 11–12 (protocol v4 or v5?), and which vendor class (V5 8-byte or V6 9-byte PrintStart) the app picks for model 785.
4. Does the 13-byte SetPageSize work, or is the 45-byte form needed? Is PageStart (0x03) needed before it?
5. Meaning of PrintStart bytes 7–8 on the B21 Pro (speed/quality, flag).
6. 0x85 count bytes: are zeros accepted, 192-px group counts, or total? Black or non-white count?
7. Is 0x83 (indexed row) accepted, or does it cause a reset as reported for other models?
8. Are the "no reply after PrintStart" and "dropped packet after PrintEnd" quirks seen over USB too, or only BLE?
9. Heartbeat reply length for types 1 and 4 on the B21 Pro, and lid-bit polarity.
10. RFID payload: is the trailing capacity / second UUID present? Units of total/used.
11. USB VID:PID, and whether 115200 and 230400 both work.
12. `blindZone` field order and whether the printer clips the 0.5 mm side margins in hardware.
13. Status A3 reply length (4, 8 or 10 bytes) and the error-byte position.
14. Calibration (0x8E): is the payload the label type, and what does the 0x8F reply carry? Does the B21 Pro answer only after feeding?
15. Do the B21 Pro sound settings (0x58) and auto-off read-back (PrinterInfo 7) answer?
