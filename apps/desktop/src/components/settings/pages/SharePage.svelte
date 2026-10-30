<script lang="ts">
  import type { Settings } from "../../../lib/api";
  import { share } from "../../../lib/share.svelte";
  import { useSettings } from "../model.svelte";
  import FieldRow from "../FieldRow.svelte";
  import SegmentedRow from "../SegmentedRow.svelte";
  import SettingRow from "../SettingRow.svelte";
  import SettingsGroup from "../SettingsGroup.svelte";
  import SettingsIntro from "../SettingsIntro.svelte";
  import SliderRow from "../SliderRow.svelte";
  import SwitchRow from "../SwitchRow.svelte";

  let { settings }: { settings: Settings } = $props();
  const m = useSettings();
  const sh = $derived(settings.share);

  const COLORS = ["#0891B2", "#1D4ED8", "#10B981", "#F59E0B", "#EF4444", "#EC4899", "#8B5CF6", "#64748B"];
  const ADD_MODES = [
    { id: "free", label: "Freely" },
    { id: "approve", label: "You approve" },
    { id: "limit", label: "Up to a limit" },
  ] as const;
  const ORDERS = [
    { id: "first_come", label: "First come", hint: "Songs play in the order they were added" },
    { id: "turns", label: "Turns", hint: "One song from each person in turn; you count as one more" },
  ] as const;
  const SKIPS = [
    { id: "nobody", label: "Nobody" },
    { id: "everyone", label: "Everyone" },
    { id: "vote", label: "By vote" },
  ] as const;
  const EDITS = [
    { id: "own", label: "Their own" },
    { id: "all", label: "Any" },
    { id: "none", label: "None" },
  ] as const;
  const CODE_MODES = [
    { id: "new", label: "New one each time" },
    { id: "fixed", label: "Always the same" },
  ] as const;
  /** Join codes are numbers; a new fixed one has four digits. */
  const CODE_MIN = 1000;
  const CODE_SPAN = 9000;
  const MAX_PORT = 65535;
  const MAX_SONGS_PER_GUEST = 50;
</script>

<SettingsIntro>One computer or phone plays the music (the hub); others join from theirs to add songs, or play along on their own speakers. On a computer it works over the network it is on (Wi-Fi or Ethernet).</SettingsIntro>

<SettingsGroup title="You">
  <FieldRow label="Your name" value={sh.name} placeholder={share.name} maxlength={32} onchange={(v) => m.saveShare({ name: v.trim() })} />
  <SettingRow label="Your colour">
    {#snippet children(ids)}
      <div class="colors" role="group" aria-labelledby={ids.label}>
        {#each COLORS as c (c)}
          <button type="button" class="swatch" class:on={sh.color.toLowerCase() === c.toLowerCase()} style:background={c} aria-label="Colour {c}" aria-pressed={sh.color.toLowerCase() === c.toLowerCase()} onclick={() => m.saveShare({ color: c })}></button>
        {/each}
        <input type="color" value={sh.color} aria-label="Another colour" onchange={(e) => m.saveShare({ color: e.currentTarget.value })} />
      </div>
    {/snippet}
  </SettingRow>
  <SliderRow
    label="Speaker delay"
    hint="This computer’s speaker latency (Bluetooth speakers lag 100-300 ms); used as a guest and as the hub"
    min={-500}
    max={1000}
    step={10}
    value={sh.speaker_delay_ms}
    format={(n) => `${n} ms`}
    onchange={(n) => m.saveShare({ speaker_delay_ms: n })}
  />
</SettingsGroup>

<SettingsGroup title="This computer as the hub">
  <FieldRow label="Session name" value={sh.session_name} placeholder={share.sessionName} maxlength={60} onchange={(v) => m.saveShare({ session_name: v.trim() })} />
  <FieldRow
    label="Port"
    hint="47800 by default; 0 picks a free one. Changes apply the next time you start sharing."
    type="number"
    value={sh.port}
    min={0}
    max={MAX_PORT}
    width="96px"
    onchange={(v) => m.saveShare({ port: Math.max(0, Math.min(MAX_PORT, Number(v) || 0)) })}
  />
  <SwitchRow label="Start in party mode" checked={sh.party_default} onchange={(on) => m.saveShare({ party_default: on })} />
  <SegmentedRow
    label="Join code"
    options={CODE_MODES}
    value={sh.fixed_code ? "fixed" : "new"}
    onchange={(id) => m.saveShare(id === "new" ? { fixed_code: false } : { fixed_code: true, code: sh.code || String(Math.floor(CODE_MIN + Math.random() * CODE_SPAN)) })}
  />
  {#if sh.fixed_code}
    <FieldRow label="The code" value={sh.code} inputmode="numeric" maxlength={8} mono width="140px" onchange={(v) => m.saveShare({ code: v.replace(/\D/g, "") })} />
  {/if}
  <SegmentedRow label="Guests add songs" options={ADD_MODES} value={sh.add} onchange={(id) => m.saveShare({ add: id })} />
  {#if sh.add === "limit"}
    <FieldRow label="Waiting songs per person" hint="At most this many songs of one guest wait in the queue" type="number" value={sh.max_per_guest} min={1} max={MAX_SONGS_PER_GUEST} width="96px" onchange={(v) => m.saveShare({ max_per_guest: Math.max(1, Number(v) || 1) })} />
  {/if}
  <SegmentedRow label="Queue order" hint={ORDERS.find((o) => o.id === sh.order)?.hint} options={ORDERS} value={sh.order} onchange={(id) => m.saveShare({ order: id })} />
  <SegmentedRow label="Who can skip" options={SKIPS} value={sh.skip} onchange={(id) => m.saveShare({ skip: id })} />
  <SegmentedRow label="Remove and move songs" options={EDITS} value={sh.edit} onchange={(id) => m.saveShare({ edit: id })} />
  <SwitchRow label="Guests can pause and change the volume" checked={sh.playback} onchange={(on) => m.saveShare({ playback: on })} />
  <SwitchRow label="Allow explicit songs" checked={sh.allow_explicit} onchange={(on) => m.saveShare({ allow_explicit: on })} />
  <SwitchRow label="Remove a guest’s songs when they leave" checked={sh.drop_songs_on_leave} onchange={(on) => m.saveShare({ drop_songs_on_leave: on })} />
  <SliderRow
    label="New song"
    hint="In a party, each new song waits until every guest has it loaded, then everyone starts at the same moment. Songs are downloaded ahead, so it is usually under a second."
    min={0}
    max={10}
    step={1}
    value={sh.party_start_wait_sec}
    format={(n) => (n ? `wait up to ${n} s for everyone` : "start at once")}
    onchange={(n) => m.saveShare({ party_start_wait_sec: n })}
  />
  <SwitchRow label="Send the visualizer to guests" hint="Guests in remote control see the meters, spectrum and mascot move with your music. A few hundred bytes a second." checked={sh.send_levels} onchange={(on) => m.saveShare({ send_levels: on })} />
</SettingsGroup>

<SettingsGroup title="Notices on the hub" description="Small and silent, they go away by themselves">
  <SwitchRow label="Someone joins" checked={sh.notice_joined} onchange={(on) => m.saveShare({ notice_joined: on })} />
  <SwitchRow label="Someone leaves" checked={sh.notice_left} onchange={(on) => m.saveShare({ notice_left: on })} />
  <SwitchRow label="Someone adds a song" checked={sh.notice_added} onchange={(on) => m.saveShare({ notice_added: on })} />
  <SwitchRow label="A song waits for your OK" checked={sh.notice_approval} onchange={(on) => m.saveShare({ notice_approval: on })} />
  <SliderRow label="Notices stay" min={1} max={30} step={1} value={sh.notice_seconds} format={(n) => `${n} s`} onchange={(n) => m.saveShare({ notice_seconds: n })} />
</SettingsGroup>

<SettingsGroup title="This computer as a guest">
  <SwitchRow label="Play the party on this computer by default" checked={sh.play_along} onchange={(on) => m.saveShare({ play_along: on })} />
  <SwitchRow label="Open on the player" hint="Joining a session shows the hub’s song in your player layout; off, it opens on the room (queue, votes and volume)" checked={sh.guest_opens_player} onchange={(on) => m.saveShare({ guest_opens_player: on })} />
  <SwitchRow
    label="Smooth sync"
    hint="Closes small gaps by playing a touch faster or slower. This computer’s player can’t change speed yet, so here it always jumps when it is more than 80 ms off; phones use it."
    checked={sh.sync_by_speed}
    onchange={(on) => m.saveShare({ sync_by_speed: on })}
  />
  <SwitchRow
    label="Download the songs ahead"
    hint="Keeps the next and the previous song on this computer and plays them from here, so the party doesn’t cut out when the network is busy. Deleted when you leave."
    checked={sh.preload_songs}
    onchange={(on) => m.saveShare({ preload_songs: on })}
  />
</SettingsGroup>

<SettingsGroup title="Finding each other">
  <SwitchRow label="Look for music nearby" checked={sh.discover} onchange={(on) => m.saveShare({ discover: on })} />
  <SwitchRow label="Offer it on any screen, not only in the listening room" checked={sh.discover_everywhere} disabled={!sh.discover} disabledReason="Turn on “Look for music nearby” first" onchange={(on) => m.saveShare({ discover_everywhere: on })} />
  <SwitchRow label="Announce the session on the network" hint="Phones and computers on this network find it without typing an address (mDNS / Bonjour). They still need the join code." checked={sh.announce_lan} onchange={(on) => m.saveShare({ announce_lan: on })} />
</SettingsGroup>

<style>
  .colors {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--s-1);
  }
  /* a 28 px dot inside a 36 px target */
  .swatch {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    box-shadow: 0 0 0 2px var(--set-card-bg), 0 0 0 3px transparent;
    transition: box-shadow 0.15s;
  }
  .swatch.on {
    box-shadow: 0 0 0 2px var(--set-card-bg), 0 0 0 4px var(--text);
  }
  .swatch:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 4px;
  }
  .colors input[type="color"] {
    width: 36px;
    height: 36px;
    padding: 2px;
  }
</style>
