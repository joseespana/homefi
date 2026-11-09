package com.perseusoft.homefi.ui.tvpair

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.ContextCompat
import com.perseusoft.homefi.R
import com.perseusoft.homefi.pair.QrDecoder
import com.perseusoft.homefi.pair.TvPairLink
import java.util.concurrent.Executors

private val MIN_TARGET = 48.dp
private val FRAME = 240.dp

/**
 * The in-app scanner for the TV's pairing QR (CameraX frames, zxing decoding, all on the device).
 * Asks for the camera with a reason first; [onLink] gets the first valid `homefi://tv` link.
 */
@Composable
fun TvScanDialog(onDismiss: () -> Unit, onLink: (TvPairLink) -> Unit) {
    val context = LocalContext.current
    var granted by remember { mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
    var denied by rememberSaveable { mutableStateOf(false) }
    var cameraMissing by remember { mutableStateOf(false) }
    val request = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        granted = it
        denied = !it
    }
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.scrim)) {
            if (granted && cameraMissing) {
                Text(
                    stringResource(R.string.tv_scan_no_camera),
                    color = MaterialTheme.colorScheme.onSurface,
                    textAlign = TextAlign.Center,
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.align(Alignment.Center).padding(32.dp),
                )
            } else if (granted) {
                CameraScan(onLink, onUnavailable = { cameraMissing = true })
                Box(
                    Modifier.align(Alignment.Center).size(FRAME)
                        .border(3.dp, MaterialTheme.colorScheme.primary, RoundedCornerShape(20.dp)),
                )
                Text(
                    stringResource(R.string.tv_scan_hint),
                    color = MaterialTheme.colorScheme.onSurface,
                    textAlign = TextAlign.Center,
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.align(Alignment.BottomCenter).padding(32.dp)
                        .background(MaterialTheme.colorScheme.surface.copy(alpha = 0.8f), RoundedCornerShape(16.dp)).padding(16.dp),
                )
            } else {
                Column(
                    Modifier.align(Alignment.Center).padding(32.dp).sizeIn(maxWidth = 420.dp)
                        .background(MaterialTheme.colorScheme.surface, RoundedCornerShape(24.dp)).padding(24.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(stringResource(R.string.tv_scan_title), style = MaterialTheme.typography.titleLarge)
                    Text(
                        stringResource(if (denied) R.string.tv_scan_denied else R.string.tv_scan_why),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                    if (denied) {
                        Button(
                            onClick = {
                                context.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null)))
                            },
                            modifier = Modifier.sizeIn(minHeight = MIN_TARGET),
                        ) { Text(stringResource(R.string.tv_scan_settings)) }
                    } else {
                        Button(onClick = { request.launch(Manifest.permission.CAMERA) }, modifier = Modifier.sizeIn(minHeight = MIN_TARGET)) {
                            Text(stringResource(R.string.tv_scan_allow))
                        }
                    }
                    OutlinedButton(onClick = onDismiss, modifier = Modifier.sizeIn(minHeight = MIN_TARGET)) { Text(stringResource(R.string.ex_tv_cancel)) }
                }
            }
            IconButton(onClick = onDismiss, modifier = Modifier.align(Alignment.TopEnd).padding(12.dp).size(MIN_TARGET)) {
                Icon(Icons.Rounded.Close, stringResource(R.string.ex_tv_cancel), tint = MaterialTheme.colorScheme.onSurface)
            }
        }
    }
}

@Composable
private fun CameraScan(onLink: (TvPairLink) -> Unit, onUnavailable: () -> Unit) {
    val context = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val onFound by rememberUpdatedState(onLink)
    val executor = remember { Executors.newSingleThreadExecutor() }
    val decoder = remember { QrDecoder() }
    AndroidView(
        modifier = Modifier.fillMaxSize(),
        factory = { ctx ->
            val view = PreviewView(ctx)
            val future = ProcessCameraProvider.getInstance(ctx)
            future.addListener({
                val provider = runCatching { future.get() }.getOrNull()
                val selector = listOf(CameraSelector.DEFAULT_BACK_CAMERA, CameraSelector.DEFAULT_FRONT_CAMERA)
                    .firstOrNull { provider?.hasCamera(it) == true }
                if (provider == null || selector == null) {
                    onUnavailable()
                    return@addListener
                }
                val preview = Preview.Builder().build().also { it.surfaceProvider = view.surfaceProvider }
                val analysis = ImageAnalysis.Builder()
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                var done = false
                analysis.setAnalyzer(executor) { image ->
                    try {
                        val plane = image.planes[0]
                        val buffer = plane.buffer
                        val bytes = ByteArray(buffer.remaining()).also { buffer.get(it) }
                        val link = decoder.decode(bytes, plane.rowStride, image.width, image.height)?.let(TvPairLink::parse)
                        if (link != null && !done) {
                            done = true
                            view.post { onFound(link) }
                        }
                    } finally {
                        image.close()
                    }
                }
                runCatching {
                    provider.unbindAll()
                    provider.bindToLifecycle(owner, selector, preview, analysis)
                }.onFailure { onUnavailable() }
            }, ContextCompat.getMainExecutor(ctx))
            view
        },
    )
    DisposableEffect(Unit) {
        onDispose {
            runCatching { ProcessCameraProvider.getInstance(context).get().unbindAll() } // no camera: nothing to release
            executor.shutdown()
        }
    }
}
