import AVFoundation
import LabelyKit
import SwiftUI
import VisionKit

/// The camera scanner (#41): VisionKit reads the first barcode or 2D code it sees, on the
/// device, and hands it to `onCode`. It asks for the camera first.
struct ScanSheet: View {
    let onDismiss: () -> Void
    let onCode: (ScannedCode) -> Void
    /// Camera access; nil while the prompt is up.
    @State private var allowed: Bool?

    var body: some View {
        let supported = DataScannerViewController.isSupported
        let scanning = allowed == true && supported && DataScannerViewController.isAvailable
        ZStack {
            Color.black.ignoresSafeArea()
            if scanning {
                Scanner(onCode: onCode).ignoresSafeArea()
            }
            VStack(spacing: 16) {
                HStack(spacing: 4) {
                    IconButton(.close, tint: .white, label: L("cancel"), action: onDismiss)
                    Text(L("scan_title")).style(.bodyStrong.with(color: .white))
                    Spacer()
                }
                .padding(8)
                Spacer()
                if allowed != nil {
                    Text(L(scanning ? "scan_hint" : supported ? "scan_no_camera" : "scan_unsupported"))
                        .style(.body.with(color: .white))
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 24)
                    if allowed == false, let url = URL(string: UIApplication.openSettingsURLString) {
                        PrimaryButton(text: L("scan_open_settings")) {
                            UIApplication.shared.open(url)
                            onDismiss()
                        }
                        .padding(.horizontal, 24)
                    }
                }
                if !scanning { Spacer() }
            }
            .padding(.bottom, 24)
        }
        .task { allowed = await AVCaptureDevice.requestAccess(for: .video) }
    }
}

/// VisionKit's data scanner, looking for the symbologies Labely prints.
private struct Scanner: UIViewControllerRepresentable {
    let onCode: (ScannedCode) -> Void

    func makeUIViewController(context: Context) -> DataScannerViewController {
        let vc = DataScannerViewController(
            recognizedDataTypes: [.barcode(symbologies: ScannedCode.symbologies)],
            qualityLevel: .balanced,
            recognizesMultipleItems: false,
            isHighFrameRateTrackingEnabled: false,
            isHighlightingEnabled: true
        )
        vc.delegate = context.coordinator
        return vc
    }

    func updateUIViewController(_ vc: DataScannerViewController, context: Context) {
        context.coordinator.onCode = onCode
        if !vc.isScanning, !context.coordinator.done { try? vc.startScanning() }
    }

    static func dismantleUIViewController(_ vc: DataScannerViewController, coordinator: Coordinator) {
        vc.stopScanning()
    }

    func makeCoordinator() -> Coordinator { Coordinator(onCode: onCode) }

    @MainActor final class Coordinator: NSObject, DataScannerViewControllerDelegate {
        var onCode: (ScannedCode) -> Void
        /// One code per scan.
        private(set) var done = false

        init(onCode: @escaping (ScannedCode) -> Void) {
            self.onCode = onCode
        }

        func dataScanner(_ dataScanner: DataScannerViewController, didAdd addedItems: [RecognizedItem], allItems: [RecognizedItem]) {
            guard !done else { return }
            for item in addedItems {
                guard case let .barcode(b) = item, let code = ScannedCode.of(b.observation.symbology, text: b.payloadStringValue) else { continue }
                done = true
                dataScanner.stopScanning()
                onCode(code)
                return
            }
        }
    }
}
