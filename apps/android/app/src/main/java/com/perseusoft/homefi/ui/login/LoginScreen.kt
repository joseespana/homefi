package com.perseusoft.homefi.ui.login

import android.content.Intent
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.material.icons.automirrored.rounded.Login
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.rememberScrollState
import android.widget.Toast
import android.content.ActivityNotFoundException
import android.net.Uri
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import com.perseusoft.homefi.ui.theme.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import com.perseusoft.homefi.ui.theme.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.perseusoft.homefi.R
import com.perseusoft.homefi.homefi
import com.perseusoft.homefi.ui.HomefiLogo

@Composable
fun LoginScreen(onLoggedIn: () -> Unit, vm: LoginViewModel = viewModel()) {
    val state by vm.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    var webLogin by remember { mutableStateOf(false) }
    val settings by context.homefi.settings.settings.collectAsStateWithLifecycle()
    // the personal-use notice comes first until it's accepted (once per notice version)
    val askTerms = !settings.termsAccepted
    var termsChecked by rememberSaveable { mutableStateOf(false) }

    if (state is LoginUiState.Done) onLoggedIn()
    // back from signing in with the browser (or another device): look right away
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { vm.checkNow() }

    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        HomefiLogo(96.dp)
        Spacer(Modifier.height(24.dp))
        Text(stringResource(R.string.app_name), style = MaterialTheme.typography.headlineLarge, fontWeight = FontWeight.Bold)
        Text(
            stringResource(R.string.login_tagline),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(40.dp))

        when (val s = state) {
            LoginUiState.Idle, is LoginUiState.Error -> {
                if (s is LoginUiState.Error) {
                    Text(s.message, color = MaterialTheme.colorScheme.error, textAlign = TextAlign.Center)
                    Spacer(Modifier.height(16.dp))
                }
                if (askTerms) {
                    TermsNotice(termsChecked, { termsChecked = it })
                    Spacer(Modifier.height(16.dp))
                }
                Button(
                    onClick = { if (askTerms) vm.acceptTermsAndStart() else vm.start() },
                    enabled = !askTerms || termsChecked,
                    modifier = Modifier.fillMaxWidth().height(52.dp),
                ) {
                    Text(stringResource(R.string.login_connect))
                }
            }
            LoginUiState.Starting, LoginUiState.Done -> CircularProgressIndicator()
            is LoginUiState.Waiting -> {
                Text(stringResource(R.string.login_code_hint), textAlign = TextAlign.Center)
                Spacer(Modifier.height(16.dp))
                Surface(color = MaterialTheme.colorScheme.primaryContainer, shape = MaterialTheme.shapes.large) {
                    SelectionContainer {
                        Text(
                            s.userCode,
                            fontFamily = FontFamily.Monospace,
                            fontSize = 34.sp,
                            letterSpacing = 6.sp,
                            fontWeight = FontWeight.Bold,
                            color = MaterialTheme.colorScheme.onPrimaryContainer,
                            modifier = Modifier.padding(horizontal = 28.dp, vertical = 16.dp),
                        )
                    }
                }
                Spacer(Modifier.height(24.dp))
                // A&K players and similar ship no browser: the page opens inside the app
                Button(onClick = { webLogin = true }, modifier = Modifier.fillMaxWidth().height(52.dp)) {
                    Icon(Icons.AutoMirrored.Rounded.Login, null, Modifier.size(ButtonDefaults.IconSize))
                    Spacer(Modifier.size(ButtonDefaults.IconSpacing))
                    Text(stringResource(R.string.login_here))
                }
                Spacer(Modifier.height(8.dp))
                OutlinedButton(
                    onClick = {
                        try {
                            context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(s.url)))
                        } catch (_: ActivityNotFoundException) {
                            Toast.makeText(context, R.string.login_no_browser, Toast.LENGTH_SHORT).show()
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Icon(Icons.AutoMirrored.Rounded.OpenInNew, null, Modifier.size(ButtonDefaults.IconSize))
                    Spacer(Modifier.size(ButtonDefaults.IconSpacing))
                    Text(stringResource(R.string.login_open_browser))
                }
                Spacer(Modifier.height(20.dp))
                QrCode(s.url, 150.dp)
                Text(
                    stringResource(R.string.login_scan),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 8.dp),
                )
                if (webLogin) TidalWebLogin(s.url) { webLogin = false }
                Spacer(Modifier.height(12.dp))
                OutlinedButton(onClick = vm::cancel, modifier = Modifier.fillMaxWidth()) {
                    Text(stringResource(R.string.cancel))
                }
                Spacer(Modifier.height(16.dp))
                Text(
                    stringResource(R.string.login_waiting, s.secondsLeft / 60, s.secondsLeft % 60),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
