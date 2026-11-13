package com.perseusoft.homefi.data

import android.app.ActivityManager
import android.content.Context

/** Devices below this (or reporting low RAM) get the light defaults: one player, subtle motion. */
private const val LOW_RAM_BYTES = 2L * 1024 * 1024 * 1024

/** A low-memory device (a Chromecast-class TV box, a small DAP): read once by whoever needs it. */
fun Context.isLowRam(): Boolean = runCatching {
    val am = getSystemService(ActivityManager::class.java)
    val info = ActivityManager.MemoryInfo().also { am.getMemoryInfo(it) }
    am.isLowRamDevice || info.totalMem in 1 until LOW_RAM_BYTES
}.getOrDefault(false)
