package com.perseusoft.homefi.tv

import android.app.Activity
import android.content.Intent
import android.speech.RecognizerIntent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.homefi
import java.util.Locale

/** The system's speech dialog (the Google TV mic): free-form speech in the phone's language. */
internal fun recognizerIntent(prompt: String?): Intent =
    Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
        .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
        .putExtra(RecognizerIntent.EXTRA_LANGUAGE, Locale.getDefault().toLanguageTag())
        .putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
        .apply { if (prompt != null) putExtra(RecognizerIntent.EXTRA_PROMPT, prompt) }

/** The best guess of a recogniser's answer, or null when it was cancelled or empty. */
internal fun spokenText(resultCode: Int, data: Intent?): String? =
    if (resultCode != Activity.RESULT_OK) null
    else data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)?.firstOrNull()?.trim()?.takeIf { it.isNotEmpty() }

/**
 * Voice input for a text field: a function that starts listening and hands the words to [onText],
 * or null (so the caller hides the mic) when `tv.voiceInput` is off or no speech service answers.
 */
@Composable
fun rememberVoiceInput(onText: (String) -> Unit): (() -> Unit)? {
    val context = LocalContext.current
    val settings by context.homefi.settings.settings.collectAsStateWithLifecycle()
    val latest by rememberUpdatedState(onText)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        spokenText(result.resultCode, result.data)?.let(latest)
    }
    val intent = remember { recognizerIntent(null) }
    val available = remember(context) { intent.resolveActivity(context.packageManager) != null }
    if (!settings.tv.voiceInput || !available) return null
    return { runCatching { launcher.launch(intent) } }
}
