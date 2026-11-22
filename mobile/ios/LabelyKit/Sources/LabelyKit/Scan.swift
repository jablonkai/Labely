import Vision

/// A code the camera read (#41), in Labely's terms: `kind` is `barcode` or `code2d`, and
/// `symbology` one of `Symbology.oneD` or `Symbology.twoD`.
public struct ScannedCode: Equatable, Sendable {
    public let kind: String
    public let symbology: String
    public let text: String

    public init(kind: String, symbology: String, text: String) {
        self.kind = kind
        self.symbology = symbology
        self.text = text
    }

    /// The code Vision read as `symbology` with `text`; nil for one Labely cannot print or an
    /// empty read. Vision reads a UPC-A as an EAN-13 with a leading zero; it prints the same.
    public static func of(_ symbology: VNBarcodeSymbology, text: String?) -> ScannedCode? {
        guard let text, !text.isEmpty, let id = labelySymbology(symbology) else { return nil }
        return ScannedCode(kind: Symbology.twoD.contains(id) ? "code2d" : "barcode", symbology: id, text: text)
    }

    /// The symbologies the scanner looks for: the ones `of` maps.
    public static let symbologies: [VNBarcodeSymbology] = [
        .ean13, .ean8, .upce, .code128, .code39, .code39Checksum, .code39FullASCII, .code39FullASCIIChecksum,
        .code93, .code93i, .itf14, .i2of5, .i2of5Checksum, .codabar, .qr, .dataMatrix, .pdf417, .aztec,
    ]

    private static func labelySymbology(_ s: VNBarcodeSymbology) -> String? {
        switch s {
        case .ean13: "ean13"
        case .ean8: "ean8"
        case .upce: "upc_e"
        case .code128: "code128"
        case .code39, .code39Checksum, .code39FullASCII, .code39FullASCIIChecksum: "code39"
        case .code93, .code93i: "code93"
        case .itf14, .i2of5, .i2of5Checksum: "itf"
        case .codabar: "codabar"
        case .qr: "qr"
        case .dataMatrix: "data_matrix"
        case .pdf417: "pdf417"
        case .aztec: "aztec"
        default: nil
        }
    }
}
