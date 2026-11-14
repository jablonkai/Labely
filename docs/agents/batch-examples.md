# Batch examples

Send each example as the arguments of `command_batch`, or as the `steps` of a
`labely run --script` file. Create the document first, for example with
`document_new {"width_mm": 50, "height_mm": 30, "printer_model": "niimbot.b21_pro"}`.

## Shipping label: text, Code128 and QR

```json
{"label": "shipping label", "steps": [
  {"id": "element.add_text", "params": {"x_mm": 2, "y_mm": 1.5, "width_mm": 46, "height_mm": 7,
    "text": "ACME Parts", "font_size_pt": 14, "bold": true, "align": "center"}},
  {"id": "element.add_barcode", "params": {"x_mm": 3, "y_mm": 10, "width_mm": 28, "height_mm": 14,
    "data": "LBL-0042", "symbology": "code128"}},
  {"id": "element.add_code2d", "params": {"x_mm": 35, "y_mm": 10, "width_mm": 13, "height_mm": 13,
    "data": "https://example.org/p/42"}},
  {"id": "element.add_line", "params": {"x_mm": 2, "y_mm": 9, "width_mm": 46, "height_mm": 0, "stroke_mm": 0.3}}
]}
```

## Using results: style the element you just added

```json
{"steps": [
  {"id": "element.add_text", "params": {"x_mm": 2, "y_mm": 2, "width_mm": 30, "height_mm": 6, "text": "Fragile"}},
  {"id": "element.set_props", "params": {"id": "$0.id", "props": {"bold": true, "invert": true}}},
  {"id": "element.duplicate", "params": {"id": "$0.id", "dx_mm": 0, "dy_mm": 8}},
  {"id": "element.set_props", "params": {"id": "$last.id", "props": {"text": "This side up"}}},
  {"id": "selection.set", "params": {"ids": ["$0.id", "$2.id"]}}
]}
```

## Variables: one layout, many values

```json
{"steps": [
  {"id": "variable.set", "params": {"name": "name", "value": "Ada Lovelace"}},
  {"id": "element.add_text", "params": {"x_mm": 2, "y_mm": 2, "width_mm": 46, "height_mm": 10,
    "text": "Hello, {{name}}", "fit": "shrink"}}
]}
```

To print the label for someone else, run `variable.set` with the new value and print again.

## Data merge: one label per CSV row, with a serial

```json
{"steps": [
  {"id": "data.attach_csv", "params": {"text": "name,sku,qty\nApple,A-1,2\nPear,P-7,1\n"}},
  {"id": "data.set_serial", "params": {"start": 1, "digits": 4, "prefix": "N"}},
  {"id": "element.add_text", "params": {"x_mm": 2, "y_mm": 2, "width_mm": 30, "height_mm": 8,
    "text": "{{name}} · {{serial}}", "fit": "shrink"}},
  {"id": "element.add_barcode", "params": {"x_mm": 2, "y_mm": 12, "width_mm": 40, "height_mm": 12,
    "data": "x"}},
  {"id": "data.bind", "params": {"id": "$3.id", "column": "sku"}}
]}
```

Use `{"path": "items.csv"}` or `data.attach_xlsx {"path": "items.xlsx", "sheet": "Stock"}` for
files. Check a record with `data.preview_record {"record": 2}` or `render.preview
{"record": 2}`, then print after the batch:

- `print.submit {}` prints every record, one label each.
- `print.submit {"records": "2,5-9"}` prints a selection (1-based, in that order).
- `print.submit {"copies_column": "qty"}` takes each record's copies from a column; 0 skips it.
- Without data, `print.submit {"count": 20}` prints 20 labels and the serial advances on each.

The serial is the label's position in the job, so `records: "5-9"` numbers them from `start`.

## Non-atomic: keep going after a failing step

```json
{"atomic": false, "stop_on_error": false, "steps": [
  {"id": "element.move", "params": {"id": 999, "x_mm": 0, "y_mm": 0}},
  {"id": "element.add_text", "params": {"x_mm": 2, "y_mm": 14, "width_mm": 30, "height_mm": 6, "text": "still added"}}
]}
```

Step 0 fails with `not_found`, and step 1 still runs. Each step reports its own
`{ok, result | error}` in `results`. In a non-atomic batch every successful step is its own
undo step.

Data that is invalid for the symbology (for example letters in an EAN-13) is accepted when the
element is added. `check` reports it as `encode_failed`, so run `check` before printing.
