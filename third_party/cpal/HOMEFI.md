# cpal 0.16.0, patched for HomeFi

A copy of [cpal](https://github.com/rustaudio/cpal) 0.16.0 from crates.io
(Apache-2.0, see `LICENSE`) with one change, wired in through `[patch.crates-io]` in the
workspace `Cargo.toml`.

## The change

`src/host/wasapi/device.rs`: output streams (and the output configs listed) carry the speaker
positions (`dwChannelMask`) of the device's own mix format when the channel counts match.
Upstream always sends `KSAUDIO_SPEAKER_DIRECTOUT` (no positions), and some virtual surround
drivers garble such a stream: Razer's "7.1 Surround Sound" played HomeFi with heavy
interference while a stream with its mask (`0x63F`) plays clean. Input streams are unchanged.

## Removing it

cpal 0.18.2 still sends `DIRECTOUT`. Once a release sends the device's mask, delete this folder
and the `[patch.crates-io]` entry.
