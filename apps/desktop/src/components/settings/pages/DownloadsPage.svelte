<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import type { Settings } from "../../../lib/api";
  import Icon from "../../Icon.svelte";
  import { QUALITIES } from "../options";
  import { useSettings } from "../model.svelte";
  import ActionRow from "../ActionRow.svelte";
  import ChoiceList from "../ChoiceList.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SwitchRow from "../SwitchRow.svelte";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  async function pickDownloadDir() {
    const dir = await open({ directory: true, defaultPath: settings.download_dir });
    if (typeof dir === "string") m.save({ download_dir: dir });
  }
</script>

<SettingsGroup title="Quality">
  <ChoiceList label="Download quality" options={QUALITIES} value={settings.quality} onchange={(id) => m.save({ quality: id })} />
</SettingsGroup>

<SettingsGroup title="Files">
  <SwitchRow label="Embed cover art" checked={settings.embed_cover} onchange={(on) => m.save({ embed_cover: on })} />
  <SwitchRow label="Embed lyrics" checked={settings.embed_lyrics} onchange={(on) => m.save({ embed_lyrics: on })} />
  <SwitchRow label="Skip files that already exist" checked={settings.skip_existing} onchange={(on) => m.save({ skip_existing: on })} />
</SettingsGroup>

<SettingsGroup title="Save downloads in">
  <ActionRow label="Downloads folder" hint={settings.download_dir}>
    <button type="button" class="icon-btn" title="Show in Finder / Explorer" aria-label="Show the downloads folder in Finder / Explorer" onclick={() => revealItemInDir(settings.download_dir)}><Icon name="external" /></button>
    <button type="button" class="btn tonal" onclick={pickDownloadDir}>Change</button>
  </ActionRow>
</SettingsGroup>
