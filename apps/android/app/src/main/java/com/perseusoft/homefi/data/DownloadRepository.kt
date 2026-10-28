package com.perseusoft.homefi.data

import android.content.Context
import com.perseusoft.homefi.core.DownloadEvent
import com.perseusoft.homefi.core.DownloadListener
import com.perseusoft.homefi.core.DownloadSettings
import com.perseusoft.homefi.core.HomefiEngine
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
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

    init {
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

    fun cancelAll() {
        _state.update { it.copy(pending = emptyList()) }
        running?.cancel()
    }

    fun clearLog() {
        _state.update { it.copy(log = emptyList(), queued = 0, finished = 0) }
    }

    private fun log(kind: LogKind, title: String, detail: String, path: String? = null) {
        val entry = LogEntry(ids.incrementAndGet(), kind, title, detail, path)
        _state.update { it.copy(log = (listOf(entry) + it.log).take(MAX_LOG)) }
    }

    private suspend fun process(job: DownloadJob) {
        val s = settings.current
        val downloadSettings = DownloadSettings(
            quality = s.quality,
            skipExisting = s.skipExisting,
            threads = s.threads.toUInt(),
            embedCover = s.embedCover,
            embedLyrics = s.embedLyrics,
            includeVideos = s.includeVideos,
            template = s.template.ifBlank { null },
            downloadDir = Storage.downloadDir(context, s.downloadDir).absolutePath,
        )
        try {
            engine.download(job.links, downloadSettings, listener)
        } catch (e: CancellationException) {
            log(LogKind.SKIPPED, job.label, "cancelled")
            throw e
        } catch (e: Exception) {
            log(LogKind.ERROR, job.label, e.userMessage())
        }
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
    }
}

