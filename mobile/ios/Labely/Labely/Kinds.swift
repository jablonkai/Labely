import LabelyKit

/// Icon of an element kind.
func kindIcon(_ type: String) -> Icon {
    switch type {
    case "text": .text
    case "barcode": .barcode
    case "code2d": .qr
    case "image": .image
    case "rect": .rect
    case "ellipse": .ellipse
    case "line": .line
    case "table": .table
    case "border": .border
    default: .label
    }
}

func kindName(_ type: String) -> String {
    switch type {
    case "text", "barcode", "code2d", "image", "rect", "ellipse", "line", "table", "icon", "border": L("kind_\(type)")
    default: L("kind_element")
    }
}

/// The element's name, else the first line of its text or data, else its kind.
func elementTitle(_ e: Element) -> String {
    if let name = e.name { return name }
    if let first = (e.text ?? e.data)?.split(separator: "\n", omittingEmptySubsequences: false).first,
       !first.trimmingCharacters(in: .whitespaces).isEmpty
    {
        return String(first)
    }
    return kindName(e.type)
}

let mediaKinds = ["gap", "black_mark", "continuous", "transparent"]

func mediaName(_ media: String) -> String {
    L("media_\(mediaKinds.contains(media) ? media : "gap")")
}
