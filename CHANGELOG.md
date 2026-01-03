# Changelog

## Unreleased

- Exclude `test-data/` folder from publishing to reduce download size

## Version 5.0.3

- Fix build_mpv failing on rust 2024 edition @ckaznable

## Version 5.0.2

- Fix segfault in rendering protocol caused by wrong generic @BKSalman

## Version 5.0.1

- Fix stream protocol overwriting cookie

## Version 5.0.0

- Expose `mpv_render_context_report_swap` @tymmesyde
- [breaking] Removed `mpv_node` support. Use strings and parse as JSON if needed
- [breaking] Moved `EventContext` implementation into `Mpv`.
- [breaking] Changed `Mpv::load_config` to `MpvInitializer::load_config`.
- [breaking] Renamed `get_internal_time` to `get_time_us`
- Expose `mpv_get_time_ns`
- Expose `mpv_create_client`
- [breaking] Removed `ProtocolContext`. Use `Protocol` instead.
- [breaking] Removed `protocol` feature flag.

## Version 4.1.0

- Expose `mpv_render_context_update` @ianhattendorf
- Fixed signed chars causing compilation issues on aarch64 @ianhattendorf

## Version 4.0.0

- [breaking] Removed all command helpers. Use `mpv.command` directly instead.
- [breaking] Removed `MpvNode.value()` and `MpvNodeValue`. Use `MpvNode` directly
- [breaking] Moved `libmpv2::MpvNode` to `libmpv2::mpv_node::MpvNode`
- [breaking] Changed `MpvNode.to_f64()`, `.to_bool()`, ... to `MpvNode.f64()`, .`bool()`, ...
- [breaking] `MpvNode.array()` and `MpvNode.map()` now own `self`
- `MpvNode` now implements `Eq`

## Version 3.0.0

- [breaking] Support libmpv version 2.0 (mpv version 0.35.0). Mpv versions <= 0.34.0 will no longer be supported.
- Add OpenGL rendering

## Version 2.0.1

- Fix `playlist_previous_*` commands using wrong mpv command ([issue](https://github.com/ParadoxSpiral/libmpv-rs/issues/17))
- Use local libmpv-sys as dependency except on crates.io

## Version 2.0.0

- Add method `Mpv::with_initializer` to set options before initialization
- [breaking] Borrow `&mut self` in `wait_event` to disallow using two events where the first points to data freed in the second `wait_event` call
- [breaking] `PropertyData<'_>` is no longer `Clone` or `PartialEq`, `Event<'_>` is no longer `Clone` to avoid cloning/comparing `MpvNode`

## Version 1.1.0

- Add an `MpvNode` that implements `GetData`, i.a. with `MpvNodeArrayIter` and `MpvNodeMapIter` variants that support e.g. properties `audio-parmas` and `playlist`

## Version 1.0.1

- Use debug formatting in impl of `Display` trait for `Error`
