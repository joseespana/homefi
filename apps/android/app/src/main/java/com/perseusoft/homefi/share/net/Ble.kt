package com.perseusoft.homefi.share.net

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.BluetoothLeAdvertiser
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.os.Build
import android.os.ParcelUuid
import android.os.SystemClock
import android.util.Log
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONObject
import java.util.UUID
import java.util.concurrent.atomic.AtomicBoolean

// Bluetooth LE discovery for shared control / party mode. Bluetooth only finds the hub and
// hands over how to join it; audio and control stay on Wi-Fi (Direct or the home network).
//
// Hub: connectable advertisement with [BLE_SERVICE] (plus a short session name as service
// data in the scan response) and a GATT server whose one read characteristic, [BLE_RECORD],
// holds the [BleRecord]. Advertising and the GATT server need no location services, which is
// the point for players without GPS (Astell&Kern SR35) below API 33.
// Guest: scans with a filter on [BLE_SERVICE], connects, reads the record once per device
// (cached), disconnects.

internal val BLE_SERVICE: UUID = UUID.fromString("e0a13854-8b89-4800-ac61-657c9da009fe")
internal val BLE_RECORD: UUID = UUID.fromString("e0a13855-8b89-4800-ac61-657c9da009fe")

private const val ANONYMOUS_MAC = "02:00:00:00:00:00"

/**
 * What the hub hands over by Bluetooth (UTF-8 JSON in [BLE_RECORD]). Never the join code.
 *
 * `{"v":1,"name":"…","port":47800,"mode":"wifi_direct"|"lan","net":"DIRECT-hf-…","pass":"…",
 * "lan":"192.168.1.20","dev":"aa:bb:cc:dd:ee:ff"}`; net/pass/lan/dev only when known.
 */
internal data class BleRecord(
    val name: String,
    val port: Int,
    val mode: ShareNetworkMode,
    /** Wi-Fi Direct group name and passphrase. */
    val networkName: String?,
    val passphrase: String?,
    /** The hub's address on the home Wi-Fi (LAN mode, or a Wi-Fi Direct hub also on the LAN). */
    val lanHost: String?,
    /** The hub's Wi-Fi Direct device address, for guests below API 29 (they connect by device). */
    val deviceAddress: String?,
) {
    fun encode(): ByteArray = JSONObject().apply {
        put("v", 1)
        put("name", fitBytes(name, 200)) // same cut as the Wi-Fi Direct TXT: same key
        put("port", port)
        put("mode", if (mode == ShareNetworkMode.LAN) "lan" else "wifi_direct")
        networkName?.let { put("net", it) }
        passphrase?.let { put("pass", it) }
        lanHost?.let { put("lan", it) }
        deviceAddress?.let { put("dev", it) }
    }.toString().toByteArray(Charsets.UTF_8)

    companion object {
        fun decode(bytes: ByteArray): BleRecord? = runCatching {
            val json = JSONObject(String(bytes, Charsets.UTF_8))
            fun text(key: String) = json.optString(key, "").trim().takeIf { it.isNotEmpty() }
            BleRecord(
                name = text("name") ?: return null,
                port = json.optInt("port", 0).takeIf { it in 1..65535 } ?: return null,
                mode = if (text("mode") == "lan") ShareNetworkMode.LAN else ShareNetworkMode.WIFI_DIRECT,
                networkName = text("net"),
                passphrase = text("pass"),
                lanHost = text("lan"),
                deviceAddress = text("dev")?.lowercase()?.takeIf { it != ANONYMOUS_MAC },
            )
        }.onFailure { Log.d(TAG, "BLE record unreadable", it) }.getOrNull()
    }
}

/** Bluetooth availability and permissions; every check fails soft (false, never throws). */
internal object Ble {
    fun adapter(context: Context): BluetoothAdapter? {
        if (!context.packageManager.hasSystemFeature(PackageManager.FEATURE_BLUETOOTH_LE)) return null
        return runCatching { (context.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager?)?.adapter }
            .getOrNull()
    }

    /** Bluetooth is on (never switched on by us). */
    fun enabled(context: Context): Boolean = runCatching { adapter(context)?.isEnabled == true }.getOrDefault(false)

    private fun has(context: Context, permission: String) =
        ContextCompat.checkSelfPermission(context, permission) == PackageManager.PERMISSION_GRANTED

    /** Advertising + GATT server: no location needed on any API level. */
    fun canAdvertise(context: Context): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            has(context, Manifest.permission.BLUETOOTH_ADVERTISE) && has(context, Manifest.permission.BLUETOOTH_CONNECT)
        } else {
            @Suppress("DEPRECATION")
            has(context, Manifest.permission.BLUETOOTH) && has(context, Manifest.permission.BLUETOOTH_ADMIN)
        }

    /**
     * Scanning + reading the record. API 31+: BLUETOOTH_SCAN (neverForLocation) and CONNECT.
     * Below: location permission (fine on 29-30) and location services on, or no results.
     */
    fun canScan(context: Context, locationEnabled: Boolean): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            has(context, Manifest.permission.BLUETOOTH_SCAN) && has(context, Manifest.permission.BLUETOOTH_CONNECT)
        } else {
            @Suppress("DEPRECATION")
            val bt = has(context, Manifest.permission.BLUETOOTH) && has(context, Manifest.permission.BLUETOOTH_ADMIN)
            val location = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                has(context, Manifest.permission.ACCESS_FINE_LOCATION)
            } else {
                has(context, Manifest.permission.ACCESS_FINE_LOCATION) || has(context, Manifest.permission.ACCESS_COARSE_LOCATION)
            }
            bt && location && locationEnabled
        }
}

// --- hub ----------------------------------------------------------------------------------

/**
 * Advertises the hub and serves its [BleRecord]. Main thread only (the GATT server's read
 * callback runs on a binder thread and only reads [payload]). Restarts by itself when
 * Bluetooth is switched back on while announcing.
 */
@SuppressLint("MissingPermission")
internal class BleAnnouncer(context: Context) {
    private val app = context.applicationContext

    private var record: BleRecord? = null

    @Volatile private var payload: ByteArray = ByteArray(0)

    @Volatile private var server: BluetoothGattServer? = null
    private var advertiser: BluetoothLeAdvertiser? = null
    private var advertising: AdvertiseCallback? = null
    private var stateReceiver: BroadcastReceiver? = null

    fun start(record: BleRecord) {
        this.record = record
        payload = record.encode()
        watchState()
        stopAdvertising()
        startNow()
    }

    fun stop() {
        stateReceiver?.let { runCatching { app.unregisterReceiver(it) } }
        stateReceiver = null
        record = null
        stopNow()
    }

    private fun startNow() {
        val current = record ?: return
        val adapter = Ble.adapter(app) ?: return skip("no Bluetooth LE here")
        if (!Ble.canAdvertise(app)) return skip("no permission to advertise")
        if (!Ble.enabled(app)) return skip("Bluetooth is off")
        val leAdvertiser = runCatching { adapter.bluetoothLeAdvertiser }.getOrNull()
            ?: return skip("this device cannot advertise (no peripheral mode)")
        if (server == null && !openServer()) return
        advertiser = leAdvertiser
        advertise(leAdvertiser, current.name, withName = true)
    }

    private fun skip(why: String) {
        Log.i(TAG, "BLE: $why, not announcing")
    }

    private fun openServer(): Boolean {
        val manager = app.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager? ?: return false
        val callback = object : BluetoothGattServerCallback() {
            override fun onCharacteristicReadRequest(
                device: BluetoothDevice,
                requestId: Int,
                offset: Int,
                characteristic: BluetoothGattCharacteristic,
            ) {
                val gatt = server ?: return
                val data = payload
                runCatching {
                    when {
                        characteristic.uuid != BLE_RECORD ->
                            gatt.sendResponse(device, requestId, BluetoothGatt.GATT_READ_NOT_PERMITTED, offset, null)
                        offset > data.size ->
                            gatt.sendResponse(device, requestId, BluetoothGatt.GATT_INVALID_OFFSET, offset, null)
                        // Longer than the MTU: the stack truncates and the client reads on by offset.
                        else -> gatt.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, data.copyOfRange(offset, data.size))
                    }
                }.onFailure { Log.d(TAG, "BLE: read response failed", it) }
            }
        }
        val opened = try {
            manager.openGattServer(app, callback)
        } catch (e: SecurityException) {
            Log.w(TAG, "BLE: GATT server permission", e)
            null
        } catch (e: RuntimeException) {
            Log.w(TAG, "BLE: GATT server", e)
            null
        } ?: return false.also { Log.i(TAG, "BLE: could not open the GATT server") }
        val service = BluetoothGattService(BLE_SERVICE, BluetoothGattService.SERVICE_TYPE_PRIMARY).apply {
            addCharacteristic(
                BluetoothGattCharacteristic(
                    BLE_RECORD,
                    BluetoothGattCharacteristic.PROPERTY_READ,
                    BluetoothGattCharacteristic.PERMISSION_READ,
                ),
            )
        }
        val added = runCatching { opened.addService(service) }.getOrDefault(false)
        if (!added) {
            Log.i(TAG, "BLE: could not add the GATT service")
            runCatching { opened.close() }
            return false
        }
        server = opened
        return true
    }

    private fun advertise(leAdvertiser: BluetoothLeAdvertiser, name: String, withName: Boolean) {
        val settings = AdvertiseSettings.Builder()
            .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_BALANCED)
            .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM)
            .setConnectable(true)
            .setTimeout(0)
            .build()
        // Flags (3) + 128-bit UUID (18) = 21 of the 31 legacy bytes.
        val data = AdvertiseData.Builder()
            .setIncludeDeviceName(false)
            .setIncludeTxPowerLevel(false)
            .addServiceUuid(ParcelUuid(BLE_SERVICE))
            .build()
        // Service data with the same UUID: 2 + 16 bytes of header leave 13 for the name.
        val response = if (withName) {
            AdvertiseData.Builder()
                .setIncludeDeviceName(false)
                .setIncludeTxPowerLevel(false)
                .addServiceData(ParcelUuid(BLE_SERVICE), fitBytes(name, SHORT_NAME_BYTES).toByteArray(Charsets.UTF_8))
                .build()
        } else {
            null
        }
        val callback = object : AdvertiseCallback() {
            override fun onStartSuccess(settingsInEffect: AdvertiseSettings) {
                Log.d(TAG, "BLE: advertising")
            }

            override fun onStartFailure(errorCode: Int) {
                Log.w(TAG, "BLE: advertising failed: $errorCode")
                // Older stacks reject 128-bit service data: advertise the UUID alone.
                if (withName && errorCode != ADVERTISE_FAILED_ALREADY_STARTED) {
                    android.os.Handler(android.os.Looper.getMainLooper()).post {
                        if (advertising === this && record != null) {
                            advertising = null
                            advertise(leAdvertiser, name, withName = false)
                        }
                    }
                }
            }
        }
        try {
            if (response != null) {
                leAdvertiser.startAdvertising(settings, data, response, callback)
            } else {
                leAdvertiser.startAdvertising(settings, data, callback)
            }
            advertising = callback
        } catch (e: SecurityException) {
            Log.w(TAG, "BLE: advertise permission", e)
        } catch (e: RuntimeException) {
            Log.w(TAG, "BLE: advertise", e)
        }
    }

    private fun stopAdvertising() {
        val callback = advertising ?: return
        advertising = null
        runCatching { advertiser?.stopAdvertising(callback) }
    }

    private fun stopNow() {
        stopAdvertising()
        advertiser = null
        server?.let { gatt -> runCatching { gatt.clearServices() }; runCatching { gatt.close() } }
        server = null
    }

    /** Bluetooth switched off: drop everything; back on: start again (only while announcing). */
    private fun watchState() {
        if (stateReceiver != null) return
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                when (intent.getIntExtra(BluetoothAdapter.EXTRA_STATE, BluetoothAdapter.ERROR)) {
                    BluetoothAdapter.STATE_ON -> if (record != null) {
                        stopNow()
                        startNow()
                    }
                    BluetoothAdapter.STATE_TURNING_OFF, BluetoothAdapter.STATE_OFF -> stopNow()
                }
            }
        }
        runCatching {
            ContextCompat.registerReceiver(
                app, receiver, IntentFilter(BluetoothAdapter.ACTION_STATE_CHANGED), ContextCompat.RECEIVER_EXPORTED,
            )
            stateReceiver = receiver
        }.onFailure { Log.d(TAG, "BLE: state receiver", it) }
    }
}

// --- guest --------------------------------------------------------------------------------

/**
 * Scans for hubs and reads their [BleRecord] (one GATT read at a time, cached per Bluetooth
 * address for [RECORD_TTL_MS]). [onHub] runs on the main thread, at most every
 * [REPORT_EVERY_MS] per hub, while the hub keeps advertising. Main thread only.
 */
@SuppressLint("MissingPermission")
internal class BleScanner(
    context: Context,
    private val scope: CoroutineScope,
    private val onHub: (BleRecord) -> Unit,
) {
    private val app = context.applicationContext

    private class Cached(
        var record: BleRecord? = null,
        var shortName: String? = null,
        var readAt: Long = 0,
        var failedAt: Long = 0,
        var failures: Int = 0,
        var reportedAt: Long = 0,
    )

    private val cache = HashMap<String, Cached>()
    private val queue = ArrayDeque<BluetoothDevice>()
    private var reading = false
    private var callback: ScanCallback? = null
    private var scanner: android.bluetooth.le.BluetoothLeScanner? = null

    val scanning: Boolean get() = callback != null

    /** Starts scanning when possible and not already; otherwise logs why and does nothing. */
    fun ensure(locationEnabled: Boolean) {
        if (!Ble.enabled(app)) {
            // Bluetooth went off: the scan died with it.
            if (callback != null) Log.i(TAG, "BLE: Bluetooth is off, scan stopped")
            callback = null
            scanner = null
            return
        }
        if (callback != null) return
        val adapter = Ble.adapter(app) ?: return
        if (!Ble.canScan(app, locationEnabled)) {
            Log.i(TAG, "BLE: missing permission or location off, not scanning")
            return
        }
        val leScanner = runCatching { adapter.bluetoothLeScanner }.getOrNull() ?: return
        val cb = object : ScanCallback() {
            override fun onScanResult(callbackType: Int, result: ScanResult) {
                scope.launch { onResult(result) }
            }

            override fun onBatchScanResults(results: MutableList<ScanResult>) {
                scope.launch { results.forEach(::onResult) }
            }

            override fun onScanFailed(errorCode: Int) {
                Log.w(TAG, "BLE: scan failed: $errorCode")
                val self = this
                scope.launch { if (callback === self) callback = null }
            }
        }
        val filters = listOf(ScanFilter.Builder().setServiceUuid(ParcelUuid(BLE_SERVICE)).build())
        val settings = ScanSettings.Builder().setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY).build()
        try {
            leScanner.startScan(filters, settings, cb)
            callback = cb
            scanner = leScanner
            Log.d(TAG, "BLE: scanning")
        } catch (e: SecurityException) {
            Log.w(TAG, "BLE: scan permission", e)
        } catch (e: RuntimeException) {
            Log.w(TAG, "BLE: scan", e)
        }
    }

    /** Stops scanning (the record cache is kept). */
    fun stop() {
        val cb = callback
        callback = null
        if (cb != null && Ble.enabled(app)) runCatching { scanner?.stopScan(cb) }
        scanner = null
        queue.clear()
    }

    private fun onResult(result: ScanResult) {
        if (callback == null) return
        val device = result.device ?: return
        val address = device.address ?: return
        val shortName = result.scanRecord?.getServiceData(ParcelUuid(BLE_SERVICE))
            ?.let { String(it, Charsets.UTF_8) }
        val now = SystemClock.elapsedRealtime()
        val entry = cache.getOrPut(address) { Cached() }
        val record = entry.record
        val fresh = record != null && now - entry.readAt < RECORD_TTL_MS &&
            (shortName == null || entry.shortName == null || shortName == entry.shortName)
        if (fresh) {
            if (now - entry.reportedAt >= REPORT_EVERY_MS) {
                entry.reportedAt = now
                onHub(record!!)
            }
            return
        }
        // Keep reporting the old record while it is re-read (it is only a minute old).
        if (record != null && now - entry.reportedAt >= REPORT_EVERY_MS && now - entry.readAt < RECORD_TTL_MS * 2) {
            entry.reportedAt = now
            onHub(record)
        }
        if (entry.failedAt != 0L && now - entry.failedAt < retryDelay(entry.failures)) return
        if (queue.none { it.address == address }) queue.addLast(device)
        entry.shortName = shortName
        readNext()
    }

    private fun readNext() {
        if (reading || callback == null) return
        val device = queue.removeFirstOrNull() ?: return
        reading = true
        scope.launch {
            val bytes = try {
                read(device)
            } finally {
                reading = false
            }
            val entry = cache.getOrPut(device.address) { Cached() }
            val now = SystemClock.elapsedRealtime()
            val record = bytes?.let(BleRecord::decode)
            if (record != null) {
                entry.record = record
                entry.readAt = now
                entry.failedAt = 0
                entry.failures = 0
                entry.reportedAt = now
                if (callback != null) onHub(record)
            } else {
                entry.failedAt = now
                entry.failures++
            }
            readNext()
        }
    }

    /** Connects, reads [BLE_RECORD] and disconnects; null on any failure. */
    private suspend fun read(device: BluetoothDevice): ByteArray? {
        val result = CompletableDeferred<ByteArray?>()
        val discovering = AtomicBoolean(false)
        val callback = object : BluetoothGattCallback() {
            override fun onConnectionStateChange(gatt: BluetoothGatt, status: Int, newState: Int) {
                if (status == BluetoothGatt.GATT_SUCCESS && newState == BluetoothProfile.STATE_CONNECTED) {
                    // A bigger MTU reads the record in one go; discovery follows either way.
                    val asked = runCatching { gatt.requestMtu(MTU) }.getOrDefault(false)
                    if (!asked) discover(gatt)
                } else {
                    result.complete(null)
                }
            }

            override fun onMtuChanged(gatt: BluetoothGatt, mtu: Int, status: Int) {
                discover(gatt)
            }

            private fun discover(gatt: BluetoothGatt) {
                if (!discovering.compareAndSet(false, true)) return
                if (!runCatching { gatt.discoverServices() }.getOrDefault(false)) result.complete(null)
            }

            override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
                val characteristic = gatt.getService(BLE_SERVICE)?.getCharacteristic(BLE_RECORD)
                if (status != BluetoothGatt.GATT_SUCCESS || characteristic == null ||
                    !runCatching { gatt.readCharacteristic(characteristic) }.getOrDefault(false)
                ) {
                    result.complete(null)
                }
            }

            override fun onCharacteristicRead(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
                value: ByteArray,
                status: Int,
            ) {
                result.complete(value.takeIf { status == BluetoothGatt.GATT_SUCCESS })
            }

            @Deprecated("API < 33")
            @Suppress("DEPRECATION")
            override fun onCharacteristicRead(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic, status: Int) {
                result.complete(characteristic.value.takeIf { status == BluetoothGatt.GATT_SUCCESS })
            }
        }
        var gatt: BluetoothGatt? = null
        return try {
            gatt = device.connectGatt(app, false, callback, BluetoothDevice.TRANSPORT_LE) ?: return null
            withTimeoutOrNull(GATT_TIMEOUT_MS) { result.await() }
                .also { if (it == null) Log.d(TAG, "BLE: no record from ${device.address}") }
        } catch (e: SecurityException) {
            Log.w(TAG, "BLE: connect permission", e)
            null
        } catch (e: RuntimeException) {
            Log.w(TAG, "BLE: connect", e)
            null
        } finally {
            gatt?.let { runCatching { it.disconnect() }; runCatching { it.close() } }
        }
    }

    private fun retryDelay(failures: Int): Long = (RETRY_MS * failures.coerceIn(1, 6))

    private companion object {
        const val RECORD_TTL_MS = 90_000L
        const val REPORT_EVERY_MS = 3_000L
        const val GATT_TIMEOUT_MS = 12_000L
        const val RETRY_MS = 10_000L
        const val MTU = 247
    }
}

/** Bytes of the session name in the scan response (31 - 2 - 16, with a byte to spare). */
private const val SHORT_NAME_BYTES = 12
