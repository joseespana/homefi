package com.perseusoft.homefi.data

import android.content.Context
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.DjSetupSummary
import com.perseusoft.homefi.core.DjSetups
import com.perseusoft.homefi.core.djSetupExtension
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The DJ setups saved in HomeFi's own folder (`<app dir>/dj/setups`, the core's store): the options plus
 * the screen layout under a name. Applying one sets [SettingsStore.dj] and the layout; the setup files
 * are the same ones the desktop writes, so one can be exported on a phone and imported on a computer.
 */
class DjSetupsStore(
    private val context: Context,
    appDir: String,
    private val settings: SettingsStore,
    private val scope: CoroutineScope,
    private val notes: Notes,
) {
    private val store = DjSetups(appDir)

    private val _list = MutableStateFlow<List<DjSetupSummary>>(emptyList())

    /** By name. */
    val list: StateFlow<List<DjSetupSummary>> = _list.asStateFlow()

    fun refresh() {
        scope.launch { reload() }
    }

    private suspend fun reload() {
        _list.value = withContext(Dispatchers.IO) { runCatching { store.list() }.getOrDefault(emptyList()) }
    }

    /** Saves the options and layout in use as a new setup called [name]. */
    fun saveCurrent(name: String) = launchTask {
        val saved = withContext(Dispatchers.IO) { store.save(name, settings.dj.value, settings.current.djLayout()) }
        notes.post(context.getString(R.string.djs_saved, saved.name))
    }

    /** Replaces setup [id] with the options and layout in use. */
    fun overwrite(id: String) = launchTask {
        val saved = withContext(Dispatchers.IO) { store.overwrite(id, settings.dj.value, settings.current.djLayout()) }
        notes.post(context.getString(R.string.djs_updated, saved.name))
    }

    fun apply(id: String) = launchTask {
        val applied = withContext(Dispatchers.IO) { store.apply(id) }
        settings.setDj(applied.settings)
        settings.setDjLayout(applied.layout)
        notes.post(context.getString(R.string.djs_applied, _list.value.firstOrNull { it.id == id }?.name.orEmpty()))
    }

    fun rename(id: String, name: String) = launchTask { withContext(Dispatchers.IO) { store.rename(id, name) } }

    fun delete(id: String) = launchTask { withContext(Dispatchers.IO) { store.delete(id) } }

    /** The `.homefidj` text of a setup, or null (with a message). */
    suspend fun export(id: String): String? = guarded { withContext(Dispatchers.IO) { store.export(id) } }

    /** Adds a setup from `.homefidj` text as a new one; says what happened. */
    suspend fun import(json: String): DjSetupSummary? {
        val summary = guarded { withContext(Dispatchers.IO) { store.import(json) } } ?: return null
        reload()
        notes.post(context.getString(R.string.djs_imported, summary.name))
        return summary
    }

    private fun launchTask(block: suspend () -> Unit) {
        scope.launch {
            guarded(block)
            reload()
        }
    }

    private suspend fun <T> guarded(block: suspend () -> T): T? = try {
        block()
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        notes.post(e.userMessage())
        null
    }

    companion object {
        /** `homefidj`, from the core. */
        val extension: String by lazy { djSetupExtension() }
    }
}
