## Menus
menu-file = File
menu-edit = Edit
menu-insert = Insert
menu-arrange = Arrange
menu-label = Label
menu-data = Data
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
cmd-data-attach-csv = Attach CSV…
cmd-data-attach-xlsx = Attach spreadsheet…
cmd-data-detach = Remove data
cmd-data-set-serial = Add serial number
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
panel-data = Data
action-insert-date = Date
action-insert-time = Time
action-insert-serial = Serial number
action-insert-code-from-image = Code from image…
status-no-code-found = No barcode or QR code was found in the image.
props-table = Table
table-row = Row
table-column = Column
table-insert-row = + Row
table-delete-row = − Row
table-insert-column = + Column
table-delete-column = − Column
table-merge-right = Merge right
table-merge-down = Merge down
table-split = Split
table-bold = Bold
cmd-element-add-table = Table
cmd-element-add-icon = Icon
cmd-element-add-border = Border
kind-table = Table
kind-icon = Icon
kind-border = Border
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
prop-flip-h = Flip horizontally
prop-flip-v = Flip vertically
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

## Data
data-none = No data attached. Each row of a CSV file or spreadsheet prints as one label.
data-records = { $count } records
data-record = Record
action-bind = Bind
action-bind-hint = Bind the selected text, barcode or 2D code to this column
data-serials = Serial numbers
serial-start = Start
serial-step = Step
serial-digits = Digits
serial-prefix = Prefix
serial-suffix = Suffix
print-records = Records
print-records-hint = For example 1,3,5-7. Empty prints all records.
print-count = Labels
