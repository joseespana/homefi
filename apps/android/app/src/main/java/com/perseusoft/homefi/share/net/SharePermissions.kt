package com.perseusoft.homefi.share.net

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import com.perseusoft.homefi.HomefiApplication

/**
 * Runtime permissions Wi-Fi Direct needs (creating/joining a group, finding hubs nearby).
 * The LAN (NSD) path needs none of these.
 *
 * - API 33+: NEARBY_WIFI_DEVICES (declared `neverForLocation`, so no location prompt).
 * - API < 33: ACCESS_FINE_LOCATION (and location services switched on). Coarse is asked
 *   along with it because Android 12 ignores a fine-only request; "approximate" alone is
 *   not enough for Wi-Fi Direct, so [granted] checks fine.
 *
 * Bluetooth LE discovery (the `share.bluetooth` setting) is asked in the same request:
 * - API 31+: BLUETOOTH_SCAN (`neverForLocation`), BLUETOOTH_ADVERTISE, BLUETOOTH_CONNECT.
 * - API < 31: BLUETOOTH / BLUETOOTH_ADMIN are granted at install. Advertising (the hub)
 *   needs nothing more; scanning (guests) needs the location permission above and
 *   location services on.
 */
object SharePermissions {
    fun needed(): Array<String> = wifi() + bluetooth()

    /** Wi-Fi Direct is usable (Bluetooth is checked apart, by [bluetoothGranted]). */
    fun granted(context: Context): Boolean {
        val required = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            Manifest.permission.NEARBY_WIFI_DEVICES
        } else {
            Manifest.permission.ACCESS_FINE_LOCATION
        }
        return has(context, required)
    }

    /** The Bluetooth runtime permissions (API 31+) are granted; always true below. */
    fun bluetoothGranted(context: Context): Boolean = bluetooth().all { has(context, it) }

    /** Whether the share settings want Bluetooth discovery (default on). */
    internal fun bluetoothWanted(context: Context): Boolean =
        runCatching { (context.applicationContext as? HomefiApplication)?.settings?.current?.share?.bluetooth }
            .getOrNull() ?: true

    /** What the request asks for: Wi-Fi Direct always, Bluetooth when the setting is on. */
    internal fun toAsk(context: Context): Array<String> =
        if (bluetoothWanted(context)) needed() else wifi()

    /** Wi-Fi Direct usable and, when wanted, Bluetooth too: no need to ask. */
    internal fun allGranted(context: Context): Boolean =
        granted(context) && (!bluetoothWanted(context) || bluetoothGranted(context))

    private fun wifi(): Array<String> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            arrayOf(Manifest.permission.NEARBY_WIFI_DEVICES)
        } else {
            arrayOf(Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_COARSE_LOCATION)
        }

    private fun bluetooth(): Array<String> =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            arrayOf(
                Manifest.permission.BLUETOOTH_SCAN,
                Manifest.permission.BLUETOOTH_ADVERTISE,
                Manifest.permission.BLUETOOTH_CONNECT,
            )
        } else {
            emptyArray()
        }

    private fun has(context: Context, permission: String) =
        ContextCompat.checkSelfPermission(context, permission) == PackageManager.PERMISSION_GRANTED
}

/**
 * Returns a function that asks for [SharePermissions.needed] (the Bluetooth ones only while
 * the Bluetooth share setting is on); [onResult] gets whether Wi-Fi Direct is usable
 * ([SharePermissions.granted]), right away without a dialog when nothing is missing.
 * Denying only Bluetooth still reports true: Bluetooth discovery is then skipped.
 */
@Composable
fun rememberSharePermissionRequest(onResult: (granted: Boolean) -> Unit): () -> Unit {
    val context = LocalContext.current
    val callback by rememberUpdatedState(onResult)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
        callback(SharePermissions.granted(context))
    }
    return remember(context, launcher) {
        {
            if (SharePermissions.allGranted(context)) {
                callback(SharePermissions.granted(context))
            } else {
                launcher.launch(SharePermissions.toAsk(context))
            }
        }
    }
}
