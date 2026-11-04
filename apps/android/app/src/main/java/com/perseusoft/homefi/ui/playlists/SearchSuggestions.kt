package com.perseusoft.homefi.ui.playlists

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.view.ViewGroup
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.TextView
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView

private const val GOOGLE_HOST = "google.com"
private const val GOOGLE_SUBDOMAIN_SUFFIX = ".google.com"
private const val HTTPS_SCHEME = "https"
private const val MIN_HEIGHT_DP = 48
private const val MAX_HEIGHT_DP = 160
private const val FALLBACK_PADDING_PX = 32

/**
 * Google's Search Suggestions for a result grounded on Google Search (Gemini), shown unmodified.
 * JavaScript, file and content access are off; a tap on a suggestion opens it in the browser
 * (only https links on google.com). [unavailableText] is shown when the device has no WebView.
 */
@Composable
fun SearchSuggestions(html: String, unavailableText: String, modifier: Modifier = Modifier) {
    AndroidView(
        factory = { ctx -> runCatching { suggestionsWebView(ctx) }.getOrElse { fallbackTextView(ctx, unavailableText) } },
        update = { view ->
            // load only when the snippet changed; the last one rides in the view's tag
            if (view is WebView && view.tag != html) {
                view.tag = html
                view.loadDataWithBaseURL(null, html, "text/html", "utf-8", null)
            }
        },
        onRelease = { (it as? WebView)?.destroy() },
        modifier = modifier.fillMaxWidth().heightIn(min = MIN_HEIGHT_DP.dp, max = MAX_HEIGHT_DP.dp),
    )
}

/** A locked-down view: no JavaScript, no storage, no file access, nothing navigates inside it. */
private fun suggestionsWebView(context: Context) = WebView(context).apply {
    layoutParams = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
    with(settings) {
        javaScriptEnabled = false
        allowFileAccess = false
        allowContentAccess = false
        domStorageEnabled = false
        setGeolocationEnabled(false)
        @Suppress("DEPRECATION") saveFormData = false
        cacheMode = WebSettings.LOAD_NO_CACHE
    }
    setBackgroundColor(Color.TRANSPARENT)
    isFocusable = false
    isFocusableInTouchMode = false
    isVerticalScrollBarEnabled = false
    isHorizontalScrollBarEnabled = false
    // a short strip: vertical drags belong to the screen's own scroll
    setOnTouchListener { _, event -> event.actionMasked == android.view.MotionEvent.ACTION_MOVE }
    webViewClient = GoogleLinksClient(context.applicationContext)
}

/** Opens google.com https links in the browser; returns true for everything so the view never navigates. */
private class GoogleLinksClient(private val appContext: Context) : WebViewClient() {
    @Deprecated("Deprecated in Java")
    override fun shouldOverrideUrlLoading(view: WebView?, url: String?): Boolean = handle(url?.let(Uri::parse))

    override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean =
        handle(request?.url)

    private fun handle(uri: Uri?): Boolean {
        if (uri != null && isGoogleHttps(uri)) {
            val open = Intent(Intent.ACTION_VIEW, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            try {
                appContext.startActivity(open)
            } catch (_: ActivityNotFoundException) {
                // no browser on this device: nothing to do
            }
        }
        return true
    }
}

private fun isGoogleHttps(uri: Uri): Boolean {
    val host = uri.host?.lowercase() ?: return false
    return uri.scheme == HTTPS_SCHEME && (host == GOOGLE_HOST || host.endsWith(GOOGLE_SUBDOMAIN_SUFFIX))
}

private fun fallbackTextView(context: Context, text: String) = TextView(context).apply {
    this.text = text
    setPadding(FALLBACK_PADDING_PX, FALLBACK_PADDING_PX, FALLBACK_PADDING_PX, FALLBACK_PADDING_PX)
}

private const val PREVIEW_HTML = """
<style>
  .container { display: flex; gap: 8px; padding: 8px; font-family: sans-serif; }
  .chip { padding: 8px 14px; border-radius: 16px; background: #e8eaed; color: #3c4043; text-decoration: none; }
  @media (prefers-color-scheme: dark) { .chip { background: #3c4043; color: #e8eaed; } }
</style>
<div class="container">
  <a class="chip" href="https://www.google.com/search?q=korn+setlist+2026">korn setlist 2026</a>
  <a class="chip" href="https://www.google.com/search?q=korn+tour+songs">korn tour songs</a>
</div>
"""

@Preview(showBackground = true)
@Composable
private fun SearchSuggestionsPreview() {
    SearchSuggestions(PREVIEW_HTML, "Google's search suggestions can't be shown on this device.")
}
