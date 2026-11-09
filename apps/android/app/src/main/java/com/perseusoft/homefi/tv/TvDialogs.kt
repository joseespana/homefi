package com.perseusoft.homefi.tv

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.perseusoft.homefi.R

private val CONFIRM_WIDTH = 340.dp

/**
 * A yes / no question on its own card (delete a playlist, end a party). The safe answer takes focus
 * first, so a stray OK never destroys anything; BACK is "no".
 */
@Composable
fun TvConfirmDialog(
    title: String,
    text: String,
    confirmLabel: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    cancelLabel: String = stringResource(R.string.cancel),
) {
    val look = LocalTvLook.current
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(Color(0xCC040510)), contentAlignment = Alignment.Center) {
            TvGlass(Modifier.width(CONFIRM_WIDTH), solid = true) {
                Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(title, style = TvType.section, color = look.ink, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    Text(text, style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink2)
                    Row(Modifier.padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        TvPill(cancelLabel, onDismiss, Modifier.tvInitialFocus(), height = 34.dp)
                        TvPill(confirmLabel, { onDismiss(); onConfirm() }, height = 34.dp, primary = true)
                    }
                }
            }
        }
    }
}
