package com.perseusoft.homefi.data

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Whether the TV already asked, once, for "All files access" to write on a USB drive. After the
 * guided screen the answer is final here: allowed, or the app's own folder on the drive.
 */
class UsbAccess(context: Context) {
    private val prefs = context.getSharedPreferences("usb_access", Context.MODE_PRIVATE)
    private val _asked = MutableStateFlow(prefs.getBoolean(KEY, false))
    val asked: StateFlow<Boolean> = _asked.asStateFlow()

    fun markAsked() {
        _asked.value = true
        prefs.edit().putBoolean(KEY, true).apply()
    }

    private companion object {
        const val KEY = "asked"
    }
}
