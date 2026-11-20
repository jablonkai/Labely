# NIIMBOT printer models — registry for Labely

Clean-room summary. The facts come from the vendor model registry and an MIT-licensed OSS model table,
restated as data. Evidence keys are the same as in `protocol.md`:

- `V:dev` = NIIMBOT 4.2.7 `App.framework/Resources/flutter_assets/assets/devices.json`. It is one JSON line, so it is cited by key.
- `nbl:<line>` = niimbluelib `src/printer_models.ts` @ `35110a71` (MIT).

## How models are identified

1. **Model id (authoritative).** Send PrinterInfo (0x40) with type 8. The u16 reply is the model code. Match it against `codes` (V:dev) / `id` (nbl). Several codes can map to one model (B3S 256/260/262, S6 257–261, B21-C2B 771/775). Some codes are shared by two names: 256 is listed for B3S, A8 and JCB3S; 512 for D11 and Hi-NB-D11; 2305 for D110 and HI_D110. Break such ties with the BLE name. **Confirmed** [E: V:dev key `codes`; nbl:957-959 `getPrinterMetaById`; protocol.md section 4].
2. **Code structure (observation).** The high byte usually names the family: 0x03xx B21 family (768 B21, 769 B21-L2B, 771/775 B21-C2B, 776 B21S-C2B, 777 B21S, **785 = 0x0311 B21 Pro**), 0x10xx B1, 0x02xx D11, 0x09xx D110, 0x0Exx B18/N1, 0x04xx P1. Codes ≥ 0xC900 (B11, B50, S1, T6…) and 0xD0xx (T2) belong to third-party engines. **Vendor-only** (inference from V:dev).
3. **BLE name prefix.** Used only to filter scans before connecting. niimbluelib filters on the first letters of known model names [E: O:nbl client/bluetooth_impl.ts:34]. The advertised name format (for example `B21_PRO-xxxx`) has not been verified.
4. **Command family.** `interfaceType` in V:dev:
   - 1 = NIIMBOT 0x55 0x55 protocol (protocol.md);
   - 2 = Zicox;
   - 3 = DZ;
   - 4 = SNBC.
   Types 2–4 use other vendors' printer languages and are out of scope for the NIIMBOT protocol [E: V:dev keys `interfaceType`, `interfaceName`].
5. **Protocol version.** Read at connect time (C1/C2, then A5 bytes 11–12). It selects the print-task variant (protocol.md section 5.2). OSS falls back to the D110M_V4 task for protocol ≥4 when the model is not listed [E: O:nbl print_tasks/index.ts:91-107].

## Column legend

| Column | Meaning |
|---|---|
| Codes | Model id(s), decimal (hex). |
| DPI | V:dev `paccuracyName`. `paccuracy` 8 = 203, 9 = 300. |
| Head px / mm | Printhead dots (nbl `printheadPixels`) and the width they give (px ÷ 8 at 203 dpi, ÷ 11.81 at 300 dpi). |
| Max W (range) | V:dev `maxPrintWidth` mm, and the label-width range the UI allows (`widthSetStart`–`widthSetEnd`). |
| Max H | V:dev `maxPrintHeight` mm (label length). |
| Label types | V:dev `paperType`, using protocol.md section 8 numbering: gap 1, black 2, cont 3, perf 4, transp 5, PVC 6, bm-gap 10, tube 11. |
| Transport | BLE for all NIIMBOT-family devices. Wi-Fi from V:dev `isSupportWifi`. USB-C serial on models that have a port: devices.json does not record USB, so check per device. |
| Family / task | Command family, then the OSS print task (nbl print_tasks/index.ts:77-85). "auto" means chosen from the protocol version. |
| RFID | V:dev `rfidType`: 0 none, 1 label RFID, 2/3 other classes (thermal-transfer/ribbon). The exact meaning of 2 and 3 is UNCERTAIN. |
| Density | V:dev `solubilitySetStart`–`End` / `Default`. |
| Dir | V:dev `printDirection` 0 → top, 90/270 → left (rotate the image 90°). |
| Tag | Confirmed = id, dpi, density range and direction all match nbl. Conflict notes say what differs. |

## Model table

| Model | Codes | Series | DPI | Head px / mm | Max W mm (range) | Max H mm | Label types | Transport | Family / task | RFID | Density min–max / def | Dir | Tag | nbl |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A20 | 2817 (0x0B01) | B203 | 203 | 400 / 50.0 | 50 (20–50) | 200 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:119 |
| A203 | 2818 (0x0B02) | B203 | 203 | 400 / 50.0 | 50 (20–50) | 200 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:130 |
| A63 | 2054 (0x0806) | B32 series | 300 | 851 / 72.1 | 75 (20–72) | 240 | gap,transp,black | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 2 | 1–15 / 10 | top | Confirmed | nbl:141 |
| A8_P | 273 (0x0111) | B3S series | 203 | 576 / 72.0 | 75 (20–72) | 200 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:163 |
| B1 | 4096 (0x1000) | B1 | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,transp | BLE | NIIMBOT / B1 | 1 | 1–5 / 3 | top | Confirmed | nbl:174 |
| B1 Pro | 4097 (0x1001) | B1 | 300 | 567 / 48.0 | 50 (20–48) | 200 | gap,black,transp | BLE | NIIMBOT / D110M_V4 | 1 | 1–5 / 3 | top | Confirmed | nbl:185 |
| B1 SE | 4098 (0x1002) | B1 | 203 | 384 / 48.0 | 54 (20–48) | 350 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:196 |
| B11 | 51457 (0xC901) | B11 series | 203 | 384 / 48.0 | 50 (10–48) | 200 | gap,black,cont,perf,transp | BLE | DZ (not 55 55) | 0 | 6–15 / 10 | top | Confirmed | nbl:207 |
| B16 | 1792 (0x0700) | Mengyin (B16) | 203 | 96 / 12.0 | 15 (12–12) | 100 | gap,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–3 / 2 | left | Confirmed | nbl:218 |
| B18 | 3584 (0x0E00) | B18 & N1 | 203 | 96 / 12.0 | 15 (12–12) | 120 | gap,transp,bm-gap,tube,cont | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 3 | 1–3 / 2 | left | Confirmed | nbl:229 |
| B18S | 3585 (0x0E01) | B18 & N1 | 203 | 96 / 12.0 | 15 (12–12) | 120 | gap,transp,bm-gap,tube,cont | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 3 | 1–3 / 2 | left | Confirmed | nbl:240 |
| B2 | 6913 (0x1B01) | B2 series | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:251 |
| B2 Pro | 6912 (0x1B00) | B2 series | 300 | 567 / 48.0 | 200 (20–48) | 200 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:262 |
| B203 | 2816 (0x0B00) | B203 | 203 | 400 / 50.0 | 50 (20–50) | 200 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:273 |
| B21 | 768 (0x0300) | B21 series | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,cont,transp | BLE | NIIMBOT / B21_V1 | 1 | 1–5 / 3 | top | Confirmed | nbl:284 |
| B21-C2B | 771,775 (0x0303,0x0307) | B21 series | 203 | 384 / 48.0 | 50 (10–48) | 200 | gap,cont,transp,black | BLE | NIIMBOT / B1 | 1 | 1–5 / 3 | top | Confirmed | nbl:306 |
| B21-L2B | 769 (0x0301) | B21 series | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,transp | BLE | NIIMBOT / B21_L2B | 1 | 1–5 / 3 | top | Confirmed | nbl:317 |
| B21S | 777 (0x0309) | B21 series | 203 | 384 / 48.0 | 50 (10–48) | 200 | gap,black,cont,transp | BLE | NIIMBOT / D110 | 1 | 1–5 / 3 | top | Confirmed | nbl:328 |
| B21S-C2B | 776 (0x0308) | B21 series | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,transp | BLE | NIIMBOT / D110 | 1 | 1–5 / 3 | top | Confirmed | nbl:339 |
| B21_Pro | 785 (0x0311) | B21 series | 300 | 591 / 50.0 | 50 (20–50) | 200 | gap,black,cont,transp | BLE | NIIMBOT / D110M_V4 | 1 | 1–5 / 3 | top | Confirmed | nbl:295 |
| B3 | 52993 (0xCF01) | B3 series | 203 | 600 / 75.0 | 75 (20–75) | 200 | gap,black,cont,transp | BLE | Zicox (not 55 55) | 0 | 1–5 / 3 | top | Confirmed | nbl:350 |
| B31 | 5632 (0x1600) | B31 | 203 | 600 / 75.0 | 77 (20–75) | 350 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:361 |
| B32 | 2049 (0x0801) | B32 series | 300 | 851 / 72.1 | 75 (20–72) | 240 | gap,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 2 | 1–15 / 10 | top | Confirmed | nbl:372 |
| B32R | 2050 (0x0802) | B32 series | 300 | 851 / 72.1 | 75 (20–72) | 240 | gap | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 2 | 1–15 / 10 | top | Confirmed | nbl:383 |
| B3S | 256,260,262 (0x0100,0x0104,0x0106) | B3S series | 203 | 576 / 72.0 | 75 (20–72) | 200 | gap,black,cont,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 0 | 1–5 / 3 | top | Confirmed | nbl:394 |
| B3S_A | 275 (0x0113) | B3S series | 203 | 576 / 72.0 | 75 (20–72) | 200 | gap,black,cont,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:405 |
| B3S_P | 272 (0x0110) | B3S series | 203 | 576 / 72.0 | 75 (20–72) | 350 | gap,black,cont,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:416 |
| B4 | 6656 (0x1A00) | B4 | 203 | 832 / 104.0 | 108 (20–104) | 350 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:427 |
| B4 Pro | 6657 (0x1A01) | B4 | 300 | 1229 / 104.1 | 108 (20–104) | 350 | gap,black,transp | BLE + Wi-Fi | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:438 |
| B50 | 51713 (0xCA01) | B50 series | 203 | 400 / 50.0 | 50 (20–50) | 200 | gap,black,cont,perf | BLE | DZ (not 55 55) | 0 | 6–15 / 10 | top | Confirmed | nbl:449 |
| B50W | 51714 (0xCA02) | B50 series | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,cont,perf | BLE | DZ (not 55 55) | 0 | 6–15 / 10 | top | Confirmed | nbl:460 |
| Betty | 2561 (0x0A01) | D101 series | 203 | 192 / 24.0 | 25 (12–24) | 200 | gap,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–3 / 2 | left | Confirmed | nbl:471 |
| C1 | 5120 (0x1400) | C1 | 300 | 178 / 15.1 | 50 (12–15) | 50 | cont | BLE | NIIMBOT / D110M_V4 | 2 | 1–5 / 3 | left | Confirmed | nbl:482 |
| D101 | 2560 (0x0A00) | D101 series | 203 | 192 / 24.0 | 25 (12–24) | 100 | gap,transp | BLE | NIIMBOT / B1 | 1 | 1–3 / 2 | left | Confirmed | nbl:493 |
| D11 | 512 (0x0200) | D11 series | 203 | 96 / 12.0 | 15 (12–12) | 100 | gap,transp | BLE | NIIMBOT / D11_V1 | 1 | 1–3 / 2 | left | Confirmed | nbl:504 |
| D110 | 2304,2305 (0x0900,0x0901) | D110 series | 203 | 96 / 12.0 | 15 (12–12) | 100 | gap,transp | BLE | NIIMBOT / D110 | 1 | 1–3 / 2 | left | Confirmed | nbl:537 |
| D41 | — () | D11 series | 203 | — / — | 15 (12–12) | 75 | gap,transp | BLE | NIIMBOT / ? | 1 | 1–3 / 2 | left | Vendor-only |  |
| D61 | — () | D11 series | 203 | — / — | 15 (12–12) | 75 | gap,transp | BLE | NIIMBOT / ? | 1 | 1–3 / 2 | left | Vendor-only |  |
| Dxx | — () | D11 series | 203 | — / — | 15 (12–12) | 200 | gap,transp | BLE | NIIMBOT / ? | 1 | 1–3 / 2 | left | Vendor-only |  |
| EP1C | 5121 (0x1401) | C1 | 300 | 178 / 15.1 | 50 (12–15) | 50 | cont | BLE | NIIMBOT / D110M_V4 | 2 | 1–5 / 3 | left | Confirmed | nbl:570 |
| EP2M_H | 4610 (0x1202) | M2 | 300 | 567 / 48.0 | 200 (20–48) | 200 | gap,transp,black,bm-gap | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 3 | 1–5 / 3 | top | Confirmed | nbl:581 |
| EP3M | 6402 (0x1902) | M3 | 300 | 851 / 72.1 | 78 (20–72) | 350 | gap,transp,black,bm-gap | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 3 | 1–5 / 3 | top | Confirmed | nbl:592 |
| Fust | 513 (0x0201) | D11 series | 203 | 96 / 12.0 | 15 (12–12) | 200 | gap,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | left | Confirmed | nbl:614 |
| H1S | 4352 (0x1100) | H1S | 203 | 96 / 12.0 | 15 (8–12) | 200 | gap,cont,transp | BLE | NIIMBOT / H1S | 1 | 1–3 / 2 | left | Confirmed | nbl:636 |
| Hi-NB-D11 | 512 (0x0200) | D11 series | 203 | 120 / 15.0 | 15 (12–15) | 100 | gap,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–3 / 2 | left | Confirmed | nbl:658 |
| JC-M90 | 51461 (0xC905) | B11 series | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,cont,perf | BLE | DZ (not 55 55) | 0 | 6–15 / 10 | top | Confirmed | nbl:669 |
| JCB3S | 256 (0x0100) | B3S series | 203 | 576 / 72.0 | 75 (20–72) | 200 | gap,black,cont,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 0 | 1–5 / 2 | top | Confirmed | nbl:680 |
| K2 | 6144 (0x1800) | K2 | 203 | 448 / 56.0 | 60 (20–56) | 300 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:691 |
| K3 | 4864 (0x1300) | K3 series | 203 | 640 / 80.0 | 82 (20–80) | 300 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:702 |
| K3_ITD | 4868 (0x1304) | K3 series | 203 | 640 / 80.0 | 82 (20–80) | 300 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:713 |
| K3_W | 4865 (0x1301) | K3 series | 203 | 640 / 80.0 | 82 (20–80) | 300 | gap,black,transp | BLE + Wi-Fi | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:724 |
| K4 | 7168 (0x1C00) | K3 series | 203 | 832 / 104.0 | 82 (20–104) | 300 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–15 / 7 | top | Confirmed | nbl:735 |
| M2_H | 4608 (0x1200) | M2 | 300 | 567 / 48.0 | 50 (20–48) | 240 | gap,transp,black,bm-gap | BLE | NIIMBOT / B1 | 3 | 1–5 / 3 | top | Confirmed | nbl:746 |
| M3 | 6400 (0x1900) | M3 | 300 | 851 / 72.1 | 78 (20–72) | 350 | gap,transp,black,bm-gap | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 3 | 1–5 / 3 | top | Confirmed | nbl:757 |
| MP3K | 4866 (0x1302) | K3 series | 203 | 640 / 80.0 | 82 (20–80) | 300 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:768 |
| MP3K_W | 4867 (0x1303) | K3 series | 203 | 640 / 80.0 | 82 (20–80) | 300 | gap,black,transp | BLE + Wi-Fi | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:779 |
| N1 | 3586 (0x0E02) | B18 & N1 | 203 | 96 / 12.0 | 15 (12–12) | 120 | gap,tube,transp,bm-gap,cont | BLE | NIIMBOT / B1 | 3 | 1–3 / 2 | left | Confirmed | nbl:790 |
| P1 | 1024 (0x0400) | P1S | 300 | 697 / 59.0 | 80 (20–59) | 150 | PVC | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 2 | 1–5 / 3 | left | Confirmed | nbl:801 |
| P18 | 1026 (0x0402) | P1S | 300 | 662 / 56.0 | 87 (20–56) | 150 | PVC | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 2 | 1–5 / 3 | left | Confirmed | nbl:812 |
| P1S | 1025 (0x0401) | P1S | 300 | 662 / 56.0 | 87 (20–56) | 150 | PVC | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 2 | 1–5 / 3 | left | Confirmed | nbl:823 |
| S1 | 51458 (0xC902) | B11 series | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,cont,perf | BLE | DZ (not 55 55) | 0 | 6–15 / 10 | top | Confirmed | nbl:834 |
| S3 | 51460 (0xC904) | B11 series | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,cont,perf | BLE | DZ (not 55 55) | 0 | 6–15 / 10 | top | Confirmed | nbl:845 |
| S6 | 261,259,258,257 (0x0105,0x0103,0x0102,0x0101) | B3S series | 203 | 576 / 72.0 | 75 (20–72) | 200 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 0 | 1–5 / 3 | top | Confirmed | nbl:856 |
| S6_P | 274 (0x0112) | B3S series | 203 | 600 / 75.0 | 75 (20–75) | 200 | gap,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 1 | 1–5 / 3 | top | Confirmed | nbl:867 |
| T2 | 53249 (0xD001) | T2 | 203 | — / — | 107 (20–104) | 280 | gap,cont | BLE | SNBC (not 55 55) | 0 | 1–20 / 15 | top | Vendor-only |  |
| T2S | 53250 (0xD002) | T2 | 203 | 832 / 104.0 | 107 (20–104) | 280 | gap,black | BLE | SNBC (not 55 55) | 0 | 1–20 / 15 | top | Confirmed | nbl:878 |
| T6 | 51715 (0xCA03) | B50 series | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,cont,perf | BLE | DZ (not 55 55) | 0 | 6–15 / 10 | top | Confirmed | nbl:889 |
| T7 | 51717 (0xCA05) | B50 series | 203 | 384 / 48.0 | 50 (20–48) | 200 | gap,black,cont,perf | BLE | DZ (not 55 55) | 0 | 6–15 / 10 | top | Confirmed | nbl:900 |
| T8 | 51718 (0xCA06) | T8 series | 300 | 567 / 48.0 | 50 (20–48) | 200 | gap,black,cont,perf | BLE | DZ (not 55 55) | 0 | 6–15 / 10 | top | Confirmed | nbl:911 |
| T8S | 2053 (0x0805) | B32 series | 300 | 851 / 72.1 | 75 (20–72) | 120 | gap | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 0 | 1–15 / 10 | top | Confirmed | nbl:922 |
| TP2M_H | 4609 (0x1201) | M2 | 300 | 591 / 50.0 | 50 (20–50) | 240 | gap,black,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 3 | 1–5 / 3 | top | Confirmed | nbl:933 |
| Z401 | 2051 (0x0803) | B32 series | 300 | 851 / 72.1 | 75 (20–72) | 240 | gap,transp | BLE | NIIMBOT / auto(v≥4→D110M_V4) | 2 | 1–15 / 10 | top | Confirmed | nbl:944 |

Rows tagged Vendor-only (D41, D61, Dxx, T2) are absent from nbl.

The density ranges and DPI of every model present in both sources agree, and so do the print directions.
No numeric Conflict was found. Note, though, that nbl lists the DZ-family models (B11, B50, B50W, S1, S3, T6, T7, T8, JC-M90) and the SNBC model T2S as if they used the NIIMBOT protocol. The vendor marks them as `interfaceType` 3 or 4. **Conflict (protocol family).**

### OSS-only models (in nbl, not in devices.json 4.2.7)

| Model | Code | DPI | Head px | nbl |
|---|---|---|---|---|
| A1_PRO | 7424 | 300 | 296 | nbl:108 |
| A8 | 256 | 203 | 576 | nbl:152 |
| D11_H | 528 | 300 | 142 | nbl:515 |
| D11_PRO | 531 | 300 | 142 | nbl:526 |
| D110_M | 2320 | 203 | 96 | nbl:548 |
| D11S | 514 | 203 | 96 | nbl:559 |
| ET10 | 5376 | 203 | 1600 | nbl:603 |
| H1 | 3840 | 203 | 96 | nbl:625 |
| HI_D110 | 2305 | 203 | 120 | nbl:647 |

All nine are in the Labely registry (ADR-0036), after the vendor models, so a shared code (256, 2305) still resolves to the vendor model unless the BLE name says otherwise.

| Model | Task | Media | Density | Direction | Max W × H mm (inferred) |
|---|---|---|---|---|---|
| A1_PRO | auto | cont (perforated is not a Labely media type) | 1–5 / 3 | left | 25 × 200 |
| A8 | auto | gap,black,cont | 1–5 / 3 | top | 75 × 200 |
| D11_H | D110M_V4 | gap,transp | 1–5 / 3 | left | 15 × 100 |
| D11_PRO | auto | gap,transp | 1–5 / 3 | left | 15 × 100 |
| D110_M | auto (B1; v≥4→D110M_V4) | gap,transp | 1–5 / 3 | left | 15 × 100 |
| D11S | D11_V1 | gap,transp | 1–3 / 2 | left | 15 × 100 |
| ET10 | auto | cont | 3–3 / 3 | top | 200 × 300 |
| H1 | auto | gap,transp | 1–3 / 2 | left | 15 × 100 |
| HI_D110 | auto | gap,transp | 1–3 / 3 | left | 15 × 100 |

- Task, media, density and direction: nbl `printer_models.ts` and `print_tasks/index.ts:77-85`.
- **Inferred:** nbl has no label size. The sizes follow registry models with the same head (A1_PRO and ET10: the head width, and a length that is a guess). UNCERTAIN.
- **RFID:** nbl does not say. The registry has it off, so the driver does not ask these printers for the roll.

## B21 Pro summary

| Property | Value | Evidence | Tag |
|---|---|---|---|
| Model code | 785 (0x0311) | V:dev B21_Pro `codes`; nbl:296 | Confirmed |
| Resolution | 300 dpi (resolution class 3 in the heartbeat) | V:dev `paccuracyName`; nbl:297 | Confirmed |
| Printhead | 591 dots ≈ 50.0 mm | nbl:299; V:dev `maxPrintWidth` 50 | Confirmed (consistent) |
| Label width / length | 20–50 mm wide (default 50×30), up to 200 mm long | V:dev `widthSetStart/End`, `defaultWidth/Heigth`, `maxPrintHeight` | Vendor-only |
| Label types | gap (1), black mark (2), continuous (3), transparent (5) | V:dev `paperType`; nbl:300 | Confirmed |
| Margins (blind zone) | 0.5 / 1.0 / 0.5 / 0.5 mm for gap, black, continuous and transparent stock | V:dev `consumables[].childProperties[].blindZone` | Vendor-only |
| Density | 1–5, default 3 | V:dev; nbl:301-303 | Confirmed |
| Orientation | "top": image rows go straight to the head, no rotation; centred (`printMethodCode` 2) | V:dev `printDirection` 0; nbl:298 | Confirmed |
| RFID | label RFID (`rfidType` 1); read with 0x1A | V:dev | Vendor-only |
| Transports | BLE (and classic BT on the B21 family per niimprint); USB-C serial; no Wi-Fi, no cutter | V:dev `isSupportWifi` false, `cutterType` NONE; O:npa README | Partly verified |
| Print task | protocol ≥4, D110M_V4 (OSS) ≈ vendor V6 (9-byte PrintStart) | nbl print_tasks/index.ts:83 | Confirmed (by structure) |

## Open questions / to verify on the B21 Pro

1. Does PrinterInfo type 8 return a 2-byte `03 11`?
2. Does heartbeat type 3 report head width 591 and resolution class 3?
3. Is 591 dots the real head width, or 592/600 with dead dots? Do a test print with edge lines.
4. Does the capabilities TLV (0xAF) agree with this table (DPI 300, density 1–5, label types, print protocol)?
5. BLE advertised name, and whether the B21 Pro also exposes a classic SPP channel like the B21.
6. USB VID:PID and the device node name on macOS (`/dev/cu.usbmodem…`?).
7. Meaning of `rfidType` 2/3, `errorPrintStrategy` (`LOW_DENSITY_2` on B21 Pro) and `blindZone` field order.
8. Does continuous roll (type 3) allow lengths beyond `maxPrintHeight` 200 mm?
