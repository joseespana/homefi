package com.perseusoft.homefi.tv.train

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.Icon
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.graphics.graphicsLayer
import com.perseusoft.homefi.ui.theme.LocalMotion
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.exercise.TvClientUi
import com.perseusoft.homefi.tv.LocalTvLook
import com.perseusoft.homefi.tv.TvGlass
import com.perseusoft.homefi.tv.TvPill
import com.perseusoft.homefi.tv.TvScreen
import com.perseusoft.homefi.tv.TvType
import com.perseusoft.homefi.tv.tvInitialFocus
import com.perseusoft.homefi.tv.tvLeftToDrawer
import com.perseusoft.homefi.tv.tvSafePadding
import com.perseusoft.homefi.ui.login.QrCode

private const val CODE_GROUP = 3
private val QR_SIZE = 190.dp

/**
 * The TV's Train section: the phone's ride on this TV, with nothing to type. While it waits it shows a
 * local QR plus a short code (the phone's HomeFi scans it, or the system camera opens HomeFi by the
 * `homefi://tv` link) and opens the phone's ride by itself; phones paired before connect on their own
 * when they start a ride. Typing the address stays as the last resort. The [controller] lives at the
 * TV's root (pairing listens app-wide), so there is one ride client for the whole TV.
 */
@Composable
fun TvTrainSection(controller: TvTrainController, entryKey: Int, modifier: Modifier = Modifier) {
    val ui by controller.client.ui.collectAsStateWithLifecycle()
    val panel by controller.panel.collectAsStateWithLifecycle()
    var typing by remember { mutableStateOf(false) }
    BackHandler(enabled = typing && ui is TvClientUi.Searching) { typing = false }
    // a ride built on this TV (or being built) shows its own flow; "Start a ride here" opens it
    val rideUi by androidx.compose.ui.platform.LocalContext.current.homefi.ride.ui.collectAsStateWithLifecycle()
    var here by remember { mutableStateOf(false) }

    Box(modifier.fillMaxSize()) {
        val state = ui
        if (here || rideUi != com.perseusoft.homefi.player.exercise.RideUi.Setup) {
            TvRideSetup(entryKey, onExit = { here = false })
        } else if (state is TvClientUi.Searching && !typing) {
            WaitingForPhone(panel, entryKey, onType = { typing = true }, onForget = controller::forgetPhones, onHere = { here = true })
        } else {
            // a new ride picked while watching (a phone paired mid-ride) restarts the page
            val rideKey = (state as? TvClientUi.Watching)?.endpoint?.let { "${it.host}:${it.port}" }
            key(rideKey) { TvScreen(controller.client) }
        }
    }
}

@Composable
private fun WaitingForPhone(panel: PairPanel, entryKey: Int, onType: () -> Unit, onForget: () -> Unit, onHere: () -> Unit) {
    val look = LocalTvLook.current
    val safe = tvSafePadding()
    Row(
        Modifier.fillMaxSize().padding(end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr), top = safe.calculateTopPadding(), bottom = safe.calculateBottomPadding(), start = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(36.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.tv_train_title), style = TvType.display, color = look.ink)
            Step(1, stringResource(R.string.tv_train_step1))
            Step(2, stringResource(R.string.tv_train_step2))
            Step(3, stringResource(R.string.tv_train_step3))
            Spacer(Modifier.height(2.dp))
            if (panel.justPaired != null) {
                PairedBadge(stringResource(R.string.tv_train_just_paired, panel.justPaired))
            } else {
                Text(stringResource(R.string.tv_train_looking), style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink2)
            }
            if (panel.phones.isNotEmpty()) {
                Text(stringResource(R.string.tv_train_paired, panel.phones.joinToString { it.name }), style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink2)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.padding(top = 4.dp)) {
                TvPill(stringResource(R.string.tv_train_here), onHere, Modifier.tvInitialFocus(key = entryKey).tvLeftToDrawer(), primary = true, height = 38.dp)
                TvPill(stringResource(R.string.ex_tv_enter_address), onType, height = 38.dp)
                if (panel.phones.isNotEmpty()) TvPill(stringResource(R.string.tv_train_forget), onForget, height = 38.dp)
            }
        }
        QrCard(panel)
    }
}

/** "Paired": a check that pops in with a ripple ring that fades out. */
@Composable
private fun PairedBadge(text: String) {
    val look = LocalTvLook.current
    val motion = LocalMotion.current
    var shown by remember(text) { mutableStateOf(false) }
    LaunchedEffect(text) { shown = true }
    val pop by animateFloatAsState(if (shown) 1f else 0f, if (motion.moves) spring(0.5f, 400f) else motion.tween(200), label = "paired-pop")
    val ripple by animateFloatAsState(if (shown) 1f else 0f, motion.tween(900), label = "paired-ripple")
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.size(40.dp), contentAlignment = Alignment.Center) {
            if (motion.enabled) {
                Box(Modifier.size(40.dp).graphicsLayer { scaleX = 0.6f + ripple * 0.7f; scaleY = 0.6f + ripple * 0.7f; alpha = (1f - ripple) * 0.7f }.background(look.ok, CircleShape))
            }
            Box(Modifier.size(30.dp).graphicsLayer { scaleX = pop; scaleY = pop }.background(look.ok, CircleShape), contentAlignment = Alignment.Center) {
                Icon(Icons.Rounded.Check, null, tint = look.onAccent, modifier = Modifier.size(20.dp))
            }
        }
        Text(text, style = TvType.section, color = look.ok)
    }
}

@Composable
private fun Step(number: Int, text: String) {
    val look = LocalTvLook.current
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.Top) {
        Box(Modifier.size(28.dp).background(look.gradient, CircleShape), contentAlignment = Alignment.Center) {
            Text(number.toString(), style = TvType.body.copy(fontWeight = FontWeight.ExtraBold), color = look.onAccent)
        }
        Text(text, style = TvType.body.copy(fontWeight = FontWeight.Normal, fontSize = 17.sp), color = look.ink, modifier = Modifier.padding(top = 2.dp))
    }
}

@Composable
private fun QrCard(panel: PairPanel) {
    val look = LocalTvLook.current
    TvGlass(Modifier.width(250.dp)) {
        Column(Modifier.padding(20.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val link = panel.link
            if (link != null) {
                QrCode(link, QR_SIZE)
                Text(stringResource(R.string.tv_train_code), style = TvType.secondary, color = look.ink2)
                Text(
                    panel.code.chunked(CODE_GROUP).joinToString(" "),
                    style = TvType.display.copy(fontSize = 34.sp, fontFamily = FontFamily.Monospace),
                    color = look.ink,
                )
                Text("${panel.host}:${panel.port}", style = TvType.secondary, color = look.ink3)
            } else {
                Text(stringResource(R.string.tv_train_no_network), style = TvType.body, color = look.ink2, modifier = Modifier.width(QR_SIZE))
            }
        }
    }
}
