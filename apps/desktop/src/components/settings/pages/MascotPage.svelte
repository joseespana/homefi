<script lang="ts">
  import type { Settings } from "../../../lib/api";
  import MascotOptions from "../../MascotOptions.svelte";
  import { useSettings } from "../model.svelte";
  import SettingRow from "../SettingRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsIntro from "../SettingsIntro.svelte";
  import SwitchRow from "../SwitchRow.svelte";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();
</script>

<SettingsIntro>A little character that dances to the music in the listening room.</SettingsIntro>

<SettingsGroup title="Mascot" span={2}>
  <SwitchRow label="Show the mascot" checked={settings.room.mascot} onchange={(on) => m.saveRoom({ mascot: on })} />
  {#if settings.room.mascot}
    <SettingRow label="Style, dance and place" hint="Mood, famous dances, turns, arms and legs, temper, size, floating" stack="always">
      <MascotOptions />
    </SettingRow>
  {/if}
</SettingsGroup>
