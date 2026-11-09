package com.perseusoft.homefi.tv

import android.content.Context
import android.hardware.input.InputManager
import android.view.InputDevice
import androidx.annotation.StringRes
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.QueueMusic
import androidx.compose.material.icons.rounded.Album
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.DirectionsBike
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Groups
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.TvDjEntry

/** The TV app's sections, in the drawer's order (docs/TV_UI.md section 4). */
enum class TvSection(@StringRes val label: Int, val icon: ImageVector) {
    HOME(R.string.tv_nav_home, Icons.Rounded.Home),
    LIBRARY(R.string.tv_nav_library, Icons.Rounded.LibraryMusic),
    SEARCH(R.string.tv_nav_search, Icons.Rounded.Search),
    TIDAL(R.string.tv_nav_tidal, Icons.Rounded.Cloud),
    DOWNLOADS(R.string.tv_nav_downloads, Icons.Rounded.Download),
    PLAYLISTS(R.string.tv_nav_playlists, Icons.AutoMirrored.Rounded.QueueMusic),
    SOUND(R.string.tv_nav_sound, Icons.Rounded.Tune),
    PARTY(R.string.tv_nav_party, Icons.Rounded.Groups),
    TRAIN(R.string.tv_nav_train, Icons.Rounded.DirectionsBike),
    DJ(R.string.tv_nav_dj, Icons.Rounded.Album),
    SETTINGS(R.string.tv_nav_settings, Icons.Rounded.Settings),
}

/** The sections the drawer lists now: TIDAL waits for the sign-in, DJ mode obeys `tv.djEntry`. */
fun visibleSections(signedIn: Boolean, djEntry: TvDjEntry): List<TvSection> =
    TvSection.entries.filter { s ->
        when (s) {
            TvSection.TIDAL -> signedIn
            TvSection.DJ -> djEntry != TvDjEntry.OFF
            else -> true
        }
    }

/** DJ mode is usable: always with `ON`, with `AUTO` only while a mouse is connected. */
fun djAvailable(djEntry: TvDjEntry, mouse: Boolean): Boolean = when (djEntry) {
    TvDjEntry.ON -> true
    TvDjEntry.AUTO -> mouse
    TvDjEntry.OFF -> false
}

private fun mouseConnected(): Boolean = InputDevice.getDeviceIds().any { id ->
    InputDevice.getDevice(id)?.let { it.sources and InputDevice.SOURCE_MOUSE == InputDevice.SOURCE_MOUSE } == true
}

/** Whether a mouse is plugged in (USB or Bluetooth), kept up to date while the screen is composed. */
@Composable
fun rememberMouseConnected(): Boolean {
    val context = LocalContext.current
    var connected by remember { mutableStateOf(mouseConnected()) }
    DisposableEffect(context) {
        val manager = context.getSystemService(Context.INPUT_SERVICE) as InputManager
        val listener = object : InputManager.InputDeviceListener {
            override fun onInputDeviceAdded(deviceId: Int) { connected = mouseConnected() }
            override fun onInputDeviceRemoved(deviceId: Int) { connected = mouseConnected() }
            override fun onInputDeviceChanged(deviceId: Int) { connected = mouseConnected() }
        }
        manager.registerInputDeviceListener(listener, null)
        onDispose { manager.unregisterInputDeviceListener(listener) }
    }
    return connected
}
