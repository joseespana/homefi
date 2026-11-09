package com.perseusoft.homefi.tv

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.RemoveCircleOutline
import androidx.compose.material.icons.rounded.Save
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.VolumeOff
import androidx.compose.material.icons.rounded.VolumeUp
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
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
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.EqBand
import com.perseusoft.homefi.core.EqProfile
import com.perseusoft.homefi.core.TasteRule
import com.perseusoft.homefi.core.TasteScope
import com.perseusoft.homefi.core.tasteCombinedProfile
import com.perseusoft.homefi.core.tasteSoundCovering
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.Equalizer
import com.perseusoft.homefi.player.EqPanel
import com.perseusoft.homefi.player.SoundNotice
import com.perseusoft.homefi.player.TryResult
import com.perseusoft.homefi.player.name
import com.perseusoft.homefi.ui.eq.AssistantViewModel
import com.perseusoft.homefi.ui.eq.EqCurve
import com.perseusoft.homefi.ui.eq.EqViewModel
import com.perseusoft.homefi.ui.eq.formatDb
import com.perseusoft.homefi.ui.eq.hasGain
import com.perseusoft.homefi.ui.eq.formatHz
import com.perseusoft.homefi.ui.eq.labelRes
import com.perseusoft.homefi.ui.eq.outputLabel
import com.perseusoft.homefi.ui.sound.REACTIONS
import com.perseusoft.homefi.ui.theme.LocalMotion
import kotlinx.coroutines.delay
import kotlin.math.roundToInt

private const val SLIDE_FRACTION = 12
private const val NOTICE_MS = 8_000L
private const val GAIN_LIMIT_DB = 24f
private const val TENTH = 10f
private val CURVE_HEIGHT = 190.dp
private val CURVE_SHAPE = RoundedCornerShape(TvDimens.panelRadius)

private enum class SoundTab { SOUNDS, CURVE, ASSISTANT }

// ------------------------------------------------------------------ the editing rules (pure, tested)

/** The band after [current] in frequency order ([direction] +1) or before it (-1), as an index of [bands]; the ends stay. */
internal fun neighbourBand(bands: List<EqBand>, current: Int?, direction: Int): Int? {
    if (bands.isEmpty()) return null
    val order = bands.indices.sortedBy { bands[it].freqHz }
    val at = order.indexOf(current).takeIf { it >= 0 } ?: return order.first()
    return order[(at + direction).coerceIn(0, order.lastIndex)]
}

/** A band's gain after [steps] presses of [stepDb], to a tenth of a dB inside the equalizer's range. */
internal fun steppedGain(gainDb: Float, steps: Int, stepDb: Float): Float =
    ((((gainDb + steps * stepDb) * TENTH).roundToInt()) / TENTH).coerceIn(-GAIN_LIMIT_DB, GAIN_LIMIT_DB)

// ------------------------------------------------------------------ the screen

/**
 * Sound (docs/TV_UI.md 6.11): the presets and the user's sounds, the output's curve edited with the D-pad, the
 * sound assistant (chat with quick replies and voice), and "How does it sound?". HDMI is honestly a system mix.
 * It reads the phone's state holders ([EqViewModel], [AssistantViewModel], `app.sound`, `app.equalizer`).
 */
@Composable
fun TvSound(entryKey: Int) {
    val app = LocalContext.current.homefi
    val eq by app.equalizer.settings.collectAsStateWithLifecycle()
    val panel by app.equalizer.panel.collectAsStateWithLifecycle()
    val assistantOpen by app.sound.assistantOpen.collectAsStateWithLifecycle()
    val howOpen by app.sound.howOpen.collectAsStateWithLifecycle()
    val scopeMenu by app.sound.soundMenu.collectAsStateWithLifecycle()
    val safe = tvSafePadding()
    val bodyEntry = remember { FocusRequester() }
    val motion = LocalMotion.current
    var tab by rememberSaveable { mutableStateOf(SoundTab.SOUNDS) }
    // asks the assistant to read a hand-off (a "Sound for this" question, a refine)
    var assistantNonce by remember { mutableIntStateOf(0) }
    var notice by remember { mutableStateOf<SoundNotice?>(null) }
    val eqVm: EqViewModel = viewModel()
    LaunchedEffect(Unit) { eqVm.reopen() }
    // other screens ask for a part of Sound: the output curve, or the assistant
    LaunchedEffect(panel) {
        when (panel) {
            EqPanel.OUTPUT_CURVE -> tab = SoundTab.CURVE
            EqPanel.ASSISTANT -> { tab = SoundTab.ASSISTANT; assistantNonce++ }
            else -> Unit
        }
        if (panel != null) app.equalizer.close()
    }
    LaunchedEffect(assistantOpen) {
        if (assistantOpen) {
            tab = SoundTab.ASSISTANT
            assistantNonce++
            app.sound.openAssistant(false)
        }
    }
    // a reaction's note: what changed, with Undo, for a few seconds
    LaunchedEffect(Unit) {
        app.sound.notices.collect { n ->
            notice = n
        }
    }
    LaunchedEffect(notice) {
        if (notice != null) {
            delay(NOTICE_MS)
            notice = null
        }
    }

    Column(Modifier.fillMaxSize().padding(top = safe.calculateTopPadding(), end = safe.calculateRightPadding(androidx.compose.ui.unit.LayoutDirection.Ltr))) {
        TvScreenHeader(stringResource(R.string.sound_title), soundSubtitle(), Modifier.padding(start = 12.dp)) { TvNowPlayingChip() }
        Row(Modifier.padding(start = 12.dp, top = 10.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            TvTabs(
                labels = listOf(R.string.sound_sounds, R.string.tv_sound_tab_curve, R.string.tv_sound_tab_assistant).map { stringResource(it) },
                selected = tab.ordinal,
                onSelect = { tab = SoundTab.entries[it] },
                entryKey = entryKey,
                downTo = bodyEntry,
            )
            Spacer(Modifier.weight(1f))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TvPill(
                    stringResource(R.string.sound_how),
                    { app.sound.openHow(true) },
                    icon = Icons.Rounded.GraphicEq,
                    height = 30.dp,
                    labelStyle = TvType.body.copy(fontSize = 14.sp),
                )
                TvPill(
                    stringResource(if (eq.enabled) R.string.tv_sound_on else R.string.tv_sound_off),
                    { app.equalizer.setEnabled(!eq.enabled) },
                    icon = if (eq.enabled) Icons.Rounded.VolumeUp else Icons.Rounded.VolumeOff,
                    selected = eq.enabled,
                    height = 30.dp,
                    labelStyle = TvType.body.copy(fontSize = 14.sp),
                )
            }
        }
        TvTryBar()
        AnimatedContent(
            targetState = tab,
            transitionSpec = {
                val dir = if (targetState.ordinal >= initialState.ordinal) 1 else -1
                val slide = if (motion.moves) slideInHorizontally(motion.tween(260)) { dir * it / SLIDE_FRACTION } else EnterTransition.None
                (fadeIn(motion.tween(220, delay = 90)) + slide) togetherWith fadeOut(motion.tween(90))
            },
            label = "tv-sound-tab",
            modifier = Modifier.weight(1f),
        ) { shown ->
            when (shown) {
                SoundTab.SOUNDS -> SoundsTab(eqVm, bodyEntry)
                SoundTab.CURVE -> CurveTab(eqVm, bodyEntry)
                SoundTab.ASSISTANT -> AssistantTab(entryKey, assistantNonce, onCurve = { tab = SoundTab.CURVE })
            }
        }
        AnimatedVisibility(
            visible = notice != null,
            enter = expandVertically(motion.tween(180)) + fadeIn(motion.tween(180)),
            exit = shrinkVertically(motion.tween(160)) + fadeOut(motion.tween(160)),
        ) { notice?.let { TvSoundNotice(it) { notice = null } } }
    }
    if (howOpen) TvHowSounds(onDismiss = { app.sound.openHow(false) })
    scopeMenu?.let { req -> TvSoundScopeMenu(req.scope) { app.sound.openMenu(null) } }
}

/** "HDMI · system mix" for the TV's own output, and the honest line under it. */
@Composable
private fun soundSubtitle(): String {
    val out = outputLabel()
    return if (out == null) stringResource(R.string.tv_sound_sub) else stringResource(R.string.tv_sound_sub_out, out)
}

// ------------------------------------------------------------------ Sounds tab

@Composable
private fun SoundsTab(eqVm: EqViewModel, bodyEntry: FocusRequester) {
    val app = LocalContext.current.homefi
    val look = LocalTvLook.current
    val presets by app.equalizer.presets.collectAsStateWithLifecycle()
    val eq by app.equalizer.settings.collectAsStateWithLifecycle()
    val taste by app.sound.taste.collectAsStateWithLifecycle()
    val now by app.sound.now.collectAsStateWithLifecycle()
    val target by eqVm.target.collectAsStateWithLifecycle()
    val shown = remember(eq, target) { app.equalizer.profileOf(target, eq).first }
    val rulesEntry = remember { FocusRequester() }
    var presetMenu by remember { mutableStateOf<String?>(null) }
    var ruleMenu by remember { mutableStateOf<TasteRule?>(null) }
    var saving by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf<TasteRule?>(null) }

    Row(Modifier.fillMaxSize().padding(start = 12.dp, top = 4.dp), horizontalArrangement = Arrangement.spacedBy(18.dp)) {
        // presets
        Column(Modifier.weight(1f)) {
            ColumnTitle(stringResource(R.string.eq_presets)) {
                TvPill(stringResource(R.string.eq_save_preset), { saving = true }, icon = Icons.Rounded.Save, height = 28.dp, labelStyle = TvType.body.copy(fontSize = 13.sp))
            }
            val names = remember(presets) { presets.keys.sorted() }
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = 8.dp, end = 4.dp, bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item(key = "flat") {
                    val active = shown == null || shown.bands.isEmpty()
                    TvRow(
                        title = stringResource(R.string.eq_flat),
                        subtitle = stringResource(R.string.tv_sound_flat_sub),
                        coverIcon = Icons.Rounded.GraphicEq,
                        tag = if (active) stringResource(R.string.tv_sound_active) else null,
                        onClick = { eqVm.flat(); if (!eq.enabled) app.equalizer.setEnabled(true) },
                        modifier = Modifier.focusRequester(bodyEntry).tvLeftToDrawer().tvRightTo(rulesEntry),
                    )
                }
                items(names, key = { "preset-$it" }) { name ->
                    val profile = presets.getValue(name)
                    TvRow(
                        title = name,
                        subtitle = stringResource(R.string.tv_sound_bands, profile.bands.size),
                        coverIcon = Icons.Rounded.Tune,
                        tag = if (shown?.name == name) stringResource(R.string.tv_sound_active) else null,
                        onClick = { eqVm.applyPreset(profile); if (!eq.enabled) app.equalizer.setEnabled(true) },
                        onLongClick = { presetMenu = name },
                        modifier = Modifier.tvLeftToDrawer().tvRightTo(rulesEntry),
                    )
                }
            }
        }
        // the user's sounds (taste layers)
        Column(Modifier.weight(1f)) {
            ColumnTitle(stringResource(R.string.sound_your_sounds)) {
                TvPill(stringResource(R.string.sound_ask_ai), { app.sound.askAssistantWith(null) }, icon = Icons.Rounded.AutoAwesome, height = 28.dp, labelStyle = TvType.body.copy(fontSize = 13.sp))
            }
            if (taste.rules.isEmpty()) {
                Text(stringResource(R.string.tv_sound_rules_empty), style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink2, modifier = Modifier.padding(top = 12.dp, end = 12.dp))
            }
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = 8.dp, end = 24.dp, bottom = 28.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                items(taste.rules, key = { "rule-${it.id}" }) { rule ->
                    val first = rule.id == taste.rules.first().id
                    val playing = now.rule?.id == rule.id && !now.auditioning
                    TvRow(
                        title = rule.label,
                        subtitle = remember(rule) { app.sound.ruleWords(rule) },
                        coverIcon = if (rule.createdBy == com.perseusoft.homefi.core.TasteCreator.AI) Icons.Rounded.AutoAwesome else Icons.Rounded.Tune,
                        tag = if (playing) stringResource(R.string.sound_now_playing) else null,
                        onClick = { tryRule(app, rule) },
                        onLongClick = { ruleMenu = rule },
                        modifier = Modifier.then(if (first) Modifier.focusRequester(rulesEntry) else Modifier),
                    )
                }
            }
        }
    }
    presetMenu?.let { name ->
        val profile = presets[name] ?: return@let
        TvActionMenu(
            name,
            stringResource(R.string.tv_sound_bands, profile.bands.size),
            listOf(
                TvAction(Icons.Rounded.Check, stringResource(R.string.tv_sound_apply)) { eqVm.applyPreset(profile); if (!eq.enabled) app.equalizer.setEnabled(true) },
                TvAction(Icons.Rounded.Delete, stringResource(R.string.eq_preset_delete_action)) { eqVm.deletePreset(name) },
            ),
        ) { presetMenu = null }
    }
    ruleMenu?.let { rule ->
        TvActionMenu(
            rule.label,
            app.sound.ruleWords(rule),
            listOf(
                TvAction(Icons.Rounded.GraphicEq, stringResource(R.string.tv_sound_try)) { tryRule(app, rule) },
                TvAction(Icons.Rounded.AutoAwesome, stringResource(R.string.tv_sound_refine)) { app.sound.refine(rule) },
                TvAction(Icons.Rounded.Delete, stringResource(R.string.sound_delete)) { deleting = rule },
            ),
        ) { ruleMenu = null }
    }
    deleting?.let { rule ->
        TvConfirmDialog(
            title = stringResource(R.string.sound_delete),
            text = stringResource(R.string.sound_rule_delete_confirm, rule.label),
            confirmLabel = stringResource(R.string.sound_delete),
            onConfirm = { app.sound.remove(rule.id) },
            onDismiss = { deleting = null },
        )
    }
    if (saving) {
        TvTextPrompt(
            title = stringResource(R.string.eq_save_preset),
            initial = "",
            placeholder = stringResource(R.string.eq_preset_name),
            doneLabel = stringResource(R.string.sound_save),
            onDone = { name -> saving = false; if (name.isNotBlank()) eqVm.savePreset(name) },
            onDismiss = { saving = false },
        )
    }
}

private fun tryRule(app: com.perseusoft.homefi.HomefiApplication, rule: TasteRule) {
    val trying = app.sound.audition.value?.rule?.id == rule.id
    if (trying) {
        app.sound.stopTrying()
        return
    }
    when (app.sound.tryNow(rule)) {
        TryResult.Playing -> Unit
        TryResult.SoundOff -> app.sound.notify(app.getString(R.string.sound_turn_on_to_try))
        TryResult.NothingPlaying -> app.sound.notify(app.getString(R.string.sound_how_nothing))
    }
}

@Composable
private fun ColumnTitle(title: String, trailing: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(end = 24.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = TvType.section, color = LocalTvLook.current.ink, modifier = Modifier.weight(1f))
        trailing()
    }
}

/** "Trying “Warm” on this song": Keep (for the scope the core suggests) or Stop, above the lists while a sound is tried. */
@Composable
private fun TvTryBar() {
    val app = LocalContext.current.homefi
    val audition by app.sound.audition.collectAsStateWithLifecycle()
    val now by app.sound.now.collectAsStateWithLifecycle()
    val motion = LocalMotion.current
    var last by remember { mutableStateOf<com.perseusoft.homefi.core.TasteAudition?>(null) }
    if (audition != null) last = audition
    AnimatedVisibility(
        visible = audition != null,
        enter = expandVertically(motion.tween(180)) + fadeIn(motion.tween(180)),
        exit = shrinkVertically(motion.tween(160)) + fadeOut(motion.tween(160)),
    ) {
        val rule = (last ?: return@AnimatedVisibility).rule
        val primary = remember(rule, now) { app.sound.suggestedScope(rule) }
        TvGlass(Modifier.fillMaxWidth().padding(start = 12.dp, end = 24.dp, top = 8.dp)) {
            Row(Modifier.padding(horizontal = 14.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Icon(Icons.Rounded.GraphicEq, null, tint = LocalTvLook.current.accent2, modifier = Modifier.size(20.dp))
                Text(stringResource(R.string.sound_trying_on, rule.label), style = TvType.body, color = LocalTvLook.current.ink, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                if (primary != null) {
                    TvPill(stringResource(R.string.sound_keep_for, scopeWord(primary)), { app.sound.keep(primary) }, primary = true, height = 30.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                }
                TvPill(stringResource(R.string.tv_sound_stop), { app.sound.stopTrying() }, height = 30.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
            }
        }
    }
}

@Composable
private fun scopeWord(scope: TasteScope): String = when (scope) {
    is TasteScope.Genre -> scope.name
    is TasteScope.Album -> stringResource(R.string.sound_scope_album)
    is TasteScope.Artist -> stringResource(R.string.sound_scope_artist)
    is TasteScope.Playlist -> stringResource(R.string.sound_scope_playlist)
    is TasteScope.Track -> stringResource(R.string.sound_scope_track)
}.lowercase()

/** The note after a reaction or an undo, with Undo when there is something to take back. */
@Composable
private fun TvSoundNotice(notice: SoundNotice, onDone: () -> Unit) {
    val app = LocalContext.current.homefi
    val look = LocalTvLook.current
    val undoable = notice.undoToken != null || notice.moves.isNotEmpty()
    TvGlass(Modifier.fillMaxWidth().padding(start = 12.dp, end = 24.dp, bottom = 12.dp)) {
        Row(Modifier.padding(horizontal = 14.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Icon(if (notice.error) Icons.Rounded.RemoveCircleOutline else Icons.Rounded.Check, null, tint = if (notice.error) look.error else look.ok, modifier = Modifier.size(20.dp))
            Text(notice.message, style = TvType.body.copy(fontSize = 15.sp), color = look.ink, maxLines = 3, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            if (undoable) {
                TvPill(
                    stringResource(R.string.sound_undo),
                    {
                        notice.undoToken?.let(app.sound::undo)
                        if (notice.moves.isNotEmpty()) app.sound.undoMoves(notice.moves)
                        onDone()
                    },
                    Modifier.tvInitialFocus(key = notice),
                    height = 30.dp,
                    labelStyle = TvType.body.copy(fontSize = 14.sp),
                )
            }
        }
    }
}

// ------------------------------------------------------------------ Curve tab

@Composable
private fun CurveTab(eqVm: EqViewModel, bodyEntry: FocusRequester) {
    val app = LocalContext.current.homefi
    val look = LocalTvLook.current
    val eq by app.equalizer.settings.collectAsStateWithLifecycle()
    val settings by app.settings.settings.collectAsStateWithLifecycle()
    val target by eqVm.target.collectAsStateWithLifecycle()
    val selected by eqVm.selected.collectAsStateWithLifecycle()
    val held by app.equalizer.held.collectAsStateWithLifecycle()
    val connected by app.outputs.outputs.collectAsStateWithLifecycle()
    val (profile, own) = remember(eq, target) { app.equalizer.profileOf(target, eq) }
    val bands = profile?.bands.orEmpty()
    var editing by remember { mutableStateOf(false) }
    val step = settings.tv.eqStepDb
    BackHandler(enabled = editing) { editing = false }
    // a removed band or a new profile: the selection stays inside the list
    LaunchedEffect(bands.size) { if (selected != null && selected !in bands.indices) eqVm.select(null) }

    Column(Modifier.fillMaxSize().padding(start = 12.dp, end = 24.dp, top = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val name = target?.let { outputLabel(it, connected) } ?: stringResource(R.string.eq_output_default)
            TvTag(stringResource(R.string.tv_sound_editing, name))
            if (target != null && !own) TvTag(stringResource(R.string.tv_sound_shared_profile))
            if (!eq.enabled) TvTag(stringResource(R.string.sound_off), tone = look.warn)
            Spacer(Modifier.weight(1f))
            if (editing) TvKeyHints(
                listOf(
                    "◀ ▶" to stringResource(R.string.tv_sound_hint_band),
                    "▲ ▼" to stringResource(R.string.tv_sound_hint_gain, formatDb(step)),
                    "OK" to stringResource(R.string.tv_sound_hint_done),
                ),
            )
        }
        TvFocusable(
            onClick = {
                if (bands.isEmpty()) {
                    eqVm.addBand()
                    editing = true
                } else {
                    if (selected == null) eqVm.select(neighbourBand(bands, null, 0))
                    editing = true
                }
            },
            modifier = Modifier
                .focusRequester(bodyEntry)
                .tvLeftToDrawer()
                .fillMaxWidth()
                .height(CURVE_HEIGHT)
                .onPreviewKeyEvent { event ->
                    if (!editing || event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                    when (event.key) {
                        Key.DirectionLeft -> { eqVm.select(neighbourBand(bands, selected, -1)); true }
                        Key.DirectionRight -> { eqVm.select(neighbourBand(bands, selected, 1)); true }
                        Key.DirectionUp -> { nudge(eqVm, bands, selected, 1, step); true }
                        Key.DirectionDown -> { nudge(eqVm, bands, selected, -1, step); true }
                        Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> { editing = false; true }
                        else -> false
                    }
                },
            shape = CURVE_SHAPE,
            focusedScale = 1.01f,
        ) { focused ->
            TvGlass(Modifier.fillMaxSize().then(if (editing) Modifier.border(2.dp, look.accent, CURVE_SHAPE) else Modifier)) {
                if (bands.isEmpty()) {
                    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
                        Text(stringResource(R.string.eq_empty), style = TvType.body, color = look.ink2)
                        Text(stringResource(R.string.tv_sound_press_ok_add), style = TvType.secondary, color = look.ink3)
                    }
                }
                EqCurve(
                    profile = profile,
                    selected = selected.takeIf { editing || focused },
                    dimmed = !eq.enabled || held,
                    maxRange = 18f,
                    labelSp = 13f,
                    pointScale = 1.5f,
                    modifier = Modifier.fillMaxSize().padding(horizontal = 10.dp, vertical = 8.dp),
                )
                if (!editing && focused && bands.isNotEmpty()) {
                    Text(
                        stringResource(R.string.tv_sound_press_ok_edit),
                        style = TvType.secondary,
                        color = look.ink,
                        modifier = Modifier.align(Alignment.TopEnd).padding(10.dp).background(Color.Black.copy(alpha = 0.45f), RoundedCornerShape(8.dp)).padding(horizontal = 8.dp, vertical = 2.dp),
                    )
                }
            }
        }
        BandInfo(bands, selected)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TvPill(stringResource(R.string.eq_band_add), { if (bands.size < Equalizer.MAX_BANDS) eqVm.addBand() }, icon = Icons.Rounded.Add, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
            if (selected != null) {
                TvPill(stringResource(R.string.eq_band_remove), { selected?.let(eqVm::removeBand) }, icon = Icons.Rounded.RemoveCircleOutline, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
            }
            TvPill(stringResource(R.string.eq_flat), { eqVm.flat() }, icon = Icons.Rounded.Refresh, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
            if (target != null && own) {
                TvPill(stringResource(R.string.eq_use_default), { eqVm.useDefault() }, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
            }
        }
    }
}

private fun nudge(eqVm: EqViewModel, bands: List<EqBand>, selected: Int?, direction: Int, stepDb: Float) {
    val index = selected ?: return
    val band = bands.getOrNull(index) ?: return
    if (!band.kind.hasGain()) return
    eqVm.updateBand(index) { it.copy(gainDb = steppedGain(it.gainDb, direction, stepDb)) }
}

/** The selected band in words: "Band 2 of 5 · Peak · 1.2k Hz · +3 dB · Q 1". */
@Composable
private fun BandInfo(bands: List<EqBand>, selected: Int?) {
    val look = LocalTvLook.current
    val band = selected?.let(bands::getOrNull)
    val text = if (band == null) {
        stringResource(R.string.tv_sound_no_band)
    } else {
        stringResource(
            R.string.tv_sound_band_info,
            (selected ?: 0) + 1,
            bands.size,
            stringResource(band.kind.labelRes()),
            formatHz(band.freqHz),
            if (band.kind.hasGain()) stringResource(R.string.eq_db, formatDb(band.gainDb)) else "-",
            "%.1f".format(band.q),
        )
    }
    Text(text, style = TvType.body.copy(fontSize = 15.sp), color = if (band == null) look.ink3 else look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
}

// ------------------------------------------------------------------ Assistant tab

@Composable
private fun AssistantTab(entryKey: Int, nonce: Int, onCurve: () -> Unit) {
    val app = LocalContext.current.homefi
    val look = LocalTvLook.current
    val vm: AssistantViewModel = viewModel()
    LaunchedEffect(nonce) { vm.takeHandoff() }
    val ui by vm.ui.collectAsStateWithLifecycle()
    val eq by app.equalizer.settings.collectAsStateWithLifecycle()
    val connected by app.outputs.outputs.collectAsStateWithLifecycle()
    val cards = (ui.proposal != null && !ui.proposalAside) || ui.tasteProposal != null || ui.error != null
    Column(Modifier.fillMaxSize()) {
        Text(
            ui.aiName?.let { stringResource(R.string.assistant_mode_ai, it) } ?: stringResource(R.string.assistant_mode_guided),
            style = TvType.secondary.copy(fontSize = 13.sp),
            color = look.ink3,
            modifier = Modifier.padding(start = 12.dp, top = 4.dp),
        )
        if (!ui.ai) {
            Text(stringResource(R.string.assistant_ai_hint), style = TvType.secondary.copy(fontSize = 13.sp), color = look.ink3, modifier = Modifier.padding(start = 12.dp, end = 24.dp))
        }
        TvChat(
            lines = ui.lines.map { TvChatLine(it.id, it.fromUser, it.text) },
            typing = ui.typing,
            replies = ui.replies,
            canSend = !ui.typing,
            onSend = vm::send,
            entryKey = entryKey,
            promptTitle = stringResource(R.string.eq_assistant),
            placeholder = stringResource(R.string.assistant_input),
            modifier = Modifier.weight(1f).padding(end = 12.dp),
            claimFocus = !cards,
            extraItems = {
                val proposal = ui.proposal
                if (proposal != null && !ui.proposalAside) {
                    item(key = "proposal") {
                        val before = remember(eq, proposal) { app.equalizer.profileOf(proposal.outputId.ifBlank { null }, eq).first }
                        val output = proposal.outputId.ifBlank { null }?.let { outputLabel(it, connected) } ?: stringResource(R.string.eq_output_default)
                        TvChatCard {
                            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                Text(stringResource(R.string.assistant_proposal, output), style = TvType.section, color = look.ink)
                                EqCurve(
                                    profile = proposal.profile,
                                    compare = before,
                                    showPoints = false,
                                    labelSp = 12f,
                                    modifier = Modifier.fillMaxWidth().height(120.dp),
                                )
                                if (proposal.explanation.isNotBlank()) {
                                    Text(proposal.explanation, style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink2, maxLines = 5, overflow = TextOverflow.Ellipsis)
                                }
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    TvPill(stringResource(R.string.assistant_apply), vm::apply, Modifier.tvInitialFocus(key = proposal).tvLeftToDrawer(), icon = Icons.Rounded.Check, primary = true, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                                    TvPill(stringResource(R.string.assistant_keep_talking), vm::keepTalking, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                                }
                            }
                        }
                    }
                }
                ui.tasteProposal?.let { rule ->
                    item(key = "taste-${rule.id}") {
                        TvChatCard {
                            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                Text(rule.label, style = TvType.section, color = look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text(remember(rule) { app.sound.ruleWords(rule) }, style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink2, maxLines = 3, overflow = TextOverflow.Ellipsis)
                                if (rule.explanation.isNotBlank()) Text(rule.explanation, style = TvType.secondary.copy(fontSize = 14.sp), color = look.ink3, maxLines = 3, overflow = TextOverflow.Ellipsis)
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    TvPill(stringResource(R.string.tv_sound_try), vm::tryTaste, Modifier.tvInitialFocus(key = rule.id).tvLeftToDrawer(), icon = Icons.Rounded.GraphicEq, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                                    TvPill(stringResource(R.string.tv_sound_keep), vm::keepTaste, icon = Icons.Rounded.Check, primary = true, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                                    TvPill(stringResource(R.string.assistant_keep_talking), vm::keepTalking, height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                                }
                            }
                        }
                    }
                }
                ui.applied?.let { id ->
                    item(key = "applied-$id") {
                        val output = id.ifBlank { null }?.let { outputLabel(it, connected) } ?: stringResource(R.string.eq_output_default)
                        Row(Modifier.padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            Icon(Icons.Rounded.Check, null, tint = look.ok, modifier = Modifier.size(18.dp))
                            Text(stringResource(R.string.assistant_applied, output), style = TvType.secondary.copy(fontSize = 14.sp), color = look.ok, modifier = Modifier.weight(1f, fill = false))
                            TvPill(stringResource(R.string.tv_sound_tab_curve), onCurve, icon = Icons.Rounded.Tune, height = 28.dp, labelStyle = TvType.body.copy(fontSize = 13.sp))
                        }
                    }
                }
                ui.error?.let { error ->
                    item(key = "error") {
                        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            Text(error, style = TvType.secondary.copy(fontSize = 14.sp), color = look.error)
                            TvPill(stringResource(R.string.assistant_retry), vm::retry, Modifier.tvInitialFocus(key = error).tvLeftToDrawer(), icon = Icons.Rounded.Refresh, height = 30.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                        }
                    }
                }
            },
            actions = { TvPill(stringResource(R.string.assistant_restart), { vm.restart() }, icon = Icons.Rounded.Refresh, height = 34.dp) },
        )
    }
}

// ------------------------------------------------------------------ How does it sound?

private val HOW_WIDTH = 560.dp

/**
 * "How does it sound?": the eight reactions as big buttons. One OK changes the sound for the music that plays
 * (never anything else), and the note on the Sound screen says what changed, with Undo.
 */
@Composable
private fun TvHowSounds(onDismiss: () -> Unit) {
    val app = LocalContext.current.homefi
    val look = LocalTvLook.current
    val now by app.sound.now.collectAsStateWithLifecycle()
    val busy by app.sound.busy.collectAsStateWithLifecycle()
    val scope = now.target
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(Color(0xCC040510)), contentAlignment = Alignment.Center) {
            TvGlass(Modifier.width(HOW_WIDTH), solid = true) {
                Column(Modifier.padding(22.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(stringResource(R.string.sound_how), style = TvType.section, color = look.ink)
                    if (now.context == null) {
                        Text(stringResource(R.string.sound_how_nothing), style = TvType.body, color = look.ink2)
                    } else {
                        Text(
                            stringResource(R.string.sound_how_hint, scope?.name() ?: stringResource(R.string.sound_this_music)),
                            style = TvType.secondary.copy(fontSize = 14.sp),
                            color = look.ink2,
                        )
                        REACTIONS.chunked(2).forEachIndexed { r, row ->
                            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                                row.forEachIndexed { c, (reaction, label, hint) ->
                                    ReactionButton(
                                        label = stringResource(label),
                                        hint = stringResource(if (busy == reaction) R.string.sound_adjusting else hint),
                                        perfect = reaction == com.perseusoft.homefi.core.SoundReaction.PERFECT,
                                        modifier = Modifier.weight(1f).then(if (r == 0 && c == 0) Modifier.tvInitialFocus() else Modifier),
                                        onClick = { if (busy == null) app.sound.react(reaction, scope) },
                                    )
                                }
                            }
                        }
                    }
                    TvPill(stringResource(R.string.tv_close), onDismiss, Modifier.then(if (now.context == null) Modifier.tvInitialFocus() else Modifier), height = 32.dp, labelStyle = TvType.body.copy(fontSize = 14.sp))
                }
            }
        }
    }
}

@Composable
private fun ReactionButton(label: String, hint: String, perfect: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val look = LocalTvLook.current
    val shape = RoundedCornerShape(12.dp)
    TvFocusable(onClick = onClick, modifier = modifier.height(50.dp), shape = shape, focusedScale = 1.04f, glow = false) { focused ->
        val fill = when {
            focused -> Modifier.background(Color.White, shape)
            perfect -> Modifier.background(look.accent.copy(alpha = 0.22f), shape).border(1.dp, look.accent, shape)
            else -> Modifier.background(Color.White.copy(alpha = 0.08f), shape).border(1.dp, look.glassLine, shape)
        }
        Column(fill.padding(horizontal = 12.dp, vertical = 6.dp), verticalArrangement = Arrangement.Center) {
            Text(label, style = TvType.body.copy(fontSize = 15.sp), color = if (focused) look.onFocus else look.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(hint, style = TvType.secondary, color = if (focused) ROW_SUB_FOCUSED else look.ink2, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

// ------------------------------------------------------------------ "Sound for this…"

/** Which sound plays for [scope] (an artist, album, playlist, song): the one that holds it is checked. */
@Composable
private fun TvSoundScopeMenu(scope: TasteScope, onDismiss: () -> Unit) {
    val app = LocalContext.current.homefi
    val taste by app.sound.taste.collectAsStateWithLifecycle()
    val output by app.equalizer.outputId.collectAsStateWithLifecycle()
    val holder = remember(taste, scope, output) { runCatching { tasteSoundCovering(taste, scope, output) }.getOrNull()?.id }
    val actions = buildList {
        taste.rules.forEach { rule ->
            add(TvAction(if (rule.id == holder) Icons.Rounded.Check else Icons.Rounded.Tune, rule.label) { app.sound.assign(rule.id, scope) })
        }
        add(TvAction(if (holder == null) Icons.Rounded.Check else Icons.Rounded.GraphicEq, stringResource(R.string.sound_everything_else)) { holder?.let { app.sound.unassign(it, scope) } })
        add(TvAction(Icons.Rounded.AutoAwesome, stringResource(R.string.sound_new_from_ai)) { app.sound.askAssistant(scope) })
        add(TvAction(Icons.Rounded.Add, stringResource(R.string.sound_new)) { app.sound.newSound(listOf(scope)) })
    }
    TvActionMenu(stringResource(R.string.sound_for_item, scope.name()), null, actions, onDismiss)
}
