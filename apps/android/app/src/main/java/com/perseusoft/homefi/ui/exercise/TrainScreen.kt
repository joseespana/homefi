package com.perseusoft.homefi.ui.exercise

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.DirectionsBike
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.foundation.background
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.font.FontWeight
import com.perseusoft.homefi.ui.theme.Brand
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.player.exercise.RideUi
import com.perseusoft.homefi.ui.theme.Card

/** The Train tab: setup, build and plan in one place; the ride itself is [RideOverlay]. */
@Composable
fun TrainScreen() {
    val session = LocalContext.current.homefi.ride
    val ui by session.ui.collectAsStateWithLifecycle()
    val form by session.form.collectAsStateWithLifecycle()
    val chat by session.chat.collectAsStateWithLifecycle()
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Hero(compact = ui != RideUi.Setup)
            when (val state = ui) {
                RideUi.Setup -> {
                    MyRides()
                    RideSetup(form, chat)
                }
                is RideUi.Building -> RideBuilding(state, session::cancelBuild)
                is RideUi.Preview -> RidePreview(state.built, state.saved, session)
                is RideUi.Failed -> RideFailed(state.message, session::backToSetup)
                is RideUi.Riding, is RideUi.Finished -> Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(stringResource(R.string.ex_in_progress), style = MaterialTheme.typography.titleMedium)
                    GradientButton(stringResource(R.string.ex_open_ride), session::open, Modifier.fillMaxWidth())
                }
            }
        }
        if (ui == RideUi.Setup) BuildBar()
    }
}

/** The Train header: a brand-gradient wash behind the big title and one line about what happens next. */
@Composable
private fun Hero(compact: Boolean) {
    val wash = Brush.horizontalGradient(listOf(Brand.Blue.copy(alpha = HERO_ALPHA), Brand.Teal.copy(alpha = HERO_ALPHA), Brand.Green.copy(alpha = HERO_ALPHA)))
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp).background(wash, RoundedCornerShape(24.dp)).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(stringResource(R.string.ex_title), style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.ExtraBold)
        if (!compact) {
            Text(stringResource(R.string.ex_setup_sub), color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

private const val HERO_ALPHA = 0.22f

/** Home's "Start a ride" card. */
@Composable
fun TrainCard(onClick: () -> Unit) {
    Card(onClick = onClick, modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp)) {
        Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Icon(Icons.AutoMirrored.Rounded.DirectionsBike, null, tint = MaterialTheme.colorScheme.primary)
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.ex_card_title), style = MaterialTheme.typography.titleMedium)
                Text(stringResource(R.string.ex_card_text), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}
