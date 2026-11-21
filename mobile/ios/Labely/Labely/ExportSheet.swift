import LabelyKit
import SwiftUI

/// The files of one export, for the share sheet or the save picker.
private struct ExportedFiles: Identifiable {
    let id = UUID()
    let urls: [URL]
}

/// M12 · Exportálás: format, resolution and records, then Share or Export. The engine writes
/// PNG and PDF as the 1-bit print raster, so the design's SVG, transparency and 1-bit switches
/// are left out.
struct ExportSheet: View {
    let ed: EditorState
    let onDismiss: () -> Void
    @State private var format = ExportFormat.png
    @State private var dpi = Export.defaultDpi
    @State private var all = false
    @State private var sharing: ExportedFiles?
    @State private var saving: ExportedFiles?

    var body: some View {
        let records = ed.data.map(\.rowCount) ?? 0
        let series = all && records > 0
        SheetBody(title: L("export"), onClose: onDismiss) {
            HStack(spacing: 12) {
                formatCard(.png, .image, "PNG")
                formatCard(.pdf, .file, "PDF")
            }
            ListGroup {
                ListRow(title: L("export_resolution")) {
                    Picker(L("export_resolution"), selection: $dpi) {
                        ForEach(Export.dpis, id: \.self) { Text($0 == Export.dpis.last ? "\($0) dpi" : "\($0)").tag($0) }
                    }
                    .pickerStyle(.segmented)
                    .fixedSize()
                }
                if records > 0 {
                    RowDivider()
                    ListRow(title: L("records")) {
                        Picker(L("records"), selection: $all) {
                            Text(L("records_current")).tag(false)
                            Text(L("records_all", records)).tag(true)
                        }
                        .pickerStyle(.segmented)
                        .fixedSize()
                    }
                }
            }
            if let doc = ed.doc {
                VStack(alignment: .leading, spacing: 4) {
                    Text(Export.fileName(docName: doc.name, record: series && format == .png ? ed.record : nil, format: format))
                        .style(.body).lineLimit(1).truncationMode(.middle)
                    Text(format == .pdf ? L("export_pages", series ? records : 1) : L("export_files", series ? records : 1)).style(.caption)
                }
            }
            HStack(spacing: 12) {
                SecondaryButton(text: L("share"), icon: .share, enabled: !ed.exporting, height: 52) {
                    run { sharing = ExportedFiles(urls: $0) }
                }
                PrimaryButton(text: L("export"), icon: .download, enabled: !ed.exporting) {
                    run { saving = ExportedFiles(urls: $0) }
                }
            }
            if ed.exporting { ProgressView().frame(maxWidth: .infinity) }
        }
        .presentationDetents([.fraction(0.65), .large])
        .sheet(item: $sharing) { files in
            ShareView(urls: files.urls).presentationDetents([.medium, .large])
        }
        .sheet(item: $saving) { files in
            // One file asks where to save it; several ask for a folder.
            SaveView(urls: files.urls).ignoresSafeArea()
        }
    }

    /// Write the files, then hand them on. A failure shows as the app's message.
    private func run(_ then: @escaping ([URL]) -> Void) {
        Task {
            let urls = await ed.export(format: format, dpi: dpi, allRecords: all)
            if !urls.isEmpty { then(urls) }
        }
    }

    private func formatCard(_ f: ExportFormat, _ icon: Icon, _ name: String) -> some View {
        let on = format == f
        let shape = RoundedRectangle(cornerRadius: 16)
        return Button { format = f } label: {
            VStack(alignment: .leading, spacing: 10) {
                LIcon(icon, on ? Palette.primary : Palette.text2, size: 24).frame(maxWidth: .infinity)
                Text(name).style(.bodyStrong.with(color: on ? Palette.primaryDark : Palette.text))
            }
            .padding(14)
            .frame(maxWidth: .infinity)
            .background(on ? Palette.primaryTint : Palette.surface, in: shape)
            .overlay(shape.strokeBorder(on ? Palette.primary : Palette.border, lineWidth: on ? 2 : 1))
            .contentShape(shape)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// The system share sheet for exported files.
private struct ShareView: UIViewControllerRepresentable {
    let urls: [URL]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: urls, applicationActivities: nil)
    }

    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
