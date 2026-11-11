package com.perseusoft.homefi.data

import com.perseusoft.homefi.ui.playlists.ChatStatus
import org.junit.Test
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull

class EvictableTest {
    private fun chat(id: String, status: ChatStatus, at: Long) = ChatEntry(id, status, at)

    @Test
    fun theOldestFailedChatMakesRoom() {
        val chats = listOf(
            chat("a", ChatStatus.Failed("x"), at = 30),
            chat("b", ChatStatus.Waiting, at = 10),
            chat("c", ChatStatus.Failed("y"), at = 20),
        )
        assertEquals("c", evictable(chats))
    }

    @Test
    fun buildsDraftsAndChatsInProgressAreNeverEvicted() {
        val chats = listOf(
            chat("a", ChatStatus.Building(0.5f), at = 1),
            chat("b", ChatStatus.Ready, at = 2),
            chat("c", ChatStatus.Thinking, at = 3),
            chat("d", ChatStatus.Waiting, at = 4),
        )
        assertNull(evictable(chats))
    }

    @Test
    fun noChatsMeansNothingToEvict() {
        assertNull(evictable(emptyList()))
    }
}
