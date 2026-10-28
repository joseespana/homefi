package com.perseusoft.homefi.data

import com.perseusoft.homefi.core.AiConfig
import com.perseusoft.homefi.core.AiProvider
import com.perseusoft.homefi.core.AiProviderInfo
import com.perseusoft.homefi.core.aiProviders

/**
 * The AI the assistant would use: provider info from the core, the user's choices from
 * [SettingsStore] and the key from [KeyVault], passed apart from every record.
 */
class AiSetup(private val settings: SettingsStore, val keys: KeyVault) {
    /** Every provider the core knows, with names and key pages. */
    val providers: List<AiProviderInfo> by lazy { runCatching { aiProviders() }.getOrDefault(emptyList()) }

    fun provider(id: String): AiProviderInfo? = providers.firstOrNull { it.id == id }

    /** Provider, model and address for [info] as the user set them (no key). */
    fun config(info: AiProviderInfo, ai: AiSettings = settings.current.ai): AiConfig {
        val base = ai.baseUrls[info.id]?.trim()?.takeIf { it.isNotEmpty() }
        val provider = when (info.provider) {
            is AiProvider.OpenAiCompatible -> AiProvider.OpenAiCompatible(base ?: info.defaultBaseUrl)
            else -> info.provider
        }
        return AiConfig(provider = provider, model = ai.models[info.id].orEmpty(), baseUrl = base)
    }

    /**
     * What the assistant needs to talk to an AI, or null for the guided wizard: a provider
     * picked, its key saved when it needs one, an address for a custom server.
     */
    fun ready(): Ready? {
        val ai = settings.current.ai
        val info = provider(ai.provider) ?: return null
        val key = if (info.needsKey) keys.get(info.id) ?: return null else null
        if (info.provider is AiProvider.OpenAiCompatible && ai.baseUrls[info.id].isNullOrBlank()) return null
        return Ready(info, config(info, ai), key)
    }

    /** A plain class whose `toString` never prints [key]. */
    class Ready(val info: AiProviderInfo, val config: AiConfig, val key: String?) {
        override fun toString() = "Ready(${info.id})"
    }
}
