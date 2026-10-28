package com.perseusoft.homefi.ui.eq

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.Send
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Equalizer
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.core.EqProposal
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.haptics
import com.perseusoft.homefi.ui.theme.Button
import com.perseusoft.homefi.ui.theme.Card
import com.perseusoft.homefi.ui.theme.LocalMotion
import com.perseusoft.homefi.ui.theme.LocalSkin
import com.perseusoft.homefi.ui.theme.OutlinedButton
import androidx.compose.foundation.lazy.LazyItemScope

/**
 * "Set up my sound": a chat with the AI (or the guided questions), quick replies as chips,
 * and the proposal as a card with the before / after curve. Back returns to the equalizer.
 */
@Composable
fun AssistantScreen(onClose: () -> Unit, onEqualizer: () -> Unit, vm: AssistantViewModel = viewModel()) {
    BackHandler(onBack = onClose)
    LaunchedEffect(Unit) { vm.takeHandoff() }
    val ui by vm.ui.collectAsStateWithLifecycle()
    val app = LocalContext.current.homefi
    val connected by app.outputs.outputs.collectAsStateWithLifecycle()
    val settings by app.equalizer.settings.collectAsStateWithLifecycle()
    val haptics = haptics()
    val list = rememberLazyListState()
    var text by rememberSaveable { mutableStateOf("") }

    // new messages, the typing dots and the proposal scroll into view
    val count = ui.lines.size + (if (ui.typing) 1 else 0) + (if (ui.proposal != null && !ui.proposalAside) 1 else 0) + (if (ui.error != null) 1 else 0)
    LaunchedEffect(count, ui.applied) {
        if (count > 0) list.animateScrollToItem(list.layoutInfo.totalItemsCount.coerceAtLeast(1) - 1)
    }

    Column(
        Modifier
            .fillMaxSize()
            .background(LocalSkin.current.base)
            .systemBarsPadding()
            .imePadding(),
    ) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onClose) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.eq_assistant), style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
                Text(
                    ui.aiName?.let { stringResource(R.string.assistant_mode_ai, it) } ?: stringResource(R.string.assistant_mode_guided),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(onClick = { haptics.tick(); vm.restart() }) { Icon(Icons.Rounded.RestartAlt, stringResource(R.string.assistant_restart)) }
        }

        LazyColumn(
            state = list,
            modifier = Modifier.weight(1f).fillMaxWidth(),
            contentPadding = PaddingValues(horizontal = 12.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (!ui.ai) {
                item(key = "hint") {
                    Text(
                        stringResource(R.string.assistant_ai_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = chatMotion().padding(horizontal = 4.dp, vertical = 4.dp),
                    )
                }
            }
            items(ui.lines, key = { "line-${it.id}" }) { line -> Bubble(line, chatMotion()) }
            if (ui.typing) item(key = "typing") { Typing(chatMotion()) }
            val proposal = ui.proposal
            if (proposal != null && !ui.proposalAside) {
                item(key = "proposal") {
                    val before = app.equalizer.profileOf(proposal.outputId.ifBlank { null }, settings).first
                    ProposalCard(
                        proposal = proposal,
                        before = before,
                        outputName = proposal.outputId.ifBlank { null }?.let { outputLabel(it, connected) } ?: stringResource(R.string.eq_output_default),
                        onApply = { haptics.confirm(); vm.apply() },
                        onKeepTalking = { haptics.tick(); vm.keepTalking() },
                        modifier = chatMotion(),
                    )
                }
            }
            ui.applied?.let { id ->
                item(key = "applied-$id") {
                    Applied(
                        outputName = id.ifBlank { null }?.let { outputLabel(it, connected) } ?: stringResource(R.string.eq_output_default),
                        onEqualizer = onEqualizer,
                        modifier = chatMotion(),
                    )
                }
            }
            ui.error?.let { error ->
                item(key = "error") {
                    Column(chatMotion().fillMaxWidth().padding(4.dp)) {
                        Text(error, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
                        TextButton(onClick = { haptics.tick(); vm.retry() }) { Text(stringResource(R.string.assistant_retry)) }
                    }
                }
            }
        }

        // quick replies, then a free answer
        if (ui.replies.isNotEmpty() && !ui.typing) {
            Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp)) {
                ui.replies.forEach { r ->
                    AssistChip(
                        onClick = { haptics.tick(); vm.send(r) },
                        label = { Text(r) },
                        modifier = Modifier.padding(end = 8.dp),
                    )
                }
            }
        }
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            val send = {
                if (text.isNotBlank()) {
                    haptics.press()
                    vm.send(text)
                    text = ""
                }
            }
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                placeholder = { Text(stringResource(R.string.assistant_input)) },
                maxLines = 4,
                shape = RoundedCornerShape(24.dp),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                keyboardActions = KeyboardActions(onSend = { send() }),
                modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(4.dp))
            IconButton(onClick = send, enabled = text.isNotBlank() && !ui.typing) {
                Icon(Icons.AutoMirrored.Rounded.Send, stringResource(R.string.assistant_send), tint = MaterialTheme.colorScheme.primary)
            }
        }
    }
}

/**
 * Rows fade in and slide as the chat grows, but leave at once: a fading-out row (the
 * proposal card) could stay drawn when the list scrolls to the newest message meanwhile.
 */
@Composable
private fun LazyItemScope.chatMotion(): Modifier {
    val motion = LocalMotion.current
    if (!motion.enabled) return Modifier
    return Modifier.animateItem(fadeInSpec = motion.itemFadeIn(), placementSpec = motion.itemPlacement(), fadeOutSpec = null)
}

@Composable
private fun Bubble(line: ChatLine, modifier: Modifier) {
    val scheme = MaterialTheme.colorScheme
    val mine = line.fromUser
    Box(modifier.fillMaxWidth(), contentAlignment = if (mine) Alignment.CenterEnd else Alignment.CenterStart) {
        Text(
            line.text,
            style = MaterialTheme.typography.bodyMedium,
            color = if (mine) scheme.onPrimaryContainer else scheme.onSurface,
            modifier = Modifier
                .widthIn(max = 300.dp)
                .clip(
                    RoundedCornerShape(
                        topStart = 18.dp,
                        topEnd = 18.dp,
                        bottomStart = if (mine) 18.dp else 4.dp,
                        bottomEnd = if (mine) 4.dp else 18.dp,
                    ),
                )
                .background(if (mine) scheme.primaryContainer else scheme.surfaceContainerHigh)
                .padding(horizontal = 14.dp, vertical = 10.dp),
        )
    }
}

/** Three dots that pulse in turn (still, with the motion setting off). */
@Composable
private fun Typing(modifier: Modifier) {
    val scheme = MaterialTheme.colorScheme
    val moves = LocalMotion.current.enabled
    val phase = if (moves) {
        val t = rememberInfiniteTransition(label = "typing")
        t.animateFloat(0f, 3f, infiniteRepeatable(tween(1200), RepeatMode.Restart), label = "typing-phase").value
    } else {
        -1f
    }
    Row(
        modifier
            .clip(RoundedCornerShape(18.dp))
            .background(scheme.surfaceContainerHigh)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val label = stringResource(R.string.assistant_typing)
        repeat(3) { i ->
            val lit = phase < 0 || (phase.toInt() == i)
            Box(
                Modifier
                    .size(8.dp)
                    .alpha(if (lit) 1f else 0.35f)
                    .clip(CircleShape)
                    .background(scheme.onSurfaceVariant),
            )
        }
        if (!moves) Text(label, style = MaterialTheme.typography.bodySmall, color = scheme.onSurfaceVariant)
    }
}

@Composable
private fun ProposalCard(
    proposal: EqProposal,
    before: com.perseusoft.homefi.core.EqProfile?,
    outputName: String,
    onApply: () -> Unit,
    onKeepTalking: () -> Unit,
    modifier: Modifier,
) {
    val scheme = MaterialTheme.colorScheme
    Card(modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp)) {
            Text(stringResource(R.string.assistant_proposal, outputName), style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
            Spacer(Modifier.height(8.dp))
            EqCurve(profile = proposal.profile, compare = before, showPoints = false, modifier = Modifier.fillMaxWidth().height(140.dp))
            Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                Legend(scheme.onSurfaceVariant, stringResource(R.string.assistant_before), dashed = true)
                Spacer(Modifier.width(16.dp))
                Legend(scheme.primary, stringResource(R.string.assistant_after), dashed = false)
            }
            if (proposal.explanation.isNotBlank()) {
                Text(proposal.explanation, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(top = 8.dp))
            }
            Row(Modifier.fillMaxWidth().padding(top = 12.dp), horizontalArrangement = Arrangement.End) {
                OutlinedButton(onClick = onKeepTalking) { Text(stringResource(R.string.assistant_keep_talking)) }
                Spacer(Modifier.width(8.dp))
                Button(onClick = onApply) { Text(stringResource(R.string.assistant_apply)) }
            }
        }
    }
}

@Composable
private fun Legend(color: androidx.compose.ui.graphics.Color, text: String, dashed: Boolean) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Row(horizontalArrangement = Arrangement.spacedBy(3.dp)) {
            repeat(if (dashed) 3 else 1) {
                Box(Modifier.size(width = if (dashed) 5.dp else 21.dp, height = 3.dp).background(color))
            }
        }
        Spacer(Modifier.width(6.dp))
        Text(text, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun Applied(outputName: String, onEqualizer: () -> Unit, modifier: Modifier) {
    Row(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.secondaryContainer)
            .padding(start = 12.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(Icons.Rounded.CheckCircle, null, tint = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.width(8.dp))
        Text(
            stringResource(R.string.assistant_applied, outputName),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSecondaryContainer,
            modifier = Modifier.weight(1f),
        )
        IconButton(onClick = onEqualizer) { Icon(Icons.Rounded.Equalizer, stringResource(R.string.eq_open)) }
    }
}
