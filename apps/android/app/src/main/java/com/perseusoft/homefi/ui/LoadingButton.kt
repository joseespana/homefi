package com.perseusoft.homefi.ui

import androidx.compose.foundation.layout.size
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

private val SPINNER_STROKE = 2.dp

/**
 * The spinner a button shows in place of its icon (or label) while its work runs. [color] defaults to the
 * content colour of the button it sits in.
 */
@Composable
fun ButtonSpinner(modifier: Modifier = Modifier, size: Dp = ButtonDefaults.IconSize, color: Color = Color.Unspecified) {
    val tint = if (color == Color.Unspecified) androidx.compose.material3.LocalContentColor.current else color
    CircularProgressIndicator(modifier.size(size), color = tint, strokeWidth = SPINNER_STROKE)
}

/**
 * One piece of slow work started by a button: [run] starts it unless it is still running (so a button can't be
 * pressed twice) and [active] drives the spinner. Lives with the composition; leaving the screen cancels it.
 */
@Stable
class LoadingTask(private val scope: CoroutineScope) {
    var active by mutableStateOf(false)
        private set

    fun run(block: suspend () -> Unit) {
        if (active) return
        active = true
        scope.launch {
            try {
                block()
            } catch (e: CancellationException) {
                throw e
            } finally {
                active = false
            }
        }
    }
}

@Composable
fun rememberLoadingTask(): LoadingTask {
    val scope = rememberCoroutineScope()
    return remember(scope) { LoadingTask(scope) }
}
