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
    @State private var entering = false
    @State private var address = ""

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
                if let failure = state.connectFailure { ConnectTips(state: state, failure: failure) }

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
                SecondaryButton(text: L("printer_manual"), icon: .network, enabled: state.busy == nil, height: 52) {
                    address = ""
                    entering = true
                }
                Text(L("printer_manual_hint")).style(.caption)
            }
            .padding(16)
        }
        .task(id: state.printer?.address) {
            await state.refreshStatus()
            await state.detectPaper()
        }
        .alert(L("printer_manual"), isPresented: $entering) {
            TextField("192.168.1.50:9100", text: $address)
                .keyboardType(.URL)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            Button(L("connect")) { state.connectManual(address) }
            Button(L("cancel"), role: .cancel) {}
        } message: {
            Text(L("printer_manual_body"))
        }
    }
}

/// Why a connect may have failed and what to try, with Try again.
private struct ConnectTips: View {
    let state: AppState
    let failure: ConnectFailure

    var body: some View {
        let ble = failure.address.hasPrefix("ble:")
        let tcp = failure.address.hasPrefix("tcp:")
        Card {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 10) {
                    IconTile(.warn, tint: Palette.amber, fill: Palette.amberTint)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(L("connect_failed", failure.name.isEmpty ? failure.address : failure.name)).style(.bodyStrong.with(size: 16))
                        Text(failure.message).style(.caption).fixedSize(horizontal: false, vertical: true)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    IconButton(.close, size: 34, label: L("cancel")) { state.dismissConnectFailure() }
                }
                ForEach(tips(ble: ble, tcp: tcp), id: \.self) { tip in
                    HStack(alignment: .top, spacing: 8) {
                        Text("•").style(.body.with(color: Palette.muted))
                        Text(tip).style(.body.with(color: Palette.text2)).fixedSize(horizontal: false, vertical: true)
                    }
                }
                PrimaryButton(text: L("try_again"), icon: .refresh, enabled: state.busy == nil) {
                    if ble { withBluetooth(state) { state.retryConnect() } } else { state.retryConnect() }
                }
            }
            .padding(14)
        }
    }

    private func tips(ble: Bool, tcp: Bool) -> [String] {
        if tcp { return [L("tip_tcp_network"), L("tip_tcp_address"), L("tip_power")] }
        if ble { return [L("tip_power"), L("tip_ble_near"), L("tip_ble_other_app"), L("tip_ble_restart")] }
        return [L("tip_power")]
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
            IconTile(ble ? .bluetooth : address.hasPrefix("tcp:") ? .network : .printer)
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
    if address.hasPrefix("tcp:") { return L("transport_tcp") }
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
                if status?.lidClosed == false {
                    Banner(text: L("lid_open"), ok: false).padding([.horizontal, .top], 16)
                } else if status?.paperPresent == false {
                    Banner(text: L("no_labels"), ok: false).padding([.horizontal, .top], 16)
                }
                HStack {
                    Stat(label: L("battery"), value: status?.batteryPercent.map { "\($0)%" } ?? "–")
                    Stat(label: L("paper"), value: status?.paperPresent.map { L($0 ? "paper_loaded" : "paper_missing") } ?? "–")
                }
                .padding([.horizontal, .top], 16)
                HStack {
                    Stat(label: L("resolution"), value: p.model.map { "\($0.dpi) dpi" } ?? "–")
                    Stat(label: L("printable_width"), value: p.model.flatMap { $0.headMm > 0 ? mmText($0.headMm) : nil } ?? "–")
                }
                .padding([.horizontal, .top], 16)
                HStack {
                    Stat(
                        label: L("labels_remaining"),
                        value: status?.paper.map { $0.total > 0 ? L("labels_of", $0.remaining, $0.total) : "–" } ?? "–"
                    )
                    Stat(label: L("lid"), value: status?.lidClosed.map { L($0 ? "lid_closed" : "lid_open_short") } ?? "–")
                }
                .padding(16)
                if let d = state.detected, d.rfid {
                    RowDivider()
                    ListRow(title: d.paper.map(paperTitle) ?? L("paper_unknown_roll"), subtitle: L("paper_loaded_roll")) {
                        IconTile(.label)
                    } trailing: {
                        EmptyView()
                    }
                }
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
            Text(value).style(.title.with(size: 20)).lineLimit(1).minimumScaleFactor(0.6)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
