package org.labely.app.ble

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothStatusCodes
import android.content.Context
import android.os.Build
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock
import org.labely.ffi.BleException
import org.labely.ffi.BleNotifySink
import org.labely.ffi.NativeBleLink

/**
 * One connected printer. Android allows one outstanding GATT operation per connection, so every
 * operation goes through [op]: it holds the queue lock, starts the operation and blocks until
 * its callback arrives (ADR-0027).
 */
@SuppressLint("MissingPermission") // AndroidBle checked before connecting; guard() maps a revoked one.
internal class GattLink private constructor(
    private val device: BluetoothDevice,
    private val sink: BleNotifySink,
) : NativeBleLink {
    private enum class Op { CONNECT, DISCOVER, MTU, DESCRIPTOR, WRITE }

    private class Pending(val op: Op) {
        val done = CountDownLatch(1)

        @Volatile var status = STATUS_UNSET
    }

    private val queue = ReentrantLock()

    @Volatile private var pending: Pending? = null

    @Volatile private var gatt: BluetoothGatt? = null

    @Volatile private var payload = DEFAULT_PAYLOAD

    @Volatile private var opened = false

    @Volatile private var closed = false

    private lateinit var characteristic: BluetoothGattCharacteristic

    private val callback = object : BluetoothGattCallback() {
        override fun onConnectionStateChange(g: BluetoothGatt, status: Int, newState: Int) {
            if (newState == BluetoothProfile.STATE_CONNECTED) {
                complete(Op.CONNECT, status)
            } else if (newState == BluetoothProfile.STATE_DISCONNECTED) {
                pending?.let { it.status = STATUS_DISCONNECTED; it.done.countDown() }
                val notify = opened && !closed
                closed = true
                runCatching { guard { g.close() } }
                if (notify) sink.disconnected()
            }
        }

        override fun onServicesDiscovered(g: BluetoothGatt, status: Int) = complete(Op.DISCOVER, status)

        override fun onMtuChanged(g: BluetoothGatt, mtu: Int, status: Int) {
            if (status == BluetoothGatt.GATT_SUCCESS) payload = mtu - ATT_HEADER
            complete(Op.MTU, status)
        }

        override fun onDescriptorWrite(g: BluetoothGatt, d: BluetoothGattDescriptor, status: Int) =
            complete(Op.DESCRIPTOR, status)

        override fun onCharacteristicWrite(g: BluetoothGatt, c: BluetoothGattCharacteristic, status: Int) =
            complete(Op.WRITE, status)

        override fun onCharacteristicChanged(g: BluetoothGatt, c: BluetoothGattCharacteristic, value: ByteArray) =
            sink.push(value)

        @Deprecated("Called instead of the three-argument overload before API 33")
        override fun onCharacteristicChanged(g: BluetoothGatt, c: BluetoothGattCharacteristic) {
            @Suppress("DEPRECATION")
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) sink.push(c.value ?: return)
        }
    }

    private fun complete(op: Op, status: Int) {
        val p = pending ?: return
        if (p.op != op) return
        p.status = status
        p.done.countDown()
    }

    /** Start one GATT operation and wait for its callback; returns the GATT status. */
    private fun op(op: Op, timeoutMs: Long, start: () -> Boolean): Int = queue.withLock {
        if (closed && op != Op.CONNECT) throw BleException.NotFound("printer disconnected")
        val p = Pending(op)
        pending = p
        try {
            if (!guard(start)) throw BleException.Failed("${op.name.lowercase()} could not start")
            if (!p.done.await(timeoutMs, TimeUnit.MILLISECONDS)) {
                throw BleException.Failed("${op.name.lowercase()} timed out")
            }
            if (p.status == STATUS_DISCONNECTED) throw BleException.NotFound("printer disconnected")
            p.status
        } finally {
            pending = null
        }
    }

    private fun open(context: Context) {
        val connected = op(Op.CONNECT, CONNECT_TIMEOUT_MS) {
            gatt = device.connectGatt(context, false, callback, BluetoothDevice.TRANSPORT_LE)
            gatt != null
        }
        val g = gatt ?: throw BleException.Failed("connectGatt returned null")
        if (connected != BluetoothGatt.GATT_SUCCESS) throw BleException.Failed("connect failed with GATT status $connected")
        check(op(Op.DISCOVER, OP_TIMEOUT_MS) { g.discoverServices() }, "service discovery")
        characteristic = Gatt.pick(g.services)
            ?: throw BleException.NotFound("${device.address} has no NIIMBOT printer service")
        // A larger MTU is an optimisation: on failure the default 20-byte payload stays.
        runCatching { op(Op.MTU, OP_TIMEOUT_MS) { g.requestMtu(REQUESTED_MTU) } }
        enableNotifications(g)
        opened = true
    }

    private fun enableNotifications(g: BluetoothGatt) {
        val c = characteristic
        if (!g.setCharacteristicNotification(c, true)) throw BleException.Failed("could not enable notifications")
        val cccd = c.getDescriptor(UUID.fromString(Gatt.CCCD)) ?: return
        val value =
            if (c.properties and BluetoothGattCharacteristic.PROPERTY_NOTIFY != 0) {
                BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
            } else {
                BluetoothGattDescriptor.ENABLE_INDICATION_VALUE
            }
        val status = op(Op.DESCRIPTOR, OP_TIMEOUT_MS) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                g.writeDescriptor(cccd, value) == BluetoothStatusCodes.SUCCESS
            } else {
                @Suppress("DEPRECATION")
                cccd.value = value
                @Suppress("DEPRECATION")
                g.writeDescriptor(cccd)
            }
        }
        check(status, "enabling notifications")
    }

    override fun write(bytes: ByteArray) {
        val g = gatt ?: throw BleException.NotFound("printer disconnected")
        val status = op(Op.WRITE, OP_TIMEOUT_MS) { startWrite(g, bytes) }
        check(status, "write")
    }

    /** Start a write-without-response, retrying while the controller's buffer is full. */
    private fun startWrite(g: BluetoothGatt, bytes: ByteArray): Boolean {
        repeat(BUSY_RETRIES) {
            val started =
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    val code = g.writeCharacteristic(characteristic, bytes, BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE)
                    if (code == BluetoothStatusCodes.ERROR_GATT_WRITE_REQUEST_BUSY) null else code == BluetoothStatusCodes.SUCCESS
                } else {
                    @Suppress("DEPRECATION")
                    characteristic.writeType = BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE
                    @Suppress("DEPRECATION")
                    characteristic.value = bytes
                    @Suppress("DEPRECATION")
                    if (g.writeCharacteristic(characteristic)) true else null
                }
            if (started != null) return started
            Thread.sleep(BUSY_WAIT_MS)
        }
        return false
    }

    override fun mtu(): UInt = payload.coerceAtLeast(DEFAULT_PAYLOAD).toUInt()

    override fun name(): String = runCatching { guard { device.name } }.getOrNull() ?: device.address

    override fun disconnect() {
        closed = true
        val g = gatt ?: return
        runCatching {
            guard {
                g.disconnect()
                g.close()
            }
        }
        pending?.let { it.status = STATUS_DISCONNECTED; it.done.countDown() }
    }

    private fun check(status: Int, what: String) {
        if (status != BluetoothGatt.GATT_SUCCESS) throw BleException.Failed("$what failed with GATT status $status")
    }

    companion object {
        private const val STATUS_UNSET = Int.MIN_VALUE
        private const val STATUS_DISCONNECTED = Int.MIN_VALUE + 1
        private const val ATT_HEADER = 3
        private const val DEFAULT_PAYLOAD = 20
        private const val REQUESTED_MTU = 247
        private const val CONNECT_TIMEOUT_MS = 15_000L
        private const val OP_TIMEOUT_MS = 5_000L
        private const val BUSY_RETRIES = 200
        private const val BUSY_WAIT_MS = 5L

        /** Connect, find the printer characteristic, raise the MTU and subscribe. */
        fun open(context: Context, device: BluetoothDevice, sink: BleNotifySink): GattLink {
            val link = GattLink(device, sink)
            try {
                link.open(context)
            } catch (e: Throwable) {
                link.disconnect()
                throw e
            }
            return link
        }
    }
}
