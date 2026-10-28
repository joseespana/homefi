package com.perseusoft.homefi.ui.eq

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.EqBand
import com.perseusoft.homefi.core.EqBandKind
import com.perseusoft.homefi.core.EqProfile
import com.perseusoft.homefi.core.HeadphoneEntry
import com.perseusoft.homefi.core.Tilt
import com.perseusoft.homefi.core.eqWithTaste
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.Equalizer
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Bass / treble / clarity on top of the profile, in dB. */
data class Taste(val bass: Float = 0f, val treble: Float = 0f, val clarity: Float = 0f) {
    val isZero: Boolean get() = bass == 0f && treble == 0f && clarity == 0f
}

data class AutoEqUi(
    val query: String = "",
    val searching: Boolean = false,
    val results: List<HeadphoneEntry>? = null,
    val loading: String? = null,
    val error: String? = null,
)

/**
 * The equalizer screen: which profile is being edited ([target]: an output id, or null for
 * the one all outputs use), the selected band, taste sliders and the AutoEq search. Edits go
 * straight to [Equalizer], so they are heard while dragging.
 */
class EqViewModel(app: Application) : AndroidViewModel(app) {
    private val eq: Equalizer = app.homefi.equalizer
    private val engine = app.homefi.engine

    private val _target = MutableStateFlow(initialTarget())
    val target: StateFlow<String?> = _target.asStateFlow()

    private val _selected = MutableStateFlow<Int?>(null)
    val selected: StateFlow<Int?> = _selected.asStateFlow()

    private val _taste = MutableStateFlow(Taste())
    val taste: StateFlow<Taste> = _taste.asStateFlow()
    /** The profile the taste sliders started from (they are relative to it). */
    private var tasteBase: EqProfile? = null

    private val _autoEq = MutableStateFlow(AutoEqUi())
    val autoEq: StateFlow<AutoEqUi> = _autoEq.asStateFlow()

    /** A short note after an action (an AutoEq profile applied...). */
    private val _notice = MutableStateFlow<String?>(null)
    val notice: StateFlow<String?> = _notice.asStateFlow()
    private var searchJob: Job? = null

    /** The output playing now when it has its own profile, else the shared one. */
    private fun initialTarget(): String? {
        val id = eq.outputId.value
        return id.takeIf { it in eq.settings.value.perOutput }
    }

    /** The screen opened again: back on the output playing now. */
    fun reopen() {
        selectTarget(initialTarget())
        _autoEq.update { it.copy(results = null, error = null) }
        _notice.value = null
    }

    private fun profile(): EqProfile? = eq.profileOf(_target.value).first

    private fun edit(transform: (EqProfile) -> EqProfile) {
        resetTaste()
        eq.editProfile(_target.value, transform)
    }

    private fun resetTaste() {
        tasteBase = null
        _taste.value = Taste()
    }

    fun selectTarget(id: String?) {
        if (id == _target.value) return
        _target.value = id
        _selected.value = null
        resetTaste()
    }

    fun select(index: Int?) {
        _selected.value = index
    }

    fun setEnabled(on: Boolean) = eq.setEnabled(on)

    fun hold(on: Boolean) = eq.hold(on)

    /** A point dragged on the curve: frequency always, gain for bands that have one. */
    fun drag(index: Int, freqHz: Float, gainDb: Float) = updateBand(index) { b ->
        b.copy(
            freqHz = freqHz.coerceIn(20f, 20_000f),
            gainDb = if (b.kind.hasGain()) (Math.round(gainDb * 10) / 10f).coerceIn(-24f, 24f) else b.gainDb,
        )
    }

    fun updateBand(index: Int, transform: (EqBand) -> EqBand) = edit { p ->
        if (index !in p.bands.indices) p else p.copy(bands = p.bands.mapIndexed { i, b -> if (i == index) transform(b) else b })
    }

    fun setKind(index: Int, kind: EqBandKind) = updateBand(index) { it.copy(kind = kind) }

    fun addBand() {
        val bands = profile()?.bands.orEmpty()
        if (bands.size >= Equalizer.MAX_BANDS) return
        edit { p -> p.copy(bands = p.bands + Equalizer.newBand(p.bands)) }
        _selected.value = bands.size
    }

    fun removeBand(index: Int) {
        edit { p -> p.copy(bands = p.bands.filterIndexed { i, _ -> i != index }) }
        _selected.value = null
    }

    fun setPreamp(db: Float) = edit { it.copy(preampDb = (Math.round(db * 2) / 2f).coerceIn(-24f, 6f)) }

    fun setAutoPreamp(on: Boolean) = eq.update { it.copy(autoPreamp = on) }

    fun applyPreset(p: EqProfile) {
        resetTaste()
        eq.setProfile(_target.value, p)
        _selected.value = null
    }

    fun flat() = applyPreset(Equalizer.flat())

    fun savePreset(name: String) {
        val p = profile() ?: Equalizer.flat()
        eq.savePreset(name, p)
    }

    fun deletePreset(name: String) = eq.deletePreset(name)

    fun useDefault() {
        val id = _target.value ?: return
        resetTaste()
        eq.useDefault(id)
        _selected.value = null
    }

    /** Taste sliders: always applied to the profile as it was when they started moving. */
    fun setTaste(t: Taste) {
        val base = tasteBase ?: (profile() ?: Equalizer.flat()).also { tasteBase = it }
        _taste.value = t
        val result = if (t.isZero) base else eqWithTaste(base, Tilt(t.bass, t.treble, t.clarity.takeIf { it != 0f }))
        eq.setProfile(_target.value, result)
    }

    fun setQuery(q: String) = _autoEq.update { it.copy(query = q) }

    fun search() {
        val q = _autoEq.value.query.trim()
        if (q.isEmpty()) return
        searchJob?.cancel()
        _autoEq.update { it.copy(searching = true, error = null) }
        searchJob = viewModelScope.launch {
            try {
                val found = engine.autoeqSearch(q)
                _autoEq.update { it.copy(searching = false, results = found.take(30)) }
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
                _autoEq.update { it.copy(searching = false, error = e.userMessage()) }
            }
        }
    }

    fun pick(entry: HeadphoneEntry) {
        _autoEq.update { it.copy(loading = entry.path, error = null) }
        viewModelScope.launch {
            try {
                val p = engine.autoeqProfile(entry)
                applyPreset(p.copy(name = p.name.ifBlank { entry.name }))
                if (!eq.settings.value.enabled) eq.setEnabled(true)
                _autoEq.update { it.copy(loading = null, results = null) }
                _notice.value = getApplication<Application>().getString(R.string.eq_autoeq_applied, entry.name)
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
                _autoEq.update { it.copy(loading = null, error = e.userMessage()) }
            }
        }
    }

    fun clearNotice() {
        _notice.value = null
    }

    override fun onCleared() {
        eq.hold(false)
    }
}
