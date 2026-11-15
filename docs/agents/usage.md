# Using Labely from an agent

## The loop

1. **Discover.** Call `command_list`, or `command_list {prefix: "element."}`. Every command
   comes with its params and output JSON Schemas, a description, and its kind (`query`, `edit`,
   `device` or `ui`).
2. **Create.** Call `document_new {width_mm, height_mm, printer_model?}`. Give the printer
   model (for example `niimbot.b21_pro`) so that checks and previews use its dpi and head
   width.
3. **Build.** Send one `command_batch` that adds every element (see
   [batch-examples.md](batch-examples.md)). A batch is atomic: either all steps apply or none
   do, and it is one undo step.
4. **Look.** Call `render_preview`. It returns the exact 1-bit print raster as an `image/png`
   block, plus `width_dots`, `height_dots` and `dpi`.
5. **Check.** Call `check`, which runs `doc.check`. Fix every issue with severity `error`, and
   consider the warnings. Rule ids are listed in [tools.md](tools.md#doccheck-rules).
6. **Print.** Call `print {address, copies?, density?}`. If the printer is not connected yet,
   run `command_run {id: "device.connect", params: {address}}` first. Printing returns
   `forbidden` until the user allows it. For long jobs pass `wait: false` and follow the
   returned `job_id` with `job.get` or `job.wait`.

Units are millimetres, angles are degrees, and font sizes are points. The origin is the top-left
corner of the label. Element ids are stable integers.

## Envelopes

Every tool returns the command envelope in two places: as JSON text and as
`structuredContent`.

```json
{"ok": true, "result": {...}, "changed": [3], "created": [4], "warnings": []}
{"ok": false, "error": {"code": "bad_params", "message": "...", "param": "width_mm",
                        "accepted": ["..."], "hint": "..."}}
```

Error codes are stable:

| Code | Meaning |
|---|---|
| `unknown_command` | No command has this id. `accepted` lists the nearest ids. |
| `bad_params` | The params are invalid. `param` points at the field. |
| `disabled` | The command cannot run now, for example because there is nothing to undo. |
| `no_document` | No document is open. |
| `not_found` | An id or path does not exist. `accepted` lists the valid ones. |
| `device_unavailable` | The printer is off, out of range or not connected. |
| `device_error` | The printer reported a problem. Follow the `hint`. |
| `cancelled` | The call was cancelled. In the studio this happens when the UI did not answer within 30 s. A cancelled print job also reports it. |
| `forbidden` | Blocked by policy: a path outside the allowed roots, or printing while it is off. |
| `internal` | A bug in Labely. The state is unchanged. |

Envelope-level params work on every command:
- `terse: true` drops `result`.
- `coalesce: "<key>"` merges consecutive edits that use the same key into one undo step.

## References inside a batch

A step can use the result of an earlier step:
- `"$0.id"` is the `id` field of step 0's result.
- `"$last.id"` is the same field of the step before.
- `"$$"` gives a literal `$`.

The reference must be the whole string value. It is then replaced by the referenced JSON
value.

## Studio (live) mode

When the studio is running, `labely mcp` forwards to it, and the agent edits the document the
user sees.
- **Waiting.** Calls wait while the user is dragging or has a dialog open.
- **Selection.** `selection.get` returns what the user selected. Use it for requests like
  "make the selected text bold".
- **Undo.** The user can undo agent edits. A batch is one step.

## Journal and scripts

`history.journal` returns the session's successful edits as `[{id, params}]`. That list is a
script: pass it as the `steps` of a `command_batch`, or save it to a file and run
`labely run --script steps.json --save out.labely`. To record a session from the CLI, add
`--journal steps.json` to `labely run`.
