package com.perseusoft.homefi

import android.app.ApplicationExitInfo
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class CrashLogTest {
    private val android11 = 30
    private val android10 = 29

    @Test
    fun `a normal exit is not a crash on Android 11 and later`() {
        listOf(ApplicationExitInfo.REASON_EXIT_SELF, ApplicationExitInfo.REASON_USER_REQUESTED, ApplicationExitInfo.REASON_LOW_MEMORY, ApplicationExitInfo.REASON_CRASH, null)
            .forEach { assertNull(previousRunProblem(android11, markerLeft = true, lastExitReason = it)) }
    }

    @Test
    fun `native crashes, ANRs and failed starts are reported on Android 11 and later`() {
        listOf(ApplicationExitInfo.REASON_CRASH_NATIVE, ApplicationExitInfo.REASON_ANR, ApplicationExitInfo.REASON_INITIALIZATION_FAILURE)
            .forEach { assertNotNull(previousRunProblem(android11, markerLeft = false, lastExitReason = it)) }
    }

    @Test
    fun `before Android 11 only a marker left behind counts`() {
        assertNotNull(previousRunProblem(android10, markerLeft = true, lastExitReason = null))
        assertNull(previousRunProblem(android10, markerLeft = false, lastExitReason = null))
    }
}
