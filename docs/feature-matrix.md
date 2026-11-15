# NIIMBOT Desktop 4.2.7 (macOS): feature matrix for Labely

Clean-room inventory, compiled from static inspection of the installed app bundle the user owns
(`/Applications/NIIMBOT.app`, Flutter macOS). All descriptions are paraphrased. No vendor code,
assets, templates or UI copy are reproduced beyond short identifiers. No cloud APIs were called
and the app was not launched.

## Method and evidence keys

| Key | Meaning |
|---|---|
| `[E: i18n <id>]` | String id in `flutter_assets/packages/niimbot_intl/assets/translation/en.json` (6,743 keys). `pc*` ids are desktop-only strings. The `app*` ids are shared with the mobile app, so on their own they do not prove a feature exists on desktop. |
| `[E: src <pkg>/<path>]` | Dart source-file URI recovered from the AOT snapshot (`App.framework/App`, via `strings`). It shows that a module exists. It does not show its behaviour. |
| `[E: sym <name>]` | Identifier or enum token found in the AOT snapshot strings |
| `[E: asset <path>]` | File in `flutter_assets` (paths relative to it) |
| `[E: dev <field>]` | Field in `assets/devices.json` (72 printer models) |
| `[E: fw <name>]` | Native framework or dylib in `Contents/Frameworks` |

**Phase plan** (the Labely phases from the project brief, see `docs/roadmap.md`):
- **Ph1**: core, protocol, render, FakePrinter and CLI, printing on a real B21 Pro. "Ph1 render" means the renderer and CLI support it; editor UI comes in the phase listed after it.
- **Ph2**: full command registry, MCP and the studio editor MVP.
- **Ph3**: editor, templates and batch printing to parity.
- **Ph4**: Android shell. **Ph5**: iOS shell. **Ph6**: more models and vendors, specialty hardware, packaging.
- **later**: Not scheduled.
- **won't**: Not planned. Each such row gives a reason.

"VIP-gated?" means there is static evidence of a membership gate or a non-member limit. "Partial" means a free tier exists with limits. UNCERTAIN means the evidence is inconclusive.

## 1. Screens / routes

| Area | Feature | Details | VIP-gated? | Evidence | Labely plan |
|---|---|---|---|---|---|
| Shell | Tabbed main window | Home tab plus one tab per open label ("board"), with a tab limit and a "+" button | No | [E: src muses_flutter/pages/app_page/tab_bar_add, board_tab_bar_button] [E: i18n pc0369] | Ph2 |
| Shell | Header bar | Printer connection status, print-job status, help menu, account avatar, macOS menu bar items (Minimize/Zoom/Hide/Window) | No | [E: src muses_flutter/pages/header/header_print_status_view, mac_menu_bar] [E: i18n pc0514–pc0519] | Ph2 |
| Home | Start panel | Choose a new blank label, select label paper, open recent, open from my templates, open a local file | No | [E: i18n pc0263–pc0268] [E: src muses_flutter/pages/home/label_create_panel] | Ph2 |
| Home | Label paper library | Browse paper by printer series, search by name/ID/barcode, custom size form, "adaptive label size" | No | [E: src pages/home/label_library_panel, label_search_panel, label_customize_panel, series_select_list] [E: i18n pc0089, pc0092, pc0277] [E: asset assets/series.json] | Ph2 (local paper DB); cloud catalogue won't |
| Home | Recently used | List of recently used labels and templates | No | [E: src pages/home/recently_used_widget] [E: i18n pc0210] | Ph2 |
| Home | Template gallery (industry templates) | Cloud catalogue of industry template categories with search | Some templates are VIP-exclusive | [E: src pages/home/industry_template_page, industry_category_list_widget] [E: i18n app01335, app01593] | won't (vendor cloud content); Labely ships its own starter set in Ph3 |
| Home | My Templates | Folders (default folder, create/rename/delete, move, duplicate), search | Partial: folder count limit, 100 folders for VIP | [E: src pages/home/my_template_widget, my_template_move_widget, my_folder_action_dialog] [E: i18n pc0173–pc0178, pc0214, pc0231, pc0295, app01076] | Ph3 (local folders, no limit) |
| Home | Shared folders | Invite members to a shared cloud folder | Yes | [E: i18n app100001331, app100001337, app100001349] | won't (needs a cloud account) |
| Home | Print history | Content history list with delete/clear. Non-members see 10 records, VIP 100 | Partial | [E: src pages/home/content_history_widget, print_history_set_widget] [E: i18n pc0080–pc0085, pc0293, app100000398] | Ph3 (local, unlimited) |
| Editor | Label editor (board) | Left panel (elements/materials/data), canvas, right attribute panel, top bar | No | [E: src niimbot_flutter_canvas/business/desktop/canvas_box_frame_desktop_left/right/app_bar] | Ph2 |
| Print | Print settings dialog | Preview, device select, copies, offset, flip, Excel range, cut mode | No | [E: src muses_flutter/pages/print/print_config_widget and siblings] | Ph2/Ph3 |
| Device | Device connect modal | Discovery, connect via USB/BLE/Wi-Fi, failure troubleshooting pages, Wi-Fi config, device list | No | [E: src pages/device_connect/connect_type_page, discovering_page, not_found_page, wifi_config_page, usb_connect_fail, bluetooth_connect_fail] | Ph2 (USB/BLE); Wi-Fi later |
| Device | Device settings page | Per-device settings, consumables, battery, firmware, calibration | No | [E: src pages/device_connect/device_setting_page, device_battery, device_consumables] [E: i18n pc0169] | Ph3 |
| Jobs | Print jobs panel | Queue of in-progress and completed jobs, pause/resume/cancel, multi-device (up to 3 C1 over USB) | No | [E: src muses_flutter/pages/device_jobs/*] [E: i18n pc0453–pc0488, pc0537] | Ph3 (single job), later (multi-device) |
| Settings | App settings dialog | Language, about, software update, network diagnostics/proxy, label-paper defaults, system messages | No | [E: src pages/header/sys_setting, change_language, software_upgrade, network_diagnostic_widget, proxy_widget, setting_label_widget, system_message] | Ph6 (minus cloud parts) |
| Account | Login and account | Login, bind phone/email, reset password, delete account (redirects to mobile) | No | [E: src pages/header/login_user_setting, bind_phone_dialog, bind_email_dialog, reset_password_dialog] [E: i18n pc0525–pc0529] | won't (no vendor account) |
| VIP | VIP dialog | Benefit list, SKUs, Alipay QR payment, subscription cancel, redeem codes | n/a | [E: src pages/header/vip_dialog, sku_widget, pay_widget, cancel_subscribe, exchange_member_dialog] [E: i18n pc0280, pc0348–pc0352, pc0416–pc0422] | won't (vendor commerce) |
| C1 | Tube/cable-marker editor | A separate editor for the C1 heat-shrink tube printer with paragraphs, symbols, serials and Excel | C1 buyers get 1 year of VIP free | [E: src niimbot_c1/desktop/c1_desktop_main, niimbot_c1/canvas/*] [E: asset assets/c1TemplateFile.json, c1_consumables.json, c1_symbols_category*.json] [E: i18n pc0489–pc0495] | Ph6 |
| Quick print | Screenshot printing | A floating "ball" window that captures a screen region and prints it to the connected printer. Needs macOS screen-recording permission | No | [E: src muses_flutter/quick_print/*, services/desktop_capture/*] [E: asset assets/images/quick_print/, assets/desktop_capture/] [E: i18n pc0623, pc0626, pc0636–pc0660] | later |
| Help | Help center / support | Tutorials, links to the vendor's user group, customer-service IM, how-to guides | No | [E: src pages/header/help_center, help_menu, user_im_widget] [E: i18n pc0204, pc0250, pc0446–pc0450] | won't (vendor support); Labely links its own docs |
| Recovery | App repair/restart prompts | Detects a hung or failed start and offers a repair restart | No | [E: i18n pc0500–pc0504] [E: src controllers/application_shutdown_coordinator] | later |

## 2. Editor: canvas and manipulation

| Area | Feature | Details | VIP-gated? | Evidence | Labely plan |
|---|---|---|---|---|---|
| Canvas | Label canvas with printable-area overlay | Shows the paper background (optional image of the paper), printable area and blind-zone lines | No | [E: src niimbot_flutter_canvas/widgets/canvas/print_area_widget, area_line_painter, background] [E: dev consumables[].childProperties[].blindZone] | Ph2 |
| Canvas | Rulers | Desktop rulers along the edges, in mm | No | [E: src widgets/box_frame/desktop_canvas_ruler, desktop_ruler_painter] | Ph2 |
| Canvas | Gridlines | Toggleable grid | No | [E: i18n app100002619] | Ph2 |
| Canvas | Snapping / auxiliary lines | Alignment guide lines while dragging | No | [E: src widgets/canvas/auxiliary_lines_widget] [E: sym snap] | Ph2 |
| Canvas | Zoom | Zoom in/out buttons, pinch/trackpad, expand/shrink canvas, "zoom-out mode" | No | [E: asset packages/niimbot_ui/assets/icons/zoomIn.svg, zoomOut.svg; niimbot_flutter_canvas/assets/canvas_expand.svg] [E: src widgets/components/niimbot_interactive_viewer] [E: fw niimbot_macos_trackpad_detector] [E: i18n app100001222] | Ph2 |
| Canvas | Canvas background colour | User-selectable editor background colour | No | [E: i18n app100002579–app100002580] [E: asset assets/config.json normalCanvasBackgroundColor] | Ph3 |
| Canvas | Rotate label paper | Rotates the whole label orientation, optionally rotating content with it | No | [E: i18n pc0009, pc0387, pc0388] [E: src business/desktop/rotate_paper_widget] | Ph2 |
| Canvas | Output (print) direction | Sets print direction relative to the feed | No | [E: src business/desktop/output_direction] [E: dev printDirection] | Ph3 |
| Canvas | Global/label settings panel | Label size, paper, material and colour when nothing is selected | No | [E: i18n pc0045–pc0048, pc0073] [E: src business/desktop/canvas_global_settings_widget] | Ph2 |
| Select | Single and multi-select, rubber-band selection, select all | Marquee rectangle, and a multi-element attribute panel | No | [E: src widgets/canvas/multi_select_rect, selection_area; multiple_element_attr_widget] [E: i18n app00506] | Ph2 |
| Align | Align and distribute | Left/centre/right, top/middle/bottom, distribute horizontally/vertically ("isometric") | No | [E: i18n pc0010–pc0017] [E: src business/desktop/canvas_desktop_align_widget] | Ph2 |
| Arrange | Z-order | Bring to top / send to bottom only. No step forward/back was found | No | [E: i18n pc0056, pc0057] | Ph2 |
| Arrange | Lock / unlock elements | Locked elements cannot be selected | No | [E: i18n app00362, app01063, app100001243] [E: sym isLock] [E: asset niimbot_flutter_canvas/assets/common/icon_element_lock.svg] | Ph2 |
| Arrange | Group / ungroup | No grouping feature was found. Only multi-select exists | n/a | [E: sym — no group/ungroup tokens found] UNCERTAIN | Ph3 (Labely adds grouping) |
| Transform | Rotate | Rotate element (the icons suggest 90° steps), plus rotation via the attribute field | No | [E: asset niimbot_ui/icons/rotate.svg, rotateLeft.svg, rotateRight.svg] [E: sym rotate] | Ph2 |
| Transform | Flip / mirror | Horizontal and vertical flip of elements. Also mirrored copies (a mirror element linked to the original) | No | [E: i18n pc0594–pc0596] [E: src business/desktop/canvas_desktop_flip_widget, widgets/canvas/element_mirror_item] [E: sym mirrorId] | Ph3 |
| Transform | Resize handles, keyboard nudge | Corner/side drag points | No | [E: src widgets/elements/resize_point, draggable_point] | Ph2 |
| Clipboard | Copy / paste / duplicate / delete | Context menu on elements, plus system clipboard (rich paste) | No | [E: i18n app00361, pc0055, app100001475, app00063] [E: src widgets/elements/element_context_menu_widget] [E: fw super_native_extensions] | Ph2 |
| History | Undo / redo | A history store exists. Depth not determined | No | [E: src provider/history/history_store] [E: asset niimbot_ui/icons/undo.svg, redo.svg] | Ph2 (Labely: ≥100 steps) |
| Keyboard | Canvas shortcuts | A dedicated shortcuts module exists. The key map was not recovered | No | [E: src widgets/canvas/canvas_core_shortcuts] UNCERTAIN | Ph3 |
| Drag&drop | Drag elements from panel; drag data fields onto elements | Dropping a data column onto an element binds it | No | [E: src business/desktop/canvas_drag_element_widget, muses_flutter/widgets/global_drag_widget] [E: i18n pc0197, pc0242] | Ph3 |
| Multi-label | Multi-background / multi-page template | Selector for templates with several backgrounds or pages | No | [E: src widgets/canvas/desktop_multi_background_selector, batch_widget] | later |
| Template | Save / Save as / rename | Saving requires login for "My Templates". Local "save to" folder is also available | Login required for cloud save | [E: i18n app00039, pc0202, pc0498, pc0499] [E: sym TEMPLATESAVEAS] | Ph2 (local file) |

## 3. Element types and properties

| Area | Feature | Details | VIP-gated? | Evidence | Labely plan |
|---|---|---|---|---|---|
| Text | Plain/rich text box | In-place editor built on a Quill-derived rich-text engine. Font, size, size +/- buttons | No | [E: src niimbot_flutter_quill/*, niimbot_flutter_canvas/widgets/elements/text_in_place_editor] [E: asset niimbot_ui/icons/textFontSizeAdd.svg] | Ph1 render · Ph2 editor |
| Text | Style | Bold, italic, underline. Strikethrough not found | No | [E: asset niimbot_ui/icons/bold.svg, italic.svg, underline.svg] [E: sym fontStyle] | Ph1 render · Ph2 editor |
| Text | Alignment | Horizontal left/centre/right/justify. Vertical top/centre/bottom within the box | No | [E: asset niimbot_ui/icons/textLeftAlign.svg, textJustifyAlign.svg, textBoxTopAlign.svg…] [E: src text_align_widget, text_box_align_widget] | Ph1 render · Ph2 editor |
| Text | Spacing | Letter spacing and line spacing | No | [E: i18n app01007, app01010] [E: sym letterSpacing, lineSpacing] | Ph1 render · Ph2 editor |
| Text | Direction | Horizontal, vertical (CJK-style), horizontal rotated 90°, curved/arc text | No | [E: asset niimbot_flutter_canvas/assets/common/text_direction/{horizontal,vertical,horizontal_90,curved}*.svg; niimbot_ui/icons/curvedText.svg] [E: i18n app100000761] [E: sym textDirection, typesettingMode] | Ph1 render · Ph3 editor (vertical, 90°), Ph6 (curved) |
| Text | Fit modes | Fixed size, auto font size to fit the box, shrink-to-fit, word wrap | No | [E: i18n app100001138, app100001173, app100001224, app100001226, app100001229, app01591] [E: asset niimbot_flutter_canvas/assets/lottie/text_style_adaptive.json] | Ph1 render · Ph2 editor |
| Text | Colour and reverse | Text colour (red/black on two-colour paper) and a reverse (white-on-black) effect | No | [E: i18n app100000430, app100002029, pc0021] [E: src color_reverse_style_attr_widget] [E: sym colorReverse] | Ph1 render · Ph3 editor (reverse), Ph6 (colour) |
| Text | Max length guard | Warns when text exceeds a length in mm | No | [E: i18n pc0402] | Ph1 render · Ph3 editor |
| Fonts | Font library | Bundled fonts plus a downloadable cloud font list. Missing fonts fall back to the default | 500+ "beautiful fonts" are VIP | [E: asset assets/fonts/, packages/niimbot_ui/assets/fonts/Harmony/*, packages/netal_plugin/assets/fontConfig.json] [E: src muses_flutter/apis/rest/font_rest_api] [E: i18n pc0290, app01217–app01248, app01505] | Ph1 render · Ph2 editor (system fonts); cloud fonts won't |
| Fonts | Upload custom font | Upload your own font file, with a size and count limit | Yes (VIP benefit "upload font") | [E: i18n pc0431–pc0441] [E: asset assets/images/vip/rights-upload-font.svg] [E: src widgets/font_upload_progress_dialog] | Ph1 render · Ph2 editor (local system/TTF loading, free) |
| Barcode | 1D barcode element | Content, symbology, human-readable text above/below/none, text height | No | [E: src barcode_attr_widget, barcode_content_widget, netal_plugin/models/netal_barcode_element] [E: asset niimbot_ui/icons/barcodeNumberAbove.svg, barcodeNumberBelow.svg] [E: sym textPosition, textHeight] | Ph1 render · Ph2 editor |
| Barcode | Symbologies | CODE128 (auto/A/B/C), CODE39, CODE93, EAN-13, EAN-8, UPC-A, UPC-E, ITF-25, Codabar, GS1-128 | No | [E: sym CODE128A/B/C, CODE39, CODE93, EAN13, EAN8, UPC_A, UPC_E, ITF25, CODEBAR, GS1_128] (some tokens may come from the scanner library) | Ph1 render · Ph2 editor (128/39/EAN/UPC), Ph1 render · Ph3 editor (rest) |
| 2D | QR code | Content, error-correction ("fault tolerance") level, style | No | [E: i18n app00004, pc0032, pc0042, pc0051] [E: src qrcode_attr_widget, netal_qrcode_element] [E: sym correctLevel] | Ph1 render · Ph2 editor |
| 2D | PDF417, Data Matrix | Other 2D symbologies | No | [E: sym PDF417, DATA_MATRIX] | Ph1 render · Ph3 editor |
| 2D | Decode QR from image | Upload a QR image (≤5 MB). The app decodes it and regenerates the code | No | [E: i18n pc0022–pc0025, pc0030, pc0031] [E: src upload_qrcode_widget] [E: fw niimbot_qbarcode] | Ph3 |
| 2D | Advanced / dynamic QR | Cloud-hosted content behind a QR (files, forms, editable later). Non-members get 3 | Partial (3 free, 100 VIP) | [E: i18n app01587, app01588, app100000209, app100000217] [E: src qrcode_live_content_widget] [E: sym dynamicQRCodeLimit, liveCodeId] | won't (cloud-hosted) |
| Image | Image element | Insert local image (≤5 MB, ≤5000 px). HEIC supported. Crop, rotate, replace | No | [E: i18n pc0075, pc0076, pc0396–pc0398] [E: src image_clipping_widget, canvas_upload_heic_handle_utils, image_replace_action] [E: fw file_picker, libniimbot_canvas_image] | Ph1 render · Ph3 editor |
| Image | Monochrome processing | Processing type with an adjustable value: threshold, a dithering kernel, and grayscale on grayscale paper | No | [E: i18n app01019, app100001756] [E: sym imageProcessingType, imageProcessingValue, DitherKernel] [E: fw libopencv_imgproc] | Ph1 render · Ph3 editor |
| Image | Bulk image import / batch printing | Up to 50 images at a time, each printed as its own label | Yes | [E: i18n pc0400, pc0401, pc0411, pc0414, pc0423] [E: src utils/canvas_crop_batch_image_utils] [E: asset assets/images/vip/rights-multi-photo.svg] | Ph3 (free) |
| Image | Screenshot as image | Capture a screen region into the canvas | No | [E: src business/desktop/desktop_screenshot] [E: i18n pc0623, pc0625] | later |
| Shapes | Graph element | Rectangle, rounded rectangle, circle, oval. Line width, fill/outline | No | [E: asset niimbot_ui/icons/rectangle.svg, roundedRectangle.svg, circle.svg, oval.svg, shapes.svg] [E: src graph_attr_widget, netal_graph_element] [E: sym graphType, cornerRadius] | Ph1 render · Ph3 editor |
| Lines | Line element | Solid, dashed and dotted styles, with width and colour | No | [E: asset niimbot_ui/icons/line.svg, lineDashed.svg, lineDotted.svg] [E: i18n app00154, app01021, app100000431] [E: sym lineType, lineWidth] | Ph1 render · Ph3 editor |
| Table | Table element | Insert/delete rows and columns, merge/split cells, row height/column width, per-cell style, clear contents, bind cells to data | No | [E: i18n pc0058–pc0069, pc0077, pc0395, app01016–app01018] [E: src widgets/elements/interactive_table_element, table_resize_line, netal_table_combine_cell_element] | Ph3 |
| Date/time | Date element | Date and time formats, current time, offset (earlier/later), a "related time" linked to another element, realtime refresh at print | Real-time labels are VIP | [E: i18n pc0026–pc0029, pc0071, pc0072, pc0270–pc0276, pc0294, pc0053] [E: src date_attr_widget, date_time_refresh] [E: sym dateFormat, timeFormat, timeOffset, associateId] [E: asset assets/images/vip/rights-time.svg] | Ph1 render · Ph3 editor (static and offset), Ph3 (refresh per print) |
| Serial | Serial-number element | Start value, increment, prefix/suffix, fixed length (zero-pad), type, priority. Count follows the Excel rows when data is bound | No | [E: i18n app00008, app00124, app00127, app01069, app01072, app100001492, app100001698, pc0033–pc0037] [E: src serial_attr_widget, serial_content_widget] [E: sym startNumber, incrementValue, fixLength] | Ph3 |
| Icons | Icon / sticker library ("material") | Categorised cloud icon library. Upload your own icons | Yes: 1700+ icons, icon upload are VIP | [E: i18n pc0008, pc0041, pc0070, pc0291, pc0427–pc0430, pc0442] [E: src muses_flutter/pages/board/material_list_widget] [E: sym materialLibVips] [E: asset assets/images/vip/rights-stickers.svg, rights-upload-icon.svg] | Ph3 (bundled open-licence icons, own SVG import); vendor library won't |
| Borders | Border/frame library | Nine-patch frames that stretch around the content | Yes: 180+ frames VIP | [E: i18n app100001204, pc0261, pc0292] [E: src widgets/elements/nine_patch/*, business/desktop/border_choose_widget] [E: sym materialBordersLibVips] [E: asset assets/images/vip/rights-border.svg] | Ph3 (own frames) |
| Input | Scan / OCR input | The desktop has a camera scanner (mobile_scanner) for barcodes/QR. OCR appears in the shared strings with a daily non-member limit | OCR: Partial (daily quota) | [E: fw mobile_scanner] [E: asset niimbot_flutter_canvas/assets/common/camera.svg] [E: i18n app100000187–app100000202, app100000201] UNCERTAIN whether OCR is exposed on desktop | later (scan), won't (cloud OCR) |
| Data | Product library / commodity element | Fields pulled from a product database (name, price, barcode) | UNCERTAIN | [E: asset niimbot_ui/icons/productLibrary.svg, productInventory.svg] [E: i18n pc0508] | won't (cloud product DB); covered by Excel binding |
| Colour | Two-colour (red/black) printing | Per-element red or black on dual-colour paper and two-colour printers | No | [E: i18n pc0018, pc0512] [E: src muses_flutter/pages/print/red_black_print_preview_widget, stores/printer/printer_canvas_black_red] | Ph6 |

## 4. Data import and batch

| Area | Feature | Details | VIP-gated? | Evidence | Labely plan |
|---|---|---|---|---|---|
| Excel | Import local Excel | xlsx (native Rust "calamine" reader). Sheet selection, header row, size/row limits (mobile string says 2000 rows) | "Excel printing" listed as VIP benefit; non-members get trial attempts | [E: fw niimbot_excel, libniimbot_calamine_excel] [E: i18n pc0189, pc0279, pc0296, app01123, app100000781, app100001287–app100001289, excelImport*] [E: src business/desktop/import_excel, niimbot_import_excel_widget] | Ph3 (free) |
| CSV | CSV import | A `text/csv` MIME token is present. No CSV import UI was found | — | [E: sym text/csv] UNCERTAIN | Ph3 |
| Binding | Column binding | Drag a field onto an element, or generate a new text/barcode/QR element from a column. Unbind one field or the whole source | No | [E: i18n pc0191–pc0201, pc0242, pc0278] [E: src data_bind_generate_widget, canvas_bind_element_change_utils] | Ph3 |
| Binding | Data source panel | Manage data sources, preview rows, edit data, history of sources, linked templates ("this edit syncs to N templates") | No | [E: src canvas_data_source_panel/*, muses_flutter/pages/data_sources_setting/*] [E: i18n pc0565–pc0590, pc0544, pc0545] | Ph3 |
| Batch | Row range selection | Print selected rows, or a range like "1,3,5–7". Single or multi page | No | [E: i18n pc0384–pc0386, pc0412, pc0413, pc0572] [E: src muses_flutter/utils/data_source_range_utils] | Ph3 |
| Batch | Copies per record | The same copies for every row, a copies count taken from a column ("link copies"), or sets | No | [E: i18n pc0482–pc0484, app100001937, app100001938, app100002507] | Ph3 |
| External | Third-party data source (Digiwin/"Dingxin" ERP) | OAuth-style authorisation, then refresh data from an external system | UNCERTAIN | [E: src pages/data_sources_setting/dingxin_*] [E: i18n pc0564, pc0566, pc0576–pc0587] | won't (vendor-partner cloud) |
| External | Import from messaging apps | Excel shared via WeChat/QQ/DingTalk (mobile flows) | — | [E: i18n excelImportFromWechat, excelImportFromQQ, excelImportFromDingding] | won't |

## 5. Import / export formats

| Area | Feature | Details | VIP-gated? | Evidence | Labely plan |
|---|---|---|---|---|---|
| Template | Native template format | Templates are JSON-based models (`TemplateData`), stored locally in Isar and in the cloud. Local open/save exists. The file extension was not confirmed | No | [E: src niimbot_template/models/template_data, template_parse, template_generate] [E: i18n pc0263, pc0268, app01243] [E: fw isar_flutter_libs] UNCERTAIN extension | Ph3 (read-only importer for interop, Labely uses its own open format) |
| Template | Share template by code | Share a cloud template via a password/code. Recognises codes shared by others | Login | [E: i18n app00335, app00398, app01196, pc0390, pc0391] [E: sym shareCode] | won't (cloud) |
| PDF | PDF import / PDF print | Pick a PDF (≤10 MB). It is split into per-page images and can be cropped. Desktop saves only page 1 into a template | No | [E: fw pdfx] [E: i18n pc0399, pc0620, pc0621, app100001848–app100001872] [E: src business/desktop/canvas_pdf_calculator, taiwan_pdf_layout_detector] | Ph3 |
| Image | Image import | PNG/JPEG/HEIC, SVG for UI | No | [E: src canvas_upload_image_utils, canvas_upload_heic_handle_utils] | Ph3 |
| Export | Export to PDF/PNG | No export-to-file action was found. Output is print-only | — | [E: sym — no export* action] UNCERTAIN | Ph3 (Labely adds PNG/PDF export) |
| Font | Font files | TTF/OTF upload | VIP | [E: sym .ttf, .otf] | Ph3 (local) |
| C1 | Tube template set | A bundled C1 template catalogue | No | [E: asset assets/c1TemplateFile.json] | Ph3 (own templates) |

## 6. Printing settings

| Area | Feature | Details | VIP-gated? | Evidence | Labely plan |
|---|---|---|---|---|---|
| Print | Copies | 1–999 integer validation | No | [E: i18n app01103, pc0540, pc0542] [E: src pages/print/print_config_copies] | Ph1 |
| Print | Density | A per-model density range (min/default/max, e.g. 1–5 default 3) | No | [E: i18n app00488, pc0133] [E: dev solubilitySetStart/Default/End] | Ph1 |
| Print | Speed / quality mode | "Optimize for speed" option and print mode | No | [E: i18n app100001172, app100001170] [E: sym printSpeed, printMode] | Ph1 |
| Print | Print mode (thermal vs thermal-transfer) | Set by the material and the printer (ribbon printers) | No | [E: dev consumables[].parentProperty.printModeValue] [E: i18n pc0113–pc0116] | Ph1 |
| Print | Label/paper type | Gap, hole, continuous, black mark, black-mark gap, transparent, tag, tube. The material list has about 38 materials | No | [E: dev paperType, consumables] [E: i18n app00280–app00283, app00818, app100000195, app100000744] | Ph1 (gap/continuous/black mark) |
| Print | Offsets | Horizontal and vertical offset in mm. Paper-feed offset | No | [E: i18n app00024, app00025, app100001510, app100001511, pc0629, app100002663] [E: src print_config_offset] | Ph1 |
| Print | Mirror / flip print | Mirror the print content (for transparent/iron-on media) | No | [E: i18n pc0597, app01424] [E: src print_config_flip_print] | Ph1 |
| Print | Print range | Single page, multi-page, custom page list | No | [E: i18n pc0384–pc0386, pc0412, pc0413] | Ph3 |
| Print | Cutter options | Cut after printing, full/half cut, half-cut depth, cut position/offset (cutter models) | No | [E: src pages/print/print_cut_mode_select] [E: i18n app100002339–app100002351, app100002621, app100002234] [E: dev cutterType] | Ph6 |
| Print | Tube print config | Tube length, tail direction/length, tube type (C1) | No | [E: src pages/print/tube_print_config_widget, template_tube_type] [E: i18n app00231, app100000601] | Ph6 |
| Print | Print preview | Rendered preview, red/black preview, empty-template preview | No | [E: src pages/print/general_template_preview, red_black_print_preview_widget] | Ph1 |
| Print | Paper mismatch handling | Warns when the loaded paper differs from the template, with "default to detected paper" | No | [E: i18n pc0497, pc0534–pc0536, pc0592, pc0593, app100002244] | Ph1 |
| Print | Error reporting | Detailed error states: cover open, out of paper, overheating, jam, ribbon missing/wrong/depleted, RFID write fail, low battery | No | [E: i18n pc0097–pc0146, pc0462–pc0472] | Ph1 (core), Ph1 (rest) |
| Print | Consumable authenticity / risk checks | Strategy checks on RFID-chip consumables. Some prompts ask for genuine paper/ribbon | No | [E: src niimbot_print_strategy/*, muses_flutter/utils/risk_shiled/*] [E: i18n pc0258, pc0241] | won't (Labely does not enforce vendor consumables; not investigated further) |
| Print | Offline print | Printing without a network | No | [E: src pages/device_connect/offline_print_widget, offline_print_utils] | Ph1 (Labely is offline-first) |
| Print | Reverse/inverted label | Reverse-colour element style | No | [E: i18n app100000420–app100000426] | Ph1 |

## 7. Device features

| Area | Feature | Details | VIP-gated? | Evidence | Labely plan |
|---|---|---|---|---|---|
| Connect | USB connection | Uses libusb and the vendor SDK. Driver install prompts. Up to 3 devices on USB | No | [E: fw libusb-1.0.0.dylib, libjcPrinterSdk.dylib] [E: i18n pc0300, pc0309, pc0319, pc0537] | Ph1 |
| Connect | Bluetooth (BLE) | Supported per model. Some models need macOS 13+ | No | [E: i18n pc0310, pc0321–pc0328, pc0506] [E: asset assets/config.json macOS Below13 Bluetooth Unsupported Devices] | Ph1 |
| Connect | Wi-Fi / network | Wi-Fi printing (K3_W, MP3K_W, B4 Pro) and Wi-Fi provisioning over BLE. One string says Wi-Fi is not supported on Mac | No | [E: i18n pc0164–pc0172, pc0301, pc0311, pc0340–pc0347, pc0520, pc0632] [E: dev isSupportWifi, isSupport5GWifi, isSupportMqtt] | later |
| Connect | Model support list | 72 models / 28 series, 203 or 300 dpi, max print width per model. Desktop support depends on the app version | No | [E: asset assets/devices.json, assets/series.json, assets/config.json versionSupportDeviceMac] [E: dev paccuracyName, widthSetEnd, maxPrintWidth] | Ph1 (B1/B21/D11/D110 first), Ph1 (more) |
| Status | Battery, label remaining | Battery level, approximate labels or metres remaining | No | [E: src pages/device_connect/device_battery] [E: i18n pc0630, pc0631] | Ph1 |
| RFID | RFID label detection | Reads the paper's RFID chip to auto-select size/type. Per-model RFID type | No | [E: src stores/printer/printer_rfid_reader, niimbot_print_strategy/src/model/chip_rfid_info] [E: dev rfidType] [E: i18n pc0087, pc0551] | Ph1 (local read; size lookup from Labely's own table) |
| RFID | RFID writing | Write data to RFID labels (RFID printers), with Excel column association | No | [E: i18n pc0131, pc0140, pc0141, app100001189, app100001191] | later |
| Calibrate | Paper calibration | Calibrate the gap/black-mark sensor | No | [E: i18n app00900, app00931, pc0521, pc0522] [E: dev isSupportCalibration] | Ph1 |
| Power | Auto shut-off | A device setting for the auto shut-down time level | No | [E: i18n app00920] [E: sym autoShutDownTimeLevel, autoShutDownTimeChanged] | Ph1 |
| Firmware | Firmware update | Checks for and downloads vendor firmware, transfers it to the device. Wi-Fi-module upgrades are not supported on Mac | No | [E: src stores/printer/printer_firmware_service, pages/device_connect/upgrade_firmware_dialog_content] [E: i18n pc0218–pc0222, pc0552–pc0556, pc0563, pc0633] | won't (needs vendor firmware from cloud; risk of bricking) |
| Reset | Factory reset | Restores the device to factory settings | No | [E: i18n pc0406, pc0407, pc0419] | later |
| Tube | Tube jam handling (C1) | Guided recovery from a jammed sleeve or tube | No | [E: src pages/print/print_error_tube_stuck, device_connect/tube_adjust_dialog_content] [E: i18n pc0420, pc0454] | Ph6 |
| Logs | Firmware fault log upload | Uploads device fault logs to the vendor | No | [E: src stores/printer/firmware_fault_log_reporter] | won't (telemetry) |
| Other | Electronic price tags / cable markers | Specific device classes are excluded from some features | No | [E: i18n pc0645, pc0646] [E: dev name ET10, JcTag] | won't (niche/proprietary) |

## 8. App settings, languages, units

| Area | Feature | Details | VIP-gated? | Evidence | Labely plan |
|---|---|---|---|---|---|
| Settings | Language | Follow system or pick one. About 30 UI languages: af, ar, bg, cs, de, el, en, es, fil, fr, hi, id, it, ja, km, ko, ms, nl, pl, pt, pt-BR, ru, sv, sw, th, tr, uk, vi, zh-Hans, zh-Hant. RTL Arabic icons exist | No | [E: asset packages/niimbot_intl/assets/translation/*.json] [E: i18n pc0078, pc0228, app100002545] [E: asset niimbot_ui/icons/*Arabic.svg] | Ph3 |
| Settings | Units | Only millimetres were found. No inch support | No | [E: i18n app00277, app01055] [E: sym — 0 "inch" tokens] | Ph2 (mm), Ph3 (add inch) |
| Settings | Label paper defaults | Use the detected paper by default, and whether to prompt on mismatch | No | [E: i18n pc0592, pc0593] [E: src pages/header/setting_label_widget] | Ph3 |
| Settings | Startup page toggle | Show a startup/boot page per device | No | [E: i18n app100001688] [E: dev displayBootPage] | later |
| Settings | Software update | Online version check per platform | No | [E: asset assets/config.json currentOnlineMacOSVersion] [E: src pages/header/software_upgrade] | later (Labely's own updater) |
| Settings | Network diagnostics | DNS, internet, proxy and vendor-service reachability checks | No | [E: i18n pc0598–pc0619] [E: src muses_flutter/network/network_probe_service] | won't (vendor-service specific) |
| Settings | Telemetry | Sentry crash reporting, analytics events ("buried" points) | No | [E: fw Sentry.framework, sentry_flutter.framework] [E: src muses_flutter/utils/buried_utils] | won't (Labely: opt-in only, if any) |
| Settings | System messages | In-app notifications from the vendor | No | [E: src pages/header/system_message] | won't |
| Settings | Local persistence | Isar DB for templates, folders, devices, data sources, print records. shared_preferences for flags | No | [E: src muses_flutter/db/models/*] [E: fw isar_flutter_libs] [local: `~/Library/Preferences/com.niimbot.print.plist` holds only a few flags such as first-run and the guide acknowledgement] | Ph2 (Labely's own storage) |

## 9. Account, cloud, VIP, commerce

| Area | Feature | Details | VIP-gated? | Evidence | Labely plan |
|---|---|---|---|---|---|
| Account | Sign-in methods | Phone, email, Apple, Google, Facebook, Twitter/X, WeChat, QQ, LINE | n/a | [E: fw GoogleSignIn, sign_in_with_apple, twitter_login] [E: asset niimbot_ui/icons/apple.svg, google.svg, faceBook.svg, weChat.svg, qq.svg, line-chat.svg] [E: src niim_login/*] | won't (vendor account) |
| Cloud | Cloud templates and sync | My Templates synced to the account. Template-data sync service | Login | [E: src muses_flutter/apis/rest/template_rest_api, cloud_file_rest_api] | won't (Labely: local files, git/cloud-drive friendly) |
| Cloud | Cloud label-paper catalogue | Paper specs from the cloud, a cached series list | No | [E: src apis/rest/material_rest_api, stores/printer/cloud_consumable_config_service] [E: asset assets/local_cache/series/] | won't (Labely ships an open paper table) |
| VIP | Membership | Monthly/yearly subscription, Alipay payment, redeem codes (7 days to 12 months), C1 free year | n/a | [E: i18n app01496–app01562, app100000120–app100000157, pc0280–pc0289, pc0489–pc0495] | won't |
| Shop | Consumables store, coupons | Links to the vendor shop, member coupons, guide prices | VIP coupons | [E: i18n pc0282, pc0283, pc0286, pc0287] [E: sym ashop host] [E: src apis/models/guide_price_model, c1_consumable_model] | won't (commerce) |
| Meetings | Meeting/nameplate sign-in ("orca") | Meetings, attendee import, sign-in QR, printing name tags. Probably a mobile/mini-app flow | UNCERTAIN | [E: i18n orca* keys] | won't (cloud service) |

## Phase 3 exit status

The Phase 3 exit audit (slice 3.7, ADR-0024) checked every Ph1–Ph3 row against the code. All
other Ph1–Ph3 rows are done. The rows below are done only in part; each remaining part has a
reason and a new target. Device rows were checked on a B21 Pro (firmware 3.12).

| Section | Row | Done | Not done: reason → target |
|---|---|---|---|
| 1 | Header bar | Printer, battery and active jobs in the status bar | Help menu has About only; docs links come with packaging → Ph6 |
| 1 | Print settings dialog | Preview as printed (mirror, 180°), copies column | Cut mode needs a cutter model → Ph6 |
| 1 | Device connect modal | Panel with troubleshooting tips and retry | Wi-Fi → later. A separate modal is not needed: the panel stays open while printing |
| 1 | Device settings page | Battery, labels left, firmware, hardware, serial | Firmware update is vendor-only and risky → won't |
| 2 | Canvas background colour | Six presets, saved | Free colour pick: the presets match label stock; no use case yet → later |
| 2 | Single and multi-select | Rubber-band selection (Shift adds), select all | Editing shared properties of several elements: align, distribute and group cover the common cases → later |
| 3 | Text alignment | Left, centre, right, justify | — |
| 3 | Fonts / custom font | Installed fonts in the studio; `font.add` for TTF/OTF/TTC files | Cloud fonts → won't |
| 3 | QR code | Data, error correction | Styled QR (dot shapes, logo) hurts scan reliability on 203/300 dpi heads → later |
| 3 | Image element | Insert, crop, rotate, fit, dither | HEIC needs libheif (LGPL, C) → later. Replacing the picture in place (today: delete and insert again) → later |
| 3 | Date/time | Static, offset, refresh per print | Format and offset pickers (bindings are typed by hand) and time linked to another element → later |
| 3 | Serial | Start, step, digits, prefix, suffix | Serial "type" and "priority" semantics were not recovered (UNCERTAIN) → later |
| 3 | Icons | Bundled set, picker | Own SVG icons (ADR-0016: untrusted SVG input needs a sanitising pass) → later |
| 3 | Borders | Eight built-in styles | User frames (nine-patch images), same reason as icons → later |
| 4 | Data source panel | Preview, records, bind, sheet and header row | Editing data in the app, source history, linked templates: Labely reads the file, the spreadsheet app edits it → won't |
| 4 | Copies per record | Fixed copies or a copies column (studio, CLI, MCP) | "Sets" (collated copies) → later |
| 5 | Native template format | — | Read-only importer needs sample files (ADR-0018) → later |
| 5 | PDF import | — | No permissive pure-Rust PDF rasteriser (ADR-0018) → later |
| 5 | Image import | PNG, JPEG, BMP, WebP | HEIC (see Image element) and SVG (see Icons) → later |
| 5 | C1 tube template set | — | Needs the C1 model and heat-shrink media (ADR-0018) → Ph6 |
| 6 | Speed / quality mode | — | The B21 Pro answers PrinterInfo 2 with "not supported" → Ph6, with models that answer |
| 6 | Print mode | — | Only thermal-transfer models use it; the B21 Pro answers 14 with "not supported" → Ph6 |
| 6 | Print range | Record ranges | Choosing pages of a multi-page document; every page prints → later |
| 8 | Language | English and Hungarian, saved | More languages need translators; the i18n check keeps keys in step → later |
| 8 | Label paper defaults | RFID detection and mismatch banner | Saved "always use detected paper" and "prompt on mismatch" settings → later |

## Summary

**Row counts by section**

| Section | Rows |
|---|---|
| 1. Screens/routes | 21 |
| 2. Editor canvas | 23 |
| 3. Elements | 30 |
| 4. Data import/batch | 8 |
| 5. Import/export | 7 |
| 6. Printing | 16 |
| 7. Device | 14 |
| 8. Settings | 9 |
| 9. Account/cloud/VIP | 6 |
| **Total** | **134** |

**Rows by plan.** Each row is counted under the first phase in its plan cell. Some rows split across phases.

| Plan | Rows |
|---|---|
| Ph1 | 38 |
| Ph2 | 26 |
| Ph3 | 31 |
| Ph6 | 6 |
| later | 10 |
| won't | 23 |

**VIP-gated features (fully gated or limited for free users)**
1. 500+ premium cloud fonts and exclusive fonts
2. Custom font upload
3. 1700+ icon/sticker library and custom icon upload
4. 180+ border/frame library
5. Real-time (print-time refreshed) date/time labels
6. Excel printing/batch printing (free users get a limited number of trials)
7. Bulk image import and printing (up to 50 images)
8. Print history: 10 records for free users, 100 for VIP
9. Folders: a small limit for free users, 100 for VIP. Shared folders are VIP only
10. Advanced/dynamic QR codes: 3 for free users, 100 for VIP. Free uploads are limited to 5 MB, VIP to 100 MB
11. Exclusive templates in the cloud gallery
12. OCR daily quota. This may be mobile-only
13. Store coupons

Evidence for these: `pc0288–pc0296`, `pc0400`, `pc0414`, `pc0427`, `app100000209`, `app100000398`, `app100001287`, `app100001349`, `assets/images/vip/rights-*.svg`. Labely plans to make every local-capable item on this list free.

**Open questions (UNCERTAIN)**
- Undo depth and the canvas keyboard shortcut map were not recovered statically.
- Is there really no element grouping, and no strikethrough?
- What is the native template file extension? Does any export-to-PDF/PNG exist?
- Is CSV import exposed? Is OCR or camera scanning exposed on desktop?
- What exactly are the image processing modes? Threshold and a dither kernel were found. Are there more?
- Some symbology tokens may belong to the bundled scanner library rather than the generator. Confirming the generator list needs a runtime check or a deeper AOT analysis.
