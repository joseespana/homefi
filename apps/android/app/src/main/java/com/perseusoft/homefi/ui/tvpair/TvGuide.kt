package com.perseusoft.homefi.ui.tvpair

import com.perseusoft.homefi.ui.ButtonSpinner
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Cast
import androidx.compose.material.icons.rounded.Language
import androidx.compose.material.icons.rounded.QrCodeScanner
import androidx.compose.material.icons.rounded.Tv
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi

private val MIN_TARGET = 48.dp
private val OPTION_ICON = 28.dp

/** The kinds of TV a person can name without knowing what an IP is. */
private enum class TvKind { ANDROID, OTHER, CAST }

/**
 * "Connect the TV": one button for everyone. It opens [TvGuideDialog], which asks what TV it is and walks
 * through it in plain words. [address] is what a TV browser types (only while the ride's TV page runs);
 * [onPrepare] starts that page now (during a ride), null when it can't.
 */
@Composable
fun ConnectTvButton(
    modifier: Modifier = Modifier,
    label: String = stringResource(R.string.tv_connect),
    address: String? = null,
    onPrepare: (() -> Unit)? = null,
) {
    var open by rememberSaveable { mutableStateOf(false) }
    val connecting by LocalContext.current.homefi.tvPairings.connecting.collectAsStateWithLifecycle()
    FilledTonalButton(onClick = { open = true }, enabled = !connecting, modifier = modifier.heightIn(min = MIN_TARGET)) {
        // the TV is being paired (the guide closed after the scan): a spinner until it answers
        if (connecting) ButtonSpinner(size = 18.dp) else Icon(Icons.Rounded.Tv, null, Modifier.size(18.dp))
        Text(label, Modifier.padding(start = 8.dp))
    }
    if (open) TvGuideDialog(address, onPrepare) { open = false }
}

/** The guide: "What TV do you have?", then the steps for that TV. */
@Composable
fun TvGuideDialog(address: String?, onPrepare: (() -> Unit)?, onDismiss: () -> Unit) {
    var kind by rememberSaveable { mutableStateOf<TvKind?>(null) }
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        BackHandler(enabled = kind != null) { kind = null }
        Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
            Column(
                Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (kind != null) {
                        IconButton(onClick = { kind = null }) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.tv_guide_back)) }
                    }
                    Text(stringResource(R.string.tv_guide_title), style = MaterialTheme.typography.headlineSmall, modifier = Modifier.weight(1f))
                    TextButton(onClick = onDismiss, modifier = Modifier.heightIn(min = MIN_TARGET)) { Text(stringResource(R.string.tv_install_close)) }
                }
                when (kind) {
                    null -> Choose { kind = it }
                    TvKind.ANDROID -> AndroidTvSteps(onDismiss)
                    TvKind.OTHER -> BrowserSteps(address, onPrepare)
                    TvKind.CAST -> Note(stringResource(R.string.tv_guide_cast_body))
                }
            }
        }
    }
}

@Composable
private fun Choose(onPick: (TvKind) -> Unit) {
    Text(stringResource(R.string.tv_guide_which), style = MaterialTheme.typography.titleMedium)
    Option(Icons.Rounded.Tv, stringResource(R.string.tv_guide_android), stringResource(R.string.tv_guide_android_hint)) { onPick(TvKind.ANDROID) }
    Option(Icons.Rounded.Language, stringResource(R.string.tv_guide_other), stringResource(R.string.tv_guide_other_hint)) { onPick(TvKind.OTHER) }
    Option(Icons.Rounded.Cast, stringResource(R.string.tv_guide_cast), stringResource(R.string.tv_guide_cast_hint)) { onPick(TvKind.CAST) }
    Note(stringResource(R.string.tv_guide_same_wifi))
}

/** HomeFi on the TV: install once, open Train, scan its code. After that every ride opens there by itself. */
@Composable
private fun AndroidTvSteps(onDone: () -> Unit) {
    val app = LocalContext.current.homefi
    var installing by rememberSaveable { mutableStateOf(false) }
    var scanning by rememberSaveable { mutableStateOf(false) }
    Step(1, stringResource(R.string.tv_guide_android_step1))
    TextButton(onClick = { installing = true }, modifier = Modifier.heightIn(min = MIN_TARGET)) { Text(stringResource(R.string.tv_guide_how_install)) }
    Step(2, stringResource(R.string.tv_guide_android_step2))
    Step(3, stringResource(R.string.tv_guide_android_step3))
    Button(onClick = { scanning = true }, modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp)) {
        Icon(Icons.Rounded.QrCodeScanner, null, Modifier.size(20.dp))
        Text(stringResource(R.string.tv_guide_scan), Modifier.padding(start = 8.dp))
    }
    Note(stringResource(R.string.tv_guide_android_once))
    if (installing) TvInstallDialog { installing = false }
    if (scanning) {
        TvScanDialog(onDismiss = { scanning = false }) { link ->
            scanning = false
            app.tvPairings.connect(link)
            onDone()
        }
    }
}

/** Any other smart TV: its web browser opens the ride. The address is the last resort, so it is shown only here. */
@Composable
private fun BrowserSteps(address: String?, onPrepare: (() -> Unit)?) {
    Step(1, stringResource(R.string.tv_guide_other_step1))
    Step(2, stringResource(R.string.tv_guide_other_step2))
    if (address != null) {
        SelectionContainer {
            Text(
                address,
                fontSize = 30.sp, fontWeight = FontWeight.ExtraBold, fontFamily = FontFamily.Monospace,
                textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.fillMaxWidth()
                    .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(16.dp)).padding(vertical = 20.dp, horizontal = 12.dp),
            )
        }
        Step(3, stringResource(R.string.tv_guide_other_step3))
    } else if (onPrepare != null) {
        val starting by LocalContext.current.homefi.ride.startingTv.collectAsStateWithLifecycle()
        Button(onClick = onPrepare, enabled = !starting, modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp)) {
            if (starting) ButtonSpinner(Modifier.padding(end = 8.dp), size = 20.dp)
            Text(stringResource(R.string.tv_guide_prepare))
        }
    } else {
        Note(stringResource(R.string.tv_guide_other_later))
    }
}

@Composable
private fun Option(icon: ImageVector, title: String, hint: String, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(20.dp))
            .clickable(role = Role.Button, onClick = onClick)
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Icon(icon, null, Modifier.size(OPTION_ICON), tint = MaterialTheme.colorScheme.primary)
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
            Text(hint, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null)
    }
}

@Composable
private fun Step(number: Int, text: String) {
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.Top) {
        Text("$number", style = MaterialTheme.typography.titleLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(top = 1.dp))
        Text(text, style = MaterialTheme.typography.bodyLarge)
    }
}

@Composable
private fun Note(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
}
