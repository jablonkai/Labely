package org.labely.app.ble

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothManager
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.labely.ffi.BleDevice
import org.labely.ffi.BleException
import org.labely.ffi.BleNotifySink
import org.labely.ffi.NativeBle
import org.labely.ffi.NativeBleLink

/** Runtime permissions the BLE path needs on this Android version. */
object BlePermissions {
    val required: Array<String> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            arrayOf(Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT)
        } else {
            arrayOf(Manifest.permission.ACCESS_FINE_LOCATION)
        }

    fun granted(context: Context): Boolean =
        required.all { context.checkSelfPermission(it) == PackageManager.PERMISSION_GRANTED }
}

/**
 * The shell's BLE stack for the engine (ADR-0025, ADR-0027). Rust calls it from its own threads
 * and every call blocks until done. Framing, chunking and the printer protocol stay in Rust.
 */
@SuppressLint("MissingPermission") // enabledAdapter() checks; guard() maps a revoked one.
class AndroidBle(context: Context) : NativeBle {
    private val context = context.applicationContext
    private val manager = this.context.getSystemService(BluetoothManager::class.java)

    override fun scan(timeoutMs: ULong, namePrefixes: List<String>): List<BleDevice> {
        val adapter = enabledAdapter()
        val scanner = adapter.bluetoothLeScanner ?: throw BleException.Unavailable("Bluetooth is off")
        val found = ConcurrentHashMap<String, BleDevice>()
        val failed = CountDownLatch(1)
        var failure = 0
        val callback = object : ScanCallback() {
            override fun onScanResult(callbackType: Int, result: ScanResult) = add(result)

            override fun onBatchScanResults(results: MutableList<ScanResult>) = results.forEach(::add)

            override fun onScanFailed(errorCode: Int) {
                failure = errorCode
                failed.countDown()
            }

            private fun add(r: ScanResult) {
                val name = r.scanRecord?.deviceName.orEmpty()
                val services = r.scanRecord?.serviceUuids.orEmpty().map { it.uuid.toString() }
                if (Gatt.scanMatches(name, services, namePrefixes)) {
                    found[r.device.address] = BleDevice(r.device.address, name, r.rssi.toShort())
                }
            }
        }
        val settings = ScanSettings.Builder().setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY).build()
        guard { scanner.startScan(null, settings, callback) }
        failed.await(timeoutMs.toLong(), TimeUnit.MILLISECONDS)
        runCatching { guard { scanner.stopScan(callback) } }
        if (failure != 0) throw BleException.Failed("scan failed with code $failure")
        return found.values.sortedByDescending { it.rssi ?: Short.MIN_VALUE }
    }

    override fun connect(id: String, sink: BleNotifySink): NativeBleLink {
        val adapter = enabledAdapter()
        if (!BluetoothAdapter.checkBluetoothAddress(id)) throw BleException.NotFound("not a Bluetooth address: $id")
        val device = guard { adapter.getRemoteDevice(id) }
        return guard { GattLink.open(context, device, sink) }
    }

    private fun enabledAdapter(): BluetoothAdapter {
        if (!BlePermissions.granted(context)) throw BleException.Unavailable("Bluetooth permission not granted")
        val adapter = manager?.adapter ?: throw BleException.Unavailable("no Bluetooth on this device")
        if (!adapter.isEnabled) throw BleException.Unavailable("Bluetooth is off")
        return adapter
    }
}

/** Turn a revoked permission into the engine's `Unavailable` error. */
internal inline fun <T> guard(block: () -> T): T =
    try {
        block()
    } catch (e: SecurityException) {
        throw BleException.Unavailable("Bluetooth permission: ${e.message}")
    }
