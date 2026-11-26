# Hungarian (`hu`) style guide

Read the surrounding `hu` entries first and reuse their words.

## Register and grammar

- Address the user informally (te): `Válassz nyomtatót`, `Koppints egy elemre…` (mobile),
  `Kattints…`, `Válaszd ki…` (studio). `data-find-hint` in `studio.ftl` is a formal outlier;
  do not copy it.
- Menu and button labels are noun phrases with `-ás/-és`: `CSV csatolása…`,
  `Exportálás PDF-be…`, `Rekord keresése…`. Keep the trailing `…` when English has it (it
  means "opens a dialog").
- No English-style title case; capitalise only the first word.
- Correct vowel harmony and definite/indefinite conjugation.
- Suffixes attach to placeholders: `{ $record }. rekord nyomtatása`. Use `a(z)` before a
  placeholder whose first sound is unknown.

## Plurals

A number is followed by a singular noun, so plural branches rarely differ:
`data-columns = { $count } oszlop` (Fluent), and both `one` and `other` read `%1$d sablon`
(Android) or `%1$lld fájl` (iOS). Keep the `one`/`other` structure where English has it.

## Glossary

| EN | HU | EN | HU |
|---|---|---|---|
| label | címke | template | sablon |
| printer | nyomtató | paper / roll | papír / tekercs |
| element | elem | record / column | rekord / oszlop |
| barcode | vonalkód | QR / 2D code | QR / 2D kód |
| print | nyomtatás | print history | nyomtatási előzmények |
| serial number | sorszám | attach | csatolás |
| undo / redo | Visszavonás / Mégis | select all | Az összes kijelölése |
| open / save / close | Megnyitás / Mentés / Bezárás | starter / mine | Kezdő / Saját |

Product names, units and codes (QR, EAN, BLE, RFID, PDF) stay as they are.
