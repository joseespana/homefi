package com.perseusoft.homefi.ui.settings

import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.SkinHeading
import com.perseusoft.homefi.ui.theme.fixedDark
import com.perseusoft.homefi.ui.ConfirmDialog
import android.Manifest
import android.content.res.Resources
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.lazy.itemsIndexed
import com.perseusoft.homefi.data.Motion
import com.perseusoft.homefi.ui.theme.ListEntrance
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.itemMotion
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.AccountCircle
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Face
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.Groups
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.Lyrics
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.Search
import com.perseusoft.homefi.ui.SelectChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import com.perseusoft.homefi.ui.theme.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import kotlin.math.roundToInt
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.BuildConfig
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.AudioQuality
import com.perseusoft.homefi.core.ShareAddMode
import com.perseusoft.homefi.core.ShareEditMode
import com.perseusoft.homefi.core.ShareQueueOrder
import com.perseusoft.homefi.core.ShareSkipMode
import com.perseusoft.homefi.core.coreVersion
import com.perseusoft.homefi.data.LYRICS_OFFSET_RANGE
import com.perseusoft.homefi.data.LYRICS_OFFSET_STEP
import com.perseusoft.homefi.data.LyricsMode
import com.perseusoft.homefi.data.LyricsSize
import com.perseusoft.homefi.data.MascotLimbs
import com.perseusoft.homefi.ui.player.label
import com.perseusoft.homefi.ui.player.lyricsOffsetText
import com.perseusoft.homefi.data.ImmersiveShade
import com.perseusoft.homefi.data.PlayerLayout
import com.perseusoft.homefi.data.ProgressStyle
import com.perseusoft.homefi.data.ProgressThickness
import com.perseusoft.homefi.data.ShareSettings
import com.perseusoft.homefi.data.Storage
import com.perseusoft.homefi.data.ThemeMode
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.share.net.ShareNetworkMode
import com.perseusoft.homefi.ui.player.DanceId
import com.perseusoft.homefi.ui.player.Mood
import com.perseusoft.homefi.ui.player.ProgressTrack
import com.perseusoft.homefi.ui.player.Specials
import com.perseusoft.homefi.ui.player.Temper
import com.perseusoft.homefi.ui.player.Turns
import com.perseusoft.homefi.ui.share.ColorPicker
import com.perseusoft.homefi.ui.share.label
import java.text.Normalizer
import kotlinx.coroutines.launch

private fun ThemeMode.label(): Int = when (this) {
    ThemeMode.SYSTEM -> R.string.theme_system
    ThemeMode.LIGHT -> R.string.theme_light
    ThemeMode.DARK -> R.string.theme_dark
}

private fun AudioQuality.label(): Int = when (this) {
    AudioQuality.LOW -> R.string.quality_low
    AudioQuality.NORMAL -> R.string.quality_normal
    AudioQuality.HIGH -> R.string.quality_high
    AudioQuality.MAX -> R.string.quality_max
}

/**
 * A row on the settings home and the sub-page it opens. [keywords] are the labels inside the
 * page, so the search finds "Port" under shared control.
 */
private enum class SettingsPage(val title: Int, val icon: ImageVector, val keywords: List<Int>) {
    ACCOUNT(R.string.settings_account, Icons.Rounded.AccountCircle, listOf(R.string.settings_logout, R.string.login_connect, R.string.settings_lazy_lists)),
    DOWNLOADS(
        R.string.settings_downloads,
        Icons.Rounded.Download,
        listOf(
            R.string.settings_quality, R.string.settings_embed_cover, R.string.settings_embed_lyrics,
            R.string.settings_videos, R.string.settings_skip_existing, R.string.settings_threads, R.string.settings_template,
        ),
    ),
    STORAGE(
        R.string.settings_storage,
        Icons.Rounded.Folder,
        listOf(R.string.settings_folder_change, R.string.settings_folder_default, R.string.settings_storage_grant),
    ),
    LOOK(
        R.string.settings_look,
        Icons.Rounded.Palette,
        listOf(
            R.string.settings_appearance, R.string.theme_light, R.string.theme_dark, R.string.settings_player,
            R.string.layout_title, R.string.layout_everywhere, R.string.immersive_shade, R.string.progress_title, R.string.progress_animated,
            R.string.settings_motion, R.string.motion_subtle, R.string.settings_haptics,
        ),
    ),
    MASCOT(
        R.string.mascot_title,
        Icons.Rounded.Face,
        listOf(R.string.mascot_style, R.string.mascot_dance, R.string.mascot_specials, R.string.mascot_turns, R.string.mascot_limbs),
    ),
    LYRICS(
        R.string.lyrics_title,
        Icons.Rounded.Lyrics,
        listOf(
            R.string.lyrics_mode, R.string.lyrics_mode_karaoke, R.string.lyrics_size, R.string.lyrics_timing,
            R.string.lyrics_auto_open, R.string.lyrics_mascot,
        ),
    ),
    SHARING(
        R.string.sharing_settings,
        Icons.Rounded.Groups,
        listOf(
            R.string.sharing_your_name, R.string.sharing_your_color, R.string.sharing_speaker_delay,
            R.string.sharing_session_name, R.string.sharing_network, R.string.sharing_group_name, R.string.sharing_group_pass, R.string.sharing_announce_lan,
            R.string.sharing_party_default, R.string.sharing_code_setting, R.string.sharing_port,
            R.string.sharing_perm_add, R.string.sharing_perm_order, R.string.sharing_perm_skip, R.string.sharing_perm_edit,
            R.string.sharing_perm_playback, R.string.sharing_perm_explicit, R.string.sharing_perm_drop,
            R.string.sharing_notices, R.string.sharing_finding, R.string.sharing_discover, R.string.sharing_discover_everywhere,
            R.string.sharing_bluetooth, R.string.sharing_play_along_default, R.string.sharing_guest_opens_player,
        ),
    ),
    ABOUT(R.string.settings_about, Icons.Rounded.Info, listOf(R.string.tidal_terms_title, R.string.tidal_terms)),
}

/** Lower case without accents or format placeholders, so "musica" finds "música". */
private fun String.searchable(): String =
    Normalizer.normalize(replace(Regex("%\\d\\$[sd]"), ""), Normalizer.Form.NFD)
        .replace(Regex("\\p{Mn}+"), "")
        .lowercase()

/** The first label of [page] that matches [query] (its title counts too), or null. */
private fun SettingsPage.match(res: Resources, query: String): String? {
    val q = query.trim().searchable()
    return (listOf(title) + keywords)
        .map { res.getString(it) }
        .firstOrNull { it.searchable().contains(q) }
        ?.replace(Regex("[:：]?\\s*%\\d\\$[sd].*"), "")
}

/**
 * Settings home: one row per category with a summary of its current values; a row opens its
 * page. System back returns to the list.
 */
@Composable
fun SettingsScreen(loggedIn: Boolean, onLogin: () -> Unit, onLoggedOut: () -> Unit) {
    val context = LocalContext.current
    val app = context.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()

    // re-evaluated when coming back from the system settings screen
    var storageTick by remember { mutableIntStateOf(0) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { storageTick++ }
    val shared = remember(storageTick) { Storage.hasSharedAccess(context) }
    val dir = remember(storageTick, settings.downloadDir) { Storage.downloadDir(context, settings.downloadDir).absolutePath }
    val legacyPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { storageTick++ }
    val account = remember(loggedIn) { app.engine.account() }

    var page by rememberSaveable { mutableStateOf<SettingsPage?>(null) }
    BackHandler(enabled = page != null) { page = null }
    val motion = LocalMotion.current

    AnimatedContent(
        targetState = page,
        // a page comes in from the end, back returns from the start
        transitionSpec = { motion.screens(forward = targetState != null) },
        label = "settings",
    ) { open ->
        if (open == null) {
            SettingsHome(
                summary = { p ->
                    when (p) {
                        SettingsPage.ACCOUNT -> account
                            ?.let { stringResource(R.string.settings_account_info, it.userId, it.countryCode) }
                            ?: stringResource(R.string.settings_account_none_short)
                        SettingsPage.DOWNLOADS ->
                            stringResource(R.string.settings_downloads_summary, stringResource(settings.quality.label()), settings.threads)
                        SettingsPage.STORAGE -> dir
                        SettingsPage.LOOK ->
                            "${stringResource(settings.theme.label())} · ${stringResource(settings.playerLayout.labelRes())}"
                        SettingsPage.MASCOT ->
                            "${stringResource(settings.mascotMood?.label ?: R.string.mascot_auto)} · " +
                                stringResource(settings.mascotDance?.label ?: R.string.mascot_dance_auto)
                        SettingsPage.LYRICS ->
                            stringResource(R.string.lyrics_summary, stringResource(settings.lyricsMode.label()), stringResource(settings.lyricsSize.label()))
                        SettingsPage.SHARING ->
                            "${stringResource(settings.share.network.label())} · ${app.share.sessionName(settings.share)}"
                        SettingsPage.ABOUT -> "HomeFi ${BuildConfig.VERSION_NAME}"
                    }
                },
                onOpen = { page = it },
            )
        } else {
            Column(Modifier.fillMaxSize()) {
                PageHeader(open.title, onBack = { page = null })
                Column(
                    Modifier
                        .weight(1f)
                        .verticalScroll(rememberScrollState())
                        .padding(bottom = 16.dp),
                ) {
                    when (open) {
                        SettingsPage.ACCOUNT -> AccountSettings(loggedIn, onLogin, onLoggedOut)
                        SettingsPage.DOWNLOADS -> DownloadSettings()
                        SettingsPage.STORAGE -> StorageSettings(shared, dir, onLegacyPermission = {
                            legacyPermission.launch(Manifest.permission.WRITE_EXTERNAL_STORAGE)
                        })
                        SettingsPage.LOOK -> {
                            AppearanceSettings()
                            HorizontalDivider(Modifier.padding(vertical = 8.dp))
                            PlayerSettings()
                        }
                        SettingsPage.MASCOT -> MascotSettings()
                        SettingsPage.LYRICS -> LyricsSettings()
                        SettingsPage.SHARING -> ShareSettingsSection()
                        SettingsPage.ABOUT -> AboutSettings()
                    }
                }
            }
        }
    }
}

/** The category list, with a search over the labels inside each page. */
@Composable
private fun SettingsHome(summary: @Composable (SettingsPage) -> String, onOpen: (SettingsPage) -> Unit) {
    val res = LocalContext.current.resources
    var query by rememberSaveable { mutableStateOf("") }
    val matches = remember(query) {
        SettingsPage.entries.mapNotNull { p ->
            if (query.isBlank()) p to null else p.match(res, query)?.let { p to it }
        }
    }

    ListEntrance {
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 16.dp)) {
        item(key = "search") {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text(stringResource(R.string.settings_search)) },
                leadingIcon = { Icon(Icons.Rounded.Search, null) },
                trailingIcon = {
                    if (query.isNotEmpty()) {
                        IconButton(onClick = { query = "" }) { Icon(Icons.Rounded.Clear, null) }
                    }
                },
                singleLine = true,
                shape = MaterialTheme.shapes.extraLarge,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
        if (matches.isEmpty()) {
            item(key = "none") { Box(itemMotion()) { Hint(R.string.settings_search_none) } }
        }
        // rows cascade in, and slide together as the search filters them
        itemsIndexed(matches, key = { _, it -> it.first.name }) { i, (p, hit) ->
            ListItem(
                colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
                leadingContent = { Icon(p.icon, null, tint = MaterialTheme.colorScheme.primary) },
                headlineContent = { Text(stringResource(p.title)) },
                // while searching, say which setting matched instead of the current values
                supportingContent = { Text(hit ?: summary(p), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                trailingContent = { Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null) },
                modifier = itemMotion(i).clickable { onOpen(p) },
            )
        }
    }
    }
}

/** Top of a sub-page; the app bar above stays the app's own. */
@Composable
private fun PageHeader(title: Int, onBack: () -> Unit) {
    Row(Modifier.padding(horizontal = 4.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
        Text(
            stringResource(title),
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun AccountSettings(loggedIn: Boolean, onLogin: () -> Unit, onLoggedOut: () -> Unit) {
    val app = LocalContext.current.homefi
    val scope = rememberCoroutineScope()
    val account = remember(loggedIn) { app.engine.account() }
    Header(R.string.settings_account)
    if (account == null) {
        Text(stringResource(R.string.settings_account_none), modifier = Modifier.padding(horizontal = 16.dp))
        TextButton(onClick = onLogin, modifier = Modifier.padding(horizontal = 8.dp)) {
            Text(stringResource(R.string.login_connect))
        }
    } else {
        Text(
            stringResource(R.string.settings_account_info, account.userId, account.countryCode),
            modifier = Modifier.padding(horizontal = 16.dp),
        )
        var confirming by remember { mutableStateOf(false) }
        if (confirming) {
            ConfirmDialog(
                title = stringResource(R.string.settings_logout_title),
                text = stringResource(R.string.settings_logout_text),
                confirm = stringResource(R.string.settings_logout),
                onConfirm = {
                    scope.launch {
                        app.downloads.cancelAll()
                        app.engine.logout()
                        onLoggedOut()
                    }
                },
                onDismiss = { confirming = false },
            )
        }
        TextButton(
            onClick = { confirming = true },
            modifier = Modifier.padding(horizontal = 8.dp),
        ) { Text(stringResource(R.string.settings_logout), color = MaterialTheme.colorScheme.error) }
    }

    val settings by app.settings.settings.collectAsStateWithLifecycle()
    Header(R.string.settings_tidal_lists)
    Toggle(R.string.settings_lazy_lists, settings.lazyLists) { v -> app.settings.update { it.copy(lazyLists = v) } }
    Hint(R.string.settings_lazy_lists_hint)
}

/** Quality, tags and naming of what gets downloaded. */
@Composable
private fun DownloadSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    Header(R.string.settings_quality)
    Row(Modifier.padding(horizontal = 16.dp)) {
        AudioQuality.entries.forEach { q ->
            SelectChip(
                selected = settings.quality == q,
                onClick = { app.settings.update { it.copy(quality = q) } },
                label = { Text(stringResource(q.label())) },
                modifier = Modifier.padding(end = 8.dp),
            )
        }
    }
    Text(
        stringResource(R.string.settings_quality_hint),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )

    Toggle(R.string.settings_embed_cover, settings.embedCover) { v -> app.settings.update { it.copy(embedCover = v) } }
    Toggle(R.string.settings_embed_lyrics, settings.embedLyrics) { v -> app.settings.update { it.copy(embedLyrics = v) } }
    Toggle(R.string.settings_videos, settings.includeVideos) { v -> app.settings.update { it.copy(includeVideos = v) } }
    Toggle(R.string.settings_skip_existing, settings.skipExisting) { v -> app.settings.update { it.copy(skipExisting = v) } }

    ListItem(
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        headlineContent = { Text(stringResource(R.string.settings_threads, settings.threads)) },
        supportingContent = {
            Slider(
                value = settings.threads.toFloat(),
                onValueChange = { v -> app.settings.update { it.copy(threads = v.toInt()) } },
                valueRange = 1f..6f,
                steps = 4,
            )
        },
    )

    OutlinedTextField(
        value = settings.template,
        onValueChange = { v -> app.settings.update { it.copy(template = v) } },
        label = { Text(stringResource(R.string.settings_template)) },
        placeholder = { Text("{album.artist}/{album.title}/{item.number:02d}. {item.title}") },
        singleLine = true,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

/** Where downloads go (and the library is read from), and the "All files access" grant. */
@Composable
private fun StorageSettings(shared: Boolean, dir: String, onLegacyPermission: () -> Unit) {
    val context = LocalContext.current
    val app = context.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    var picking by remember { mutableStateOf(false) }

    if (picking) {
        FolderPicker(
            initial = settings.downloadDir.ifBlank { dir },
            onPick = { folder ->
                picking = false
                app.settings.update { it.copy(downloadDir = folder.absolutePath) }
                app.library.refresh()
            },
            onDismiss = { picking = false },
        )
    }

    Header(R.string.settings_storage)
    Text(dir, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(horizontal = 16.dp))
    if (settings.downloadDir.isNotBlank() && settings.downloadDir != dir) {
        Text(
            stringResource(R.string.settings_folder_fallback, settings.downloadDir),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.error,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
        )
    }
    Row(Modifier.padding(horizontal = 8.dp)) {
        TextButton(onClick = { picking = true }) { Text(stringResource(R.string.settings_folder_change)) }
        if (settings.downloadDir.isNotBlank()) {
            TextButton(onClick = {
                app.settings.update { it.copy(downloadDir = "") }
                app.library.refresh()
            }) { Text(stringResource(R.string.settings_folder_default)) }
        }
    }
    if (!shared) {
        Text(
            stringResource(R.string.settings_storage_hint),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
        )
        OutlinedButton(
            onClick = {
                val intent = Storage.accessSettingsIntent(context)
                if (intent != null) {
                    context.startActivity(intent)
                } else if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) {
                    onLegacyPermission()
                }
            },
            modifier = Modifier.padding(horizontal = 16.dp),
        ) { Text(stringResource(R.string.settings_storage_grant)) }
    }
}

@Composable
private fun AppearanceSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    Header(R.string.settings_appearance)
    Row(Modifier.padding(horizontal = 16.dp)) {
        ThemeMode.entries.forEach { mode ->
            SelectChip(
                selected = settings.theme == mode,
                onClick = { app.settings.update { it.copy(theme = mode) } },
                label = { Text(stringResource(mode.label())) },
                modifier = Modifier.padding(end = 8.dp),
            )
        }
    }
    // a skin that is always dark (or light) wins over this choice
    val fixed = if (settings.layoutEverywhere) settings.playerLayout.fixedDark() else null
    if (fixed != null) {
        Text(
            stringResource(
                if (fixed) R.string.theme_fixed_dark else R.string.theme_fixed_light,
                stringResource(settings.playerLayout.labelRes()),
            ),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
        )
    }
    MotionChoice(settings.motion) { m -> app.settings.update { it.copy(motion = m) } }
    Toggle(R.string.settings_haptics, settings.haptics) { v -> app.settings.update { it.copy(haptics = v) } }
    Hint(R.string.settings_haptics_hint)
}

/** Transitions everywhere: automatic, full, subtle or none (the desktop has the same four). */
@Composable
private fun MotionChoice(motion: Motion, onPick: (Motion) -> Unit) {
    Label(R.string.settings_motion)
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        Motion.entries.forEach { m -> Chip(motion == m, m.labelRes()) { onPick(m) } }
    }
    Hint(R.string.settings_motion_hint)
}

private fun Motion.labelRes(): Int = when (this) {
    Motion.AUTO -> R.string.motion_auto
    Motion.FULL -> R.string.motion_full
    Motion.SUBTLE -> R.string.motion_subtle
    Motion.OFF -> R.string.motion_off
}

@Composable
private fun AboutSettings() {
    Text(
        "HomeFi ${BuildConfig.VERSION_NAME} · core ${coreVersion()}",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(16.dp),
    )
    val settings by LocalContext.current.homefi.settings.settings.collectAsStateWithLifecycle()
    Header(R.string.tidal_terms_title)
    Text(
        stringResource(R.string.tidal_terms),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp),
    )
    Text(
        if (settings.termsAccepted && settings.tidalTermsAcceptedAt > 0) {
            val date = java.text.DateFormat.getDateInstance(java.text.DateFormat.MEDIUM).format(java.util.Date(settings.tidalTermsAcceptedAt))
            stringResource(R.string.tidal_terms_accepted_on, date)
        } else {
            stringResource(R.string.tidal_terms_not_accepted)
        },
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

/** Layout and seek bar of the full-screen player, with live previews. */
@Composable
private fun PlayerSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    Header(R.string.settings_player)
    Text(
        stringResource(R.string.layout_title),
        style = MaterialTheme.typography.labelLarge,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
    )
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        PlayerLayout.entries.forEach { l ->
            SelectChip(
                selected = settings.playerLayout == l,
                onClick = { app.settings.update { it.copy(playerLayout = l) } },
                label = { Text(stringResource(l.labelRes())) },
                modifier = Modifier.padding(end = 8.dp),
            )
        }
    }
    Toggle(R.string.layout_everywhere, settings.layoutEverywhere) { v -> app.settings.update { it.copy(layoutEverywhere = v) } }
    Hint(if (settings.layoutEverywhere) R.string.layout_everywhere_hint else R.string.layout_everywhere_off_hint)
    Reveal(settings.playerLayout == PlayerLayout.IMMERSIVE) {
        Text(
            stringResource(R.string.immersive_shade),
            style = MaterialTheme.typography.labelLarge,
            modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp),
        )
        Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            ImmersiveShade.entries.forEach { shade ->
                SelectChip(
                    selected = settings.immersiveShade == shade,
                    onClick = { app.settings.update { it.copy(immersiveShade = shade) } },
                    label = { Text(stringResource(shade.labelRes())) },
                    modifier = Modifier.padding(end = 8.dp),
                )
            }
        }
        Hint(R.string.immersive_shade_hint)
    }
    Text(
        stringResource(R.string.progress_title),
        style = MaterialTheme.typography.labelLarge,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp),
    )
    ProgressStyle.entries.forEach { style ->
        val selected = settings.progressStyle == style
        Row(
            Modifier
                .fillMaxWidth()
                .clickable { app.settings.update { it.copy(progressStyle = style) } }
                .padding(horizontal = 16.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RadioButton(selected = selected, onClick = null)
            Text(stringResource(style.labelRes()), Modifier.width(96.dp).padding(start = 8.dp))
            ProgressTrack(
                fraction = 0.62f,
                style = style,
                thickness = settings.progressThickness,
                animate = settings.progressAnimated,
                seed = 7,
                modifier = Modifier.weight(1f).height(28.dp),
            )
        }
    }
    Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
        ProgressThickness.entries.forEach { t ->
            SelectChip(
                selected = settings.progressThickness == t,
                onClick = { app.settings.update { it.copy(progressThickness = t) } },
                label = { Text(stringResource(t.labelRes())) },
                modifier = Modifier.padding(end = 8.dp),
            )
        }
    }
    Toggle(R.string.progress_animated, settings.progressAnimated) { v -> app.settings.update { it.copy(progressAnimated = v) } }
}

/** How the mascot looks and dances (the same choices as on the desktop). */
@Composable
private fun MascotSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    Hint(R.string.mascot_hint)

    Label(R.string.mascot_style)
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        Chip(settings.mascotMood == null, R.string.mascot_auto) { app.settings.update { it.copy(mascotMood = null) } }
        Mood.entries.forEach { m ->
            Chip(settings.mascotMood == m, m.label) { app.settings.update { it.copy(mascotMood = m) } }
        }
    }
    Hint(settings.mascotMood?.genres ?: R.string.mascot_auto_hint)

    Label(R.string.mascot_dance)
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        Chip(settings.mascotDance == null, R.string.mascot_dance_auto) { app.settings.update { it.copy(mascotDance = null) } }
        DanceId.entries.forEach { d ->
            Chip(settings.mascotDance == d, d.label) { app.settings.update { it.copy(mascotDance = d) } }
        }
    }
    Hint(settings.mascotDance?.hint ?: R.string.mascot_dance_auto_hint)

    // only matters while no dance is pinned
    Reveal(settings.mascotDance == null) {
        Label(R.string.mascot_specials)
        Row(Modifier.padding(horizontal = 16.dp)) {
            Specials.entries.forEach { sp ->
                Chip(settings.mascotSpecials == sp, sp.labelRes()) { app.settings.update { it.copy(mascotSpecials = sp) } }
            }
        }
    }

    Label(R.string.mascot_turns)
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        Turns.entries.forEach { t ->
            Chip(settings.mascotTurns == t, t.labelRes()) { app.settings.update { it.copy(mascotTurns = t) } }
        }
    }

    Label(R.string.mascot_limbs)
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        MascotLimbs.entries.forEach { l ->
            Chip(settings.mascotLimbs == l, l.labelRes()) { app.settings.update { it.copy(mascotLimbs = l) } }
        }
    }

    Label(R.string.mascot_temper)
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        Temper.entries.forEach { t ->
            Chip(settings.mascotTemper == t, t.labelRes()) { app.settings.update { it.copy(mascotTemper = t) } }
        }
    }
    Hint(settings.mascotTemper.hintRes())
    Toggle(R.string.mascot_long_press, settings.mascotLongPress) { v -> app.settings.update { it.copy(mascotLongPress = v) } }
    Reveal(settings.mascotX != 0f || settings.mascotY != 0f || settings.mascotScale != 1f) {
        TextButton(
            onClick = { app.settings.update { it.copy(mascotX = 0f, mascotY = 0f, mascotScale = 1f) } },
            modifier = Modifier.padding(horizontal = 8.dp),
        ) { Text(stringResource(R.string.mascot_reset)) }
    }
}

/** The lyrics view in the player: mode, size, timing, HomeFito, opening by itself. */
@Composable
private fun LyricsSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    Label(R.string.lyrics_mode)
    Row(Modifier.padding(horizontal = 16.dp)) {
        LyricsMode.entries.forEach { m ->
            Chip(settings.lyricsMode == m, m.label()) { app.settings.update { it.copy(lyricsMode = m) } }
        }
    }
    Hint(R.string.lyrics_mode_hint)

    Label(R.string.lyrics_size)
    Row(Modifier.padding(horizontal = 16.dp)) {
        LyricsSize.entries.forEach { sz ->
            Chip(settings.lyricsSize == sz, sz.label()) { app.settings.update { it.copy(lyricsSize = sz) } }
        }
    }

    ListItem(
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        headlineContent = { Text(stringResource(R.string.lyrics_timing, lyricsOffsetText(settings.lyricsOffsetMs))) },
        supportingContent = {
            Column {
                Slider(
                    value = settings.lyricsOffsetMs.toFloat(),
                    onValueChange = { v ->
                        val ms = ((v / LYRICS_OFFSET_STEP).roundToInt() * LYRICS_OFFSET_STEP).coerceIn(LYRICS_OFFSET_RANGE)
                        if (ms != settings.lyricsOffsetMs) app.settings.update { it.copy(lyricsOffsetMs = ms) }
                    },
                    // snaps to LYRICS_OFFSET_STEP in onValueChange (40 tick marks would only clutter it)
                    valueRange = LYRICS_OFFSET_RANGE.first.toFloat()..LYRICS_OFFSET_RANGE.last.toFloat(),
                )
                Text(stringResource(R.string.lyrics_timing_hint), style = MaterialTheme.typography.bodySmall)
            }
        },
    )
    Reveal(settings.lyricsOffsetMs != 0) {
        TextButton(
            onClick = { app.settings.update { it.copy(lyricsOffsetMs = 0) } },
            modifier = Modifier.padding(horizontal = 8.dp),
        ) { Text(stringResource(R.string.lyrics_timing_reset)) }
    }

    Toggle(R.string.lyrics_auto_open, settings.lyricsAutoOpen) { v -> app.settings.update { it.copy(lyricsAutoOpen = v) } }
    Hint(R.string.lyrics_auto_open_hint)
    Toggle(R.string.lyrics_mascot, settings.lyricsMascot) { v -> app.settings.update { it.copy(lyricsMascot = v) } }
    Hint(R.string.lyrics_mascot_hint)
}

/**
 * Shared control (car) and party mode: who you are to others, the hub's rules, and this
 * phone as a guest.
 */
@Composable
private fun ShareSettingsSection() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val s = settings.share
    fun set(transform: (ShareSettings) -> ShareSettings) = app.settings.updateShare(transform)

    Hint(R.string.sharing_settings_hint)

    // both roles: how others see you and this phone's speaker
    Header(R.string.sharing_you)
    OutlinedTextField(
        value = s.name,
        onValueChange = { v -> set { it.copy(name = v.take(32)) } },
        label = { Text(stringResource(R.string.sharing_your_name)) },
        placeholder = { Text(Build.MODEL) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
    )
    Label(R.string.sharing_your_color)
    ColorPicker(s.color, { c -> set { it.copy(color = c) } }, Modifier.padding(horizontal = 16.dp))
    ListItem(
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        headlineContent = { Text(stringResource(R.string.sharing_speaker_delay, s.speakerDelayMs)) },
        supportingContent = {
            Column {
                Slider(
                    value = s.speakerDelayMs.toFloat(),
                    onValueChange = { v -> set { it.copy(speakerDelayMs = (v / 10).toInt() * 10) } },
                    valueRange = -500f..500f,
                )
                Text(stringResource(R.string.sharing_speaker_delay_setting_hint), style = MaterialTheme.typography.bodySmall)
            }
        },
    )

    HorizontalDivider(Modifier.padding(vertical = 8.dp))
    Header(R.string.sharing_as_host)
    OutlinedTextField(
        value = s.sessionName,
        onValueChange = { v -> set { it.copy(sessionName = v.take(40)) } },
        label = { Text(stringResource(R.string.sharing_session_name)) },
        placeholder = { Text(app.share.sessionName(s.copy(sessionName = ""))) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
    )

    Label(R.string.sharing_network)
    Row(Modifier.padding(horizontal = 16.dp)) {
        ShareNetworkMode.entries.forEach { m -> Chip(s.network == m, m.label()) { set { it.copy(network = m) } } }
    }
    WifiDirectGroup(s, ::set)
    Toggle(R.string.sharing_party_default, s.partyDefault) { v -> set { it.copy(partyDefault = v) } }

    Label(R.string.sharing_code_setting)
    Row(Modifier.padding(horizontal = 16.dp)) {
        Chip(!s.fixedCode, R.string.sharing_code_random) { set { it.copy(fixedCode = false) } }
        Chip(s.fixedCode, R.string.sharing_code_fixed) { set { it.copy(fixedCode = true) } }
    }
    Reveal(s.fixedCode) {
        OutlinedTextField(
            value = s.code,
            onValueChange = { v -> set { it.copy(code = v.filter(Char::isDigit).take(8)) } },
            label = { Text(stringResource(R.string.sharing_code)) },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
        )
    }
    OutlinedTextField(
        value = s.port.toString(),
        onValueChange = { v -> v.filter(Char::isDigit).take(5).toIntOrNull()?.let { p -> set { it.copy(port = p.coerceIn(0, 65535)) } } },
        label = { Text(stringResource(R.string.sharing_port)) },
        supportingText = { Text(stringResource(R.string.sharing_port_hint)) },
        singleLine = true,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
    )

    Label(R.string.sharing_perm_add, top = 16.dp)
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        ShareAddMode.entries.forEach { m -> Chip(s.add == m, m.labelRes()) { set { it.copy(add = m) } } }
    }
    Reveal(s.add == ShareAddMode.LIMIT) {
        ListItem(
            colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
            headlineContent = { Text(stringResource(R.string.sharing_perm_max, s.maxPerGuest)) },
            supportingContent = {
                Slider(
                    value = s.maxPerGuest.toFloat(),
                    onValueChange = { v -> set { it.copy(maxPerGuest = v.toInt()) } },
                    valueRange = 1f..10f,
                    steps = 8,
                )
            },
        )
    }
    Label(R.string.sharing_perm_order)
    Row(Modifier.padding(horizontal = 16.dp)) {
        ShareQueueOrder.entries.forEach { o -> Chip(s.order == o, o.labelRes()) { set { it.copy(order = o) } } }
    }
    Hint(if (s.order == ShareQueueOrder.TURNS) R.string.sharing_order_turns_hint else R.string.sharing_order_first_hint)
    Label(R.string.sharing_perm_skip)
    Row(Modifier.padding(horizontal = 16.dp)) {
        ShareSkipMode.entries.forEach { m -> Chip(s.skip == m, m.labelRes()) { set { it.copy(skip = m) } } }
    }
    Label(R.string.sharing_perm_edit)
    Row(Modifier.padding(horizontal = 16.dp)) {
        ShareEditMode.entries.forEach { m -> Chip(s.edit == m, m.labelRes()) { set { it.copy(edit = m) } } }
    }
    Toggle(R.string.sharing_perm_playback, s.playback) { v -> set { it.copy(playback = v) } }
    Toggle(R.string.sharing_perm_explicit, s.allowExplicit) { v -> set { it.copy(allowExplicit = v) } }
    Toggle(R.string.sharing_perm_drop, s.dropSongsOnLeave) { v -> set { it.copy(dropSongsOnLeave = v) } }

    ListItem(
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        headlineContent = {
            Text(
                if (s.partyStartWaitSec == 0) stringResource(R.string.sharing_start_wait_off)
                else stringResource(R.string.sharing_start_wait, s.partyStartWaitSec),
            )
        },
        supportingContent = {
            Slider(
                value = s.partyStartWaitSec.toFloat(),
                onValueChange = { v -> set { it.copy(partyStartWaitSec = v.roundToInt()) } },
                valueRange = 0f..10f,
                steps = 9,
            )
        },
    )
    Hint(R.string.sharing_start_wait_hint)
    Toggle(R.string.sharing_send_levels, s.sendLevels) { v -> set { it.copy(sendLevels = v) } }
    Hint(R.string.sharing_send_levels_hint)

    Label(R.string.sharing_notices, top = 16.dp)
    Hint(R.string.sharing_notices_hint)
    Toggle(R.string.sharing_notice_joined_setting, s.noticeJoined) { v -> set { it.copy(noticeJoined = v) } }
    Toggle(R.string.sharing_notice_left_setting, s.noticeLeft) { v -> set { it.copy(noticeLeft = v) } }
    Toggle(R.string.sharing_notice_added_setting, s.noticeAdded) { v -> set { it.copy(noticeAdded = v) } }
    Toggle(R.string.sharing_notice_approval_setting, s.noticeApproval) { v -> set { it.copy(noticeApproval = v) } }
    ListItem(
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        headlineContent = { Text(stringResource(R.string.sharing_notice_seconds, s.noticeSeconds)) },
        supportingContent = {
            Slider(
                value = s.noticeSeconds.toFloat(),
                onValueChange = { v -> set { it.copy(noticeSeconds = v.toInt()) } },
                valueRange = 2f..10f,
                steps = 7,
            )
        },
    )

    HorizontalDivider(Modifier.padding(vertical = 8.dp))
    Header(R.string.sharing_as_guest)
    Toggle(R.string.sharing_play_along_default, s.playAlong) { v -> set { it.copy(playAlong = v) } }
    Toggle(R.string.sharing_guest_opens_player, s.guestOpensPlayer) { v -> set { it.copy(guestOpensPlayer = v) } }
    Hint(R.string.sharing_guest_opens_player_hint)
    Toggle(R.string.sharing_sync_by_speed, s.syncBySpeed) { v -> set { it.copy(syncBySpeed = v) } }
    Hint(R.string.sharing_sync_by_speed_hint)
    Toggle(R.string.sharing_preload_songs, s.preloadSongs) { v -> set { it.copy(preloadSongs = v) } }
    Hint(R.string.sharing_preload_songs_hint)

    // both roles: how hubs and guests find each other
    HorizontalDivider(Modifier.padding(vertical = 8.dp))
    Header(R.string.sharing_finding)
    Toggle(R.string.sharing_discover, s.discover) { v -> set { it.copy(discover = v) } }
    // only matters while looking nearby
    Toggle(R.string.sharing_discover_everywhere, s.discoverEverywhere, enabled = s.discover) { v ->
        set { it.copy(discoverEverywhere = v) }
    }
    // a hub on its own Wi-Fi Direct group; a LAN hub is already on the home Wi-Fi
    Toggle(R.string.sharing_announce_lan, s.announceLan, enabled = s.network == ShareNetworkMode.WIFI_DIRECT) { v ->
        set { it.copy(announceLan = v) }
    }
    Toggle(R.string.sharing_bluetooth, s.bluetooth) { v -> set { it.copy(bluetooth = v) } }
    // once guests are in, a Wi-Fi Direct hub stops scanning unless asked
    Toggle(R.string.sharing_visible_with_guests, s.visibleWithGuests, enabled = s.network == ShareNetworkMode.WIFI_DIRECT) { v ->
        set { it.copy(visibleWithGuests = v) }
    }
    Hint(R.string.sharing_visible_with_guests_hint)
}

/** "DIRECT-" plus at least two characters, as Wi-Fi Direct wants; null while it can't be saved. */
private fun groupNameToSave(typed: String): String? {
    val v = typed.trim()
    return when {
        v.isEmpty() -> ""
        v.startsWith("DIRECT-") -> v.takeIf { it.length >= "DIRECT-".length + 2 }?.take(32)
        else -> "DIRECT-hf-$v".take(32)
    }
}

/**
 * A fixed Wi-Fi Direct name and password, so a player without a camera joins once and
 * remembers it. Blank means generated each time. What is typed stays as typed; only valid
 * values are saved.
 */
@Composable
private fun WifiDirectGroup(s: ShareSettings, set: ((ShareSettings) -> ShareSettings) -> Unit) {
    var name by rememberSaveable { mutableStateOf(s.groupName) }
    var pass by rememberSaveable { mutableStateOf(s.groupPass) }
    val saved = groupNameToSave(name)
    OutlinedTextField(
        value = name,
        onValueChange = { v ->
            name = v.take(32)
            groupNameToSave(name)?.let { g -> set { it.copy(groupName = g) } }
        },
        label = { Text(stringResource(R.string.sharing_group_name)) },
        isError = saved == null,
        supportingText = when {
            saved == null -> { { Text(stringResource(R.string.sharing_group_name_short)) } }
            // the prefix was added for them
            saved.isNotEmpty() && saved != name.trim() -> { { Text(saved) } }
            else -> null
        },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
    )
    val shortPass = pass.isNotEmpty() && pass.length < 8
    OutlinedTextField(
        value = pass,
        onValueChange = { v ->
            pass = v.take(63)
            if (pass.isEmpty() || pass.length >= 8) set { it.copy(groupPass = pass) }
        },
        label = { Text(stringResource(R.string.sharing_group_pass)) },
        isError = shortPass,
        supportingText = if (shortPass) { { Text(stringResource(R.string.sharing_group_pass_short)) } } else null,
        singleLine = true,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
    )
    Hint(R.string.sharing_group_hint)
}

private fun ShareAddMode.labelRes(): Int = when (this) {
    ShareAddMode.FREE -> R.string.sharing_add_free
    ShareAddMode.APPROVE -> R.string.sharing_add_approve
    ShareAddMode.LIMIT -> R.string.sharing_add_limit
}

private fun ShareQueueOrder.labelRes(): Int = when (this) {
    ShareQueueOrder.FIRST_COME -> R.string.sharing_order_first
    ShareQueueOrder.TURNS -> R.string.sharing_order_turns
}

private fun ShareSkipMode.labelRes(): Int = when (this) {
    ShareSkipMode.NOBODY -> R.string.sharing_skip_nobody
    ShareSkipMode.EVERYONE -> R.string.sharing_skip_everyone
    ShareSkipMode.VOTE -> R.string.sharing_skip_vote
}

private fun ShareEditMode.labelRes(): Int = when (this) {
    ShareEditMode.OWN -> R.string.sharing_edit_own
    ShareEditMode.ALL -> R.string.sharing_edit_all
    ShareEditMode.NONE -> R.string.sharing_edit_none
}

@Composable
private fun Label(text: Int, top: Dp = 12.dp) {
    Text(
        stringResource(text),
        style = MaterialTheme.typography.labelLarge,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = top, bottom = 4.dp),
    )
}

/** A hint under a setting; when the choice changes it, the new text fades in. */
@Composable
private fun Hint(text: Int) {
    val motion = LocalMotion.current
    AnimatedContent(targetState = text, transitionSpec = { motion.swap() }, label = "hint") { shown ->
        Text(
            stringResource(shown),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp),
        )
    }
}

/** Options that only matter for one choice open and close below it. */
@Composable
private fun Reveal(visible: Boolean, content: @Composable () -> Unit) {
    val motion = LocalMotion.current
    AnimatedVisibility(visible, enter = motion.reveal(), exit = motion.conceal()) {
        Column { content() }
    }
}

@Composable
private fun Chip(selected: Boolean, label: Int, onClick: () -> Unit) {
    SelectChip(
        selected = selected,
        onClick = onClick,
        label = { Text(stringResource(label)) },
        modifier = Modifier.padding(end = 8.dp),
    )
}

private fun Specials.labelRes(): Int = when (this) {
    Specials.OFF -> R.string.specials_off
    Specials.SOMETIMES -> R.string.specials_sometimes
    Specials.OFTEN -> R.string.specials_often
}

private fun Turns.labelRes(): Int = when (this) {
    Turns.OFF -> R.string.turns_off
    Turns.DANCES -> R.string.turns_dances
    Turns.ALL -> R.string.turns_all
}

private fun Temper.labelRes(): Int = when (this) {
    Temper.OFF -> R.string.temper_off
    Temper.PATIENT -> R.string.temper_patient
    Temper.NORMAL -> R.string.temper_normal
    Temper.SHORT -> R.string.temper_short
}

private fun Temper.hintRes(): Int = when (this) {
    Temper.OFF -> R.string.temper_off_hint
    Temper.PATIENT -> R.string.temper_patient_hint
    Temper.NORMAL -> R.string.temper_normal_hint
    Temper.SHORT -> R.string.temper_short_hint
}

private fun MascotLimbs.labelRes(): Int = when (this) {
    MascotLimbs.WHITE -> R.string.limbs_white
    MascotLimbs.NAVY -> R.string.limbs_navy
    MascotLimbs.ACCENT -> R.string.limbs_accent
    MascotLimbs.OFF -> R.string.limbs_off
}

private fun PlayerLayout.labelRes(): Int = when (this) {
    PlayerLayout.CLASSIC -> R.string.layout_classic
    PlayerLayout.IMMERSIVE -> R.string.layout_immersive
    PlayerLayout.VINYL -> R.string.layout_vinyl
    PlayerLayout.STUDIO -> R.string.layout_studio
    PlayerLayout.RETRO_AMP -> R.string.layout_retro_amp
    PlayerLayout.LUNA -> R.string.layout_luna
    PlayerLayout.ALUMINUM -> R.string.layout_aluminum
    PlayerLayout.WEB2000 -> R.string.layout_web2000
}

private fun ProgressStyle.labelRes(): Int = when (this) {
    ProgressStyle.CLASSIC -> R.string.progress_classic
    ProgressStyle.WAVY -> R.string.progress_wavy
    ProgressStyle.WAVEFORM -> R.string.progress_waveform
    ProgressStyle.NEON -> R.string.progress_neon
    ProgressStyle.LED -> R.string.progress_led
    ProgressStyle.MINIMAL -> R.string.progress_minimal
}

private fun ProgressThickness.labelRes(): Int = when (this) {
    ProgressThickness.THIN -> R.string.progress_thin
    ProgressThickness.MEDIUM -> R.string.progress_medium
    ProgressThickness.THICK -> R.string.progress_thick
}

@Composable
private fun Header(text: Int) {
    SkinHeading(
        stringResource(text),
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@Composable
private fun Toggle(text: Int, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    ListItem(
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surface),
        headlineContent = {
            // greys out (and back) along with its switch
            val ink by animateColorAsState(
                if (enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f),
                LocalMotion.current.color(),
                label = "toggle",
            )
            Text(stringResource(text), color = ink)
        },
        trailingContent = {
            val haptics = haptics()
            Switch(checked = checked, onCheckedChange = { v -> haptics.toggle(v); onChange(v) }, enabled = enabled)
        },
    )
}

private fun ImmersiveShade.labelRes() = when (this) {
    ImmersiveShade.AUTO -> R.string.shade_auto
    ImmersiveShade.STRONG -> R.string.shade_strong
    ImmersiveShade.SOFT -> R.string.shade_soft
    ImmersiveShade.OFF -> R.string.shade_off
}
