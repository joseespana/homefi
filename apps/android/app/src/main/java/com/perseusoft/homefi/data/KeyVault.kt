package com.perseusoft.homefi.data

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The user's AI keys, one per provider id, encrypted with an AES-GCM key that never
 * leaves the Android Keystore (API 23+, so the SR35's 24 too; no extra library). The
 * preferences file only holds `iv:ciphertext`. Keys are read right before a call and
 * never logged or shown back: the UI only learns which providers have one ([saved]).
 */
class KeyVault(context: Context) {
    private val prefs = context.getSharedPreferences("ai_keys", Context.MODE_PRIVATE)

    private val _saved = MutableStateFlow(prefs.all.keys.toSet())
    /** Provider ids with a key. */
    val saved: StateFlow<Set<String>> = _saved.asStateFlow()

    fun has(providerId: String): Boolean = providerId in _saved.value

    /** The key, or null when there is none or it can't be read (Keystore reset). */
    fun get(providerId: String): String? {
        val stored = prefs.getString(providerId, null) ?: return null
        return runCatching {
            val (iv, data) = stored.split(':', limit = 2).map { Base64.decode(it, Base64.NO_WRAP) }
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(128, iv))
            String(cipher.doFinal(data), Charsets.UTF_8)
        }.getOrNull()
    }

    /** Saves [key] for [providerId]; false when the Keystore refused (nothing is stored then). */
    fun put(providerId: String, key: String): Boolean {
        val value = runCatching {
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.ENCRYPT_MODE, secretKey())
            val data = cipher.doFinal(key.trim().toByteArray(Charsets.UTF_8))
            Base64.encodeToString(cipher.iv, Base64.NO_WRAP) + ":" + Base64.encodeToString(data, Base64.NO_WRAP)
        }.getOrNull() ?: return false
        prefs.edit().putString(providerId, value).apply()
        _saved.value = _saved.value + providerId
        return true
    }

    fun remove(providerId: String) {
        prefs.edit().remove(providerId).apply()
        _saved.value = _saved.value - providerId
    }

    @Synchronized
    private fun secretKey(): SecretKey {
        val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (store.getEntry(ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    private companion object {
        const val KEYSTORE = "AndroidKeyStore"
        const val ALIAS = "homefi_ai_keys"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
    }
}
