package com.perseusoft.homefi.tv

import android.content.Intent
import android.os.Bundle
import android.view.KeyEvent
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.tv.train.TvTrainController
import kotlinx.coroutines.Job
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch

private const val MS_PER_MINUTE = 60_000L

/**
 * HomeFi's entry on Android TV / Google TV (the `LEANBACK_LAUNCHER`; on a TV `MainActivity`
 * forwards its intents here). It hosts [TvRoot], consumes the intents the phone app handles
 * (open player, Tidal links, audio files), keeps the screen on while music plays and gives the
 * remote's fast-forward / rewind keys their seek. Media keys (play, pause, next, previous) reach
 * `PlaybackService` through its session without any code here.
 */
class TvActivity : ComponentActivity() {
    companion object {
        const val EXTRA_SECTION = "homefi.section"
    }

    private val idle = TvIdle()
    private val playerRequests = Channel<Unit>(Channel.CONFLATED)
    private val sectionRequests = Channel<TvSection>(Channel.CONFLATED)
    /** The one ride client and pairing endpoint of the TV: lives while the app is in front, whatever the section. */
    private val train by lazy { TvTrainController(homefi) }
    private val intents by lazy { TvIntents(this, lifecycleScope) }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // a phone must never stay here (a restored task, an old shortcut, an `am start`): back to the touch app
        if (homefi.deviceForm.mustHandOver(activityIsTv = true)) {
            startActivity(
                Intent(intent).setClass(this, com.perseusoft.homefi.MainActivity::class.java)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK),
            )
            finishAndRemoveTask()
            return
        }
        enableEdgeToEdge()
        setContent { TvRoot(idle, train, playerRequests.receiveAsFlow(), sectionRequests.receiveAsFlow(), onExit = ::finish) }
        keepScreenOnWhilePlaying()
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                train.start()
                try {
                    awaitCancellation()
                } finally {
                    train.stop()
                }
            }
        }
        if (savedInstanceState == null) handle(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    private fun handle(intent: Intent?) {
        if (intents.handle(intent)) playerRequests.trySend(Unit)
        // a shortcut or a test opens a section directly: `--es homefi.section LIBRARY`
        intent?.getStringExtra(EXTRA_SECTION)?.let { name -> TvSection.entries.firstOrNull { it.name.equals(name, true) } }?.let(sectionRequests::trySend)
    }

    /** Ambient mode swallows the key that wakes it; every other key just restarts the idle timer. */
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (idle.onKey(event.keyCode, event.action == KeyEvent.ACTION_DOWN)) return true
        return super.dispatchKeyEvent(event)
    }

    /** Fast-forward and rewind seek by `tv.dpadSeekSeconds` wherever the user is. */
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        val direction = when (keyCode) {
            KeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> 1
            KeyEvent.KEYCODE_MEDIA_REWIND -> -1
            else -> return super.onKeyDown(keyCode, event)
        }
        val app = homefi
        val state = app.player.state.value
        if (state.current != null) {
            val seconds = app.settings.current.tv.dpadSeekSeconds * direction
            app.player.seekTo(seekTarget(state.positionMs, state.durationMs, seconds))
        }
        return true
    }

    /**
     * `tv.keepAwakeWhilePlaying`: the screen stays on while music plays; after pausing it may
     * sleep once `tv.awakeAfterPauseMinutes` have passed (so the system screensaver can take over).
     */
    private fun keepScreenOnWhilePlaying() {
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                var sleepLater: Job? = null
                combine(homefi.player.state, homefi.settings.settings) { player, settings ->
                    player.isPlaying to settings.tv
                }.distinctUntilChanged().collect { (playing, tv) ->
                    sleepLater?.cancel()
                    if (tv.keepAwakeWhilePlaying && playing) {
                        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                    } else {
                        // paused (or the setting is off): a grace period, then the system may sleep the screen
                        val grace = if (tv.keepAwakeWhilePlaying) tv.awakeAfterPauseMinutes * MS_PER_MINUTE else 0L
                        sleepLater = launch {
                            delay(grace)
                            window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                        }
                    }
                }
            }
        }
    }
}
