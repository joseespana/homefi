package com.perseusoft.homefi.tv

import android.content.Context
import android.content.Intent
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.SdStorage
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.Storage

/**
 * Opens Android's "All files access" page for HomeFi. When this TV does not have that page the
 * user is told, and HomeFi keeps to its own folder on the drive.
 */
fun openUsbAccessSettings(context: Context) {
    val intent = Storage.accessSettingsIntent(context)?.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    if (intent == null || runCatching { context.startActivity(intent) }.isFailure) {
        Toast.makeText(context, context.getString(R.string.tv_dl_usb_no_page), Toast.LENGTH_LONG).show()
    }
}

/**
 * The guided screen for "All files access" on a TV: what it is for, three steps, or the app folder on the
 * drive. It is asked every time a download is about to go to a USB drive without the permission, and from
 * Downloads and Settings; "Use the app folder" is the user's no.
 */
@Composable
fun TvUsbGuide(onOpenSettings: () -> Unit, onAppFolder: () -> Unit) {
    val look = LocalTvLook.current
    Dialog(onDismissRequest = onAppFolder, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(Color(0xE6040510)), contentAlignment = Alignment.Center) {
            TvGlass(Modifier.width(520.dp), solid = true) {
                Column(Modifier.padding(28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Icon(Icons.Rounded.SdStorage, null, tint = look.accent2, modifier = Modifier.size(32.dp))
                    Text(stringResource(R.string.tv_dl_guide_title), style = TvType.title.copy(fontSize = 24.sp), color = look.ink)
                    Text(stringResource(R.string.tv_dl_guide_body), style = TvType.body.copy(fontWeight = FontWeight.Normal, fontSize = 15.sp), color = look.ink2)
                    listOf(R.string.tv_dl_guide_1, R.string.tv_dl_guide_2, R.string.tv_dl_guide_3).forEachIndexed { i, step ->
                        Text("${i + 1}.  " + stringResource(step), style = TvType.body.copy(fontSize = 15.sp), color = look.ink)
                    }
                    Text(stringResource(R.string.tv_dl_guide_fallback), style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink3)
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        TvPill(stringResource(R.string.tv_dl_guide_open), onOpenSettings, Modifier.tvInitialFocus(), primary = true, height = 36.dp)
                        TvPill(stringResource(R.string.tv_dl_guide_app), onAppFolder, height = 36.dp)
                    }
                }
            }
        }
    }
}
