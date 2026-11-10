package com.perseusoft.homefi.tv

import android.annotation.SuppressLint
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.RideTvEndpoint
import com.perseusoft.homefi.player.exercise.RideTvClient
import com.perseusoft.homefi.player.exercise.TvClientUi
import com.perseusoft.homefi.player.exercise.pageUrl
import kotlinx.coroutines.delay

private const val AUTO_OPEN_MS = 2_000L
private const val DEFINE_POSITION = "window.homefiSongPositionMs=function(){return HomefiTv.songPositionMs();};"
private const val UNDEFINE_POSITION = "delete window.homefiSongPositionMs;"

@Composable
fun TvScreen(client: RideTvClient) {
    val ui by client.ui.collectAsStateWithLifecycle()
    var typing by remember { mutableStateOf(false) }
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
        when (val state = ui) {
            is TvClientUi.Watching -> RideWeb(state, client)
            is TvClientUi.Choose -> Choose(state.rides, client::pick) { typing = true }
            TvClientUi.Searching -> Waiting(client.savedAddress(), onType = { typing = true }, onOpen = { client.openAddress(it) })
        }
    }
    if (typing && ui !is TvClientUi.Watching) {
        AddressDialog(client.savedAddress().orEmpty(), onDismiss = { typing = false }) { text ->
            client.openAddress(text).also { if (it) typing = false }
        }
    }
}

@Composable
private fun Waiting(saved: String?, onType: () -> Unit, onOpen: (String) -> Unit) {
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Text(stringResource(R.string.app_name), fontSize = 36.sp, color = MaterialTheme.colorScheme.primary)
            Text(stringResource(R.string.ex_tv_waiting), fontSize = 56.sp)
            Text(stringResource(R.string.ex_tv_waiting_hint), fontSize = 22.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(stringResource(R.string.ex_tv_blocked_hint), fontSize = 20.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Column(Modifier.width(560.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (saved != null) RideRow(stringResource(R.string.ex_tv_open_saved, saved), Modifier.focusRequester(first)) { onOpen(saved) }
                RideRow(stringResource(R.string.ex_tv_enter_address), if (saved == null) Modifier.focusRequester(first) else Modifier, onType)
            }
        }
    }
}

/** "Enter the address": the phone shows it on its ride screen. The TV's keyboard opens on the field. */
@Composable
private fun AddressDialog(initial: String, onDismiss: () -> Unit, onOpen: (String) -> Boolean) {
    var text by remember { mutableStateOf(initial) }
    var bad by remember { mutableStateOf(false) }
    val field = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { field.requestFocus() } }
    androidx.compose.material3.AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.ex_tv_enter_address)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.ex_tv_address_hint), color = MaterialTheme.colorScheme.onSurfaceVariant)
                androidx.compose.material3.OutlinedTextField(
                    value = text,
                    onValueChange = { text = it; bad = false },
                    singleLine = true,
                    isError = bad,
                    supportingText = { if (bad) Text(stringResource(R.string.ex_tv_address_bad)) },
                    placeholder = { Text("192.168.1.20:47801") },
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(
                        keyboardType = androidx.compose.ui.text.input.KeyboardType.Uri,
                        imeAction = androidx.compose.ui.text.input.ImeAction.Go,
                    ),
                    keyboardActions = androidx.compose.foundation.text.KeyboardActions(onGo = { bad = !onOpen(text) }),
                    modifier = Modifier.fillMaxWidth().focusRequester(field),
                )
            }
        },
        confirmButton = {
            androidx.compose.material3.TextButton(onClick = { bad = !onOpen(text) }) { Text(stringResource(R.string.ex_tv_open)) }
        },
        dismissButton = {
            androidx.compose.material3.TextButton(onClick = onDismiss) { Text(stringResource(R.string.ex_tv_cancel)) }
        },
    )
}

/** Rides found: one opens by itself after a moment; several are picked with the D-pad. */
@Composable
private fun Choose(rides: List<RideTvEndpoint>, onPick: (RideTvEndpoint) -> Unit, onType: () -> Unit) {
    val only = rides.singleOrNull()
    LaunchedEffect(only?.host, only?.port) {
        if (only != null) {
            delay(AUTO_OPEN_MS)
            onPick(only)
        }
    }
    Column(Modifier.fillMaxSize().padding(64.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Text(stringResource(R.string.ex_tv_rides), fontSize = 40.sp)
        if (only != null) {
            Text(stringResource(R.string.ex_tv_starting, only.name), fontSize = 28.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        val first = remember { FocusRequester() }
        LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
        rides.forEachIndexed { i, ride ->
            RideRow(ride.name, if (i == 0) Modifier.focusRequester(first) else Modifier) { onPick(ride) }
        }
        RideRow(stringResource(R.string.ex_tv_enter_address), Modifier, onType)
    }
}

@Composable
private fun RideRow(name: String, modifier: Modifier, onClick: () -> Unit) {
    var focused by remember { mutableStateOf(false) }
    Text(
        name,
        fontSize = 32.sp,
        modifier = modifier
            .fillMaxWidth()
            .onFocusChanged { focused = it.isFocused }
            .onKeyEvent { e ->
                val select = e.key == Key.DirectionCenter || e.key == Key.Enter || e.key == Key.NumPadEnter
                if (select && e.type == KeyEventType.KeyUp) { onClick(); true } else false
            }
            .focusable()
            .border(if (focused) 6.dp else 2.dp, if (focused) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surface, RoundedCornerShape(16.dp))
            .padding(24.dp),
    )
}

/**
 * The ride, drawn by the core's own page (the single TV renderer). When the music plays here the page
 * asks this app where its playback is (`window.homefiSongPositionMs`) so the pulse follows what is heard.
 */
@Composable
private fun RideWeb(state: TvClientUi.Watching, client: RideTvClient) {
    BackHandler(onBack = client::stopWatching)
    RideWebView(state.endpoint.pageUrl, state.listening, client::songPositionMs)
}

/**
 * The WebView on the core's ride page at [pageUrl]. With [followSong] the page asks [songPositionMs] (this
 * app's playback, -1 when unknown) so the pulse follows what is heard. Shared by the phone's ride watched
 * here and by a ride run on this TV (a local address).
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun RideWebView(pageUrl: String, followSong: Boolean, songPositionMs: () -> Long) {
    val view = LocalView.current
    DisposableEffect(Unit) {
        view.keepScreenOn = true
        onDispose { view.keepScreenOn = false }
    }
    val position by androidx.compose.runtime.rememberUpdatedState(songPositionMs)
    var web by remember { mutableStateOf<WebView?>(null) }
    LaunchedEffect(web, followSong) {
        web?.evaluateJavascript(if (followSong) DEFINE_POSITION else UNDEFINE_POSITION, null)
    }
    AndroidView(
        modifier = Modifier.fillMaxSize(),
        factory = { context ->
            WebView(context).apply {
                // match_parent, not AndroidView's default wrap_content: a wrap-content WebView sizes
                // its layout viewport to the page, so the page's 100vh became 0 and the TV stayed black
                layoutParams = android.view.ViewGroup.LayoutParams(
                    android.view.ViewGroup.LayoutParams.MATCH_PARENT,
                    android.view.ViewGroup.LayoutParams.MATCH_PARENT,
                )
                setBackgroundColor(android.graphics.Color.BLACK)
                settings.javaScriptEnabled = true
                settings.mediaPlaybackRequiresUserGesture = false
                addJavascriptInterface(object {
                    @JavascriptInterface
                    fun songPositionMs(): Double = position().toDouble()
                }, "HomefiTv")
                webViewClient = object : WebViewClient() {
                    override fun onPageFinished(view: WebView, url: String?) {
                        if (position() >= 0) view.evaluateJavascript(DEFINE_POSITION, null)
                    }
                }
                loadUrl(pageUrl)
                web = this
            }
        },
        onRelease = { it.destroy() },
    )
}
