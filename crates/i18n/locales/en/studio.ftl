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
prop-x-mm = X
prop-y-mm = Y
prop-width-mm = Width
prop-height-mm = Height
prop-rotation-deg = Rotation (°)
prop-locked = Locked
prop-hidden = Hidden
prop-flip-h = Flip horizontally
prop-flip-v = Flip vertically
prop-corner-radius-mm = Corner radius
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
print-offset = Offset
print-offset-x = X
print-offset-y = Y
print-mirror = Mirror (transparent or iron-on media)
print-rotate-180 = Turn 180° (other output direction)
menu-align = Align
align-left = Left edges
align-center = Horizontal centres
align-right = Right edges
align-top = Top edges
align-middle = Vertical centres
align-bottom = Bottom edges
distribute-horizontal = Distribute horizontally
distribute-vertical = Distribute vertically
action-snap = Snap to guides
action-show-grid = Show grid
prop-grid-mm = Grid (mm)
menu-label-color = Label colour
color-white = White
color-yellow = Yellow
color-pink = Pink
color-blue = Blue
color-green = Green
color-transparent = Transparent
cmd-element-group = Group
cmd-element-ungroup = Ungroup
cmd-element-lock = Lock / unlock

## Templates and export
cmd-template-new = New from template…
cmd-template-save = Save as template…
cmd-export-png = Export PNG…
cmd-export-pdf = Export PDF…
cmd-data-attach-images = Attach images…
action-save = Save
templates-title = Templates
templates-search = Search
templates-starters = Starter templates
templates-mine = My templates
templates-new-folder = New folder
templates-rename-folder = Rename
templates-delete-folder = Delete folder
templates-empty = No templates here yet. Save a label with File > Save as template.
templates-pick = Pick a template; double-click opens it.
templates-open = Open
templates-duplicate = Duplicate
templates-rename = Rename
templates-move-to = Move to…
templates-delete = Delete
templates-folder = Folder
templates-exists = A template with this name exists.
templates-replace = Replace
status-template-saved = Saved as template.
status-exported = Exported.
template-name-badge = Name badge
template-price-tag = Price tag
template-asset-tag = Asset tag
template-address = Address
template-qr-link = QR link
template-storage-box = Storage box
template-jar = Jar
template-cable-flag = Cable flag
template-wifi = Wi-Fi
template-shipping = Shipping

## Print queue and history
cmd-job-history = Print history…
panel-jobs = Print jobs
jobs-none = No print jobs yet.
jobs-paused = The queue is paused: queued jobs wait.
job-queued = queued
job-printing = printing
job-done = done
job-failed = failed
job-cancelled = cancelled
action-pause = Pause
action-resume = Resume
action-clear-finished = Clear finished
action-clear-history = Clear history
history-title = Print history
history-empty = Nothing printed yet.
history-pages = { $count ->
    [one] { $count } page
   *[other] { $count } pages
}
status-queued = Queued for printing.
status-job-done = Printed: { $name }

## Label paper and printer settings
cmd-paper-list = Label paper…
cmd-paper-detect = Detect loaded paper
cmd-device-calibrate = Printer settings and calibration…
media-gap = Gap labels
media-black-mark = Black mark
media-continuous = Continuous roll
media-transparent = Transparent
papers-title = Label paper
papers-search = Search
papers-fitting = Only paper that fits the printer
papers-none = No matching paper.
papers-use = Use
papers-link = Link
papers-link-button = Link to a paper…
papers-link-hint = Pick the paper with RFID { $barcode }, or add it below.
papers-add = Add your own paper
papers-add-button = Add paper
paper-loaded = Loaded: { $name } ({ $count } left)
paper-mismatch = The label has a different size or type.
paper-unknown = Unknown paper loaded (RFID { $barcode }, { $count } left).
device-title = Printer settings
device-auto-off = Switch off after
auto-off-1 = 15 minutes
auto-off-2 = 30 minutes
auto-off-3 = 45 minutes
auto-off-4 = 60 minutes
device-sound-power = Power beep
device-sound-bluetooth = Bluetooth beep
device-unsupported = not supported
device-calibrate = Calibrate paper
device-calibrate-hint = The printer feeds labels until it finds a label edge. Do this after loading a new kind of paper.
device-calibrate-button = Calibrate
action-printer-settings = Settings…
status-paper-linked = Paper linked to the RFID tag.
status-settings-saved = Printer settings saved.
status-calibrated = Calibration done.
status-pick-printer = Connect a printer first, or pick one in the printers panel.
status-no-rfid = The printer reports no RFID paper.

## Shell (slice 3.6b)
cmd-doc-rotate-label = Rotate label paper
menu-recent = Open recent
action-clear-recent = Clear list
status-recent-missing = { $path } no longer exists; removed from the recent list.
close-title = Unsaved changes
close-unsaved = Save the changes to “{ $name }” before closing?
action-discard = Don’t save
action-copy = Copy
action-cut = Cut
action-paste = Paste
action-show-rulers = Rulers
menu-units = Units
units-mm = Millimetres
units-inch = Inches
