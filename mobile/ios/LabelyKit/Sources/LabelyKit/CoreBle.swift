import CoreBluetooth
import Foundation
import LabelyFFI

/// The shell's BLE stack for the engine (ADR-0025, ADR-0032), over CoreBluetooth. Rust calls it
/// from its own threads and every call blocks until done. Framing, chunking and the printer
/// protocol stay in Rust.
///
/// All CoreBluetooth state lives on `queue`, which is also the central's delegate queue. Rust
/// threads reach it with `queue.sync`, and wait for callbacks with semaphores, never on `queue`.
public final class CoreBle: NSObject, NativeBle, CBCentralManagerDelegate, @unchecked Sendable {
    let queue = DispatchQueue(label: "labely-ble")
    private var central: CBCentralManager?
    private let stateChanged = DispatchSemaphore(value: 0)
    /// Peripherals seen in a scan, kept so `connect` can find them and CoreBluetooth keeps them.
    private var seen: [UUID: CBPeripheral] = [:]
    private var found: [UUID: BleDevice] = [:]
    private var prefixes: [String] = []
    private var links: [UUID: CoreBleLink] = [:]

    /// How long to wait for Bluetooth to report its state; the first use waits for the
    /// permission prompt.
    private static let stateTimeout: TimeInterval = 60

    override public init() {
        super.init()
    }

    // MARK: NativeBle

    public func scan(timeoutMs: UInt64, namePrefixes: [String]) throws -> [BleDevice] {
        let central = try ready()
        queue.sync {
            found = [:]
            prefixes = namePrefixes
            central.scanForPeripherals(withServices: nil, options: [CBCentralManagerScanOptionAllowDuplicatesKey: false])
        }
        Thread.sleep(forTimeInterval: Double(timeoutMs) / 1000)
        return queue.sync {
            if central.isScanning { central.stopScan() }
            return found.values.sorted { ($0.rssi ?? .min) > ($1.rssi ?? .min) }
        }
    }

    public func connect(id: String, sink: BleNotifySink) throws -> NativeBleLink {
        let central = try ready()
        guard let uuid = UUID(uuidString: id) else {
            throw BleError.NotFound(detail: "not a peripheral identifier: \(id)")
        }
        let link: CoreBleLink? = queue.sync {
            guard let p = seen[uuid] ?? central.retrievePeripherals(withIdentifiers: [uuid]).first else { return nil }
            seen[uuid] = p
            let link = CoreBleLink(central: central, peripheral: p, sink: sink, queue: queue)
            links[uuid] = link
            return link
        }
        guard let link else { throw BleError.NotFound(detail: "no Bluetooth device \(id)") }
        do {
            try link.open()
        } catch {
            link.disconnect()
            throw error
        }
        return link
    }

    /// The central once Bluetooth is on; otherwise `Unavailable`.
    private func ready() throws -> CBCentralManager {
        let deadline = DispatchTime.now() + Self.stateTimeout
        while true {
            let (central, state) = queue.sync { () -> (CBCentralManager, CBManagerState) in
                if let c = self.central { return (c, c.state) }
                // Creating the central asks for the Bluetooth permission the first time.
                let c = CBCentralManager(delegate: self, queue: queue)
                self.central = c
                return (c, c.state)
            }
            switch state {
            case .poweredOn:
                return central
            case .unauthorized:
                throw BleError.Unavailable(detail: "Bluetooth permission not granted")
            case .poweredOff:
                throw BleError.Unavailable(detail: "Bluetooth is off")
            case .unsupported:
                throw BleError.Unavailable(detail: "no Bluetooth on this device")
            default:
                if stateChanged.wait(timeout: deadline) == .timedOut {
                    throw BleError.Unavailable(detail: "Bluetooth did not become ready")
                }
            }
        }
    }

    // MARK: CBCentralManagerDelegate (on `queue`)

    public func centralManagerDidUpdateState(_ central: CBCentralManager) {
        stateChanged.signal()
        // Turning Bluetooth off drops every link without a disconnect callback.
        if central.state != .poweredOn {
            for link in links.values { link.dropped() }
            links = [:]
        }
    }

    public func centralManager(
        _ central: CBCentralManager,
        didDiscover peripheral: CBPeripheral,
        advertisementData: [String: Any],
        rssi RSSI: NSNumber
    ) {
        let name = advertisementData[CBAdvertisementDataLocalNameKey] as? String ?? peripheral.name ?? ""
        let services = (advertisementData[CBAdvertisementDataServiceUUIDsKey] as? [CBUUID] ?? []).map(\.uuidString)
        guard Gatt.scanMatches(name: name, services: services, prefixes: prefixes) else { return }
        seen[peripheral.identifier] = peripheral
        // 127 means the RSSI is not available.
        let rssi = RSSI.intValue == 127 ? nil : Int16(clamping: RSSI.intValue)
        found[peripheral.identifier] = BleDevice(id: peripheral.identifier.uuidString, name: name, rssi: rssi)
    }

    public func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        links[peripheral.identifier]?.complete(.connect, nil)
    }

    public func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        links[peripheral.identifier]?.complete(.connect, error ?? BleError.Failed(detail: "connect failed"))
    }

    public func centralManager(
        _ central: CBCentralManager,
        didDisconnectPeripheral peripheral: CBPeripheral,
        error: Error?
    ) {
        links.removeValue(forKey: peripheral.identifier)?.dropped()
    }
}

/// One connected printer. One operation runs at a time (`ops`): it is started on `queue`, and
/// the caller blocks until its delegate callback arrives, with a timeout.
final class CoreBleLink: NSObject, NativeBleLink, CBPeripheralDelegate, @unchecked Sendable {
    enum Op: String { case connect, services, characteristics, notify, ready }

    private final class Pending {
        let op: Op
        let done = DispatchSemaphore(value: 0)
        var error: Error?
        var disconnected = false

        init(_ op: Op) { self.op = op }
    }

    private static let connectTimeout: TimeInterval = 15
    private static let opTimeout: TimeInterval = 5
    private static let defaultPayload = 20

    private let central: CBCentralManager
    private let peripheral: CBPeripheral
    private let sink: BleNotifySink
    private let queue: DispatchQueue
    private let ops = NSLock()

    // State below is only touched on `queue`.
    private var pending: Pending?
    private var characteristic: CBCharacteristic?
    private var payload = defaultPayload
    private var opened = false
    private var closed = false

    init(central: CBCentralManager, peripheral: CBPeripheral, sink: BleNotifySink, queue: DispatchQueue) {
        self.central = central
        self.peripheral = peripheral
        self.sink = sink
        self.queue = queue
        super.init()
    }

    /// Connect, find the printer characteristic and subscribe. iOS negotiates the MTU itself.
    func open() throws {
        ops.lock()
        defer { ops.unlock() }
        try op(.connect, Self.connectTimeout) {
            self.peripheral.delegate = self
            self.central.connect(self.peripheral)
        }
        try op(.services, Self.opTimeout) { self.peripheral.discoverServices(nil) }
        let services = queue.sync { (peripheral.services ?? []).filter { Gatt.isKnownService($0.uuid.uuidString) } }
        for s in services {
            try op(.characteristics, Self.opTimeout) { self.peripheral.discoverCharacteristics(nil, for: s) }
        }
        let chosen: CBCharacteristic? = queue.sync {
            let all = services.flatMap { s in (s.characteristics ?? []).map { (s, $0) } }
            let candidates = all.map { s, c in
                let p = c.properties
                let usable = !p.isDisjoint(with: [.notify, .indicate]) && p.contains(.writeWithoutResponse)
                return (service: s.uuid.uuidString, characteristic: c.uuid.uuidString, usable: usable)
            }
            return Gatt.pick(candidates).map { all[$0].1 }
        }
        guard let chosen else {
            throw BleError.NotFound(detail: "\(peripheral.identifier) has no NIIMBOT printer service")
        }
        try op(.notify, Self.opTimeout) {
            self.characteristic = chosen
            self.peripheral.setNotifyValue(true, for: chosen)
        }
        queue.sync {
            payload = peripheral.maximumWriteValueLength(for: .withoutResponse)
            opened = true
        }
    }

    /// Start one operation on `queue` and wait for its callback. Holds `ops`.
    private func op(_ kind: Op, _ timeout: TimeInterval, start: @escaping () -> Void) throws {
        let p = Pending(kind)
        let alive = queue.sync {
            if closed { return false }
            pending = p
            start()
            return true
        }
        guard alive else { throw BleError.NotFound(detail: "printer disconnected") }
        let result = p.done.wait(timeout: .now() + timeout)
        queue.sync { if pending === p { pending = nil } }
        if result == .timedOut { throw BleError.Failed(detail: "\(kind.rawValue) timed out") }
        if p.disconnected { throw BleError.NotFound(detail: "printer disconnected") }
        if let e = p.error { throw BleError.Failed(detail: "\(kind.rawValue) failed: \(e.localizedDescription)") }
    }

    /// Finish the pending operation `kind`. On `queue`.
    func complete(_ kind: Op, _ error: Error?) {
        guard let p = pending, p.op == kind else { return }
        p.error = error
        p.done.signal()
    }

    /// The link dropped without `disconnect`. On `queue`.
    func dropped() {
        let notify = opened && !closed
        closed = true
        if let p = pending {
            p.disconnected = true
            p.done.signal()
        }
        if notify { sink.disconnected() }
    }

    // MARK: NativeBleLink

    func write(bytes: Data) throws {
        ops.lock()
        defer { ops.unlock() }
        while true {
            let sent = try queue.sync { () throws -> Bool in
                guard !closed, let c = characteristic else { throw BleError.NotFound(detail: "printer disconnected") }
                guard peripheral.canSendWriteWithoutResponse else { return false }
                peripheral.writeValue(bytes, for: c, type: .withoutResponse)
                return true
            }
            if sent { return }
            // The controller's buffer is full: wait until it can take more.
            try op(.ready, Self.opTimeout) {
                if self.peripheral.canSendWriteWithoutResponse { self.complete(.ready, nil) }
            }
        }
    }

    func mtu() -> UInt32 {
        UInt32(max(queue.sync { payload }, Self.defaultPayload))
    }

    func name() -> String {
        queue.sync { peripheral.name } ?? peripheral.identifier.uuidString
    }

    func disconnect() {
        queue.sync {
            closed = true
            central.cancelPeripheralConnection(peripheral)
            if let p = pending {
                p.disconnected = true
                p.done.signal()
            }
        }
    }

    // MARK: CBPeripheralDelegate (on `queue`)

    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        complete(.services, error)
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        complete(.characteristics, error)
    }

    func peripheral(_ peripheral: CBPeripheral, didUpdateNotificationStateFor characteristic: CBCharacteristic, error: Error?) {
        complete(.notify, error)
    }

    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        guard error == nil, characteristic == self.characteristic, let value = characteristic.value else { return }
        sink.push(bytes: value)
    }

    func peripheralIsReady(toSendWriteWithoutResponse peripheral: CBPeripheral) {
        complete(.ready, nil)
    }
}
