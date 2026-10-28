package com.perseusoft.homefi.data

import com.perseusoft.homefi.core.Account
import com.perseusoft.homefi.core.HomefiEngine
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * The signed-in Tidal account. `engine.account()` reads the credentials file, so it is
 * read once on the IO dispatcher at start and again after logging in or out ([refresh]),
 * never during composition.
 */
class AccountStore(private val engine: HomefiEngine, private val scope: CoroutineScope) {
    private val _account = MutableStateFlow<Account?>(null)
    /** `null` while signed out (or before the first read, see [loaded]). */
    val account: StateFlow<Account?> = _account.asStateFlow()

    private val _loaded = MutableStateFlow(false)
    /** The first read finished: until then "signed out" may just mean "not known yet". */
    val loaded: StateFlow<Boolean> = _loaded.asStateFlow()

    private val first = CompletableDeferred<Unit>()
    private var job: Job? = null

    init {
        refresh()
    }

    /** As far as known now (never blocks). */
    val isLoggedIn: Boolean get() = _account.value != null

    /** Reads the account again (after a login, a logout or an expired session). */
    fun refresh() {
        // the newest read wins (a login may land while the first read still runs)
        job?.cancel()
        job = scope.launch(Dispatchers.IO) {
            _account.value = runCatching { engine.account() }.getOrNull()
            _loaded.value = true
            first.complete(Unit)
        }
    }

    /** Signed out locally: no need to read the file to know. */
    fun signedOut() {
        _account.value = null
    }

    /** The account once the first read is done (for code that runs right at start). */
    suspend fun current(): Account? {
        first.await()
        return _account.value
    }
}
