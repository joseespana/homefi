# cpal 0.16.0, patched for HomeFi

A copy of [cpal](https://github.com/rustaudio/cpal) 0.16.0 from crates.io
(Apache-2.0, see `LICENSE`) with two changes, wired in through `[patch.crates-io]` in the
workspace `Cargo.toml`.

## The change

`src/host/wasapi/device.rs`: output streams (and the output configs listed) carry the speaker
positions (`dwChannelMask`) of the device's own mix format when the channel counts match.
Upstream always sends `KSAUDIO_SPEAKER_DIRECTOUT` (no positions), and some virtual surround
drivers garble such a stream: Razer's "7.1 Surround Sound" played HomeFi with heavy
interference while a stream with its mask (`0x63F`) plays clean. Input streams are unchanged.

`src/host/coreaudio/macos/{enumerate,mod}.rs`: the size CoreAudio fills in is passed as a
mutable value (`NonNull::from(&mut data_size)`). Upstream declares it immutable
(`let data_size = 0u32; … NonNull::from(&data_size)`), so an optimised build may assume it stays 0:
the device list or the sample-rate ranges then get a zero-capacity buffer and CoreAudio writes the
real list to address `0x4`, a crash in `HALDeviceList::GetData` or `HALC_ProxyIOContext::GetPropertyData`
seen several times on macOS. The lists are also read at the length CoreAudio reports writing.

## Removing it

cpal 0.18.2 still sends `DIRECTOUT`. Once a release sends the device's mask and passes CoreAudio
writable sizes, delete this folder and the `[patch.crates-io]` entry.
