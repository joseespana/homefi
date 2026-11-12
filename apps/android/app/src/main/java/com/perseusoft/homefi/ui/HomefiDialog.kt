package com.perseusoft.homefi.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.scaleIn
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.perseusoft.homefi.ui.theme.Brand

/**
 * One button of a [HomefiDialog]. [danger] paints it in the error colour (it removes or ends something); a disabled
 * one ([enabled] false) waits for the form above it; [icon] leads the label.
 */
class DialogAction(
    val label: String,
    val danger: Boolean = false,
    val enabled: Boolean = true,
    val icon: ImageVector? = null,
    val onClick: () -> Unit,
)

private const val DIALOG_MAX_DP = 420
private const val COMPACT_HEIGHT_DP = 480
private const val BUTTON_DP = 52
private const val ICON_BADGE_DP = 56
private const val ICON_BADGE_ALPHA = 0.18f
private const val BORDER_ALPHA = 0.35f
private const val ENTER_SCALE = 0.92f
private const val DARK_CONTENT_LUMINANCE = 0.45f

/**
 * HomeFi's own dialog, used instead of the stock Android one: a rounded card with a soft accent border, an optional
 * icon badge, a centred title and text, and full-width pill buttons stacked so the thumb reaches them. [primary] is the
 * filled pill (the brand gradient unless [accent] is given), [others] are tonal pills under it, and [dismiss] is the
 * quiet text button at the bottom that calls [onDismiss] ([quiet] replaces it when that button does something else).
 * A tap outside or Back calls [onDismiss] unless [dismissible] is false (a choice that must be made). [subtitle] goes
 * under the title (a path, a name); the body scrolls unless [scrollBody] is false (it holds its own list).
 */
@Composable
fun HomefiDialog(
    title: String,
    onDismiss: () -> Unit,
    primary: DialogAction? = null,
    others: List<DialogAction> = emptyList(),
    dismiss: String? = null,
    quiet: DialogAction? = null,
    text: String? = null,
    subtitle: String? = null,
    dismissible: Boolean = true,
    scrollBody: Boolean = true,
    icon: ImageVector? = null,
    accent: Color? = null,
    content: (@Composable ColumnScope.() -> Unit)? = null,
) {
    val tint = accent ?: MaterialTheme.colorScheme.primary
    // on a short screen (a phone on its side) the badge goes and the margins shrink, so the body keeps its room
    val compact = LocalConfiguration.current.screenHeightDp < COMPACT_HEIGHT_DP
    val edge = if (compact) 16.dp else 24.dp
    val shown = remember { MutableTransitionState(false).apply { targetState = true } }
    Dialog(
        onDismissRequest = { if (dismissible) onDismiss() },
        properties = DialogProperties(
            usePlatformDefaultWidth = false, dismissOnBackPress = dismissible, dismissOnClickOutside = dismissible,
        ),
    ) {
        AnimatedVisibility(shown, enter = fadeIn() + scaleIn(initialScale = ENTER_SCALE)) {
            Surface(
                modifier = Modifier.padding(edge).widthIn(max = DIALOG_MAX_DP.dp).fillMaxWidth(),
                shape = RoundedCornerShape(28.dp),
                color = MaterialTheme.colorScheme.surfaceContainerHigh,
                border = BorderStroke(1.dp, tint.copy(alpha = BORDER_ALPHA)),
                shadowElevation = 12.dp,
            ) {
                Column(
                    Modifier.padding(horizontal = 24.dp, vertical = edge),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    if (icon != null && !compact) {
                        Box(
                            Modifier.size(ICON_BADGE_DP.dp).clip(CircleShape).background(tint.copy(alpha = ICON_BADGE_ALPHA)),
                            contentAlignment = Alignment.Center,
                        ) { Icon(icon, null, tint = tint, modifier = Modifier.size(28.dp)) }
                    }
                    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(title, fontSize = 22.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center)
                        if (subtitle != null) {
                            Text(
                                subtitle, style = MaterialTheme.typography.bodySmall, textAlign = TextAlign.Center, maxLines = 2,
                                overflow = TextOverflow.StartEllipsis, color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                    if (text != null || content != null) {
                        val body = Modifier.weight(1f, fill = false).fillMaxWidth()
                        Column(
                            if (scrollBody) body.verticalScroll(rememberScrollState()) else body,
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            if (text != null) {
                                Text(
                                    text, style = MaterialTheme.typography.bodyMedium, textAlign = TextAlign.Center,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.fillMaxWidth(),
                                )
                            }
                            content?.invoke(this)
                        }
                    }
                    Column(Modifier.fillMaxWidth().padding(top = if (compact) 0.dp else 8.dp), verticalArrangement = Arrangement.spacedBy(if (compact) 4.dp else 8.dp)) {
                        primary?.let { PrimaryPill(it, accent) }
                        others.forEach { OtherPill(it) }
                        val bottom = quiet ?: dismiss?.let { DialogAction(it, onClick = onDismiss) }
                        if (bottom != null) {
                            TextButton(onClick = bottom.onClick, enabled = bottom.enabled, modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp)) {
                                Text(bottom.label, fontSize = 15.sp)
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun PrimaryPill(a: DialogAction, accent: Color?) {
    val solid = if (a.danger) MaterialTheme.colorScheme.error else accent
    val pill = Modifier.fillMaxWidth().heightIn(min = BUTTON_DP.dp).clip(RoundedCornerShape(50))
    Button(
        onClick = a.onClick,
        enabled = a.enabled,
        modifier = if (solid == null && a.enabled) pill.background(Brand.gradient) else pill,
        shape = RoundedCornerShape(50),
        colors = if (solid == null) {
            ButtonDefaults.buttonColors(containerColor = Color.Transparent, contentColor = Color.White)
        } else {
            ButtonDefaults.buttonColors(containerColor = solid, contentColor = readableOn(solid))
        },
    ) { PillLabel(a, 16) }
}

@Composable
private fun OtherPill(a: DialogAction) {
    FilledTonalButton(
        onClick = a.onClick,
        enabled = a.enabled,
        modifier = Modifier.fillMaxWidth().heightIn(min = BUTTON_DP.dp),
        shape = RoundedCornerShape(50),
        colors = if (a.danger) {
            ButtonDefaults.filledTonalButtonColors(contentColor = MaterialTheme.colorScheme.error)
        } else {
            ButtonDefaults.filledTonalButtonColors()
        },
    ) { PillLabel(a, 15) }
}

@Composable
private fun PillLabel(a: DialogAction, sizeSp: Int) {
    if (a.icon != null) Icon(a.icon, null, Modifier.padding(end = 8.dp).size(20.dp))
    Text(a.label, fontSize = sizeSp.sp, fontWeight = FontWeight.Bold)
}

/** Black on light colours (the ride's zone colours), white on dark ones. */
private fun readableOn(c: Color): Color = if (c.luminance() > DARK_CONTENT_LUMINANCE) Color.Black else Color.White
