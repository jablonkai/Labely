import CoreBluetooth
import LabelyKit
import SwiftUI

/// Run `action` unless the user denied Bluetooth. Before the first use iOS asks for the
/// permission itself when the engine starts a scan or a connect.
@MainActor
func withBluetooth(_ state: AppState, _ action: () -> Void) {
    switch CBManager.authorization {
    case .denied, .restricted: state.message = .text(L("ble_denied"))
    default: action()
    }
}

/// The printers a picker offers: the remembered ones, then (in debug builds) the simulator.
@MainActor
func knownPrinters(_ state: AppState) -> [KnownPrinter] {
    let known = state.known.filter { $0.address != state.printer?.address }
    let sim = state.debug && !known.contains { $0.address == AppState.simAddress } && state.printer?.address != AppState.simAddress
    return known + (sim ? [KnownPrinter(address: AppState.simAddress, name: L("simulator"))] : [])
}

/// M8 · Nyomtatók: the connected printer, known printers and a Bluetooth scan.
struct PrintersScreen: View {
    let state: AppState

    var body: some View {
        let known = knownPrinters(state)
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                LargeTitle(title: L("tab_printers"), sub: L("printers_sub")) {
                    if state.busy == AppState.scanning {
                        ProgressView().controlSize(.large).frame(width: 52, height: 52)
                    } else {
                        IconButton(.search, tint: Palette.primary, fill: Palette.primaryTint, size: 52, label: L("printers_search")) {
                            withBluetooth(state) { state.scan() }
                        }
                    }
                }
                ConnectedCard(state: state)

                if !known.isEmpty {
                    SectionHeader(title: L("printers_known"), action: L("printers_search")) {
                        withBluetooth(state) { state.scan() }
                    }
                    ListGroup {
                        ForEach(Array(known.enumerated()), id: \.element.id) { i, k in
                            if i > 0 { RowDivider() }
                            PrinterRow(state: state, address: k.address, name: k.name)
                        }
                    }
                }

                if let all = state.scanResults {
                    let found = all.filter { d in d.address != state.printer?.address && !known.contains { $0.address == d.address } }
                    SectionHeader(title: L("printers_found"))
                    ListGroup {
                        if found.isEmpty {
                            ListRow(title: L("printers_none_found"), subtitle: L("printers_none_found_hint"))
                        }
                        ForEach(Array(found.enumerated()), id: \.element.address) { i, d in
                            if i > 0 { RowDivider() }
                            PrinterRow(state: state, address: d.address, name: d.name)
                        }
                    }
                }
            }
            .padding(16)
        }
        .task(id: state.printer?.address) { await state.refreshStatus() }
    }
}

struct PrinterRow: View {
    let state: AppState
    let address: String
    let name: String

    var body: some View {
        let ble = address.hasPrefix("ble:")
        ListRow(
            title: name.isEmpty ? address : name,
            subtitle: transportName(address) + " · " + (address.split(separator: ":", maxSplits: 1).last.map(String.init) ?? address)
        ) {
            IconTile(ble ? .bluetooth : .printer)
        } trailing: {
            if state.busy == address {
                ProgressView()
            } else {
                Button {
                    if ble { withBluetooth(state) { state.connect(address, name: name) } } else { state.connect(address, name: name) }
                } label: {
                    Text(L("connect")).style(.bodyStrong.with(color: state.busy == nil ? Palette.primary : Palette.muted))
                }
                .buttonStyle(.plain)
                .disabled(state.busy != nil)
            }
        }
        .contextMenu {
            if state.known.contains(where: { $0.address == address }) {
                Button(role: .destructive) {
                    state.forget(KnownPrinter(address: address, name: name))
                } label: { Text(L("delete")) }
            }
        }
    }
}

func transportName(_ address: String) -> String {
    if address.hasPrefix("ble:") { return L("transport_ble") }
    if address.hasPrefix("usb:") { return "USB" }
    return L("transport_sim")
}

private struct ConnectedCard: View {
    let state: AppState
    @State private var settings = false

    var body: some View {
        Card {
            if let p = state.printer {
                HStack(spacing: 16) {
                    IconTile(.printer, size: 64)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(p.model?.name ?? p.name).style(.title.with(size: 19))
                        let sub = [p.serial, p.firmware.map { L("firmware", $0) }].compactMap { $0 }.joined(separator: " · ")
                        if !sub.isEmpty { Text(sub).style(.caption) }
                        HStack(spacing: 6) {
                            Pill(text: L("connected"), fg: Palette.green, bg: Palette.greenTint)
                            Pill(text: transportName(p.address), fg: Palette.text2, bg: Palette.field)
                        }
                    }
                    Spacer(minLength: 0)
                }
                .padding(16)
                RowDivider()
                let status = state.printerStatus
                HStack {
                    Stat(label: L("battery"), value: status?.batteryPercent.map { "\($0)%" } ?? "–")
                    Stat(label: L("paper"), value: status?.paperPresent.map { L($0 ? "paper_loaded" : "paper_missing") } ?? "–")
                }
                .padding(16)
                HStack(spacing: 12) {
                    SecondaryButton(text: L("printer_settings"), icon: .settings, enabled: state.busy == nil) { settings = true }
                    SecondaryButton(text: L("disconnect"), icon: .close, enabled: state.busy == nil) { state.disconnect() }
                }
                .padding([.horizontal, .bottom], 16)
                .sheet(isPresented: $settings) { PrinterSettingsSheet(state: state) { settings = false } }
            } else {
                ListRow(title: L("no_printer"), subtitle: L("no_printer_hint")) {
                    IconTile(.printer, tint: Palette.muted, fill: Palette.bg, size: 56)
                } trailing: {
                    EmptyView()
                }
            }
        }
    }
}

/// The connected printer's own settings (`device.settings`): auto power-off and the beeps, only
/// those it reports, and the calibration.
private struct PrinterSettingsSheet: View {
    let state: AppState
    let onDismiss: () -> Void

    var body: some View {
        SheetBody(title: L("printer_settings"), onClose: onDismiss) {
            if let s = state.printerSettings {
                ListGroup {
                    if let level = s.autoOffLevel {
                        VStack(alignment: .leading, spacing: 10) {
                            Text(L("auto_off")).style(.bodyStrong.with(size: 16))
                            Picker(L("auto_off"), selection: Binding(get: { level }, set: { state.setPrinterSettings(autoOffLevel: $0) })) {
                                ForEach(Array(PrinterSettings.autoOffLevels), id: \.self) { Text(autoOffName($0)).tag($0) }
                            }
                            .pickerStyle(.segmented)
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 14)
                    }
                    if let on = s.soundPower {
                        if s.autoOffLevel != nil { RowDivider() }
                        ListRow(title: L("sound_power"), subtitle: L("sound_power_sub")) {
                            Toggle("", isOn: Binding(get: { on }, set: { state.setPrinterSettings(soundPower: $0) }))
                                .labelsHidden().tint(Palette.primary)
                        }
                    }
                    if let on = s.soundBluetooth {
                        if s.autoOffLevel != nil || s.soundPower != nil { RowDivider() }
                        ListRow(title: L("sound_bluetooth"), subtitle: L("sound_bluetooth_sub")) {
                            Toggle("", isOn: Binding(get: { on }, set: { state.setPrinterSettings(soundBluetooth: $0) }))
                                .labelsHidden().tint(Palette.primary)
                        }
                    }
                    if s.isEmpty { ListRow(title: L("printer_settings_none")) }
                }
            } else {
                ProgressView().frame(maxWidth: .infinity).padding(.vertical, 24)
            }
            SecondaryButton(text: L("calibrate"), icon: .scan, enabled: state.busy == nil, height: 52) { state.calibrate() }
        }
        .presentationDetents([.medium, .large])
        .task { await state.loadPrinterSettings().value }
        // A printer that went away has no settings to show.
        .onChange(of: state.printer == nil) { if state.printer == nil { onDismiss() } }
    }

    /// NIIMBOT printers switch off after 15, 30, 45 or 60 minutes; others only number the levels.
    private func autoOffName(_ level: Int) -> String {
        state.printer?.model?.id.hasPrefix("niimbot.") == true ? L("minutes", level * 15) : String(level)
    }
}

private struct Pill: View {
    let text: String
    let fg: Color
    let bg: Color

    var body: some View {
        Text(text).style(.caption.with(weight: .semibold, color: fg))
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .background(bg, in: Capsule())
    }
}

private struct Stat: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading) {
            Text(label).style(.caption)
            Text(value).style(.title.with(size: 20))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
