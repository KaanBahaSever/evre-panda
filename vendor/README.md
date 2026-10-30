# Vendored crates

Two upstream crates from [livekit/rust-sdks](https://github.com/livekit/rust-sdks)
(Apache-2.0, see `../LICENSE-APACHE`), exactly as published on crates.io
except for the changes listed here. Cargo uses them through
`[patch.crates-io]` in the workspace's `Cargo.toml`. Each change is marked
with an `evre-panda:` comment in the code.

| Crate | Version | Changed | Why |
|---|---|---|---|
| `livekit` | 0.9.6 | `src/room/options.rs`, `src/room/participant/local_participant.rs`: `TrackPublishOptions::stereo` (default `false`) sends the `TF_STEREO` audio feature when a track is published | LiveKit keeps `stereo=1` in its answer only for audio tracks announced as stereo; without it a two-channel music track may come out mono. The bot SDK publishes Panda's track with `stereo: true` |
| `webrtc-sys` | 0.3.50 | `build.rs`: accepts `zig c++` as the clang (cargo-zigbuild), keeps the hermetic libc++ in front with `-I` under zig, compiles the lazy-load `.c` stubs under a C++ name under zig, and probes the compiler with an empty file instead of `/dev/null` | Cross-compiling the Linux binary from Windows (`scripts/release.py --linux`). Native builds behave as upstream |

The other LiveKit crates are used unchanged from crates.io; `Cargo.lock`
holds them at the versions these two belong to (libwebrtc 0.3.53).

To move to a newer LiveKit, the changes have to be redone on the new
versions (the Evre monorepo's `tools/panda-oss/export.py` holds them as
exact replacements and refuses to export when one no longer applies).
