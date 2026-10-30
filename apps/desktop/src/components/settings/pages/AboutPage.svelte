<script lang="ts">
  import type { Settings, UpdateSettings } from "../../../lib/api";
  import { app } from "../../../lib/state.svelte";
  import { TERMS_TEXT, TERMS_VERSION } from "../../../lib/terms";
  import { manualReasonText, updates } from "../../../lib/update.svelte";
  import Icon from "../../Icon.svelte";
  import { useSettings } from "../model.svelte";
  import ActionRow from "../ActionRow.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingRow from "../SettingRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsNote from "../SettingsNote.svelte";
  import SwitchRow from "../SwitchRow.svelte";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  const UPDATE_DEFAULTS: UpdateSettings = { auto_check: true, prereleases: false, install: "ask" };
  const updateSettings = $derived<UpdateSettings>({ ...UPDATE_DEFAULTS, ...(settings.updates ?? {}) });
  const saveUpdates = (patch: Partial<UpdateSettings>) => m.save({ updates: { ...updateSettings, ...patch } });
  const INSTALLS = [
    { id: "ask", label: "Ask" },
    { id: "idle", label: "When HomeFi is idle" },
  ] as const;

  const upd = $derived(updates.view);
  const busy = $derived(updates.checking || (!!upd && ["checking", "downloading", "installing"].includes(upd.status)));
  const updateLine = $derived.by(() => {
    if (!upd) return "";
    switch (upd.status) {
      case "checking":
        return "Looking for a new version…";
      case "available":
        return `HomeFi ${upd.version} is available${upd.prerelease ? " (beta)" : ""}`;
      case "downloading":
        return `Downloading HomeFi ${upd.version}…${upd.total ? ` ${Math.round((upd.downloaded / upd.total) * 100)}%` : ""}`;
      case "ready":
        return `HomeFi ${upd.version} is ready: restart to finish`;
      case "installing":
        return `Installing HomeFi ${upd.version}…`;
      case "error":
        return upd.error ?? "Couldn't check for updates.";
      case "upToDate":
        return `HomeFi ${upd.current} is the newest version`;
      default:
        return `HomeFi ${upd.current}`;
    }
  });
</script>

<SettingsGroup title="HomeFi">
  <SettingRow label="Version" hint="{app.info?.platform} · ffmpeg {app.info?.ffmpeg ? 'found' : 'not found (not needed for FLAC / Hi-Res)'}">
    <span class="version">{app.info?.version}</span>
  </SettingRow>
  <SettingRow label="Data folder" hint={app.info?.appDir} />
</SettingsGroup>

<SettingsGroup title="Updates" description="Privacy: checking only asks GitHub for HomeFi's public releases (api.github.com and the release files). Nothing about you, your account or your music is sent.">
  {#if upd}
    {#if !upd.inPlace}<SettingsNote>{manualReasonText(upd.manualReason)}</SettingsNote>{/if}
    <ActionRow label={updateLine} hint={upd.lastChecked ? `Last checked ${new Date(upd.lastChecked).toLocaleString()}` : "Not checked yet"}>
      {#if upd.status === "available" && upd.inPlace}
        <button type="button" class="btn primary" onclick={() => updates.download()}>Update now</button>
      {:else if upd.status === "available" || (upd.status === "error" && upd.version && !upd.inPlace)}
        <button type="button" class="btn primary" onclick={() => updates.openPage()}><Icon name="external" size={14} /> Download</button>
      {:else if upd.status === "ready"}
        <button type="button" class="btn primary" onclick={() => updates.restart("now")}>Restart now</button>
      {/if}
      <button type="button" class="btn tonal" disabled={busy} onclick={() => updates.check()}>Check now</button>
    </ActionRow>
    {#if upd.version && ["available", "downloading", "ready"].includes(upd.status)}
      <ActionRow label="What's new in {upd.version}">
        <button type="button" class="btn tonal" onclick={() => updates.openPage()}><Icon name="external" size={14} /> Release notes</button>
      </ActionRow>
    {/if}
  {/if}
  <SwitchRow label="Check for updates automatically" hint="When HomeFi starts and every 6 hours while it's open. “Check now” works either way." checked={updateSettings.auto_check} onchange={(on) => saveUpdates({ auto_check: on })} />
  <SwitchRow label="Include pre-releases" hint="Beta versions get new things first, and may have rough edges" checked={updateSettings.prereleases} onchange={(on) => saveUpdates({ prereleases: on })} />
  <SegmentedRow
    label="Install updates"
    hint={updateSettings.install === "ask" ? "A small card offers the new version; nothing restarts until you say so" : "Downloaded by itself, installed after 5 minutes with no music, downloads or shared session (HomeFi restarts)"}
    options={INSTALLS}
    value={updateSettings.install}
    onchange={(id) => saveUpdates({ install: id })}
  />
</SettingsGroup>

<SettingsGroup title="Personal use">
  <SettingRow label="Terms" stack="always">
    <p class="terms">{TERMS_TEXT}</p>
  </SettingRow>
  <SettingRow
    label={settings.tidal_terms_accepted >= TERMS_VERSION ? `Accepted${settings.tidal_terms_accepted_at ? ` on ${new Date(settings.tidal_terms_accepted_at).toLocaleDateString()}` : ""}` : "Not accepted yet"}
    hint={settings.tidal_terms_accepted >= TERMS_VERSION ? undefined : "You'll be asked when you connect Tidal."}
  />
</SettingsGroup>

<style>
  .version {
    font: var(--set-label-font);
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .terms {
    margin: 0;
    max-width: 65ch;
    font: 400 13px/1.55 var(--font);
    color: var(--muted);
    user-select: text;
  }
</style>
