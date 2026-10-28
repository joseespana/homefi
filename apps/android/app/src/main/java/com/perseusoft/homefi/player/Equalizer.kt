package com.perseusoft.homefi.player

import com.perseusoft.homefi.core.EqBand
import com.perseusoft.homefi.core.EqBandKind
import com.perseusoft.homefi.core.EqProfile
import com.perseusoft.homefi.core.EqSettings
import com.perseusoft.homefi.core.EqSource
import com.perseusoft.homefi.core.eqClamp
import com.perseusoft.homefi.data.SettingsStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/** The full-screen panels of the equalizer, over the rest of the app. */
enum class EqPanel { EQUALIZER, ASSISTANT }

/** The equalizer screen's tabs: the profile per output, and the taste rules with loudness. */
enum class EqTab { EQUALIZER, TASTE }

/**
 * The equalizer as the app sees it: the settings (saved shortly after each change, so a
 * band dragged at 60 fps doesn't write 60 times), the output whose profile applies, the
 * "hold to hear without" switch, saved presets, and which panel is open. [SoundLayers]
 * combines it with the taste rules and loudness for the audio threads.
 */
class Equalizer(private val store: SettingsStore, outputs: AudioOutputs, private val scope: CoroutineScope) {
    val settings: StateFlow<EqSettings> = store.eq
    val presets: StateFlow<Map<String, EqProfile>> = store.eqPresets

    /** The output playing now (its [AudioOutput.eqId]). */
    val outputId: StateFlow<String> = outputs.current
        .map { it?.eqId ?: SPEAKER }
        .stateIn(scope, SharingStarted.Eagerly, outputs.current.value?.eqId ?: SPEAKER)

    private val _held = MutableStateFlow(false)
    val held: StateFlow<Boolean> = _held.asStateFlow()

    private val _active = MutableStateFlow(false)
    /** Filters run on the current track (equalizer, taste layer or loudness): not bit-perfect. Set by [SoundLayers]. */
    val active: StateFlow<Boolean> = _active.asStateFlow()

    internal fun publishActive(on: Boolean) {
        _active.value = on
    }

    private val _tab = MutableStateFlow(EqTab.EQUALIZER)
    val tab: StateFlow<EqTab> = _tab.asStateFlow()

    fun selectTab(tab: EqTab) {
        _tab.value = tab
    }

    private val _panel = MutableStateFlow<EqPanel?>(null)
    val panel: StateFlow<EqPanel?> = _panel.asStateFlow()

    private var saveJob: Job? = null

    /** The assistant was opened from the equalizer screen, so back returns there. */
    private var assistantFromEq = false

    /** Opens [panel]; the equalizer on [tab] when given (else where it was). */
    fun open(panel: EqPanel, tab: EqTab? = null) {
        if (panel == EqPanel.ASSISTANT) assistantFromEq = _panel.value == EqPanel.EQUALIZER
        if (tab != null) _tab.value = tab
        _panel.value = panel
    }

    /** Back from the open panel: the assistant returns to the equalizer it came from. */
    fun back() {
        _panel.value = if (_panel.value == EqPanel.ASSISTANT && assistantFromEq) EqPanel.EQUALIZER else null
    }

    fun close() {
        _panel.value = null
    }

    fun set(next: EqSettings) {
        store.setEq(next)
        saveJob?.cancel()
        saveJob = scope.launch {
            delay(SAVE_DELAY_MS)
            store.saveEq()
        }
    }

    fun update(transform: (EqSettings) -> EqSettings) = set(transform(settings.value))

    fun setEnabled(on: Boolean) = update { it.copy(enabled = on) }

    /** "Hold to hear without EQ". */
    fun hold(on: Boolean) {
        _held.value = on
    }

    /** The profile [target] uses (null = the default one), and whether it is its own. */
    fun profileOf(target: String?, s: EqSettings = settings.value): Pair<EqProfile?, Boolean> = when (target) {
        null -> s.defaultProfile to true
        else -> s.perOutput[target]?.let { it to true } ?: (s.defaultProfile to false)
    }

    /**
     * Changes the profile of [target] (null = default). An output that used the default
     * gets its own copy first.
     */
    fun editProfile(target: String?, transform: (EqProfile) -> EqProfile) = update { s ->
        val base = profileOf(target, s).first ?: flat()
        val edited = transform(base)
        if (target == null) s.copy(defaultProfile = edited) else s.copy(perOutput = s.perOutput + (target to edited))
    }

    /** Replaces the profile of [target] (an AutoEq or assistant result, a preset). */
    fun setProfile(target: String?, profile: EqProfile) = editProfile(target) { eqClamp(profile) }

    /** [target] goes back to the default profile. */
    fun useDefault(target: String) = update { it.copy(perOutput = it.perOutput - target) }

    /** The assistant's confirmed proposal: saved for its output and the equalizer on. */
    fun applyProposal(outputId: String, profile: EqProfile) = update { s ->
        val p = eqClamp(profile)
        if (outputId.isBlank()) {
            s.copy(enabled = true, defaultProfile = p)
        } else {
            s.copy(enabled = true, perOutput = s.perOutput + (outputId to p))
        }
    }

    fun savePreset(name: String, profile: EqProfile) {
        val key = name.trim().ifEmpty { return }
        store.setEqPresets(presets.value + (key to profile.copy(name = key)))
    }

    fun deletePreset(name: String) = store.setEqPresets(presets.value - name)

    companion object {
        const val SPEAKER = "speaker"
        const val MAX_BANDS = 10
        private const val SAVE_DELAY_MS = 400L

        fun flat(name: String = "") = EqProfile(name = name, preampDb = 0f, bands = emptyList(), source = EqSource.Manual)

        /** A new band where there's room: the middle of the widest gap, on a log scale. */
        fun newBand(bands: List<EqBand>): EqBand {
            val freqs = (listOf(20f, 20_000f) + bands.map { it.freqHz }).sorted()
            var best = 1_000f
            var widest = 0.0
            for (i in 0 until freqs.lastIndex) {
                val gap = kotlin.math.ln(freqs[i + 1].toDouble() / freqs[i])
                if (gap > widest) {
                    widest = gap
                    best = kotlin.math.sqrt(freqs[i].toDouble() * freqs[i + 1]).toFloat()
                }
            }
            return EqBand(kind = EqBandKind.PEAK, freqHz = best.coerceIn(20f, 20_000f), gainDb = 0f, q = 1f, enabled = true)
        }
    }
}
