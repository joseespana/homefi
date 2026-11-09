//! Offline tests of the mixer (no audio device) and of the loader.

use std::f32::consts::TAU;
use std::sync::Arc;

use homefi_core::dj::mixer_math::EQ_KILL_DB;
use homefi_core::dj::{CrossfaderCurve, Grid, MixerConfig};
use ringbuf::traits::{Consumer, Split};
use ringbuf::HeapRb;

use super::control::{DeckId, DjControl, DjEqBand};
use super::deck::DeckAudio;
use super::loader::load_track;
use super::mixer::{Command, DjMixer, DjShared};
use super::DjError;

const RATE: u32 = 48_000;
const A: DeckId = DeckId::A;
const B: DeckId = DeckId::B;

fn cfg() -> MixerConfig {
    MixerConfig { auto_gain: false, ..MixerConfig::default() }
}

fn mixer() -> DjMixer {
    DjMixer::new(RATE, cfg(), Arc::new(DjShared::default()))
}

fn song(samples: Vec<f32>, rate: u32, grid: Option<Grid>) -> Arc<DeckAudio> {
    Arc::new(DeckAudio::new(samples, rate, grid, 0.0))
}

/// A stereo sine (same on both channels).
fn sine(freq: f32, amp: f32, seconds: f32, grid: Option<Grid>) -> Arc<DeckAudio> {
    let n = (seconds * RATE as f32) as usize;
    let samples = (0..n)
        .flat_map(|i| {
            let v = amp * (TAU * freq * i as f32 / RATE as f32).sin();
            [v, v]
        })
        .collect();
    song(samples, RATE, grid)
}

/// Stereo output of `frames` frames, left channel only.
fn render_left(m: &mut DjMixer, frames: usize) -> Vec<f32> {
    let mut out = vec![0f32; frames * 2];
    m.render(&mut out, 2);
    out.iter().step_by(2).copied().collect()
}

/// Renders in the odd-sized pieces a real callback would ask for.
fn run(m: &mut DjMixer, frames: usize) -> Vec<f32> {
    let mut left = Vec::with_capacity(frames);
    let mut left_over = frames;
    let mut size = 441;
    while left_over > 0 {
        let n = size.min(left_over);
        left.extend(render_left(m, n));
        left_over -= n;
        size = if size == 441 { 997 } else { 441 };
    }
    left
}

/// Amplitude of `freq` in `x` (Goertzel).
fn amp_at(x: &[f32], freq: f32) -> f32 {
    let w = TAU * freq / RATE as f32;
    let (mut re, mut im) = (0f32, 0f32);
    for (i, s) in x.iter().enumerate() {
        re += s * (w * i as f32).cos();
        im += s * (w * i as f32).sin();
    }
    2.0 * (re * re + im * im).sqrt() / x.len() as f32
}

fn db(ratio: f32) -> f32 {
    20.0 * ratio.log10()
}

fn grid(first_ms: f64, bpm: f64) -> Option<Grid> {
    Grid::new(first_ms, 60_000.0 / bpm)
}

fn silence(m: &mut DjMixer, ms: usize) {
    run(m, RATE as usize * ms / 1000);
}

#[test]
fn crossfader_blends_the_decks_with_equal_power() {
    let mut m = mixer();
    m.load(0, sine(440.0, 0.5, 4.0, None));
    m.load(1, sine(1000.0, 0.5, 4.0, None));
    m.apply(DjControl::Play { deck: A });
    m.apply(DjControl::Play { deck: B });
    let window = RATE as usize / 2;

    let mut levels = Vec::new();
    for x in [0.0, 0.5, 1.0] {
        m.apply(DjControl::Crossfader { value: x });
        run(&mut m, RATE as usize / 4); // let the fader settle
        let out = run(&mut m, window);
        levels.push((amp_at(&out, 440.0), amp_at(&out, 1000.0)));
    }
    let [(a0, b0), (am, bm), (a1, b1)] = [levels[0], levels[1], levels[2]];
    assert!((a0 - 0.5).abs() < 0.02 && b0 < 0.01, "fader at A: {a0} {b0}");
    assert!((a1 - 0.0).abs() < 0.01 && (b1 - 0.5).abs() < 0.02, "fader at B: {a1} {b1}");
    let half = 0.5 * std::f32::consts::FRAC_1_SQRT_2;
    assert!((am - half).abs() < 0.02 && (bm - half).abs() < 0.02, "centre: {am} {bm}");
    // equal power: the combined level at the centre equals the level at the ends
    assert!(((am * am + bm * bm).sqrt() - 0.5).abs() < 0.02);

    // the cut curve keeps both at full volume in the middle
    m.apply(DjControl::SetCurve { curve: CrossfaderCurve::Cut });
    m.apply(DjControl::Crossfader { value: 0.5 });
    run(&mut m, RATE as usize / 4);
    let out = run(&mut m, window);
    assert!((amp_at(&out, 440.0) - 0.5).abs() < 0.02 && (amp_at(&out, 1000.0) - 0.5).abs() < 0.02);
}

/// A mixer with the fader hard on deck A and everything settled, for bit-exact tests.
fn exact_mixer(audio: Arc<DeckAudio>) -> DjMixer {
    let mut m = mixer();
    m.load(0, audio);
    m.apply(DjControl::Crossfader { value: 0.0 });
    silence(&mut m, 500);
    m
}

fn ramp_song(frames: usize) -> Arc<DeckAudio> {
    // unique, small values: any wrong sample shows
    let samples = (0..frames).flat_map(|i| [i as f32 * 1e-5, -(i as f32) * 1e-5]).collect();
    song(samples, RATE, None)
}

#[test]
fn unity_playback_is_bit_exact() {
    let mut m = exact_mixer(ramp_song(20_000));
    m.apply(DjControl::Play { deck: A });
    let out = run(&mut m, 5_000);
    for (i, v) in out.iter().enumerate() {
        assert_eq!(*v, i as f32 * 1e-5, "frame {i}");
    }
}

#[test]
fn loops_are_exact_to_the_sample() {
    const START: f64 = 1_000.0;
    const LEN: usize = 7_321; // not a nice number on purpose
    let mut m = exact_mixer(ramp_song(60_000));
    m.apply(DjControl::Seek { deck: A, ms: START / RATE as f64 * 1000.0 });
    m.apply(DjControl::LoopIn { deck: A });
    m.apply(DjControl::Seek { deck: A, ms: (START + LEN as f64) / RATE as f64 * 1000.0 });
    m.apply(DjControl::LoopOut { deck: A });
    let view = m.view().decks[0];
    let (s, e) = view.loop_frames.expect("loop set");
    assert!((s - START).abs() < 1e-6 && ((e - s) - LEN as f64).abs() < 1e-6, "{s} {e}");

    // the playhead is at the loop's end: playing wraps at once
    m.apply(DjControl::Seek { deck: A, ms: START / RATE as f64 * 1000.0 });
    m.apply(DjControl::Play { deck: A });
    let out = run(&mut m, LEN * 4 + 123);
    for (i, v) in out.iter().enumerate() {
        let expected = (START as usize + i % LEN) as f32 * 1e-5;
        assert_eq!(*v, expected, "frame {i}");
    }
    // leaving the loop lets the song run on
    m.apply(DjControl::LoopExit { deck: A });
    let next = run(&mut m, LEN * 2);
    let after = (START as usize + (LEN * 4 + 123) % LEN) as f32 * 1e-5;
    assert_eq!(next[0], after);
    assert_eq!(*next.last().unwrap(), (START as usize + (LEN * 4 + 123) % LEN + LEN * 2 - 1) as f32 * 1e-5);
}

#[test]
fn beat_loops_follow_the_grid_and_quantize() {
    let mut m = mixer();
    m.load(0, sine(220.0, 0.3, 20.0, grid(100.0, 120.0)));
    // 1.07 s: nearest beat is 1.1 s (beats at 100 + 500k ms)
    m.apply(DjControl::Seek { deck: A, ms: 1_070.0 });
    m.apply(DjControl::LoopBeats { deck: A, beats: 4 });
    let (s, e) = m.view().decks[0].loop_frames.unwrap();
    assert!((s - 1_100.0 * 48.0).abs() < 1.0, "snapped to the beat: {s}");
    assert!((e - s - 4.0 * 500.0 * 48.0).abs() < 1.0, "4 beats long: {}", e - s);
    // no grid, no beat loop
    let mut plain = mixer();
    plain.load(0, sine(220.0, 0.3, 5.0, None));
    plain.apply(DjControl::LoopBeats { deck: A, beats: 4 });
    assert!(plain.view().decks[0].loop_frames.is_none());
    // too short to be a loop
    m.apply(DjControl::LoopExit { deck: A });
    m.apply(DjControl::LoopIn { deck: A });
    m.apply(DjControl::LoopOut { deck: A });
    assert!(m.view().decks[0].loop_frames.is_none());
}

#[test]
fn hot_cues_set_jump_and_clear() {
    let mut m = mixer();
    m.load(0, sine(220.0, 0.3, 20.0, None));
    m.apply(DjControl::Seek { deck: A, ms: 2_500.0 });
    m.apply(DjControl::HotCue { deck: A, index: 3 }); // empty: sets
    assert!((m.view().decks[0].hot_frames[3].unwrap() - 2.5 * RATE as f64).abs() < 1e-6);
    m.apply(DjControl::Seek { deck: A, ms: 9_000.0 });
    m.apply(DjControl::HotCue { deck: A, index: 3 }); // set: jumps
    assert!((m.deck(0).pos - 2.5 * RATE as f64).abs() < 1e-6);
    // playing continues from the cue
    m.apply(DjControl::Play { deck: A });
    run(&mut m, 480);
    assert!((m.deck(0).pos - (2.5 * RATE as f64 + 480.0)).abs() < 1e-6);
    m.apply(DjControl::ClearHotCue { deck: A, index: 3 });
    assert!(m.view().decks[0].hot_frames[3].is_none());
    // out-of-range indexes are ignored
    m.apply(DjControl::HotCue { deck: A, index: 200 });
    m.apply(DjControl::SetHotCue { deck: A, index: 8 });
}

#[test]
fn cue_button_works_like_a_cdj() {
    let mut m = mixer();
    m.load(0, sine(220.0, 0.3, 20.0, None));
    m.apply(DjControl::Seek { deck: A, ms: 1_000.0 });
    m.apply(DjControl::Cue { deck: A, pressed: true }); // stopped elsewhere: sets the cue
    m.apply(DjControl::Cue { deck: A, pressed: false });
    assert!((m.view().decks[0].cue_frames - 48_000.0).abs() < 1e-6);
    m.apply(DjControl::Play { deck: A });
    run(&mut m, 4_800);
    m.apply(DjControl::Cue { deck: A, pressed: true }); // playing: back to the cue, stopped
    assert!(!m.view().decks[0].playing && (m.deck(0).pos - 48_000.0).abs() < 1e-6);
    m.apply(DjControl::Cue { deck: A, pressed: true }); // on the cue: preview while held
    run(&mut m, 2_400);
    assert!(m.view().decks[0].playing);
    m.apply(DjControl::Cue { deck: A, pressed: false });
    assert!(!m.view().decks[0].playing && (m.deck(0).pos - 48_000.0).abs() < 1e-6);
}

#[test]
fn varispeed_follows_tempo_bend_and_sample_rate() {
    let mut m = mixer();
    m.load(0, sine(220.0, 0.3, 30.0, None));
    m.apply(DjControl::Tempo { deck: A, percent: 8.0 });
    m.apply(DjControl::Play { deck: A });
    run(&mut m, 4_800);
    assert!((m.deck(0).pos - 4_800.0 * 1.08).abs() < 1e-6, "{}", m.deck(0).pos);
    // beyond the range is limited
    m.apply(DjControl::Tempo { deck: A, percent: 30.0 });
    assert!((m.view().decks[0].tempo - 0.08).abs() < 1e-9);
    // bend adds bend_pct (4 %) while held
    m.apply(DjControl::Tempo { deck: A, percent: 0.0 });
    m.apply(DjControl::Bend { deck: A, amount: 1.0 });
    let before = m.deck(0).pos;
    run(&mut m, 1_000);
    assert!((m.deck(0).pos - before - 1_000.0 * 1.04).abs() < 1e-6);

    // a 44.1 kHz song on a 48 kHz output plays at 44100/48000
    let mut m = mixer();
    m.load(0, song(vec![0.1; 44_100 * 2 * 5], 44_100, None));
    m.apply(DjControl::Play { deck: A });
    run(&mut m, 4_800);
    assert!((m.deck(0).pos - 4_800.0 * 44_100.0 / 48_000.0).abs() < 1e-6);
}

#[test]
fn the_song_ends_cleanly() {
    let mut m = exact_mixer(ramp_song(1_000));
    m.apply(DjControl::Play { deck: A });
    let out = run(&mut m, 1_500);
    assert!(!m.view().decks[0].playing);
    assert!(out[1_000..].iter().all(|v| v.abs() < 1e-3), "silence after the end");
}

#[test]
fn sync_matches_tempo_and_beats() {
    let mut m = mixer();
    m.load(0, sine(220.0, 0.3, 60.0, grid(0.0, 120.0)));
    m.load(1, sine(330.0, 0.3, 60.0, grid(100.0, 126.0)));
    m.apply(DjControl::Seek { deck: A, ms: 3_130.0 });
    m.apply(DjControl::Seek { deck: B, ms: 5_000.0 });
    m.apply(DjControl::Play { deck: A });
    m.apply(DjControl::Sync { deck: B });
    let v = m.view();
    assert!(v.decks[1].synced && !v.decks[0].synced);
    assert!((v.decks[1].tempo - (120.0 / 126.0 - 1.0)).abs() < 1e-9);
    assert!((v.decks[1].current_bpm - 120.0).abs() < 1e-9);

    let phase_gap = |m: &DjMixer| {
        let (ga, gb) = (grid(0.0, 120.0).unwrap(), grid(100.0, 126.0).unwrap());
        let d = ga.phase(m.deck(0).pos_ms()) - gb.phase(m.deck(1).pos_ms());
        (d + 0.5).rem_euclid(1.0) - 0.5
    };
    // playing: the beats are on top of each other, and stay that way
    m.apply(DjControl::Play { deck: B });
    assert!(phase_gap(&m).abs() < 1e-6, "{}", phase_gap(&m));
    run(&mut m, RATE as usize * 3);
    assert!(phase_gap(&m).abs() < 1e-4, "{}", phase_gap(&m));

    // moving the leader's tempo drags the follower along
    m.apply(DjControl::Tempo { deck: A, percent: 4.0 });
    run(&mut m, 480);
    assert!((m.view().decks[1].current_bpm - 124.8).abs() < 1e-6);
    // touching the follower's tempo keeps the pair together: it leads, the other follows
    m.apply(DjControl::Tempo { deck: B, percent: 0.0 });
    let v = m.view();
    assert!(!v.decks[1].synced && v.decks[0].synced, "the roles swap");
    run(&mut m, 480);
    let v = m.view();
    assert!((v.decks[0].current_bpm - 126.0).abs() < 1e-6, "{}", v.decks[0].current_bpm);
    // Sync off ends it
    m.apply(DjControl::SyncOff { deck: A });
    m.apply(DjControl::Tempo { deck: B, percent: 2.0 });
    run(&mut m, 480);
    assert!(!m.view().decks[0].synced && (m.view().decks[0].current_bpm - 126.0).abs() < 1e-6);
    // out of range: nothing changes
    let mut far = mixer();
    far.load(0, sine(220.0, 0.3, 10.0, grid(0.0, 90.0)));
    far.load(1, sine(330.0, 0.3, 10.0, grid(0.0, 140.0)));
    far.apply(DjControl::Play { deck: A });
    far.apply(DjControl::Sync { deck: B });
    assert!(!far.view().decks[1].synced && far.view().decks[1].tempo == 0.0);
}

#[test]
fn a_song_loaded_next_to_a_playing_one_takes_its_tempo() {
    let mut m = mixer();
    m.load(0, sine(220.0, 0.3, 30.0, grid(0.0, 124.0)));
    m.apply(DjControl::Play { deck: A });
    m.load(1, sine(330.0, 0.3, 30.0, grid(0.0, 128.0)));
    let v = m.view().decks[1];
    assert!(v.synced && (v.current_bpm - 124.0).abs() < 1e-9);
    // with the setting off it stays as it is
    let mut m = DjMixer::new(RATE, MixerConfig { sync_on_load: false, ..cfg() }, Arc::new(DjShared::default()));
    m.load(0, sine(220.0, 0.3, 30.0, grid(0.0, 124.0)));
    m.apply(DjControl::Play { deck: A });
    m.load(1, sine(330.0, 0.3, 30.0, grid(0.0, 128.0)));
    assert!(!m.view().decks[1].synced);
}

/// Two 120 BPM songs ready for a transition: A playing, B waiting.
fn mix_setup() -> DjMixer {
    let mut m = mixer();
    m.load(0, sine(60.0, 0.4, 30.0, grid(0.0, 120.0)));
    m.load(1, sine(80.0, 0.4, 30.0, grid(250.0, 120.0)));
    m.apply(DjControl::Crossfader { value: 0.0 });
    silence(&mut m, 300);
    m.apply(DjControl::Seek { deck: A, ms: 1_620.0 }); // 120 ms before the beat at 2.0 s... beats every 500 ms
    m.apply(DjControl::Play { deck: A });
    m
}

#[test]
fn auto_mix_starts_on_the_beat_ramps_the_fader_and_swaps_the_bass() {
    let mut m = mix_setup();
    // A is at 1620 ms: its next beat is at 2000 ms, 380 ms away
    let wait_frames = (380.0 * 48.0) as usize;
    m.apply(DjControl::AutoMix { to: B, beats: 8 });
    let v = m.view();
    let auto = v.auto_mix.expect("armed");
    assert!(auto.armed && auto.progress == 0.0 && auto.to == 1 && auto.beats == 8);
    assert!(!v.decks[1].playing, "B waits for the beat");
    assert!(v.decks[1].synced);
    // B was put on one of its own beats (250 + 500k ms)
    let b_ms = m.deck(1).pos_ms();
    assert!(((b_ms - 250.0).rem_euclid(500.0)).abs() < 1e-6, "{b_ms}");

    // one frame before the beat B still waits, on the beat it plays: sample accurate
    run(&mut m, wait_frames - 1);
    assert!(!m.view().decks[1].playing);
    assert!(m.view().auto_mix.unwrap().armed);
    run(&mut m, 1);
    run(&mut m, 1); // the start is applied at the head of the next block
    let v = m.view();
    assert!(v.decks[1].playing, "B started");
    assert!(!v.auto_mix.unwrap().armed);
    // the two songs are on the same beat
    let ga = grid(0.0, 120.0).unwrap();
    let gb = grid(250.0, 120.0).unwrap();
    let gap = ga.phase(m.deck(0).pos_ms()) - gb.phase(m.deck(1).pos_ms());
    assert!(((gap + 0.5).rem_euclid(1.0) - 0.5).abs() < 0.002, "gap {gap}");

    // bass: the incoming song comes in without it, the outgoing one has it
    run(&mut m, 4_800);
    assert!(m.deck(1).eq_now_db(0) < EQ_KILL_DB + 1.0, "{}", m.deck(1).eq_now_db(0));
    assert!(m.deck(0).eq_now_db(0).abs() < 0.5);
    let early = m.view().auto_mix.unwrap().progress;
    assert!(early < 0.1);

    // half way: crossfader in the middle
    let total = 8 * 24_000; // 8 beats of 500 ms
    run(&mut m, total / 2 - 4_800 - 2);
    let half = m.view();
    assert!((half.auto_mix.unwrap().progress - 0.5).abs() < 0.002);
    assert!((half.crossfader - 0.5).abs() < 0.002, "{}", half.crossfader);

    // after the swap window the outgoing bass is gone and the incoming one is back
    run(&mut m, total * 3 / 10);
    run(&mut m, 2_400); // let the EQ glide
    assert!(m.deck(0).eq_now_db(0) < EQ_KILL_DB + 1.0, "{}", m.deck(0).eq_now_db(0));
    assert!(m.deck(1).eq_now_db(0).abs() < 0.5, "{}", m.deck(1).eq_now_db(0));

    // the end: fader on B, A stopped, nothing left running
    run(&mut m, total / 5 + 2_000);
    let end = m.view();
    assert!(end.auto_mix.is_none());
    assert_eq!(end.crossfader, 1.0);
    assert!(!end.decks[0].playing && end.decks[1].playing);
    run(&mut m, 4_800);
    assert!(m.deck(0).eq_now_db(0).abs() < 0.5, "A's bass is back on its knob for next time");
}

#[test]
fn auto_mix_without_the_bass_swap_only_moves_the_fader() {
    let mut m = mix_setup();
    m.handle(Command::Config(MixerConfig { bass_swap: false, ..cfg() }));
    m.apply(DjControl::AutoMix { to: B, beats: 8 });
    let total = 8 * 24_000;
    run(&mut m, (380.0 * 48.0) as usize + 2 + 4_800);
    assert!(m.deck(1).eq_now_db(0).abs() < 0.5, "B keeps its bass: {}", m.deck(1).eq_now_db(0));
    run(&mut m, total * 7 / 10);
    assert!(m.deck(0).eq_now_db(0).abs() < 0.5, "A keeps its bass: {}", m.deck(0).eq_now_db(0));
    assert!(m.view().crossfader > 0.6, "the fader still moves");
}

#[test]
fn auto_mix_ramp_length_is_exact() {
    let mut m = mix_setup();
    m.apply(DjControl::AutoMix { to: B, beats: 8 });
    run(&mut m, (380.0 * 48.0) as usize + 1);
    // now Running: exactly 8 * 24000 frames until it ends
    let total = 8 * 24_000;
    let mut left = total;
    let mut guard = 0;
    while m.view().auto_mix.is_some() && guard < 10_000 {
        let n = left.min(97);
        run(&mut m, n);
        left -= n;
        guard += 1;
        if left == 0 {
            break;
        }
    }
    // (one block of the running state was already consumed by the +1 above)
    assert!(m.view().auto_mix.is_none(), "{} frames left", left);
    assert_eq!(m.view().crossfader, 1.0);
}

#[test]
fn touching_the_crossfader_cancels_an_auto_mix() {
    let mut m = mix_setup();
    m.apply(DjControl::AutoMix { to: B, beats: 16 });
    run(&mut m, 30_000);
    assert!(m.view().auto_mix.is_some_and(|a| !a.armed && a.progress > 0.0));
    m.apply(DjControl::Crossfader { value: 0.25 });
    assert!(m.view().auto_mix.is_none());
    run(&mut m, 24_000);
    let v = m.view();
    assert!((v.crossfader - 0.25).abs() < 0.001, "the fader goes where the user put it: {}", v.crossfader);
    assert!(v.decks[0].playing && v.decks[1].playing, "both keep playing");
    // the bass holds are released
    assert!(m.deck(0).eq_now_db(0).abs() < 0.5 && m.deck(1).eq_now_db(0).abs() < 0.5);

    // an explicit cancel, also while armed
    let mut m = mix_setup();
    m.apply(DjControl::AutoMix { to: B, beats: 8 });
    m.apply(DjControl::AutoMixCancel);
    run(&mut m, 48_000);
    assert!(m.view().auto_mix.is_none() && !m.view().decks[1].playing);
    // ejecting a deck in the mix cancels it too
    let mut m = mix_setup();
    m.apply(DjControl::AutoMix { to: B, beats: 8 });
    m.apply(DjControl::Eject { deck: B });
    assert!(m.view().auto_mix.is_none() && !m.view().decks[1].loaded);
}

#[test]
fn auto_mix_without_grids_or_without_a_playing_song() {
    // no grids: starts at once and fades over `beats` at 120 BPM
    let mut m = mixer();
    m.load(0, sine(200.0, 0.3, 30.0, None));
    m.load(1, sine(300.0, 0.3, 30.0, None));
    m.apply(DjControl::Crossfader { value: 0.0 });
    silence(&mut m, 300);
    m.apply(DjControl::Play { deck: A });
    m.apply(DjControl::AutoMix { to: B, beats: 8 });
    run(&mut m, 10);
    assert!(m.view().decks[1].playing);
    run(&mut m, 8 * 24_000);
    assert!(m.view().auto_mix.is_none() && m.view().crossfader == 1.0);

    // nothing playing: the song just starts and the fader goes to it
    let mut m = mixer();
    m.load(1, sine(300.0, 0.3, 30.0, None));
    m.apply(DjControl::AutoMix { to: B, beats: 16 });
    assert!(m.view().decks[1].playing && m.view().auto_mix.is_none());
    run(&mut m, 48_000);
    assert!((m.view().crossfader - 1.0).abs() < 0.001);
    // an empty target is ignored
    let mut m = mixer();
    m.apply(DjControl::AutoMix { to: A, beats: 8 });
    assert!(m.view().auto_mix.is_none());
    // odd lengths take the nearest allowed; 0 takes the setting
    let mut m = mix_setup();
    m.apply(DjControl::AutoMix { to: B, beats: 20 });
    assert_eq!(m.view().auto_mix.unwrap().beats, 16);
    let mut m = mix_setup();
    m.apply(DjControl::AutoMix { to: B, beats: 0 });
    assert_eq!(m.view().auto_mix.unwrap().beats, 16);
}

#[test]
fn eq_kill_filter_and_trim_shape_the_sound() {
    let mut m = exact_mixer(sine(60.0, 0.4, 6.0, None));
    m.apply(DjControl::Play { deck: A });
    run(&mut m, RATE as usize / 2);
    let open = amp_at(&run(&mut m, RATE as usize / 2), 60.0);
    m.apply(DjControl::EqKill { deck: A, band: DjEqBand::Low, on: true });
    run(&mut m, RATE as usize / 2);
    let killed = amp_at(&run(&mut m, RATE as usize / 2), 60.0);
    assert!(db(killed / open) <= -26.0, "kill only took {} dB", db(killed / open));
    assert!(m.view().decks[0].eq_kill[0]);
    m.apply(DjControl::EqKill { deck: A, band: DjEqBand::Low, on: false });
    run(&mut m, RATE as usize / 2);
    let back = amp_at(&run(&mut m, RATE as usize / 2), 60.0);
    assert!((back / open - 1.0).abs() < 0.02);

    // trim in dB
    m.apply(DjControl::Trim { deck: A, db: -6.0 });
    run(&mut m, RATE as usize / 2);
    let trimmed = amp_at(&run(&mut m, RATE as usize / 2), 60.0);
    assert!((db(trimmed / open) + 6.0).abs() < 0.3);
    m.apply(DjControl::Trim { deck: A, db: 0.0 });

    // a closed low-pass takes the highs out
    let mut m = exact_mixer(sine(5000.0, 0.4, 6.0, None));
    m.apply(DjControl::Play { deck: A });
    let open = amp_at(&run(&mut m, RATE as usize), 5000.0);
    m.apply(DjControl::Filter { deck: A, value: -1.0 });
    run(&mut m, RATE as usize / 2);
    let filtered = amp_at(&run(&mut m, RATE as usize / 2), 5000.0);
    assert!(db(filtered / open) < -20.0, "{}", db(filtered / open));
}

#[test]
fn moving_a_knob_never_clicks() {
    let mut m = exact_mixer(sine(100.0, 0.4, 8.0, None));
    m.apply(DjControl::Play { deck: A });
    run(&mut m, 4_800);
    let max_step = |x: &[f32]| x.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0f32, f32::max);
    let baseline = max_step(&run(&mut m, 4_800));
    for control in [
        DjControl::Eq { deck: A, band: DjEqBand::Low, db: -12.0 },
        DjControl::Eq { deck: A, band: DjEqBand::Low, db: 6.0 },
        DjControl::Filter { deck: A, value: 0.5 },
        DjControl::Filter { deck: A, value: -0.7 },
        DjControl::Filter { deck: A, value: 0.0 },
        DjControl::Volume { deck: A, value: 0.2 },
        DjControl::Trim { deck: A, db: -9.0 },
        DjControl::Crossfader { value: 0.9 },
        DjControl::Master { value: 0.3 },
        DjControl::EqKill { deck: A, band: DjEqBand::Mid, on: true },
    ] {
        m.apply(control);
        let step = max_step(&run(&mut m, 4_800));
        // a 100 Hz sine moves at most 0.4 * 2 pi * 100 / 48000 = 0.0052 per sample. A filter sweep
        // rings a little by nature; a click would be an order of magnitude more.
        let limit = if matches!(control, DjControl::Filter { .. }) { 0.03 } else { baseline * 1.6 + 0.002 };
        assert!(step < limit, "{control:?}: step {step} vs {baseline}");
    }
}

#[test]
fn the_sampler_plays_pads_over_the_mix() {
    let mut m = mixer();
    silence(&mut m, 100);
    m.apply(DjControl::Sampler { pad: 7 });
    assert!(m.view().pads_active[7]);
    let out = run(&mut m, RATE as usize / 10);
    assert!(out.iter().any(|v| v.abs() > 0.1), "the hit is audible");
    run(&mut m, RATE as usize * 3);
    assert!(!m.view().pads_active[7], "the pad ends");
    m.apply(DjControl::Sampler { pad: 99 }); // ignored
    m.apply(DjControl::Sampler { pad: 0 });
    m.apply(DjControl::SamplerVolume { value: 0.0 });
    let quiet = run(&mut m, 4_800);
    assert!(quiet.iter().all(|v| v.abs() < 1e-6));
}

#[test]
fn a_pad_plays_the_sound_the_layout_gave_it() {
    let half_second = RATE as usize / 2;
    let mut default = mixer();
    default.apply(DjControl::Sampler { pad: 7 });
    run(&mut default, half_second);
    assert!(default.view().pads_active[7], "the default Hit lasts 0.6 s");
    let all_lasers = [homefi_core::dj::SfxKind::Laser; homefi_core::dj::PAD_COUNT];
    let mut custom = DjMixer::with_pads(RATE, cfg(), Arc::new(DjShared::default()), &all_lasers);
    custom.apply(DjControl::Sampler { pad: 7 });
    run(&mut custom, half_second);
    assert!(!custom.view().pads_active[7], "a Laser lasts 0.4 s");
}

#[test]
fn the_limiter_keeps_the_master_below_full_scale() {
    let mut m = exact_mixer(sine(200.0, 0.95, 4.0, None));
    m.load(1, sine(210.0, 0.95, 4.0, None));
    m.apply(DjControl::Crossfader { value: 0.5 });
    m.apply(DjControl::SetCurve { curve: CrossfaderCurve::Cut });
    m.apply(DjControl::Master { value: 1.5 });
    m.apply(DjControl::Play { deck: A });
    m.apply(DjControl::Play { deck: B });
    let out = run(&mut m, RATE as usize);
    assert!(out.iter().all(|v| v.abs() <= 1.0 && v.is_finite()));
    assert!(m.shared().take().limiting, "the limiter reports that it worked");
    assert!(out.iter().any(|v| v.abs() > 0.9));
}

#[test]
fn channel_layouts_other_than_stereo() {
    let mut m = exact_mixer(sine(200.0, 0.4, 2.0, None));
    m.apply(DjControl::Play { deck: A });
    let mut mono = vec![0f32; 480];
    m.render(&mut mono, 1);
    assert!(mono.iter().any(|v| v.abs() > 0.05));
    let mut six = vec![1f32; 6 * 100];
    m.render(&mut six, 6);
    assert!(six.chunks(6).all(|f| f[2..].iter().all(|v| *v == 0.0)), "extra channels are silent");
    assert!(six.chunks(6).any(|f| f[0].abs() > 0.05 && f[0] == f[1]));
    // a partial trailing frame is silenced
    let mut odd = vec![1f32; 7];
    m.render(&mut odd, 2);
    assert_eq!(odd[6], 0.0);
}

#[test]
fn loading_ejecting_and_the_published_state() {
    let shared = Arc::new(DjShared::default());
    let mut m = DjMixer::new(RATE, cfg(), shared.clone());
    let (trash_tx, mut trash) = HeapRb::<Arc<DeckAudio>>::new(4).split();
    m.set_trash(trash_tx);

    let first = sine(200.0, 0.4, 2.0, grid(0.0, 100.0));
    m.handle(Command::Load { deck: 0, audio: first.clone() });
    assert!(trash.try_pop().is_none());
    m.handle(Command::Load { deck: 0, audio: sine(300.0, 0.4, 2.0, None) });
    // the replaced song comes back for the engine thread to free
    let back = trash.try_pop().expect("old song returned");
    assert!(Arc::ptr_eq(&back, &first));
    m.handle(Command::Control(DjControl::Eject { deck: A }));
    assert!(trash.try_pop().is_some());
    assert!(!m.view().decks[0].loaded);

    // a new song starts with its cue on the first beat and a clean slate
    m.handle(Command::Load { deck: 1, audio: sine(200.0, 0.4, 2.0, grid(333.0, 120.0)) });
    assert!((m.view().decks[1].cue_frames - 333.0 * 48.0).abs() < 1e-6);

    // the callback publishes; a reader sees the state and the peaks once
    m.handle(Command::Control(DjControl::Play { deck: B }));
    run(&mut m, 4_800);
    let first_read = shared.take();
    assert!(first_read.decks[1].playing && first_read.decks[1].peak[0] > 0.05 && first_read.peak[0] > 0.05);
    let second_read = shared.take();
    assert_eq!(second_read.decks[1].peak, [0.0; 2]);
    assert!(second_read.decks[1].playing);

    // settings arrive through the ring too
    m.handle(Command::Config(MixerConfig { tempo_range_pct: 50.0, ..cfg() }));
    m.apply(DjControl::Tempo { deck: B, percent: 40.0 });
    assert!((m.view().decks[1].tempo - 0.4).abs() < 1e-9);
}

#[test]
fn hostile_values_are_harmless() {
    let mut m = mixer();
    m.load(0, sine(200.0, 0.4, 2.0, grid(0.0, 120.0)));
    for c in [
        DjControl::Seek { deck: A, ms: f64::NAN },
        DjControl::Seek { deck: A, ms: -1e12 },
        DjControl::Seek { deck: A, ms: 1e15 },
        DjControl::Nudge { deck: A, ms: f64::INFINITY },
        DjControl::Tempo { deck: A, percent: f32::NAN },
        DjControl::Bend { deck: A, amount: f32::NAN },
        DjControl::Trim { deck: A, db: f32::INFINITY },
        DjControl::Eq { deck: A, band: DjEqBand::Mid, db: f32::NAN },
        DjControl::Eq { deck: A, band: DjEqBand::High, db: 1e9 },
        DjControl::Filter { deck: A, value: f32::NAN },
        DjControl::Volume { deck: A, value: -4.0 },
        DjControl::Crossfader { value: f32::NAN },
        DjControl::Master { value: f32::INFINITY },
        DjControl::LoopBeats { deck: A, beats: 0 },
        DjControl::LoopBeats { deck: A, beats: u32::MAX },
        DjControl::AutoMix { to: B, beats: 8 },
        DjControl::Play { deck: A },
    ] {
        m.apply(c);
        let out = run(&mut m, 2_000);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0), "{c:?}");
    }
}

// ---- loader -------------------------------------------------------------------------------

/// A 16-bit PCM WAV.
fn wav(channels: u16, rate: u32, samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::new();
    v.extend(b"RIFF");
    v.extend((36 + data_len).to_le_bytes());
    v.extend(b"WAVEfmt ");
    v.extend(16u32.to_le_bytes());
    v.extend(1u16.to_le_bytes());
    v.extend(channels.to_le_bytes());
    v.extend(rate.to_le_bytes());
    v.extend((rate * u32::from(channels) * 2).to_le_bytes());
    v.extend((channels * 2).to_le_bytes());
    v.extend(16u16.to_le_bytes());
    v.extend(b"data");
    v.extend(data_len.to_le_bytes());
    for s in samples {
        v.extend(s.to_le_bytes());
    }
    v
}

/// Mono click track at `bpm`, `seconds` long.
fn click_wav(bpm: f64, seconds: f64, channels: u16) -> Vec<u8> {
    let rate = 44_100u32;
    let n = (seconds * rate as f64) as usize;
    let period = (60.0 / bpm * rate as f64) as usize;
    let mut mono = vec![0i16; n];
    let mut at = 0;
    while at < n {
        for k in 0..220 {
            if let Some(s) = mono.get_mut(at + k) {
                let env = (std::f32::consts::PI * k as f32 / 220.0).sin();
                *s = (env * (TAU * 1000.0 * k as f32 / rate as f32).sin() * 20_000.0) as i16;
            }
        }
        at += period;
    }
    let samples: Vec<i16> = mono.iter().flat_map(|s| std::iter::repeat_n(*s, channels as usize)).collect();
    wav(channels, rate, &samples)
}

#[test]
fn loading_a_song_decodes_analyzes_and_caches() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.wav");
    std::fs::write(&path, click_wav(120.0, 16.0, 2)).unwrap();
    let cache = homefi_core::dj::AnalysisCache::new(dir.path().join("cache"));
    let track = load_track(&path, "Click", u64::MAX, 1024, Some(&cache), &|| false).unwrap();
    assert_eq!(track.info.title, "Click");
    assert_eq!(track.info.sample_rate, 44_100);
    assert!((track.info.duration_ms - 16_000.0).abs() < 5.0);
    assert!(track.info.has_grid && (track.info.bpm - 120.0).abs() < 0.5, "{}", track.info.bpm);
    assert_eq!(track.info.bytes, 16 * 44_100 * 8);
    assert_eq!(track.audio.frames(), 16 * 44_100);
    assert!(track.analysis.overview.len() > 700);
    assert!(track.info.auto_gain_db > 0.0, "a quiet click track gets a boost");
    // the analysis was stored and is used next time
    assert!(cache.get(&homefi_core::dj::AnalysisCache::key_for_file(&path)).is_some());
    let again = load_track(&path, "Click", u64::MAX, 1024, Some(&cache), &|| false).unwrap();
    assert_eq!(*again.analysis, *track.analysis);

    // mono files play on both channels
    let mono_path = dir.path().join("mono.wav");
    std::fs::write(&mono_path, click_wav(120.0, 6.0, 1)).unwrap();
    let mono = load_track(&mono_path, "Mono", u64::MAX, 1024, None, &|| false).unwrap();
    assert_eq!(mono.audio.frames(), 6 * 44_100);
}

#[test]
fn loading_refuses_what_does_not_fit_and_what_is_broken() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.wav");
    std::fs::write(&path, click_wav(120.0, 10.0, 2)).unwrap();

    // 10 s need ~3.4 MB; allow 1 MB
    match load_track(&path, "x", 1024 * 1024, 1, None, &|| false) {
        Err(DjError::TooBig { needed_mb, limit_mb }) => {
            assert!(needed_mb >= 3 && limit_mb == 1);
            let words = DjError::TooBig { needed_mb, limit_mb }.to_string();
            assert!(words.contains("memory") && !words.contains("Err"), "{words}");
        }
        other => panic!("expected TooBig, got {:?}", other.map(|t| t.info)),
    }
    // a newer load cancels this one
    assert!(matches!(load_track(&path, "x", u64::MAX, 1024, None, &|| true), Err(DjError::Cancelled)));
    // garbage and missing files fail politely
    let junk = dir.path().join("junk.wav");
    std::fs::write(&junk, b"definitely not audio").unwrap();
    assert!(matches!(load_track(&junk, "x", u64::MAX, 1024, None, &|| false), Err(DjError::Unreadable(_))));
    assert!(matches!(
        load_track(&dir.path().join("missing.flac"), "x", u64::MAX, 1024, None, &|| false),
        Err(DjError::Unreadable(_))
    ));
}

/// Opens the real default output device and plays nothing for a moment. Needs a sound card, so it
/// only runs on request: `cargo test -p homefi-player dj::tests::real_device -- --ignored`.
#[test]
#[ignore = "needs an audio output device"]
fn real_device_starts_and_stops() {
    let settings = homefi_core::dj::DjSettings::default();
    let player = super::DjPlayer::start(&settings, None, |_| {}).expect("start");
    player.control(DjControl::Master { value: 0.5 });
    std::thread::sleep(std::time::Duration::from_millis(300));
    let snap = player.snapshot();
    eprintln!("{:?}", snap.output);
    assert!(snap.running && snap.output.sample_rate > 0);
    assert!((snap.master - 0.5).abs() < 1e-6);
    player.stop();
}
