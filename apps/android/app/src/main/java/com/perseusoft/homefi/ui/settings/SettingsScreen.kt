package com.perseusoft.homefi.ui.settings

import com.perseusoft.homefi.ui.AiIcon
import com.perseusoft.homefi.ui.haptics
import androidx.compose.material.icons.rounded.GraphicEq
import com.perseusoft.homefi.ui.theme.SkinHeading
import com.perseusoft.homefi.ui.theme.fixedDark
import com.perseusoft.homefi.ui.ConfirmDialog
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import com.perseusoft.homefi.data.SettingsWidth
import com.perseusoft.homefi.ui.theme.skinOf
import kotlinx.coroutines.delay
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
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.lazy.LazyListScope
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
import androidx.compose.material.icons.rounded.SwapHoriz
import com.perseusoft.homefi.ui.SelectChip
import com.perseusoft.homefi.core.AlbumEditionPref
import com.perseusoft.homefi.core.AlbumExplicitPref
import com.perseusoft.homefi.core.AlbumSpatialPref
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
import com.perseusoft.homefi.data.MascotHold
import com.perseusoft.homefi.data.MascotLimbs
import com.perseusoft.homefi.ui.player.label
import com.perseusoft.homefi.ui.player.lyricsOffsetText
import com.perseusoft.homefi.data.ImmersiveShade
import com.perseusoft.homefi.data.PlayerLayout
import com.perseusoft.homefi.data.ProgressStyle
import com.perseusoft.homefi.data.ProgressThickness
import com.perseusoft.homefi.data.ShareSettings
import com.perseusoft.homefi.data.SignalStyle
import com.perseusoft.homefi.data.Storage
import com.perseusoft.homefi.data.ThemeMode
import com.perseusoft.homefi.data.UpdateStatus
import com.perseusoft.homefi.ui.WhatsNewDialog
import androidx.compose.foundation.layout.Spacer
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

/** The captions that group the page list, as on the desktop. */
private enum class SettingsSection(val title: Int) {
    APPEARANCE(R.string.settings_group_appearance),
    SOUND(R.string.settings_group_sound),
    CONNECTIONS(R.string.settings_group_connections),
    APP(R.string.settings_group_app),
}

/**
 * A row on the settings home and the sub-page it opens. [keywords] are the labels inside the
 * page, so the search finds "Port" under shared control. Declaration order is the list order.
 */
private enum class SettingsPage(val title: Int, val icon: ImageVector, val section: SettingsSection, val keywords: List<Int>) {
    LOOK(
        R.string.settings_look,
        Icons.Rounded.Palette,
        SettingsSection.APPEARANCE,
        listOf(
            R.string.settings_appearance, R.string.theme_light, R.string.theme_dark, R.string.settings_player,
            R.string.layout_title, R.string.layout_everywhere, R.string.immersive_shade, R.string.progress_title, R.string.progress_animated,
            R.string.settings_motion, R.string.motion_subtle, R.string.settings_haptics, R.string.settings_width,
            R.string.signal_chain, R.string.signal_style_detailed, R.string.signal_animate, R.string.signal_bitperfect, R.string.signal_resampled,
        ),
    ),
    MASCOT(
        R.string.mascot_title,
        Icons.Rounded.Face,
        SettingsSection.APPEARANCE,
        listOf(R.string.mascot_style, R.string.mascot_dance, R.string.mascot_specials, R.string.mascot_turns, R.string.mascot_limbs),
    ),
    LYRICS(
        R.string.lyrics_title,
        Icons.Rounded.Lyrics,
        SettingsSection.APPEARANCE,
        listOf(
            R.string.lyrics_mode, R.string.lyrics_mode_karaoke, R.string.lyrics_size, R.string.lyrics_timing,
            R.string.lyrics_auto_open, R.string.lyrics_mascot,
        ),
    ),
    SOUND(
        R.string.settings_sound,
        Icons.Rounded.GraphicEq,
        SettingsSection.SOUND,
        listOf(
            R.string.eq_title, R.string.eq_on, R.string.eq_open, R.string.eq_assistant, R.string.eq_preamp_auto,
            R.string.eq_autoeq, R.string.eq_presets, R.string.eq_taste, R.string.eq_bass, R.string.eq_treble,
            R.string.sound_layers_on, R.string.sound_rules, R.string.sound_loudness, R.string.sound_how,
            R.string.sound_settings_react_button, R.string.sound_settings_layer_chip, R.string.sound_settings_suggestions,
        ),
    ),
    PLAYBACK(
        R.string.settings_playback,
        Icons.Rounded.SwapHoriz,
        SettingsSection.SOUND,
        listOf(
            R.string.crossfade_on, R.string.crossfade_keywords, R.string.crossfade_curve, R.string.crossfade_smart,
            R.string.crossfade_album_runs, R.string.crossfade_manual, R.string.crossfade_low_power,
        ),
    ),
    AI(
        R.string.settings_ai,
        AiIcon,
        SettingsSection.SOUND,
        listOf(
            R.string.ai_provider, R.string.ai_key, R.string.ai_get_key, R.string.ai_test, R.string.ai_model,
            R.string.ai_base_url, R.string.ai_remove, R.string.eq_assistant,
        ),
    ),
    SHARING(
        R.string.sharing_settings,
        Icons.Rounded.Groups,
        SettingsSection.CONNECTIONS,
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
    DOWNLOADS(
        R.string.settings_downloads,
        Icons.Rounded.Download,
        SettingsSection.CONNECTIONS,
        listOf(
            R.string.settings_quality, R.string.settings_embed_cover, R.string.settings_embed_lyrics,
            R.string.settings_videos, R.string.settings_skip_existing, R.string.settings_threads, R.string.settings_template,
        ),
    ),
    STORAGE(
        R.string.settings_storage,
        Icons.Rounded.Folder,
        SettingsSection.CONNECTIONS,
        listOf(R.string.settings_folder_change, R.string.settings_folder_default, R.string.settings_storage_grant),
    ),
    ACCOUNT(
        R.string.settings_account,
        Icons.Rounded.AccountCircle,
        SettingsSection.CONNECTIONS,
        listOf(
            R.string.settings_logout, R.string.login_connect, R.string.settings_lazy_lists, R.string.settings_album_versions,
            R.string.settings_album_group, R.string.settings_album_explicit, R.string.settings_album_spatial, R.string.settings_album_edition,
            R.string.settings_album_keywords,
        ),
    ),
    ABOUT(
        R.string.settings_about,
        Icons.Rounded.Info,
        SettingsSection.APP,
        listOf(
            R.string.update_title, R.string.update_check_now, R.string.update_auto, R.string.update_prereleases,
            R.string.update_keywords, R.string.tidal_terms_title, R.string.tidal_terms,
        ),
    ),
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

/** How long "Saved" stays next to the page title after a change. */
private const val SAVED_VISIBLE_MS = 1200L

/**
 * Settings, laid out by the width it is given. Compact (under 600 dp): the page list, then a
 * page (system back returns). Medium and landscape: the same, with the content centred in a
 * 640 dp column (840 dp with "Settings width: Wide"). Expanded (840 dp and up): the page list
 * as a 280 dp pane next to the selected page.
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
    val account = app.account.account.collectAsStateWithLifecycle().value

    var page by rememberSaveable { mutableStateOf<SettingsPage?>(null) }
    val motion = LocalMotion.current
    val contentWidth = if (settings.settingsWidth == SettingsWidth.WIDE) WIDE_CONTENT_WIDTH else COMFORTABLE_CONTENT_WIDTH

    val summary: @Composable (SettingsPage) -> String = { p ->
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
            SettingsPage.SOUND -> soundSummary()
            SettingsPage.PLAYBACK -> playbackSummary()
            SettingsPage.AI -> aiSummary(settings.ai)
            SettingsPage.SHARING ->
                "${stringResource(settings.share.network.label())} · ${app.share.sessionName(settings.share)}"
            SettingsPage.ABOUT -> {
                val update by app.updates.state.collectAsStateWithLifecycle()
                val newer = update.release?.version?.takeIf {
                    update.status in setOf(UpdateStatus.AVAILABLE, UpdateStatus.DOWNLOADING, UpdateStatus.READY)
                }
                if (newer != null) stringResource(R.string.update_summary, BuildConfig.VERSION_NAME, newer)
                else "HomeFi ${BuildConfig.VERSION_NAME}"
            }
        }
    }
    val body: @Composable (SettingsPage) -> Unit = { p ->
        PageBody(p, loggedIn, onLogin, onLoggedOut, shared, dir) {
            legacyPermission.launch(Manifest.permission.WRITE_EXTERNAL_STORAGE)
        }
    }

    BoxWithConstraints(Modifier.fillMaxSize()) {
        if (maxWidth >= EXPANDED_MIN_WIDTH) {
            val open = page ?: SettingsPage.entries.first()
            Row(Modifier.fillMaxSize()) {
                SettingsHome(
                    summary = summary,
                    selected = open,
                    onOpen = { page = it },
                    contentWidth = NAV_PANE_WIDTH,
                    modifier = Modifier.width(NAV_PANE_WIDTH),
                )
                VerticalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                AnimatedContent(
                    targetState = open,
                    transitionSpec = { motion.swap() },
                    label = "settings-pane",
                    modifier = Modifier.weight(1f),
                ) { p ->
                    SettingsPageScaffold(p.title, onBack = null, contentWidth = contentWidth) { body(p) }
                }
            }
        } else {
            BackHandler(enabled = page != null) { page = null }
            AnimatedContent(
                targetState = page,
                // a page comes in from the end, back returns from the start
                transitionSpec = { motion.screens(forward = targetState != null) },
                label = "settings",
            ) { open ->
                if (open == null) {
                    SettingsHome(summary = summary, selected = null, onOpen = { page = it }, contentWidth = contentWidth)
                } else {
                    SettingsPageScaffold(open.title, onBack = { page = null }, contentWidth = contentWidth) { body(open) }
                }
            }
        }
    }
}

@Composable
private fun PageBody(
    page: SettingsPage,
    loggedIn: Boolean,
    onLogin: () -> Unit,
    onLoggedOut: () -> Unit,
    shared: Boolean,
    dir: String,
    onLegacyPermission: () -> Unit,
) {
    when (page) {
        SettingsPage.ACCOUNT -> AccountSettings(loggedIn, onLogin, onLoggedOut)
        SettingsPage.DOWNLOADS -> DownloadSettings()
        SettingsPage.STORAGE -> StorageSettings(shared, dir, onLegacyPermission)
        SettingsPage.LOOK -> {
            AppearanceSettings()
            PlayerSettings()
        }
        SettingsPage.MASCOT -> MascotSettings()
        SettingsPage.LYRICS -> LyricsSettings()
        SettingsPage.SOUND -> SoundSettings()
        SettingsPage.PLAYBACK -> PlaybackSettings()
        SettingsPage.AI -> AiSettingsSection()
        SettingsPage.SHARING -> ShareSettingsSection()
        SettingsPage.ABOUT -> AboutSettings()
    }
}

/**
 * The page list grouped under captions, with the search in a sticky header above it. While
 * searching the list is flat and says which setting matched. [selected] marks the open page
 * in the two-pane layout.
 */
@Composable
private fun SettingsHome(
    summary: @Composable (SettingsPage) -> String,
    selected: SettingsPage?,
    onOpen: (SettingsPage) -> Unit,
    contentWidth: Dp,
    modifier: Modifier = Modifier,
) {
    val res = LocalContext.current.resources
    var query by rememberSaveable { mutableStateOf("") }
    val searching = query.isNotBlank()
    val matches = remember(query) {
        SettingsPage.entries.mapNotNull { p ->
            if (!searching) p to null else p.match(res, query)?.let { p to it }
        }
    }
    val listState = rememberLazyListState()
    val scrolled by remember { derivedStateOf { listState.firstVisibleItemIndex > 0 || listState.firstVisibleItemScrollOffset > 0 } }

    Box(modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        ListEntrance {
            LazyColumn(
                state = listState,
                modifier = Modifier.widthIn(max = contentWidth).fillMaxSize(),
                contentPadding = PaddingValues(bottom = 16.dp),
            ) {
                stickyHeader(key = "search") {
                    Column(Modifier.background(MaterialTheme.colorScheme.surface)) {
                        OutlinedTextField(
                            value = query,
                            onValueChange = { query = it },
                            placeholder = { Text(stringResource(R.string.settings_search)) },
                            leadingIcon = { Icon(Icons.Rounded.Search, null) },
                            trailingIcon = {
                                if (query.isNotEmpty()) {
                                    IconButton(onClick = { query = "" }) { Icon(Icons.Rounded.Clear, stringResource(R.string.settings_search_clear)) }
                                }
                            },
                            singleLine = true,
                            shape = MaterialTheme.shapes.extraLarge,
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp, vertical = 8.dp),
                        )
                        if (scrolled) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                    }
                }
                if (matches.isEmpty()) {
                    item(key = "none") {
                        Column(
                            Modifier.fillMaxWidth().padding(32.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(Icons.Rounded.Search, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                            Text(
                                stringResource(R.string.settings_search_none),
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            TextButton(onClick = { query = "" }) { Text(stringResource(R.string.settings_search_clear)) }
                        }
                    }
                }
                var rowIndex = 0
                SettingsSection.entries.forEach { section ->
                    val inSection = matches.filter { it.first.section == section }
                    if (inSection.isEmpty()) return@forEach
                    if (!searching) {
                        item(key = "section-${section.name}") {
                            SkinHeading(
                                stringResource(section.title),
                                style = MaterialTheme.typography.labelLarge,
                                color = MaterialTheme.colorScheme.primary,
                                modifier = Modifier
                                    .padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp)
                                    .semantics { heading() },
                            )
                        }
                    }
                    // rows cascade in, and slide together as the search filters them
                    inSection.forEach { (p, hit) ->
                        val i = rowIndex++
                        item(key = p.name) {
                            val on = p == selected
                            ListItem(
                                colors = ListItemDefaults.colors(
                                    containerColor = if (on) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surface,
                                ),
                                leadingContent = { Icon(p.icon, null, tint = MaterialTheme.colorScheme.primary) },
                                headlineContent = { Text(stringResource(p.title)) },
                                // while searching, say which setting matched instead of the current values
                                supportingContent = { Text(hit ?: summary(p), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                trailingContent = {
                                    if (selected == null) Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null)
                                },
                                modifier = itemMotion(i)
                                    .heightIn(min = 56.dp)
                                    .selectable(selected = on, role = Role.Button) { onOpen(p) },
                            )
                        }
                    }
                }
            }
        }
    }
}

/**
 * A page: sticky header (back button in the single-pane layouts, title, a short "Saved" after
 * a change) over the scrolling content, centred in a column of at most [contentWidth].
 */
@Composable
private fun SettingsPageScaffold(
    title: Int,
    onBack: (() -> Unit)?,
    contentWidth: Dp,
    content: @Composable () -> Unit,
) {
    val scroll = rememberScrollState()
    Column(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surface), contentAlignment = Alignment.TopCenter) {
            PageHeader(title, onBack, Modifier.widthIn(max = contentWidth))
        }
        if (scroll.value > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
        Box(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .verticalScroll(scroll),
            contentAlignment = Alignment.TopCenter,
        ) {
            Column(Modifier.widthIn(max = contentWidth).fillMaxWidth().padding(bottom = 24.dp)) { content() }
        }
    }
}

/** Top of a page; the app bar above stays the app's own. */
@Composable
private fun PageHeader(title: Int, onBack: (() -> Unit)?, modifier: Modifier = Modifier) {
    Row(
        modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 4.dp).heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (onBack != null) {
            IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
        } else {
            Spacer(Modifier.width(16.dp))
        }
        Text(
            stringResource(title),
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.semantics { heading() },
        )
        Spacer(Modifier.weight(1f))
        SavedIndicator()
        Spacer(Modifier.width(12.dp))
    }
}

/** "Saved" for a moment after any setting changes; read out politely, instant with motion off. */
@Composable
private fun SavedIndicator() {
    val settings by LocalContext.current.homefi.settings.settings.collectAsStateWithLifecycle()
    val motion = LocalMotion.current
    var visible by remember { mutableStateOf(false) }
    var first by remember { mutableStateOf(true) }
    LaunchedEffect(settings) {
        // the value the page opened with is not a change
        if (first) { first = false; return@LaunchedEffect }
        visible = true
        delay(SAVED_VISIBLE_MS)
        visible = false
    }
    AnimatedVisibility(
        visible,
        enter = if (motion.enabled) fadeIn() else EnterTransition.None,
        exit = if (motion.enabled) fadeOut() else ExitTransition.None,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
        ) {
            Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(16.dp))
            Spacer(Modifier.width(4.dp))
            Text(
                stringResource(R.string.settings_saved),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun AccountSettings(loggedIn: Boolean, onLogin: () -> Unit, onLoggedOut: () -> Unit) {
    val app = LocalContext.current.homefi
    val scope = rememberCoroutineScope()
    val account = app.account.account.collectAsStateWithLifecycle().value
    SettingsGroup(title = stringResource(R.string.settings_account)) {
        if (account == null) {
            SettingRow(
                label = stringResource(R.string.settings_account_none),
                trailing = { TextButton(onClick = onLogin) { Text(stringResource(R.string.login_connect)) } },
            )
        } else {
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
                            app.account.signedOut()
                            onLoggedOut()
                        }
                    },
                    onDismiss = { confirming = false },
                )
            }
            SettingRow(
                label = stringResource(R.string.settings_account_info, account.userId, account.countryCode),
                trailing = {
                    TextButton(onClick = { confirming = true }) {
                        Text(stringResource(R.string.settings_logout), color = MaterialTheme.colorScheme.error)
                    }
                },
            )
        }
    }

    val settings by app.settings.settings.collectAsStateWithLifecycle()
    SettingsGroup(title = stringResource(R.string.settings_tidal_lists)) {
        SwitchRow(
            stringResource(R.string.settings_lazy_lists), settings.lazyLists,
            hint = stringResource(R.string.settings_lazy_lists_hint),
        ) { v -> app.settings.update { it.copy(lazyLists = v) } }
    }

    // the same four options as the desktop's Settings → Tidal → Album versions
    SettingsGroup(title = stringResource(R.string.settings_album_versions)) {
        SwitchRow(
            stringResource(R.string.settings_album_group), settings.albumVersionsGroup,
            hint = stringResource(R.string.settings_album_group_hint),
        ) { v -> app.settings.update { it.copy(albumVersionsGroup = v) } }
        Reveal(settings.albumVersionsGroup) {
            RowDivider()
            SegmentedRow(
                stringResource(R.string.settings_album_explicit),
                listOf(AlbumExplicitPref.EXPLICIT, AlbumExplicitPref.CLEAN), settings.albumExplicit,
                label = { stringResource(if (it == AlbumExplicitPref.EXPLICIT) R.string.album_pref_explicit else R.string.album_pref_clean) },
                onSelect = { v -> app.settings.update { it.copy(albumExplicit = v) } },
                hint = stringResource(R.string.settings_album_explicit_hint),
            )
            RowDivider()
            SegmentedRow(
                stringResource(R.string.settings_album_spatial),
                listOf(AlbumSpatialPref.STEREO, AlbumSpatialPref.DOLBY_ATMOS), settings.albumSpatial,
                label = { stringResource(if (it == AlbumSpatialPref.STEREO) R.string.album_pref_stereo else R.string.album_pref_atmos) },
                onSelect = { v -> app.settings.update { it.copy(albumSpatial = v) } },
                hint = stringResource(R.string.settings_album_spatial_hint),
            )
            RowDivider()
            SegmentedRow(
                stringResource(R.string.settings_album_edition),
                listOf(AlbumEditionPref.ORIGINAL, AlbumEditionPref.REMASTER), settings.albumEdition,
                label = { stringResource(if (it == AlbumEditionPref.ORIGINAL) R.string.album_pref_original else R.string.album_pref_remaster) },
                onSelect = { v -> app.settings.update { it.copy(albumEdition = v) } },
                hint = stringResource(R.string.settings_album_edition_hint),
            )
        }
    }
}

/** Quality, tags and naming of what gets downloaded. */
@Composable
private fun DownloadSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    SettingsGroup(title = stringResource(R.string.settings_quality)) {
        SegmentedRow(
            stringResource(R.string.settings_quality), AudioQuality.entries, settings.quality,
            label = { stringResource(it.label()) },
            onSelect = { q -> app.settings.update { it.copy(quality = q) } },
            hint = stringResource(R.string.settings_quality_hint),
        )
    }
    SettingsGroup(title = stringResource(R.string.settings_downloads)) {
        SwitchRow(stringResource(R.string.settings_embed_cover), settings.embedCover) { v -> app.settings.update { it.copy(embedCover = v) } }
        RowDivider()
        SwitchRow(stringResource(R.string.settings_embed_lyrics), settings.embedLyrics) { v -> app.settings.update { it.copy(embedLyrics = v) } }
        RowDivider()
        SwitchRow(stringResource(R.string.settings_videos), settings.includeVideos) { v -> app.settings.update { it.copy(includeVideos = v) } }
        RowDivider()
        SwitchRow(stringResource(R.string.settings_skip_existing), settings.skipExisting) { v -> app.settings.update { it.copy(skipExisting = v) } }
        RowDivider()
        StackedSetting(stringResource(R.string.settings_threads, settings.threads)) {
            Slider(
                value = settings.threads.toFloat(),
                onValueChange = { v -> app.settings.update { it.copy(threads = v.toInt()) } },
                valueRange = 1f..6f,
                steps = 4,
            )
        }
        RowDivider()
        StackedSetting(stringResource(R.string.settings_template)) {
            OutlinedTextField(
                value = settings.template,
                onValueChange = { v -> app.settings.update { it.copy(template = v) } },
                placeholder = { Text("{album.artist}/{album.title}/{item.number:02d}. {item.title}") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
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

    SettingsGroup(title = stringResource(R.string.settings_storage)) {
        if (settings.downloadDir.isNotBlank() && settings.downloadDir != dir) {
            SettingsNote(stringResource(R.string.settings_folder_fallback, settings.downloadDir), error = true)
            RowDivider()
        }
        ActionRow(
            label = dir,
            onClick = { picking = true },
            trailing = {
                Row {
                    TextButton(onClick = { picking = true }) { Text(stringResource(R.string.settings_folder_change)) }
                    if (settings.downloadDir.isNotBlank()) {
                        TextButton(onClick = {
                            app.settings.update { it.copy(downloadDir = "") }
                            app.library.refresh()
                        }) { Text(stringResource(R.string.settings_folder_default)) }
                    }
                }
            },
        )
    }
    if (!shared) {
        SettingsGroup {
            SettingsNote(stringResource(R.string.settings_storage_hint))
            OutlinedButton(
                onClick = {
                    val intent = Storage.accessSettingsIntent(context)
                    if (intent != null) {
                        context.startActivity(intent)
                    } else if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) {
                        onLegacyPermission()
                    }
                },
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            ) { Text(stringResource(R.string.settings_storage_grant)) }
        }
    }
}

@Composable
private fun AppearanceSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    SettingsGroup(title = stringResource(R.string.settings_appearance)) {
        OptionGallery(
            options = ThemeMode.entries,
            selected = settings.theme,
            label = { stringResource(it.label()) },
            onSelect = { mode -> app.settings.update { it.copy(theme = mode) } },
        ) { mode -> ThemePreview(mode, settings.playerLayout) }
        // a skin that is always dark (or light) wins over this choice
        val fixed = if (settings.layoutEverywhere) settings.playerLayout.fixedDark() else null
        if (fixed != null) {
            SettingsNote(
                stringResource(
                    if (fixed) R.string.theme_fixed_dark else R.string.theme_fixed_light,
                    stringResource(settings.playerLayout.labelRes()),
                ),
            )
        }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.settings_motion), Motion.entries, settings.motion,
            label = { stringResource(it.labelRes()) },
            onSelect = { m -> app.settings.update { it.copy(motion = m) } },
            hint = stringResource(R.string.settings_motion_hint),
        )
        RowDivider()
        SwitchRow(
            stringResource(R.string.settings_haptics), settings.haptics,
            hint = stringResource(R.string.settings_haptics_hint),
        ) { v -> app.settings.update { it.copy(haptics = v) } }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.settings_width), SettingsWidth.entries, settings.settingsWidth,
            label = { stringResource(if (it == SettingsWidth.WIDE) R.string.settings_width_wide else R.string.settings_width_comfortable) },
            onSelect = { w -> app.settings.update { it.copy(settingsWidth = w) } },
            hint = stringResource(R.string.settings_width_hint),
        )
    }
}

/** A miniature of the app in a theme, drawn with the real colours of the current skin. */
@Composable
private fun ThemePreview(mode: ThemeMode, layout: PlayerLayout) {
    val light = remember(layout) { skinOf(layout, dark = false).colors }
    val dark = remember(layout) { skinOf(layout, dark = true).colors }
    Row(
        Modifier
            .fillMaxWidth()
            .height(PREVIEW_HEIGHT)
            .clip(MaterialTheme.shapes.small),
    ) {
        if (mode != ThemeMode.DARK) ThemeSwatch(light, Modifier.weight(1f))
        if (mode != ThemeMode.LIGHT) ThemeSwatch(dark, Modifier.weight(1f))
    }
}

private val PREVIEW_HEIGHT = 56.dp

@Composable
private fun ThemeSwatch(colors: ColorScheme, modifier: Modifier) {
    Column(
        modifier.fillMaxHeight().background(colors.background).padding(8.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(Modifier.fillMaxWidth(0.6f).height(8.dp).clip(CircleShape).background(colors.onBackground))
        Box(Modifier.fillMaxWidth(0.9f).height(6.dp).clip(CircleShape).background(colors.primary))
        Box(Modifier.fillMaxWidth(0.4f).height(6.dp).clip(CircleShape).background(colors.surfaceVariant))
    }
}

private fun Motion.labelRes(): Int = when (this) {
    Motion.AUTO -> R.string.motion_auto
    Motion.FULL -> R.string.motion_full
    Motion.SUBTLE -> R.string.motion_subtle
    Motion.OFF -> R.string.motion_off
}

@Composable
private fun AboutSettings() {
    val settings by LocalContext.current.homefi.settings.settings.collectAsStateWithLifecycle()
    SettingsGroup {
        SettingRow(label = "HomeFi ${BuildConfig.VERSION_NAME} · core ${coreVersion()}")
    }
    UpdateSettingsSection()
    SettingsGroup(title = stringResource(R.string.tidal_terms_title)) {
        Text(
            stringResource(R.string.tidal_terms),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
        )
        RowDivider()
        SettingsNote(
            if (settings.termsAccepted && settings.tidalTermsAcceptedAt > 0) {
                val date = java.text.DateFormat.getDateInstance(java.text.DateFormat.MEDIUM).format(java.util.Date(settings.tidalTermsAcceptedAt))
                stringResource(R.string.tidal_terms_accepted_on, date)
            } else {
                stringResource(R.string.tidal_terms_not_accepted)
            },
        )
    }
}

/** Settings → About → Updates: the same options as the desktop's (installing is Android's to ask). */
@Composable
private fun UpdateSettingsSection() {
    val app = LocalContext.current.homefi
    val updates = app.updates
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    if (!updates.enabled) {
        // the SR35 build (another app's id) and debug builds
        SettingsGroup(title = stringResource(R.string.update_title)) { SettingsNote(stringResource(R.string.update_usb)) }
        return
    }
    val ui by updates.state.collectAsStateWithLifecycle()
    var notes by remember { mutableStateOf(false) }
    val release = ui.release
    if (notes && release != null) WhatsNewDialog(release, onDismiss = { notes = false }, onOpenPage = updates::openPage)
    val line = when (ui.status) {
        UpdateStatus.CHECKING -> stringResource(R.string.update_checking)
        UpdateStatus.UP_TO_DATE -> stringResource(R.string.update_up_to_date, BuildConfig.VERSION_NAME)
        UpdateStatus.AVAILABLE -> stringResource(R.string.update_available, release?.version.orEmpty())
        UpdateStatus.DOWNLOADING -> stringResource(R.string.update_downloading, release?.version.orEmpty())
        UpdateStatus.READY -> stringResource(R.string.update_ready, release?.version.orEmpty())
        UpdateStatus.ERROR -> ui.error ?: stringResource(R.string.update_failed, release?.version.orEmpty())
        UpdateStatus.IDLE -> "HomeFi ${BuildConfig.VERSION_NAME}"
    }
    val checked = if (ui.lastChecked > 0) {
        val at = java.text.DateFormat.getDateTimeInstance(java.text.DateFormat.MEDIUM, java.text.DateFormat.SHORT).format(java.util.Date(ui.lastChecked))
        stringResource(R.string.update_last_checked, at)
    } else {
        stringResource(R.string.update_never_checked)
    }
    SettingsGroup(title = stringResource(R.string.update_title)) {
        SettingRow(label = line, hint = checked)
        Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedButton(
                onClick = { updates.check(manual = true) },
                enabled = ui.status != UpdateStatus.CHECKING && ui.status != UpdateStatus.DOWNLOADING,
            ) { Text(stringResource(R.string.update_check_now)) }
            Spacer(Modifier.width(8.dp))
            when (ui.status) {
                UpdateStatus.AVAILABLE -> TextButton(onClick = updates::download) { Text(stringResource(R.string.update_update)) }
                UpdateStatus.READY -> TextButton(onClick = updates::install) { Text(stringResource(R.string.update_install)) }
                else -> Unit
            }
            if (release != null && ui.status != UpdateStatus.UP_TO_DATE) {
                TextButton(onClick = { notes = true }) { Text(stringResource(R.string.update_whats_new)) }
            }
        }
        RowDivider()
        SwitchRow(
            stringResource(R.string.update_auto), settings.updateAutoCheck,
            hint = stringResource(R.string.update_auto_hint),
        ) { v -> app.settings.update { it.copy(updateAutoCheck = v) } }
        RowDivider()
        SwitchRow(
            stringResource(R.string.update_prereleases), settings.updatePrereleases,
            hint = stringResource(R.string.update_prereleases_hint),
        ) { v -> app.settings.update { it.copy(updatePrereleases = v) } }
        RowDivider()
        SettingsNote(stringResource(R.string.update_install_note))
        SettingsNote(stringResource(R.string.update_privacy))
    }
}

/** Layout and seek bar of the full-screen player, with live previews. */
@Composable
private fun PlayerSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    SettingsGroup(title = stringResource(R.string.settings_player)) {
        SegmentedRow(
            stringResource(R.string.layout_title), PlayerLayout.entries, settings.playerLayout,
            label = { stringResource(it.labelRes()) },
            onSelect = { l -> app.settings.update { it.copy(playerLayout = l) } },
        )
        RowDivider()
        SwitchRow(
            stringResource(R.string.layout_everywhere), settings.layoutEverywhere,
            hint = stringResource(if (settings.layoutEverywhere) R.string.layout_everywhere_hint else R.string.layout_everywhere_off_hint),
        ) { v -> app.settings.update { it.copy(layoutEverywhere = v) } }
        Reveal(settings.playerLayout == PlayerLayout.IMMERSIVE) {
            RowDivider()
            SegmentedRow(
                stringResource(R.string.immersive_shade), ImmersiveShade.entries, settings.immersiveShade,
                label = { stringResource(it.labelRes()) },
                onSelect = { shade -> app.settings.update { it.copy(immersiveShade = shade) } },
                hint = stringResource(R.string.immersive_shade_hint),
            )
        }
    }
    SettingsGroup(title = stringResource(R.string.progress_title)) {
        OptionGallery(
            options = ProgressStyle.entries,
            selected = settings.progressStyle,
            label = { stringResource(it.labelRes()) },
            onSelect = { style -> app.settings.update { it.copy(progressStyle = style) } },
        ) { style ->
            ProgressTrack(
                fraction = 0.62f,
                style = style,
                thickness = settings.progressThickness,
                animate = settings.progressAnimated,
                seed = 7,
                modifier = Modifier.fillMaxWidth().height(28.dp),
            )
        }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.progress_thickness), ProgressThickness.entries, settings.progressThickness,
            label = { stringResource(it.labelRes()) },
            onSelect = { t -> app.settings.update { it.copy(progressThickness = t) } },
        )
        RowDivider()
        SwitchRow(stringResource(R.string.progress_animated), settings.progressAnimated) { v ->
            app.settings.update { it.copy(progressAnimated = v) }
        }
    }
    SignalChainSettings()
}

/** The player's signal chain: Compact / Detailed / Off, and whether its links flow. */
@Composable
private fun SignalChainSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    // three states over two settings: off, or on with a style
    val current = when {
        !settings.signalChain -> null
        else -> settings.signalStyle
    }
    val options = listOf(SignalStyle.COMPACT, SignalStyle.DETAILED, null)
    SettingsGroup(title = stringResource(R.string.signal_chain)) {
        SegmentedRow(
            stringResource(R.string.signal_chain), options, current,
            label = { stringResource(when (it) { SignalStyle.COMPACT -> R.string.signal_style_compact; SignalStyle.DETAILED -> R.string.signal_style_detailed; else -> R.string.signal_style_off }) },
            onSelect = { style ->
                app.settings.update { if (style == null) it.copy(signalChain = false) else it.copy(signalChain = true, signalStyle = style) }
            },
            hint = stringResource(if (settings.signalChain) R.string.signal_chain_hint else R.string.signal_chain_off_hint),
        )
        RowDivider()
        SwitchRow(stringResource(R.string.signal_animate), settings.signalAnimate, enabled = settings.signalChain) { v ->
            app.settings.update { it.copy(signalAnimate = v) }
        }
    }
}

/** How the mascot looks and dances (the same choices as on the desktop). */
@Composable
private fun MascotSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    SettingsGroup(title = stringResource(R.string.settings_mascot_look), description = stringResource(R.string.mascot_hint)) {
        SegmentedRow(
            stringResource(R.string.mascot_style), listOf<Mood?>(null) + Mood.entries, settings.mascotMood,
            label = { stringResource(it?.label ?: R.string.mascot_auto) },
            onSelect = { m -> app.settings.update { it.copy(mascotMood = m) } },
            hint = stringResource(settings.mascotMood?.genres ?: R.string.mascot_auto_hint),
        )
        RowDivider()
        SegmentedRow(
            stringResource(R.string.mascot_limbs), MascotLimbs.entries, settings.mascotLimbs,
            label = { stringResource(it.labelRes()) },
            onSelect = { l -> app.settings.update { it.copy(mascotLimbs = l) } },
        )
    }
    SettingsGroup(title = stringResource(R.string.settings_mascot_moves)) {
        SegmentedRow(
            stringResource(R.string.mascot_dance), listOf<DanceId?>(null) + DanceId.entries, settings.mascotDance,
            label = { stringResource(it?.label ?: R.string.mascot_dance_auto) },
            onSelect = { d -> app.settings.update { it.copy(mascotDance = d) } },
            hint = stringResource(settings.mascotDance?.hint ?: R.string.mascot_dance_auto_hint),
        )
        // only matters while no dance is pinned
        Reveal(settings.mascotDance == null) {
            RowDivider()
            SegmentedRow(
                stringResource(R.string.mascot_specials), Specials.entries, settings.mascotSpecials,
                label = { stringResource(it.labelRes()) },
                onSelect = { sp -> app.settings.update { it.copy(mascotSpecials = sp) } },
            )
        }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.mascot_turns), Turns.entries, settings.mascotTurns,
            label = { stringResource(it.labelRes()) },
            onSelect = { t -> app.settings.update { it.copy(mascotTurns = t) } },
        )
    }
    SettingsGroup(title = stringResource(R.string.mascot_temper)) {
        ChoiceList(
            title = stringResource(R.string.mascot_temper),
            options = Temper.entries,
            selected = settings.mascotTemper,
            label = { stringResource(it.labelRes()) },
            hint = { stringResource(it.hintRes()) },
            onSelect = { t -> app.settings.update { it.copy(mascotTemper = t) } },
        )
        RowDivider()
        ChoiceList(
            title = stringResource(R.string.mascot_hold),
            options = MascotHold.entries,
            selected = settings.mascotHold,
            label = { stringResource(it.labelRes()) },
            hint = { stringResource(it.hintRes()) },
            onSelect = { h -> app.settings.update { it.copy(mascotHold = h) } },
        )
        RowDivider()
        SwitchRow(stringResource(R.string.mascot_dance_button), settings.mascotDanceButton) { v ->
            app.settings.update { it.copy(mascotDanceButton = v) }
        }
        Reveal(settings.mascotX != 0f || settings.mascotY != 0f || settings.mascotScale != 1f) {
            RowDivider()
            ActionRow(
                stringResource(R.string.mascot_reset),
                onClick = { app.settings.update { it.copy(mascotX = 0f, mascotY = 0f, mascotScale = 1f) } },
            )
        }
    }
}

/** The lyrics view in the player: mode, size, timing, HomeFito, opening by itself. */
@Composable
private fun LyricsSettings() {
    val app = LocalContext.current.homefi
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    SettingsGroup(title = stringResource(R.string.lyrics_title)) {
        SegmentedRow(
            stringResource(R.string.lyrics_mode), LyricsMode.entries, settings.lyricsMode,
            label = { stringResource(it.label()) },
            onSelect = { m -> app.settings.update { it.copy(lyricsMode = m) } },
            hint = stringResource(R.string.lyrics_mode_hint),
        )
        RowDivider()
        SegmentedRow(
            stringResource(R.string.lyrics_size), LyricsSize.entries, settings.lyricsSize,
            label = { stringResource(it.label()) },
            onSelect = { sz -> app.settings.update { it.copy(lyricsSize = sz) } },
        )
        RowDivider()
        StackedSetting(
            stringResource(R.string.lyrics_timing, lyricsOffsetText(settings.lyricsOffsetMs)),
            hint = stringResource(R.string.lyrics_timing_hint),
        ) {
            Slider(
                value = settings.lyricsOffsetMs.toFloat(),
                onValueChange = { v ->
                    val ms = ((v / LYRICS_OFFSET_STEP).roundToInt() * LYRICS_OFFSET_STEP).coerceIn(LYRICS_OFFSET_RANGE)
                    if (ms != settings.lyricsOffsetMs) app.settings.update { it.copy(lyricsOffsetMs = ms) }
                },
                // snaps to LYRICS_OFFSET_STEP in onValueChange (40 tick marks would only clutter it)
                valueRange = LYRICS_OFFSET_RANGE.first.toFloat()..LYRICS_OFFSET_RANGE.last.toFloat(),
            )
        }
        Reveal(settings.lyricsOffsetMs != 0) {
            TextButton(
                onClick = { app.settings.update { it.copy(lyricsOffsetMs = 0) } },
                modifier = Modifier.padding(horizontal = 8.dp),
            ) { Text(stringResource(R.string.lyrics_timing_reset)) }
        }
    }
    SettingsGroup {
        SwitchRow(
            stringResource(R.string.lyrics_auto_open), settings.lyricsAutoOpen,
            hint = stringResource(R.string.lyrics_auto_open_hint),
        ) { v -> app.settings.update { it.copy(lyricsAutoOpen = v) } }
        RowDivider()
        SwitchRow(
            stringResource(R.string.lyrics_mascot), settings.lyricsMascot,
            hint = stringResource(R.string.lyrics_mascot_hint),
        ) { v -> app.settings.update { it.copy(lyricsMascot = v) } }
    }
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

    // both roles: how others see you and this phone's speaker
    SettingsGroup(title = stringResource(R.string.sharing_you), description = stringResource(R.string.sharing_settings_hint)) {
        FieldRow { m ->
            OutlinedTextField(
                value = s.name,
                onValueChange = { v -> set { it.copy(name = v.take(32)) } },
                label = { Text(stringResource(R.string.sharing_your_name)) },
                placeholder = { Text(Build.MODEL) },
                singleLine = true,
                modifier = m,
            )
        }
        RowDivider()
        StackedSetting(stringResource(R.string.sharing_your_color)) {
            ColorPicker(s.color, { c -> set { it.copy(color = c) } }, Modifier)
        }
        RowDivider()
        SliderRow(
            stringResource(R.string.sharing_speaker_delay, s.speakerDelayMs), s.speakerDelayMs.toFloat(), -500f..500f,
            onChange = { v -> set { it.copy(speakerDelayMs = (v / 10).toInt() * 10) } },
            hint = stringResource(R.string.sharing_speaker_delay_setting_hint),
        )
    }

    SettingsGroup(title = stringResource(R.string.sharing_as_host)) {
        FieldRow { m ->
            OutlinedTextField(
                value = s.sessionName,
                onValueChange = { v -> set { it.copy(sessionName = v.take(40)) } },
                label = { Text(stringResource(R.string.sharing_session_name)) },
                placeholder = { Text(app.share.sessionName(s.copy(sessionName = ""))) },
                singleLine = true,
                modifier = m,
            )
        }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.sharing_network), ShareNetworkMode.entries, s.network,
            label = { stringResource(it.label()) },
            onSelect = { m -> set { it.copy(network = m) } },
        )
        WifiDirectGroup(s, ::set)
        RowDivider()
        SwitchRow(stringResource(R.string.sharing_party_default), s.partyDefault) { v -> set { it.copy(partyDefault = v) } }
        RowDivider()
        SegmentedRow(
            stringResource(R.string.sharing_code_setting), listOf(false, true), s.fixedCode,
            label = { stringResource(if (it) R.string.sharing_code_fixed else R.string.sharing_code_random) },
            onSelect = { fixed -> set { it.copy(fixedCode = fixed) } },
        )
        Reveal(s.fixedCode) {
            FieldRow { m ->
                OutlinedTextField(
                    value = s.code,
                    onValueChange = { v -> set { it.copy(code = v.filter(Char::isDigit).take(8)) } },
                    label = { Text(stringResource(R.string.sharing_code)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    modifier = m,
                )
            }
        }
        FieldRow { m ->
            OutlinedTextField(
                value = s.port.toString(),
                onValueChange = { v -> v.filter(Char::isDigit).take(5).toIntOrNull()?.let { p -> set { it.copy(port = p.coerceIn(0, 65535)) } } },
                label = { Text(stringResource(R.string.sharing_port)) },
                supportingText = { Text(stringResource(R.string.sharing_port_hint)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = m,
            )
        }
    }

    SettingsGroup(title = stringResource(R.string.settings_sharing_permissions)) {
        SegmentedRow(
            stringResource(R.string.sharing_perm_add), ShareAddMode.entries, s.add,
            label = { stringResource(it.labelRes()) },
            onSelect = { m -> set { it.copy(add = m) } },
        )
        Reveal(s.add == ShareAddMode.LIMIT) {
            RowDivider()
            SliderRow(
                stringResource(R.string.sharing_perm_max, s.maxPerGuest), s.maxPerGuest.toFloat(), 1f..10f,
                onChange = { v -> set { it.copy(maxPerGuest = v.toInt()) } }, steps = 8,
            )
        }
        RowDivider()
        ChoiceList(
            title = stringResource(R.string.sharing_perm_order),
            options = ShareQueueOrder.entries,
            selected = s.order,
            label = { stringResource(it.labelRes()) },
            hint = { stringResource(if (it == ShareQueueOrder.TURNS) R.string.sharing_order_turns_hint else R.string.sharing_order_first_hint) },
            onSelect = { o -> set { it.copy(order = o) } },
        )
        RowDivider()
        SegmentedRow(
            stringResource(R.string.sharing_perm_skip), ShareSkipMode.entries, s.skip,
            label = { stringResource(it.labelRes()) },
            onSelect = { m -> set { it.copy(skip = m) } },
        )
        RowDivider()
        SegmentedRow(
            stringResource(R.string.sharing_perm_edit), ShareEditMode.entries, s.edit,
            label = { stringResource(it.labelRes()) },
            onSelect = { m -> set { it.copy(edit = m) } },
        )
        RowDivider()
        SwitchRow(stringResource(R.string.sharing_perm_playback), s.playback) { v -> set { it.copy(playback = v) } }
        RowDivider()
        SwitchRow(stringResource(R.string.sharing_perm_explicit), s.allowExplicit) { v -> set { it.copy(allowExplicit = v) } }
        RowDivider()
        SwitchRow(stringResource(R.string.sharing_perm_drop), s.dropSongsOnLeave) { v -> set { it.copy(dropSongsOnLeave = v) } }
        RowDivider()
        SliderRow(
            if (s.partyStartWaitSec == 0) stringResource(R.string.sharing_start_wait_off)
            else stringResource(R.string.sharing_start_wait, s.partyStartWaitSec),
            s.partyStartWaitSec.toFloat(), 0f..10f,
            onChange = { v -> set { it.copy(partyStartWaitSec = v.roundToInt()) } }, steps = 9,
            hint = stringResource(R.string.sharing_start_wait_hint),
        )
        RowDivider()
        SwitchRow(
            stringResource(R.string.sharing_send_levels), s.sendLevels,
            hint = stringResource(R.string.sharing_send_levels_hint),
        ) { v -> set { it.copy(sendLevels = v) } }
    }

    SettingsGroup(title = stringResource(R.string.sharing_notices), description = stringResource(R.string.sharing_notices_hint)) {
        SwitchRow(stringResource(R.string.sharing_notice_joined_setting), s.noticeJoined) { v -> set { it.copy(noticeJoined = v) } }
        RowDivider()
        SwitchRow(stringResource(R.string.sharing_notice_left_setting), s.noticeLeft) { v -> set { it.copy(noticeLeft = v) } }
        RowDivider()
        SwitchRow(stringResource(R.string.sharing_notice_added_setting), s.noticeAdded) { v -> set { it.copy(noticeAdded = v) } }
        RowDivider()
        SwitchRow(stringResource(R.string.sharing_notice_approval_setting), s.noticeApproval) { v -> set { it.copy(noticeApproval = v) } }
        RowDivider()
        SliderRow(
            stringResource(R.string.sharing_notice_seconds, s.noticeSeconds), s.noticeSeconds.toFloat(), 2f..10f,
            onChange = { v -> set { it.copy(noticeSeconds = v.toInt()) } }, steps = 7,
        )
    }

    SettingsGroup(title = stringResource(R.string.sharing_as_guest)) {
        SwitchRow(stringResource(R.string.sharing_play_along_default), s.playAlong) { v -> set { it.copy(playAlong = v) } }
        RowDivider()
        SwitchRow(
            stringResource(R.string.sharing_guest_opens_player), s.guestOpensPlayer,
            hint = stringResource(R.string.sharing_guest_opens_player_hint),
        ) { v -> set { it.copy(guestOpensPlayer = v) } }
        RowDivider()
        SwitchRow(
            stringResource(R.string.sharing_sync_by_speed), s.syncBySpeed,
            hint = stringResource(R.string.sharing_sync_by_speed_hint),
        ) { v -> set { it.copy(syncBySpeed = v) } }
        RowDivider()
        SwitchRow(
            stringResource(R.string.sharing_preload_songs), s.preloadSongs,
            hint = stringResource(R.string.sharing_preload_songs_hint),
        ) { v -> set { it.copy(preloadSongs = v) } }
    }

    // both roles: how hubs and guests find each other
    SettingsGroup(title = stringResource(R.string.sharing_finding)) {
        SwitchRow(stringResource(R.string.sharing_discover), s.discover) { v -> set { it.copy(discover = v) } }
        RowDivider()
        // only matters while looking nearby
        SwitchRow(stringResource(R.string.sharing_discover_everywhere), s.discoverEverywhere, enabled = s.discover) { v ->
            set { it.copy(discoverEverywhere = v) }
        }
        RowDivider()
        // a hub on its own Wi-Fi Direct group; a LAN hub is already on the home Wi-Fi
        SwitchRow(stringResource(R.string.sharing_announce_lan), s.announceLan, enabled = s.network == ShareNetworkMode.WIFI_DIRECT) { v ->
            set { it.copy(announceLan = v) }
        }
        RowDivider()
        SwitchRow(stringResource(R.string.sharing_bluetooth), s.bluetooth) { v -> set { it.copy(bluetooth = v) } }
        RowDivider()
        // once guests are in, a Wi-Fi Direct hub stops scanning unless asked
        SwitchRow(
            stringResource(R.string.sharing_visible_with_guests), s.visibleWithGuests,
            enabled = s.network == ShareNetworkMode.WIFI_DIRECT,
            hint = stringResource(R.string.sharing_visible_with_guests_hint),
        ) { v -> set { it.copy(visibleWithGuests = v) } }
    }
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
    RowDivider()
    FieldRow { m -> OutlinedTextField(
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
        modifier = m,
    ) }
    val shortPass = pass.isNotEmpty() && pass.length < 8
    FieldRow { m -> OutlinedTextField(
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
        modifier = m,
    ) }
    SettingsNote(stringResource(R.string.sharing_group_hint))
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
internal fun Label(text: Int, top: Dp = 12.dp) {
    Text(
        stringResource(text),
        style = MaterialTheme.typography.labelLarge,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = top, bottom = 4.dp),
    )
}

/** A hint under a setting; when the choice changes it, the new text fades in. */
@Composable
internal fun Hint(text: Int) {
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
internal fun Reveal(visible: Boolean, content: @Composable () -> Unit) {
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

private fun MascotHold.labelRes(): Int = when (this) {
    MascotHold.MENU -> R.string.hold_menu
    MascotHold.STYLE -> R.string.hold_style
    MascotHold.OFF -> R.string.hold_off
}

private fun MascotHold.hintRes(): Int = when (this) {
    MascotHold.MENU -> R.string.hold_menu_hint
    MascotHold.STYLE -> R.string.hold_style_hint
    MascotHold.OFF -> R.string.hold_off_hint
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
internal fun Header(text: Int) {
    SkinHeading(
        stringResource(text),
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@Composable
internal fun Toggle(text: Int, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    SwitchRow(stringResource(text), checked, enabled = enabled, onChange = onChange)
}

private fun ImmersiveShade.labelRes() = when (this) {
    ImmersiveShade.AUTO -> R.string.shade_auto
    ImmersiveShade.STRONG -> R.string.shade_strong
    ImmersiveShade.SOFT -> R.string.shade_soft
    ImmersiveShade.OFF -> R.string.shade_off
}
