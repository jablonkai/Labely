import Foundation

/// What was copied last, to paste a step further each time into the same document.
struct Clip: Equatable {
    var docId: String
    var text: String
    var pastes = 0
}

/// Selection and arranging: several elements at once, as one undo step each.
extension EditorState {
    /// Step of the small and the large nudge, in millimetres.
    public static let nudgeSmall = 0.5
    public static let nudgeLarge = 5.0

    // MARK: - Selection

    /// Tap on element `id`: select it with its group. With `multiSelect` on (or `add`), it is
    /// added to the selection, or taken out when it was in. `nil` is a tap on empty space.
    public func tap(_ id: Int64?, add: Bool = false) {
        guard let doc else { return }
        guard let id, let e = doc.elements.first(where: { $0.id == id }) else {
            selection = []
            multiSelect = false
            return
        }
        let members = doc.members(of: e).map(\.id)
        if multiSelect || add {
            if selection.contains(id) {
                selection.removeAll(where: members.contains)
            } else {
                selection += members.filter { !selection.contains($0) }
            }
        } else {
            selection = members
        }
    }

    /// Select `ids` with their groups.
    public func select(_ ids: [Int64]) {
        selection = doc?.expanded(ids) ?? ids
    }

    public func selectAll() {
        selection = (doc?.elements ?? []).filter { !$0.hidden }.map(\.id)
        multiSelect = selection.count > 1
    }

    // MARK: - Running several commands

    /// Run `steps` as one undo step (`command.batch`); one step runs as it is. With `select`,
    /// the elements they create become the selection.
    @discardableResult
    public func batch(_ steps: [(String, JSON)], select: Bool = false) -> Task<Void, Never> {
        guard let first = steps.first else { return Task {} }
        if steps.count == 1 { return exec(first.0, first.1, select: select) }
        let list: [JSON] = steps.map { ["id": JSON($0.0), "params": $0.1.with("doc_id", JSON(docId))] }
        return exec("command.batch", ["steps": .array(list)], select: select, addDoc: false)
    }

    // MARK: - Moving

    /// Move `elements` by `(dx, dy)` mm, rounded to 0.1 mm, as one undo step.
    @discardableResult
    public func move(_ elements: [Element], dx: Double, dy: Double) -> Task<Void, Never> {
        let steps: [(String, JSON)] = elements.filter { !$0.locked }.map { e in
            ("element.move", ["id": .int(e.id), "x_mm": JSON(round1(e.xMm + dx)), "y_mm": JSON(round1(e.yMm + dy))])
        }
        return batch(steps)
    }

    @discardableResult
    public func nudge(dx: Double, dy: Double) -> Task<Void, Never> {
        move(selectedElements, dx: dx, dy: dy)
    }

    // MARK: - Rotation and flipping

    @discardableResult
    public func rotate(_ e: Element, to deg: Double) -> Task<Void, Never> {
        exec("element.rotate", ["id": .int(e.id), "rotation_deg": JSON(Rotation.normalised(deg))])
    }

    /// Turn every selected element a quarter turn clockwise.
    @discardableResult
    public func turn90() -> Task<Void, Never> {
        batch(selectedElements.filter { !$0.locked }.map { e in
            ("element.rotate", ["id": .int(e.id), "rotation_deg": JSON(Rotation.normalised(e.rotationDeg + 90))])
        })
    }

    @discardableResult
    public func flip(horizontal: Bool) -> Task<Void, Never> {
        let key = horizontal ? "flip_h" : "flip_v"
        return batch(selectedElements.filter { !$0.locked }.map { e in
            ("element.set_props", ["id": .int(e.id), "props": [key: JSON(!(horizontal ? e.flipH : e.flipV))]])
        })
    }

    /// Set the ink colour of the unlocked part of the selection: red or black.
    @discardableResult
    public func setRed(_ red: Bool) -> Task<Void, Never> {
        batch(selectedElements.filter { !$0.locked && $0.isRed != red }.map { e in
            ("element.set_props", ["id": .int(e.id), "props": ["color": JSON(red ? "red" : "black")]])
        })
    }

    /// A two-colour printer is connected, or is the document's printer.
    public var redPrinter: Bool {
        if app.printer?.model?.printsRed == true { return true }
        guard let id = doc?.label.printerModel else { return false }
        return app.model(id)?.printsRed == true
    }

    // MARK: - Align, distribute, group

    private var selectionIds: JSON { .array(selectedElements.map { .int($0.id) }) }

    /// Align the selection on `edge` (left, center, right, top, middle, bottom): to each other
    /// with two or more elements, to the label with one.
    @discardableResult
    public func align(_ edge: String) -> Task<Void, Never> {
        guard !selection.isEmpty else { return Task {} }
        return exec("element.align", ["ids": selectionIds, "edge": JSON(edge)])
    }

    /// Space three or more elements evenly along `axis` (horizontal, vertical).
    @discardableResult
    public func distribute(_ axis: String) -> Task<Void, Never> {
        guard selection.count >= 3 else { return Task {} }
        return exec("element.distribute", ["ids": selectionIds, "axis": JSON(axis)])
    }

    public var canGroup: Bool { selection.count >= 2 }
    public var canUngroup: Bool { selectedElements.contains { $0.group != nil } }

    @discardableResult
    public func group() -> Task<Void, Never> {
        guard canGroup else { return Task {} }
        return exec("element.group", ["ids": selectionIds])
    }

    @discardableResult
    public func ungroup() -> Task<Void, Never> {
        guard canUngroup else { return Task {} }
        return exec("element.ungroup", ["ids": selectionIds])
    }

    // MARK: - Lock, hide, name

    @discardableResult
    public func setLocked(_ ids: [Int64], _ locked: Bool) -> Task<Void, Never> {
        guard !ids.isEmpty else { return Task {} }
        return exec("element.lock", ["ids": .array(ids.map { .int($0) }), "locked": JSON(locked)])
    }

    @discardableResult
    public func setHidden(_ ids: [Int64], _ hidden: Bool) -> Task<Void, Never> {
        batch(ids.map { ("element.set_props", ["id": .int($0), "props": ["hidden": JSON(hidden)]]) })
    }

    /// Name the element; an empty name removes it.
    @discardableResult
    public func rename(_ e: Element, to name: String) -> Task<Void, Never> {
        let name = name.trimmingCharacters(in: .whitespaces)
        return exec("element.set_props", ["id": .int(e.id), "props": .object(["name": name.isEmpty ? .null : JSON(name)])])
    }

    // MARK: - Order, copies, delete

    /// Move the selection in the stack: front, back, forward or backward.
    @discardableResult
    public func reorder(_ to: String) -> Task<Void, Never> {
        // Keep the elements' own order: the top one goes first to the front, the bottom one
        // first to the back.
        let els = to == "front" || to == "backward" ? selectedElements : selectedElements.reversed()
        return batch(els.map { ("element.reorder", ["id": .int($0.id), "to": JSON(to)]) })
    }

    @discardableResult
    public func duplicateSelection() -> Task<Void, Never> {
        batch(selectedElements.map { ("element.duplicate", ["id": .int($0.id), "dx_mm": 1.0, "dy_mm": 1.0]) }, select: true)
    }

    @discardableResult
    public func deleteSelection() -> Task<Void, Never> {
        guard !selection.isEmpty else { return Task {} }
        return exec("element.delete", ["ids": selectionIds])
    }

    /// Add a linked mirror copy of every selected element that is not itself a copy; the copies
    /// follow their source. `axis` is horizontal (left–right) or vertical (top–bottom).
    @discardableResult
    public func mirror(_ axis: String) -> Task<Void, Never> {
        batch(selectedElements.filter { $0.mirrorOf == nil }.map { ("element.mirror", ["id": .int($0.id), "axis": JSON(axis)]) }, select: true)
    }

    /// Cut the link of a mirror copy, so it can be edited by itself.
    @discardableResult
    public func unlinkMirror(_ e: Element) -> Task<Void, Never> {
        exec("element.set_props", ["id": .int(e.id), "props": .object(["mirror": .null])])
    }

    // MARK: - Clipboard

    /// The selection as pasteboard text; with `cut`, it is deleted too. `nil` with no selection.
    public func copy(cut: Bool = false) -> String? {
        guard let doc else { return nil }
        let els = selectedElements
        guard !els.isEmpty else { return nil }
        let text = ElementClipboard.encode(els, assets: doc.assets)
        app.clip = Clip(docId: docId, text: text)
        if cut { deleteSelection() }
        return text
    }

    /// Paste pasteboard text: copied elements (also from another document or the studio) are
    /// added and selected; any other text becomes a text element.
    @discardableResult
    public func paste(_ text: String) -> Task<Void, Never> {
        guard let (elements, assets) = ElementClipboard.decode(text) else {
            let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
            return t.isEmpty ? Task {} : addText(t)
        }
        guard !elements.isEmpty else { return Task {} }
        // Into the document it came from, a paste lands 1 mm off the originals, and each
        // further paste another millimetre on.
        var offset = 0.0
        if var clip = app.clip, clip.text == text {
            offset = Double(clip.pastes + (clip.docId == docId ? 1 : 0))
            clip.pastes += 1
            app.clip = clip
        }
        return exec("element.paste", [
            "elements": .array(elements), "assets": assets, "dx_mm": JSON(offset), "dy_mm": JSON(offset),
        ], select: true)
    }
}
