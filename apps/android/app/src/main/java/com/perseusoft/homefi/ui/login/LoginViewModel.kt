package com.perseusoft.homefi.ui.login

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.perseusoft.homefi.core.LoginState
import com.perseusoft.homefi.data.userMessage
import com.perseusoft.homefi.homefi
import kotlinx.coroutines.Job
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

sealed interface LoginUiState {
    data object Idle : LoginUiState
    data object Starting : LoginUiState
    data class Waiting(val userCode: String, val url: String, val secondsLeft: Long) : LoginUiState
    data class Error(val message: String) : LoginUiState
    data object Done : LoginUiState
}

class LoginViewModel(app: Application) : AndroidViewModel(app) {
    private val engine = app.homefi.engine
    private val settings = app.homefi.settings
    private val _state = MutableStateFlow<LoginUiState>(LoginUiState.Idle)
    val state: StateFlow<LoginUiState> = _state.asStateFlow()
    private var job: Job? = null
    /** Wakes the poll early, e.g. when the user comes back from the browser. */
    private val wake = Channel<Unit>(Channel.CONFLATED)

    fun start() {
        job?.cancel()
        job = viewModelScope.launch {
            _state.value = LoginUiState.Starting
            try {
                val device = engine.startLogin()
                val deadline = System.currentTimeMillis() + device.expiresInSecs.toLong() * 1000
                while (true) {
                    val left = (deadline - System.currentTimeMillis()) / 1000
                    if (left <= 0) {
                        _state.value = LoginUiState.Error("The code expired, try again.")
                        return@launch
                    }
                    _state.value = LoginUiState.Waiting(device.userCode, device.url, left)
                    withTimeoutOrNull(device.intervalSecs.toLong() * 1000) { wake.receive() }
                    // Android freezes the app while the browser is in front: a poll
                    // caught in it may never return, so each one gets a deadline.
                    val result = try {
                        withTimeout(25_000) { engine.pollLogin(device.deviceCode) }
                    } catch (_: TimeoutCancellationException) {
                        LoginState.PENDING
                    } catch (e: kotlinx.coroutines.CancellationException) {
                        throw e
                    } catch (_: Exception) {
                        // a network hiccup: keep trying until the code expires
                        LoginState.PENDING
                    }
                    when (result) {
                        LoginState.PENDING -> continue
                        LoginState.SUCCESS -> {
                            _state.value = LoginUiState.Done
                            return@launch
                        }
                        LoginState.EXPIRED -> {
                            _state.value = LoginUiState.Error("The code expired, try again.")
                            return@launch
                        }
                    }
                }
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.value = LoginUiState.Error(e.userMessage())
            }
        }
    }

    /** The personal-use notice was just accepted on the login screen: remember it, then sign in. */
    fun acceptTermsAndStart() {
        settings.acceptTerms()
        start()
    }

    /** Check right away (the app is back in front after signing in elsewhere). */
    fun checkNow() {
        wake.trySend(Unit)
    }

    fun cancel() {
        job?.cancel()
        _state.value = LoginUiState.Idle
    }
}
