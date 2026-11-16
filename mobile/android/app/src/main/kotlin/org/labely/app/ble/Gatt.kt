package org.labely.app.ble

import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattService

/**
 * The NIIMBOT GATT knowledge of `labely_transport::gatt` (docs/protocol.md §3.1), mirrored for
 * the shell, which picks the characteristic before Rust sees the link.
 */
internal object Gatt {
    const val NIIMBOT_SERVICE = "e7810a71-73ae-499d-8c15-faa9aef0c3f2"

    /** Known (service, characteristic) pairs in preference order; `ffe1` is a 16-bit alias. */
    private val KNOWN_PAIRS = listOf(
        NIIMBOT_SERVICE to "bef8d6c9-9c21-4c9e-b632-bd58c1009f9f",
        "0000e0ff-3c17-d293-8e48-14fe2e4da212" to "ffe1",
        "49535343-fe7d-4ae5-8fa9-9fafd205e455" to "49535343-8841-43f4-a8d4-ecbe34729bb3",
    )

    /** Client Characteristic Configuration descriptor. */
    const val CCCD = "00002902-0000-1000-8000-00805f9b34fb"

    /** A full UUID against a full UUID or a 16-bit alias, case-insensitively. */
    fun uuidMatches(uuid: String, pattern: String): Boolean {
        val u = uuid.lowercase()
        val p = pattern.lowercase()
        return if (p.length == 4) u.startsWith("0000$p-") else u == p
    }

    /** Same rule as `gatt::scan_matches`. */
    fun scanMatches(name: String, services: List<String>, prefixes: List<String>): Boolean =
        prefixes.isEmpty() ||
            services.any { uuidMatches(it, NIIMBOT_SERVICE) } ||
            prefixes.any { name.startsWith(it, ignoreCase = true) }

    /** The printer's notify + write-without-response characteristic, if the device has one. */
    fun pick(services: List<BluetoothGattService>): BluetoothGattCharacteristic? {
        for ((svc, ch) in KNOWN_PAIRS) {
            for (s in services.filter { uuidMatches(it.uuid.toString(), svc) }) {
                s.characteristics.firstOrNull { uuidMatches(it.uuid.toString(), ch) && usable(it) }
                    ?.let { return it }
            }
        }
        return null
    }

    private fun usable(c: BluetoothGattCharacteristic): Boolean {
        val p = c.properties
        val notify = p and (BluetoothGattCharacteristic.PROPERTY_NOTIFY or BluetoothGattCharacteristic.PROPERTY_INDICATE) != 0
        return notify && p and BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE != 0
    }
}
