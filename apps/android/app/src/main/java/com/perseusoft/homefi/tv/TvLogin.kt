package com.perseusoft.homefi.tv

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.ui.graphics.graphicsLayer
import com.perseusoft.homefi.ui.theme.LocalMotion
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.HomefiLogo
import com.perseusoft.homefi.ui.login.LoginUiState
import com.perseusoft.homefi.ui.login.LoginViewModel
import com.perseusoft.homefi.ui.login.QrCode

private const val SECONDS_PER_MINUTE = 60

/** The host people type when they do not scan (it is on the QR's page as well). */
internal fun linkHost(url: String): String = url.removePrefix("https://").removePrefix("http://").substringBefore('/')

/** `ABCDEFGH` as `ABCD`, `EFGH`; other lengths stay in one group. */
internal fun codeGroups(code: String): List<String> {
    val clean = code.filter { !it.isWhitespace() && it != '-' && it != '·' }
    return if (clean.length == 8) listOf(clean.take(4), clean.drop(4)) else listOf(clean)
}

/**
 * Sign in to TIDAL from a phone: the personal-use notice first, then the device code big, its
 * QR and the countdown. The device-code flow is the phone's (`LoginViewModel`); the in-app web
 * login is not offered on a TV. Nothing is shown of the session, only the code.
 */
@Composable
fun TvLogin(entryKey: Int, onLoggedIn: () -> Unit, vm: LoginViewModel = viewModel()) {
    val state by vm.state.collectAsStateWithLifecycle()
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val look = LocalTvLook.current
    val needsNotice = !settings.termsAccepted

    if (state is LoginUiState.Done) LaunchedEffect(Unit) { onLoggedIn() }
    // signed in somewhere else (the phone's approval) while the TV was away
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { vm.checkNow() }
    // once the notice is accepted the code comes by itself: nothing to press
    LaunchedEffect(state is LoginUiState.Idle, needsNotice) {
        if (state is LoginUiState.Idle && !needsNotice) vm.start()
    }

    val safe = tvSafePadding()
    Column(Modifier.fillMaxSize().padding(end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr), top = safe.calculateTopPadding(), bottom = safe.calculateBottomPadding())) {
        Row(Modifier.weight(1f).fillMaxSize().padding(start = 12.dp), horizontalArrangement = Arrangement.spacedBy(45.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    HomefiLogo(42.dp)
                    Text(stringResource(R.string.app_name), style = TvType.display.copy(fontSize = 28.sp), color = look.ink)
                }
                Spacer(Modifier.height(16.dp))
                Text(
                    stringResource(if (needsNotice) R.string.tv_login_notice_title else R.string.tv_login_title),
                    style = TvType.display.copy(fontSize = 36.sp, lineHeight = 38.sp),
                    color = look.ink,
                    modifier = Modifier.width(400.dp),
                )
                Spacer(Modifier.height(12.dp))
                when {
                    needsNotice -> NoticeStage(entryKey) { vm.acceptTermsAndStart() }
                    else -> CodeStage(state, entryKey, vm)
                }
            }
            if (!needsNotice) {
                QrCard(state)
            }
        }
        TvKeyHints(
            listOf("OK" to stringResource(R.string.tv_hint_ok_select), "Back" to stringResource(R.string.tv_hint_back_exit)),
            Modifier.padding(start = 12.dp, top = 6.dp),
        )
    }
}

@Composable
private fun NoticeStage(entryKey: Int, onAccept: () -> Unit) {
    val look = LocalTvLook.current
    Column(Modifier.width(430.dp)) {
        TvGlass(Modifier.width(430.dp).height(190.dp)) {
            Text(
                stringResource(R.string.tidal_terms),
                style = TvType.secondary.copy(fontSize = 13.sp, lineHeight = 18.sp),
                color = look.ink2,
                modifier = Modifier.verticalScroll(rememberScrollState()).padding(14.dp),
            )
        }
        Spacer(Modifier.height(16.dp))
        TvPill(stringResource(R.string.tv_login_accept), onAccept, Modifier.tvInitialFocus(key = entryKey).tvLeftToDrawer(), icon = Icons.Rounded.Check, primary = true, height = 40.dp)
    }
}

@Composable
private fun CodeStage(state: LoginUiState, entryKey: Int, vm: LoginViewModel) {
    val look = LocalTvLook.current
    val waiting = state as? LoginUiState.Waiting
    val expired = state is LoginUiState.Error
    val link = waiting?.url?.let(::linkHost) ?: "link.tidal.com"
    Text(
        stringResource(R.string.tv_login_body, link),
        style = TvType.body.copy(fontWeight = FontWeight.Normal, fontSize = 16.sp, lineHeight = 22.sp),
        color = look.ink2,
        modifier = Modifier.width(410.dp),
    )
    Spacer(Modifier.height(18.dp))
    CodeTiles(waiting?.userCode, dimmed = expired || waiting == null)
    Spacer(Modifier.height(20.dp))
    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        TvPill(
            stringResource(R.string.tv_login_new),
            vm::start,
            Modifier.tvInitialFocus(key = entryKey).tvLeftToDrawer(),
            icon = Icons.Rounded.Refresh,
            primary = true,
            height = 40.dp,
        )
        val status = when {
            state is LoginUiState.Error -> state.message.takeIf { it.isNotBlank() } ?: stringResource(R.string.tv_login_expired)
            waiting != null -> stringResource(
                R.string.tv_login_expires,
                "%02d:%02d".format(waiting.secondsLeft / SECONDS_PER_MINUTE, waiting.secondsLeft % SECONDS_PER_MINUTE),
            )
            else -> stringResource(R.string.tv_login_starting)
        }
        Text(status, style = TvType.secondary.copy(fontSize = 13.sp), color = if (expired) look.error else look.ink2, modifier = Modifier.width(420.dp))
    }
}

@Composable
private fun CodeTiles(code: String?, dimmed: Boolean) {
    val look = LocalTvLook.current
    val groups = code?.let(::codeGroups) ?: listOf("", "")
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        groups.forEachIndexed { g, group ->
            if (g > 0) Box(Modifier.size(5.dp).background(look.ink3, RoundedCornerShape(50)))
            val chars = if (group.isEmpty()) List(4) { ' ' } else group.toList()
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                chars.forEach { c ->
                    Box(
                        Modifier
                            .size(width = 38.dp, height = 46.dp)
                            .background(Color.White.copy(alpha = if (dimmed) 0.04f else 0.09f), RoundedCornerShape(12.dp))
                            .border(1.dp, look.accent.copy(alpha = if (dimmed) 0.2f else 0.5f), RoundedCornerShape(12.dp)),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            c.toString().trim(),
                            style = TvType.display.copy(fontSize = 28.sp, fontWeight = FontWeight.ExtraBold),
                            color = if (dimmed) look.ink3 else look.ink,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun QrCard(state: LoginUiState) {
    val look = LocalTvLook.current
    val waiting = state as? LoginUiState.Waiting
    TvGlass(Modifier.width(280.dp), radius = 14.dp) {
        Column(Modifier.padding(22.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(Modifier.size(214.dp), contentAlignment = Alignment.Center) {
                if (waiting != null) {
                    // the QR fades and settles in when the code arrives
                    var revealed by remember { androidx.compose.runtime.mutableStateOf(false) }
                    LaunchedEffect(Unit) { revealed = true }
                    val reveal by animateFloatAsState(if (revealed) 1f else 0f, LocalMotion.current.tween(450), label = "qr")
                    Box(Modifier.graphicsLayer { alpha = reveal; scaleX = 0.94f + 0.06f * reveal; scaleY = 0.94f + 0.06f * reveal }) {
                        QrCode(waiting.url.let { if (it.startsWith("http")) it else "https://$it" }, 198.dp)
                    }
                } else {
                    Box(Modifier.fillMaxSize().background(Color.White.copy(alpha = 0.06f), RoundedCornerShape(12.dp)))
                }
            }
            Spacer(Modifier.height(12.dp))
            Text(stringResource(R.string.tv_login_scan), style = TvType.body.copy(fontWeight = FontWeight.Normal, fontSize = 14.sp), color = look.ink2, textAlign = TextAlign.Center)
        }
    }
}
