package com.perseusoft.homefi.data

import android.content.Context
import com.perseusoft.homefi.core.DownloadEvent
import com.perseusoft.homefi.core.DownloadListener
import com.perseusoft.homefi.core.DownloadSettings
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.core.HomefiEngine
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.io.File
import java.util.concurrent.atomic.AtomicLong

data class ActiveTask(
    val task: ULong,
    val title: String,
    val quality: String,
    val downloaded: Long = 0,
    val total: Long? = null,
)

enum class LogKind { DONE, EXISTS, SKIPPED, ERROR }

/** `path` is set for files that exist on disk, so they can be played. */
data class LogEntry(val id: Long, val kind: LogKind, val title: String, val detail: String, val path: String? = null)

data class DownloadJob(val id: Long, val label: String, val links: List<String>)

data class DownloadState(
    val pending: List<DownloadJob> = emptyList(),
    val current: DownloadJob? = null,
    val active: Map<ULong, ActiveTask> = emptyMap(),
    val log: List<LogEntry> = emptyList(),
    val queued: Int = 0,
    val finished: Int = 0,
    /** Jobs that wait for a USB drive that was pulled out (nothing is deleted); they resume when it is back. */
    val paused: List<DownloadJob> = emptyList(),
    /** Name of the drive the paused jobs wait for. */
    val pausedDrive: String? = null,
    /** TV with `tv.downloadTarget = ask`: the job that waits for "Internal or USB?". */
    val choosing: DownloadJob? = null,
) {
    val isBusy: Boolean get() = current != null || pending.isNotEmpty()
}

/** Serial download queue backed by the Rust engine. */
class DownloadRepository(
    private val context: Context,
    private val engine: HomefiEngine,
    private val settings: SettingsStore,
    private val scope: CoroutineScope,
) {
    private val _state = MutableStateFlow(DownloadState())
    val state: StateFlow<DownloadState> = _state.asStateFlow()

    private val jobs = Channel<DownloadJob>(Channel.UNLIMITED)
    private val ids = AtomicLong()
    private var running: Job? = null

    /** The removable volume the running job writes to; pulling it out pauses the job. */
    @Volatile
    private var usbInUse: File? = null
    @Volatile
    private var pauseRequested = false
    private var pausedRoot: File? = null
    private var choice: CompletableDeferred<DownloadTarget?>? = null

    init {
        // a drive pulled out under a download pauses it; plugged back in, it goes on by itself
        scope.launch {
            context.homefi.volumes.volumes.collect { volumes ->
                val inUse = usbInUse
                if (inUse != null && volumes.none { it.root == inUse }) {
                    pauseRequested = true
                    running?.cancel()
                }
                val waitingFor = pausedRoot
                val back = if (waitingFor != null) volumes.any { it.root == waitingFor } else volumes.any { it.removable }
                if (_state.value.paused.isNotEmpty() && back) resume()
            }
        }
        scope.launch {
            for (job in jobs) {
                if (_state.value.pending.none { it.id == job.id }) continue // cancelled
                _state.update { it.copy(pending = it.pending - job, current = job) }
                running = launch { process(job) }
                running?.join()
                _state.update { it.copy(current = null, active = emptyMap()) }
            }
        }
    }

    fun enqueue(label: String, links: List<String>) {
        val job = DownloadJob(ids.incrementAndGet(), label, links)
        _state.update { it.copy(pending = it.pending + job) }
        jobs.trySend(job)
        DownloadService.start(context)
    }

    private fun enqueueAgain(job: DownloadJob) {
        _state.update { it.copy(pending = it.pending + job) }
        jobs.trySend(job)
        DownloadService.start(context)
    }

    fun cancelAll() {
        _state.update { it.copy(pending = emptyList(), paused = emptyList(), pausedDrive = null, choosing = null) }
        pausedRoot = null
        choice?.complete(null)
        running?.cancel()
    }

    /** Drops one job that has not started. */
    fun cancelPending(id: Long) {
        _state.update { state -> state.copy(pending = state.pending.filterNot { it.id == id }) }
    }

    /** Puts the jobs that waited for a drive back in the queue. */
    fun resume() {
        val waiting = _state.value.paused
        pausedRoot = null
        _state.update { it.copy(paused = emptyList(), pausedDrive = null) }
        waiting.forEach(::enqueueAgain)
    }

    /** The answer of the "Internal or USB?" question (`null` = cancel that job). */
    fun choose(target: DownloadTarget?) {
        choice?.complete(target)
    }

    fun clearLog() {
        _state.update { it.copy(log = emptyList(), queued = 0, finished = 0) }
    }

    private fun log(kind: LogKind, title: String, detail: String, path: String? = null) {
        val entry = LogEntry(ids.incrementAndGet(), kind, title, detail, path)
        _state.update { it.copy(log = (listOf(entry) + it.log).take(MAX_LOG)) }
    }

    /** Where [job] writes now; `null` when it waits (drive missing) or the user cancelled the question. */
    private suspend fun destination(job: DownloadJob, s: AppSettings): DownloadDestination.Dir? {
        val internal = Storage.downloadDir(context, s.downloadDir)
        if (context.homefi.deviceForm != DeviceForm.TV) return DownloadDestination.Dir(internal, onUsb = false)
        val monitor = context.homefi.volumes
        val usb = monitor.usb()
        val usbDir = usb?.let { TvStorage.downloadDirOn(context, it, Storage.hasSharedAccess(context)) }
        var target = s.tv.downloadTarget
        if (TvStorage.destination(target, internal, usbDir) == DownloadDestination.Ask) {
            val answer = CompletableDeferred<DownloadTarget?>().also { choice = it }
            _state.update { it.copy(choosing = job) }
            target = try {
                answer.await()
            } finally {
                choice = null
                _state.update { it.copy(choosing = null) }
            } ?: return null
        }
        return when (val d = TvStorage.destination(target, internal, usbDir)) {
            is DownloadDestination.Dir -> d
            else -> {
                pauseJob(job, usb?.root, context.getString(R.string.tv_dl_usb))
                null
            }
        }
    }

    /** Keeps [job] for when the drive is back. */
    private fun pauseJob(job: DownloadJob, root: File?, drive: String) {
        pausedRoot = root ?: pausedRoot
        _state.update { it.copy(paused = it.paused + job, pausedDrive = drive) }
        log(LogKind.SKIPPED, job.label, context.getString(R.string.tv_dl_paused_usb))
    }

    private suspend fun process(job: DownloadJob) {
        val s = settings.current
        val dest = destination(job, s) ?: return
        val usbRoot = if (dest.onUsb) context.homefi.volumes.usb()?.root else null
        if (dest.onUsb) rememberLibraryFolder(dest.dir)
        usbInUse = usbRoot
        pauseRequested = false
        val downloadSettings = DownloadSettings(
            quality = s.quality,
            skipExisting = s.skipExisting,
            threads = s.threads.toUInt(),
            embedCover = s.embedCover,
            embedLyrics = s.embedLyrics,
            includeVideos = s.includeVideos,
            template = s.template.ifBlank { null },
            downloadDir = dest.dir.absolutePath,
            // the core refuses a job when the drive would drop under this; the phone keeps today's behaviour (off)
            minFreeMb = if (context.homefi.deviceForm == DeviceForm.TV) s.tv.minFreeMb.toUInt() else NO_FREE_SPACE_CHECK,
        )
        try {
            engine.download(job.links, downloadSettings, listener)
        } catch (e: CancellationException) {
            // pulled-out drive: the job waits for it (and the rest of the queue with it), nothing is lost
            if (pauseRequested) pauseJob(job, usbRoot, context.getString(R.string.tv_dl_usb))
            else log(LogKind.SKIPPED, job.label, "cancelled")
            throw e
        } catch (e: Exception) {
            log(LogKind.ERROR, job.label, e.userMessage())
        } finally {
            usbInUse = null
        }
    }

    /** A drive's download folder joins the library roots, so what was saved there plays (and is marked unavailable when it is gone). */
    private fun rememberLibraryFolder(dir: File) {
        val path = dir.absolutePath
        if (path !in settings.current.tv.libraryVolumes) settings.updateTv { it.copy(libraryVolumes = it.libraryVolumes + path) }
    }

    private val listener = object : DownloadListener {
        override fun onEvent(event: DownloadEvent) {
            when (event) {
                is DownloadEvent.Queued -> _state.update { it.copy(queued = it.queued + 1) }
                is DownloadEvent.Started -> _state.update {
                    it.copy(active = it.active + (event.task to ActiveTask(event.task, event.title, event.quality)))
                }
                is DownloadEvent.Progress -> _state.update { state ->
                    val task = state.active[event.task] ?: return@update state
                    state.copy(
                        active = state.active + (event.task to task.copy(
                            downloaded = event.downloaded.toLong(),
                            total = event.total?.toLong(),
                        ))
                    )
                }
                is DownloadEvent.Finished -> {
                    _state.update { it.copy(active = it.active - event.task, finished = it.finished + 1) }
                    Storage.scan(context, event.path)
                    log(LogKind.DONE, event.title, "${event.quality} · ${event.relativePath}", event.path)
                }
                is DownloadEvent.Exists -> {
                    _state.update { it.copy(finished = it.finished + 1) }
                    log(LogKind.EXISTS, event.title, event.relativePath, event.path)
                }
                is DownloadEvent.Skipped -> {
                    _state.update { it.copy(finished = it.finished + 1) }
                    log(LogKind.SKIPPED, event.title, event.reason)
                }
                is DownloadEvent.Failed -> log(LogKind.ERROR, event.context, event.message)
            }
        }
    }

    private companion object {
        const val MAX_LOG = 300

        /** `DownloadSettings.minFreeMb` value that turns the core's free-space rule off. */
        val NO_FREE_SPACE_CHECK = 0u
    }
}

