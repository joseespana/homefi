<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import type { Settings } from "../../../lib/api";
  import { app } from "../../../lib/state.svelte";
  import { useSettings } from "../model.svelte";
  import PathList from "../PathList.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsIntro from "../SettingsIntro.svelte";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();

  async function addLibraryDir() {
    const dir = await open({ directory: true });
    if (typeof dir === "string" && !settings.library_dirs.includes(dir)) m.save({ library_dirs: [...settings.library_dirs, dir] });
  }

  async function removeLibraryDir(dir: string) {
    const ok = await app.confirm({
      title: "Remove this music folder?",
      text: `${dir} will stop showing in your library. The files stay on disk.`,
      confirmLabel: "Remove folder",
      danger: true,
    });
    if (ok) m.save({ library_dirs: settings.library_dirs.filter((d) => d !== dir) });
  }
</script>

<SettingsIntro>HomeFi plays and lists the music in these folders. The downloads folder is always included.</SettingsIntro>

<SettingsGroup title="Folders">
  <PathList label="Music folders" paths={settings.library_dirs} addLabel="Add music folder" emptyText="No folders yet. Add the folders with your music." onadd={addLibraryDir} onremove={removeLibraryDir} />
</SettingsGroup>
