package com.perseusoft.homefi.player

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioFormat
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

enum class OutputKind { USB, WIRED, BLUETOOTH, SPEAKER, HDMI, LINE, OTHER }

/** An audio output the user can pick (USB DAC, headphone jack, Bluetooth, ...). */
data class AudioOutput(
    /** Stable across reconnects: `type:product name`. */
    val key: String,
    val name: String,
    val kind: OutputKind,
    /** Empty when the device accepts any rate the mixer sends. */
    val sampleRates: List<Int>,
    val bitDepths: List<Int>,
    val maxChannels: Int?,
) {
    /**
     * The equalizer's id for this output (the key of its per-output profiles): one per route
     * type, and per product for USB DACs and Bluetooth (`usb:<name>`, `bt:<name>`), so the
     * profile follows the headphones rather than the port.
     */
    val eqId: String get() = eqIdOf(kind, name)
}

/** See [AudioOutput.eqId]. */
fun eqIdOf(kind: OutputKind, name: String): String = when (kind) {
    OutputKind.USB -> if (name.isBlank()) "usb" else "usb:${name.trim()}"
    OutputKind.BLUETOOTH -> if (name.isBlank()) "bt" else "bt:${name.trim()}"
    OutputKind.WIRED -> "wired"
    OutputKind.SPEAKER -> "speaker"
    OutputKind.HDMI -> "hdmi"
    OutputKind.LINE -> "line"
    OutputKind.OTHER -> "other"
}

/**
 * Tracks the connected outputs and the one the user prefers. The preference is
 * kept by key, so a DAC that is unplugged and plugged back in is used again;
 * while it is missing, playback follows the system route.
 */
class AudioOutputs(context: Context) {
    private val audio = context.getSystemService(AudioManager::class.java)
    private val prefs = context.getSharedPreferences("audio_output", Context.MODE_PRIVATE)

    private val _outputs = MutableStateFlow<List<AudioOutput>>(emptyList())
    val outputs: StateFlow<List<AudioOutput>> = _outputs.asStateFlow()

    private val _preferredKey = MutableStateFlow(prefs.getString(KEY, null))
    /** `null` = automatic (system routing). */
    val preferredKey: StateFlow<String?> = _preferredKey.asStateFlow()

    private val _preferredDevice = MutableStateFlow<AudioDeviceInfo?>(null)
    /** The connected device to route to, or `null` for the system default. */
    val preferredDevice: StateFlow<AudioDeviceInfo?> = _preferredDevice.asStateFlow()

    private val _current = MutableStateFlow<AudioOutput?>(null)
    /**
     * Where playback goes now (best guess before Android 13): the preferred output when it is
     * connected, else the system route. The equalizer picks its profile from this.
     */
    val current: StateFlow<AudioOutput?> = _current.asStateFlow()

    private var devices: Map<String, AudioDeviceInfo> = emptyMap()

    init {
        audio.registerAudioDeviceCallback(object : AudioDeviceCallback() {
            override fun onAudioDevicesAdded(added: Array<out AudioDeviceInfo>) = refresh()
            override fun onAudioDevicesRemoved(removed: Array<out AudioDeviceInfo>) = refresh()
        }, Handler(Looper.getMainLooper()))
        refresh()
    }

    fun select(key: String?) {
        prefs.edit().putString(KEY, key).apply()
        _preferredKey.value = key
        _preferredDevice.value = key?.let(devices::get)
        updateCurrent()
    }

    /** Where media audio goes right now when no preference applies (Android 13+). */
    fun systemRoute(): AudioOutput? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return null
        val attrs = AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).build()
        return audio.getAudioDevicesForAttributes(attrs).firstOrNull()?.let(::toOutput)
    }

    private fun refresh() {
        val found = audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
            .filter { kindOf(it.type) != null && it.isSink }
            .associateBy(::keyOf)
        devices = found
        _outputs.value = found.values.mapNotNull(::toOutput).sortedBy { it.kind.ordinal }
        _preferredDevice.value = _preferredKey.value?.let(found::get)
        updateCurrent()
    }

    private fun updateCurrent() {
        val list = _outputs.value
        _current.value = list.firstOrNull { it.key == _preferredKey.value }
            ?: systemRoute()
            // before Android 13: a plugged-in DAC, jack or headset wins over the speaker
            ?: list.firstOrNull()
    }

    private fun keyOf(d: AudioDeviceInfo) = "${d.type}:${d.productName}"

    private fun toOutput(d: AudioDeviceInfo): AudioOutput? {
        val kind = kindOf(d.type) ?: return null
        return AudioOutput(
            key = keyOf(d),
            name = d.productName?.toString()?.takeIf { it.isNotBlank() && kind != OutputKind.SPEAKER }.orEmpty(),
            kind = kind,
            sampleRates = d.sampleRates.distinct().sorted(),
            bitDepths = d.encodings.toList().mapNotNull(::bitsOf).distinct().sorted(),
            maxChannels = d.channelCounts.maxOrNull(),
        )
    }

    companion object {
        private const val KEY = "preferred"

        private fun kindOf(type: Int): OutputKind? = when (type) {
            AudioDeviceInfo.TYPE_USB_DEVICE, AudioDeviceInfo.TYPE_USB_HEADSET, AudioDeviceInfo.TYPE_USB_ACCESSORY -> OutputKind.USB
            AudioDeviceInfo.TYPE_WIRED_HEADPHONES, AudioDeviceInfo.TYPE_WIRED_HEADSET -> OutputKind.WIRED
            AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, AudioDeviceInfo.TYPE_BLE_HEADSET, AudioDeviceInfo.TYPE_BLE_SPEAKER,
            AudioDeviceInfo.TYPE_BLE_BROADCAST -> OutputKind.BLUETOOTH
            AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> OutputKind.SPEAKER
            AudioDeviceInfo.TYPE_HDMI, AudioDeviceInfo.TYPE_HDMI_ARC, AudioDeviceInfo.TYPE_HDMI_EARC -> OutputKind.HDMI
            AudioDeviceInfo.TYPE_LINE_ANALOG, AudioDeviceInfo.TYPE_LINE_DIGITAL, AudioDeviceInfo.TYPE_AUX_LINE,
            AudioDeviceInfo.TYPE_DOCK -> OutputKind.LINE
            else -> null // earpiece, telephony, SCO (calls), virtual devices
        }

        private fun bitsOf(encoding: Int): Int? = when (encoding) {
            AudioFormat.ENCODING_PCM_16BIT -> 16
            AudioFormat.ENCODING_PCM_24BIT_PACKED -> 24
            AudioFormat.ENCODING_PCM_32BIT, AudioFormat.ENCODING_PCM_FLOAT -> 32
            else -> null
        }
    }
}
