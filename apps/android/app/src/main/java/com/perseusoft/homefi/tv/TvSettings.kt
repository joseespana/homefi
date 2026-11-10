package com.perseusoft.homefi.tv

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ChevronRight
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.BuildConfig
import com.perseusoft.homefi.R
import com.perseusoft.homefi.data.AppSettings
import com.perseusoft.homefi.data.Storage
import com.perseusoft.homefi.data.UpdateStatus
import com.perseusoft.homefi.homefi
import kotlinx.coroutines.launch

/**
 * Settings (docs/TV_UI.md 6.9): the pages on the left, the rows of the open page on the right. The D-pad
 * reaches every row: RIGHT from a page goes to its rows, BACK (or LEFT on a switch) comes back. A value row
 * changes with LEFT / RIGHT and opens a list on OK when it has many options; changes apply at once.
 * The rows are data ([TvSettingsModel.kt]) over the phone's `SettingsStore`; the AI page has its own screen.
 * [initial] opens a page directly ("set up an AI" from the playlist chat); [entryKey] asks for first focus.
 */
@Composable
fun TvSettings(entryKey: Int, initial: TvSettingsCategory?) {
    val safe = tvSafePadding()
    var selected by rememberSaveable { mutableStateOf(initial ?: TvSettingsCategory.APPEARANCE) }
    // a request from another screen (and the same one again later) moves to that page
    LaunchedEffect(initial, entryKey) { if (initial != null) selected = initial }
    val categoryFocus = remember { TvSettingsCategory.entries.associateWith { FocusRequester() } }
    val firstRow = remember { FocusRequester() }
    var inPanel by remember { mutableStateOf(false) }

    Row(
        Modifier
            .onPreviewKeyEvent { event ->
                // BACK from a row returns to the pages (handled here, as a key, so it never depends on who else listens for BACK)
                if (event.key != Key.Back || !inPanel) return@onPreviewKeyEvent false
                if (event.type == KeyEventType.KeyDown) runCatching { categoryFocus.getValue(selected).requestFocus() }
                true
            }
            .fillMaxSize()
            .padding(top = safe.calculateTopPadding(), bottom = safe.calculateBottomPadding(), end = safe.calculateRightPadding(LayoutDirection.Ltr)),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Column(Modifier.width(PAGES_WIDTH).fillMaxHeight()) {
            TvScreenHeader(stringResource(R.string.tv_nav_settings), null, Modifier.padding(start = 12.dp))
            LazyColumn(
                Modifier.fillMaxSize(),
                contentPadding = PaddingValues(start = 12.dp, end = 10.dp, top = 12.dp, bottom = 12.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                itemsIndexed(TvSettingsCategory.entries, key = { _, c -> c.name }) { _, category ->
                    CategoryItem(
                        category = category,
                        current = category == selected,
                        requester = categoryFocus.getValue(category),
                        entryKey = entryKey,
                        rightTo = firstRow,
                        onFocused = { selected = category; inPanel = false },
                    )
                }
            }
        }
        TvGlass(Modifier.weight(1f).fillMaxHeight().padding(bottom = 4.dp)) {
            Column(Modifier.fillMaxSize().padding(top = 14.dp)) {
                Column(Modifier.padding(horizontal = 18.dp)) {
                    Text(stringResource(selected.title), style = TvType.section, color = LocalTvLook.current.ink, maxLines = 1)
                    Text(stringResource(selected.about), style = TvType.secondary, color = LocalTvLook.current.ink2, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
                if (selected == TvSettingsCategory.AI) {
                    TvAiSettings(firstRow, categoryFocus.getValue(selected)) { inPanel = true }
                } else {
                    SettingRows(selected, firstRow, categoryFocus.getValue(selected)) { inPanel = true }
                }
            }
        }
    }
}

private val PAGES_WIDTH = 190.dp

@Composable
private fun CategoryItem(
    category: TvSettingsCategory,
    current: Boolean,
    requester: FocusRequester,
    entryKey: Int,
    rightTo: FocusRequester,
    onFocused: () -> Unit,
) {
    val look = LocalTvLook.current
    val shape = RoundedCornerShape(12.dp)
    TvFocusable(
        onClick = { runCatching { rightTo.requestFocus() } },
        modifier = Modifier
            .fillMaxWidth()
            .height(CATEGORY_HEIGHT)
            .tvInitialFocus(requester, key = entryKey, enabled = current)
            .tvLeftToDrawer()
            .tvRightTo(rightTo),
        shape = shape,
        focusedScale = 1.03f,
        glow = false,
        onFocused = { if (it) onFocused() },
    ) { focused ->
        val fill = when {
            focused -> Modifier.background(Color.White, shape)
            current -> Modifier.background(look.accent.copy(alpha = 0.22f), shape)
            else -> Modifier
        }
        Row(
            Modifier.fillMaxSize().then(fill).padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Icon(category.icon, null, tint = if (focused) look.onFocus else if (current) look.accent2 else look.ink2, modifier = Modifier.size(18.dp))
            Text(stringResource(category.title), style = TvType.body.copy(fontSize = 15.sp), color = if (focused) look.onFocus else look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

private val CATEGORY_HEIGHT = 38.dp

/** The rows of a data-driven page, with the pickers and prompts they open. */
@Composable
private fun SettingRows(category: TvSettingsCategory, firstRow: FocusRequester, back: FocusRequester, onRowFocused: () -> Unit) {
    val context = LocalContext.current
    val app = context.homefi
    val scope = rememberCoroutineScope()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val account by app.account.account.collectAsStateWithLifecycle()
    val update by app.updates.state.collectAsStateWithLifecycle()
    val nav = LocalTvNav.current
    val rows = remember(category) { settingsRows(category) }
    val state = rememberLazyListState()
    var picking by remember { mutableStateOf<TvSettingRow.Choice?>(null) }
    var typing by remember { mutableStateOf<TvSettingRow.Text?>(null) }
    var signOut by remember { mutableStateOf(false) }
    var usbGuide by remember { mutableStateOf(false) }
    // the page changes: back to its top
    LaunchedEffect(category) { state.scrollToItem(0) }

    LazyColumn(
        Modifier.fillMaxSize(),
        state = state,
        contentPadding = PaddingValues(start = 12.dp, end = 14.dp, top = 12.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        itemsIndexed(rows, key = { _, r -> "${category.name}-${r.id}" }) { index, row ->
            val enabled = row.enabled(settings)
            val rowFocus = { focused: Boolean -> if (focused) onRowFocused() }
            val rowModifier = Modifier
                .tvLeftTo(back)
                .then(if (index == 0) Modifier.focusRequester(firstRow) else Modifier)
            when (row) {
                is TvSettingRow.Switch -> SettingLine(
                    row, enabled, rowModifier,
                    onClick = { app.settings.update { s -> row.set(s, !row.get(s)) } },
                    onFocused = rowFocus,
                    value = { focused -> SwitchKnob(row.get(settings), focused) },
                )
                is TvSettingRow.Choice -> {
                    val current = row.selected(settings).coerceIn(0, row.options.lastIndex)
                    val change = { step: Int -> app.settings.update { s -> row.select(s, cycleIndex(row.selected(s), row.options.size, step)) } }
                    SettingLine(
                        row, enabled,
                        rowModifier.onPreviewKeyEvent { event ->
                            val step = when (event.key) {
                                Key.DirectionLeft -> -1
                                Key.DirectionRight -> 1
                                else -> 0
                            }
                            if (step == 0 || !enabled) return@onPreviewKeyEvent false
                            if (event.type == KeyEventType.KeyDown) change(step)
                            true
                        },
                        onClick = { if (row.options.size >= PICKER_FROM_OPTIONS) picking = row else change(1) },
                        onFocused = rowFocus,
                        value = { focused -> ValueText(row.options[current].text(), focused, arrows = true) },
                    )
                }
                is TvSettingRow.Text -> {
                    val text = row.get(settings)
                    SettingLine(
                        row, enabled, rowModifier,
                        onClick = { typing = row },
                        onFocused = rowFocus,
                        value = { focused -> ValueText(text.ifBlank { stringResource(row.emptyLabel) }, focused, arrows = false) },
                    )
                }
                is TvSettingRow.Action -> {
                    val shown = actionValue(row, settings, account != null, update.status)
                    SettingLine(
                        row, enabled, rowModifier,
                        onFocused = rowFocus,
                        onClick = {
                            when (row.action) {
                                TvSettingAction.SIGN_OUT -> if (account == null) nav.go(TvSection.HOME) else signOut = true
                                TvSettingAction.USB_ACCESS -> usbGuide = true
                                TvSettingAction.FORGET_DRIVES -> {
                                    app.settings.updateTv { it.copy(libraryVolumes = emptyList()) }
                                    app.library.refresh()
                                }
                                TvSettingAction.CHECK_UPDATES -> if (app.updates.enabled) app.updates.check(manual = true)
                                TvSettingAction.WHATS_NEW -> scope.launch { app.whatsNew.latest()?.let { app.whatsNew.show(listOf(it)) } }
                            }
                        },
                        value = { focused -> if (shown != null) ValueText(shown, focused, arrows = false) else Icon(Icons.Rounded.ChevronRight, null, tint = if (focused) LocalTvLook.current.onFocus else LocalTvLook.current.ink2, modifier = Modifier.size(20.dp)) },
                    )
                }
            }
        }
    }

    picking?.let { row ->
        val labels = row.options.map { it.text() }
        TvOptionPicker(
            title = stringResource(row.title),
            options = labels,
            selected = row.selected(settings).coerceIn(0, labels.lastIndex),
            onPick = { i -> app.settings.update { s -> row.select(s, i) }; picking = null },
            onDismiss = { picking = null },
        )
    }
    typing?.let { row ->
        TvTextPrompt(
            title = stringResource(row.title),
            initial = row.get(settings),
            onDone = { text -> app.settings.update { s -> row.set(s, text.take(row.maxLength)) }; typing = null },
            onDismiss = { typing = null },
            placeholder = stringResource(row.title),
        )
    }
    if (signOut) {
        TvConfirmDialog(
            title = stringResource(R.string.settings_logout_title),
            text = stringResource(R.string.settings_logout_text),
            confirmLabel = stringResource(R.string.settings_logout),
            onConfirm = {
                scope.launch {
                    app.downloads.cancelAll()
                    app.engine.logout()
                    app.account.signedOut()
                }
            },
            onDismiss = { signOut = false },
        )
    }
    if (usbGuide) {
        TvUsbGuide(
            onOpenSettings = { usbGuide = false; openUsbAccessSettings(context) },
            onAppFolder = { usbGuide = false },
        )
    }
}

/** What an action row shows on its right: the account, the drive access, the update state, the version. */
@Composable
private fun actionValue(row: TvSettingRow.Action, settings: AppSettings, signedIn: Boolean, update: UpdateStatus): String? {
    val context = LocalContext.current
    return when (row.action) {
        TvSettingAction.SIGN_OUT -> stringResource(if (signedIn) R.string.settings_logout else R.string.login_connect)
        TvSettingAction.USB_ACCESS -> stringResource(if (Storage.hasSharedAccess(context)) R.string.tv_set_allowed else R.string.tv_set_not_allowed)
        TvSettingAction.FORGET_DRIVES -> row.value?.invoke(settings)?.text()
        TvSettingAction.CHECK_UPDATES -> when {
            !context.homefi.updates.enabled -> stringResource(R.string.tv_set_updates_off)
            update == UpdateStatus.CHECKING -> stringResource(R.string.update_checking)
            update == UpdateStatus.UP_TO_DATE -> stringResource(R.string.update_up_to_date, BuildConfig.VERSION_NAME)
            update == UpdateStatus.AVAILABLE -> stringResource(R.string.update_available, context.homefi.updates.state.value.release?.version.orEmpty())
            else -> "HomeFi ${BuildConfig.VERSION_NAME}"
        }
        TvSettingAction.WHATS_NEW -> BuildConfig.VERSION_NAME
    }
}

@Composable
internal fun TvLabel.text(): String = when (this) {
    is TvLabel.Plain -> text
    is TvLabel.Res -> stringResource(id, *args.toTypedArray())
}

/** One row of a page (its words from resources). */
@Composable
private fun SettingLine(
    row: TvSettingRow,
    enabled: Boolean,
    modifier: Modifier,
    onClick: () -> Unit,
    onFocused: (Boolean) -> Unit,
    value: @Composable (focused: Boolean) -> Unit,
) = TvSettingLine(stringResource(row.title), row.hint?.let { stringResource(it) }, enabled, modifier, onClick, onFocused = onFocused, value = value)

/**
 * One line of Settings: title and hint on the left, the value on the right; white when focused, dim when it
 * cannot change now, red for a [danger]ous one (remove a key). The AI page uses it too.
 */
@Composable
internal fun TvSettingLine(
    title: String,
    hint: String?,
    enabled: Boolean,
    modifier: Modifier,
    onClick: () -> Unit,
    danger: Boolean = false,
    onFocused: (Boolean) -> Unit = {},
    value: @Composable (focused: Boolean) -> Unit,
) {
    val look = LocalTvLook.current
    val shape = RoundedCornerShape(12.dp)
    TvFocusable(
        onClick = onClick,
        modifier = modifier.fillMaxWidth().heightIn(min = ROW_MIN_HEIGHT),
        shape = shape,
        focusedScale = 1.02f,
        glow = false,
        enabled = enabled,
        onFocused = onFocused,
    ) { focused ->
        Row(
            Modifier
                .fillMaxWidth()
                .alpha(if (enabled) 1f else DISABLED_ALPHA)
                .then(if (focused) Modifier.background(Color.White, shape) else Modifier)
                .padding(horizontal = 12.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Column(Modifier.weight(1f)) {
                val ink = when {
                    focused -> look.onFocus
                    danger -> look.error
                    else -> look.ink
                }
                Text(title, style = TvType.cardTitle.copy(fontSize = 15.sp), color = ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                hint?.let { Text(it, style = TvType.secondary, color = if (focused) ROW_SUB_FOCUSED else look.ink2, maxLines = 2, overflow = TextOverflow.Ellipsis) }
            }
            value(focused)
        }
    }
}

private val ROW_MIN_HEIGHT = 44.dp
private const val DISABLED_ALPHA = 0.4f

/** The value of a Choice, Text or Action row; with [arrows] a focused row shows that LEFT / RIGHT change it. */
@Composable
internal fun ValueText(text: String, focused: Boolean, arrows: Boolean) {
    val look = LocalTvLook.current
    val ink = if (focused) look.onFocus else look.accent2
    Text(
        (if (arrows && focused) "◀  " else "") + text + (if (arrows && focused) "  ▶" else ""),
        style = TvType.body.copy(fontSize = 15.sp),
        color = ink,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier.widthIn(max = VALUE_MAX_WIDTH),
    )
}

private val VALUE_MAX_WIDTH = 260.dp

/** The switch of a Switch row: a track that is the accent gradient when on, a knob on the right then. */
@Composable
private fun SwitchKnob(on: Boolean, focused: Boolean) {
    val look = LocalTvLook.current
    Box(
        Modifier
            .size(width = 40.dp, height = 22.dp)
            .background(if (on) look.gradient else androidx.compose.ui.graphics.SolidColor(if (focused) Color(0x33000000) else Color.White.copy(alpha = 0.18f)), CircleShape),
        contentAlignment = if (on) Alignment.CenterEnd else Alignment.CenterStart,
    ) {
        Box(Modifier.padding(3.dp).size(16.dp).background(Color.White, CircleShape)) {
            if (on) Icon(Icons.Rounded.Check, null, tint = look.accent, modifier = Modifier.size(16.dp))
        }
    }
}

/** A long list of options on a card of its own; the current one has first focus, OK picks, BACK keeps the old one. */
@Composable
fun TvOptionPicker(title: String, options: List<String>, selected: Int, onPick: (Int) -> Unit, onDismiss: () -> Unit) {
    val look = LocalTvLook.current
    val state = rememberLazyListState(initialFirstVisibleItemIndex = (selected - PICKER_LEAD).coerceAtLeast(0))
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(Color(0xCC040510)), contentAlignment = Alignment.Center) {
            TvGlass(Modifier.width(PICKER_WIDTH).heightIn(max = PICKER_MAX_HEIGHT), solid = true) {
                Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(title, style = TvType.section, color = look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    LazyColumn(Modifier.weight(1f, fill = false), state = state, verticalArrangement = Arrangement.spacedBy(4.dp), contentPadding = PaddingValues(vertical = 8.dp, horizontal = 6.dp)) {
                        itemsIndexed(options, key = { i, _ -> i }) { index, label ->
                            TvFocusable(
                                onClick = { onPick(index) },
                                modifier = Modifier.fillMaxWidth().height(40.dp).then(if (index == selected) Modifier.tvInitialFocus() else Modifier),
                                shape = RoundedCornerShape(12.dp),
                                focusedScale = 1.02f,
                                glow = false,
                            ) { focused ->
                                Row(
                                    Modifier.fillMaxSize().then(if (focused) Modifier.background(Color.White, RoundedCornerShape(12.dp)) else Modifier).padding(horizontal = 12.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                                ) {
                                    Text(label, style = TvType.body.copy(fontSize = 15.sp), color = if (focused) look.onFocus else look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                                    if (index == selected) Icon(Icons.Rounded.Check, null, tint = if (focused) look.onFocus else look.accent2, modifier = Modifier.size(18.dp))
                                }
                            }
                        }
                    }
                    TvPill(stringResource(R.string.cancel), onDismiss, height = 32.dp)
                }
            }
        }
    }
}

private val PICKER_WIDTH = 360.dp
private val PICKER_MAX_HEIGHT = 440.dp

/** The picker opens with the current option a few rows from the top, so its neighbours show. */
private const val PICKER_LEAD = 3
