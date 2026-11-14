## Menus
menu-file = File
menu-edit = Edit
menu-insert = Insert
menu-arrange = Arrange
menu-label = Label
menu-view = View
menu-print = Print
menu-language = Language

## Commands (cmd-<id> with dots and underscores as dashes)
cmd-doc-new = New label…
cmd-doc-open = Open…
cmd-doc-save = Save
cmd-doc-close = Close
cmd-doc-set-label = Label settings
cmd-doc-check = Check label
cmd-element-add-text = Text
cmd-element-add-barcode = Barcode
cmd-element-add-code2d = QR / 2D code
cmd-element-add-image = Image…
cmd-element-add-rect = Rectangle
cmd-element-add-ellipse = Ellipse
cmd-element-add-line = Line
cmd-element-delete = Delete
cmd-element-duplicate = Duplicate
cmd-element-reorder = Bring to front
cmd-history-undo = Undo
cmd-history-redo = Redo
cmd-render-preview = Print preview
cmd-print-submit = Print…
cmd-device-scan = Find printers
cmd-selection-set = Select all

## Actions
action-select-all = Select all
action-zoom-in = Zoom in
action-zoom-out = Zoom out
action-zoom-fit = Fit to window
action-about = About Labely
action-allow-agent-print = Allow agents to print
action-run-check = Run check
action-scan = Scan
action-status = Status
action-connect = Connect
action-disconnect = Disconnect
action-create = Create
action-cancel = Cancel
action-print = Print
action-close = Close

## Panels
panel-layers = Layers
panel-properties = Properties
panel-check = Check
panel-printers = Printers
panel-preview = Print preview
layer-toggle-visible = Show or hide
layer-toggle-lock = Lock or unlock
layer-front = Bring to front
layer-forward = Bring forward
layer-backward = Send backward
layer-back = Send to back

## Element kinds
kind-text = Text
kind-barcode = Barcode
kind-code2d = 2D code
kind-image = Image
kind-rect = Rectangle
kind-ellipse = Ellipse
kind-line = Line

## Properties
props-multiple = { $count } elements selected
props-frame = Position and size
props-content = Content
props-label = Label
prop-name = Name
prop-x-mm = X (mm)
prop-y-mm = Y (mm)
prop-width-mm = Width (mm)
prop-height-mm = Height (mm)
prop-rotation-deg = Rotation (°)
prop-locked = Locked
prop-hidden = Hidden
prop-corner-radius-mm = Corner radius (mm)
prop-media = Paper type
prop-density = Density
prop-printer-model = Printer model
model-none = (none)

## Check and printers
check-not-run = Not checked yet.
check-clean = No problems found.
printers-none = No printers yet. Scan, or type an address in the print dialog.
print-printer = Printer
print-copies = Copies
print-density-default = 0 = model default

## Status
status-elements = { $count ->
    [one] { $count } element
   *[other] { $count } elements
}
status-saved = Saved.
status-printed = Sent to the printer.
status-no-dialogs = File dialogs are not available here.
mcp-on-hint = Agents can connect with `labely mcp`.
mcp-off = MCP off
mcp-failed = MCP server did not start: { $error }

## Misc
welcome-text = Design and print labels. No account, no cloud.
default-text = Text
about-version = Version
about-text = Free and open source (MIT OR Apache-2.0). Works offline.
