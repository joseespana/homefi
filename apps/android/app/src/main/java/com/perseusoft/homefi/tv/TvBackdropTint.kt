package com.perseusoft.homefi.tv

import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import coil3.SingletonImageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.size.Size
import coil3.toBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * The cover the backdrop's light follows: the card that has focus sets it, the shell reads it and
 * paints the cover's colour (docs/TV_UI.md section 1: "the backdrop is the cover's dominant colour").
 */
@Stable
class TvBackdropCover {
    var cover: Any? by mutableStateOf(null)

    /** Where the card that started the music sits on screen: the player's cover grows from there, once. */
    var origin: androidx.compose.ui.geometry.Rect? = null
}

val LocalTvBackdropCover = staticCompositionLocalOf { TvBackdropCover() }

private const val SAMPLE_PX = 16
private const val MIN_SATURATION = 0.18f
private const val MAX_LIGHT = 0.8f
private const val MIN_LIGHT = 0.2f

/** The colour a cover gives the room: the average of its saturated pixels, kept calm (never white, never black). Null while unknown. */
@Composable
fun rememberCoverTint(cover: Any?): Color? {
    val context = LocalContext.current
    var tint by androidx.compose.runtime.remember { mutableStateOf<Color?>(null) }
    LaunchedEffect(cover) {
        if (cover == null) return@LaunchedEffect
        tint = withContext(Dispatchers.IO) {
            val request = ImageRequest.Builder(context).data(cover).size(Size(SAMPLE_PX, SAMPLE_PX)).allowHardware(false).build()
            val image = (SingletonImageLoader.get(context).execute(request) as? SuccessResult)?.image
            image?.toBitmap(SAMPLE_PX, SAMPLE_PX)?.let(::dominantColor)
        } ?: tint
    }
    return if (cover == null) null else tint
}

/** Average of the pixels that carry colour, with its lightness held in a calm band. */
internal fun dominantColor(bitmap: Bitmap): Color? {
    var r = 0f
    var g = 0f
    var b = 0f
    var weight = 0f
    val hsv = FloatArray(3)
    for (x in 0 until bitmap.width) for (y in 0 until bitmap.height) {
        val px = bitmap.getPixel(x, y)
        android.graphics.Color.colorToHSV(px, hsv)
        val w = if (hsv[1] >= MIN_SATURATION) hsv[1] * hsv[2] else 0f
        r += android.graphics.Color.red(px) * w
        g += android.graphics.Color.green(px) * w
        b += android.graphics.Color.blue(px) * w
        weight += w
    }
    if (weight <= 0f) return null
    val picked = floatArrayOf(0f, 0f, 0f)
    android.graphics.Color.colorToHSV(android.graphics.Color.rgb((r / weight).toInt(), (g / weight).toInt(), (b / weight).toInt()), picked)
    picked[2] = picked[2].coerceIn(MIN_LIGHT, MAX_LIGHT)
    return Color(android.graphics.Color.HSVToColor(picked))
}
