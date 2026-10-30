use crate::events::{Event, PropertyData};
use crate::*;

use std::ffi::CStr;
use std::thread;
use std::time::Duration;

#[test]
fn initializer() {
    let mpv = Mpv::with_initializer(|init| {
        init.set_property("osc", true)?;
        init.set_property("input-default-bindings", true)?;
        init.set_property("volume", 30)?;

        Ok(())
    })
    .unwrap();

    assert_eq!(true, mpv.get_property("osc").unwrap());
    assert_eq!(true, mpv.get_property("input-default-bindings").unwrap());
    assert_eq!(30i64, mpv.get_property("volume").unwrap());
}

#[test]
fn client_with_name() {
    let mpv = Mpv::new().unwrap();
    let client = mpv.create_client(Some("name")).unwrap();
    let name = unsafe { libmpv2_sys::mpv_client_name(client.ctx.as_ptr()) };
    let cstr = unsafe { CStr::from_ptr(name) };
    assert_eq!(cstr.to_str().unwrap(), "name");
}

#[test]
fn client_with_duplicate_name() {
    let mpv = Mpv::new().unwrap();
    let client1 = mpv.create_client(Some("name")).unwrap();
    let client2 = mpv.create_client(Some("name")).unwrap();
    let name1 = unsafe { libmpv2_sys::mpv_client_name(client1.ctx.as_ptr()) };
    let name2 = unsafe { libmpv2_sys::mpv_client_name(client2.ctx.as_ptr()) };
    let cstr1 = unsafe { CStr::from_ptr(name1) };
    let cstr2 = unsafe { CStr::from_ptr(name2) };
    assert_eq!(cstr1.to_str().unwrap(), "name");
    assert_eq!(cstr2.to_str().unwrap(), "name2");
}

#[test]
fn client_without_name() {
    let mpv = Mpv::new().unwrap();
    let client1 = mpv.create_client(None).unwrap();
    let client2 = mpv.create_client(None).unwrap();
    let client3 = mpv.create_client(None).unwrap();
    let client4 = mpv.create_client(None).unwrap();
    let client5 = mpv.create_client(None).unwrap();
    let name1 = unsafe { libmpv2_sys::mpv_client_name(client1.ctx.as_ptr()) };
    let name2 = unsafe { libmpv2_sys::mpv_client_name(client2.ctx.as_ptr()) };
    let name3 = unsafe { libmpv2_sys::mpv_client_name(client3.ctx.as_ptr()) };
    let name4 = unsafe { libmpv2_sys::mpv_client_name(client4.ctx.as_ptr()) };
    let name5 = unsafe { libmpv2_sys::mpv_client_name(client5.ctx.as_ptr()) };
    let cstr1 = unsafe { CStr::from_ptr(name1) };
    let cstr2 = unsafe { CStr::from_ptr(name2) };
    let cstr3 = unsafe { CStr::from_ptr(name3) };
    let cstr4 = unsafe { CStr::from_ptr(name4) };
    let cstr5 = unsafe { CStr::from_ptr(name5) };
    assert_eq!(cstr1.to_str().unwrap(), "client");
    assert_eq!(cstr2.to_str().unwrap(), "client2");
    assert_eq!(cstr3.to_str().unwrap(), "client3");
    assert_eq!(cstr4.to_str().unwrap(), "client4");
    assert_eq!(cstr5.to_str().unwrap(), "client5");
}

#[test]
fn properties() {
    let mpv = Mpv::new().unwrap();
    mpv.set_property("volume", 0).unwrap();
    mpv.set_property("vo", "null").unwrap();
    mpv.set_property("ytdl-format", "best[width<240]").unwrap();
    mpv.set_property("sub-gauss", 0.6).unwrap();

    assert_eq!(0i64, mpv.get_property("volume").unwrap());
    let vo: MpvStr = mpv.get_property("vo").unwrap();
    assert_eq!("null", &*vo);
    assert_eq!(true, mpv.get_property("ytdl").unwrap());
    let subg: f64 = mpv.get_property("sub-gauss").unwrap();
    assert_eq!(
        0.6,
        f64::round(subg * f64::powi(10.0, 4)) / f64::powi(10.0, 4)
    );
    mpv.command(
        "loadfile",
        &["test-data/speech_12kbps_mb.wav", "append-play"],
    )
    .unwrap();
    thread::sleep(Duration::from_millis(250));

    let title: MpvStr = mpv.get_property("media-title").unwrap();
    assert_eq!(&*title, "speech_12kbps_mb.wav");
}

macro_rules! assert_event_occurs {
    ($ctx:ident, $timeout:literal, $( $expected:pat),+) => {
        loop {
            match $ctx.wait_event($timeout) {
                $( Some($expected) )|+ => {
                    break;
                },
                None => {
                    continue
                },
                other => panic!("Event did not occur, got: {:?}", other),
            }
        }
    }
}

// Asserts that events from mpv are in the subset of given events
macro_rules! assert_events_subset {
    ($ctx:ident, $timeout:literal, $( $expected:pat),+) => {
        loop {
            match $ctx.wait_event($timeout) {
                $( Some($expected) )|+ => {
                    continue;
                },
                None => {
                    break
                },
                other => panic!("Event did not occur, got: {:?}", other),
            }
        }
    }
}

#[test]
fn events() {
    let mpv = Mpv::new().unwrap();
    mpv.disable_deprecated_events().unwrap();

    mpv.observe_property("volume", Format::Int64, 0).unwrap();
    mpv.observe_property("media-title", Format::String, 1)
        .unwrap();

    mpv.set_property("vo", "null").unwrap();

    // speed up playback so test finishes faster
    mpv.set_property("speed", 100).unwrap();

    assert_event_occurs!(
        mpv,
        3.,
        Ok(Event::PropertyChange {
            name: "volume",
            change: PropertyData::Int64(100),
            reply_userdata: 0,
        })
    );

    mpv.set_property("volume", 0).unwrap();
    assert_event_occurs!(
        mpv,
        10.,
        Ok(Event::PropertyChange {
            name: "volume",
            change: PropertyData::Int64(0),
            reply_userdata: 0,
        })
    );
    assert!(mpv.wait_event(3.).is_none());
    mpv.command("loadfile", &["test-data/jellyfish.mp4", "append-play"])
        .unwrap();

    assert_events_subset!(
        mpv,
        3.,
        Ok(Event::StartFile),
        Ok(Event::PropertyChange {
            name: "media-title",
            change: PropertyData::Str("jellyfish.mp4"),
            reply_userdata: 1,
        }),
        Ok(Event::AudioReconfig),
        Ok(Event::FileLoaded),
        Ok(Event::VideoReconfig),
        Ok(Event::PlaybackRestart),
        Ok(Event::EndFile(mpv_end_file_reason::Eof))
    );

    mpv.command("loadfile", &["test-data/speech_12kbps_mb.wav", "replace"])
        .unwrap();

    assert_events_subset!(
        mpv,
        3.,
        Ok(Event::StartFile),
        Ok(Event::PropertyChange {
            name: "media-title",
            change: PropertyData::Str("speech_12kbps_mb.wav"),
            reply_userdata: 1,
        }),
        Ok(Event::AudioReconfig),
        Ok(Event::FileLoaded),
        Ok(Event::VideoReconfig),
        Ok(Event::PlaybackRestart),
        Ok(Event::EndFile(mpv_end_file_reason::Stop)),
        Ok(Event::EndFile(mpv_end_file_reason::Eof))
    );
}

#[test]
fn config_file() {
    let mpv = Mpv::with_initializer(|init| {
        init.load_config("test-data/mpv.conf").unwrap();

        Ok(())
    })
    .unwrap();
    assert_eq!(mpv.get_property::<i64>("volume").unwrap(), 50);
}

#[cfg(feature = "render")]
mod render_context {
    use super::*;
    use crate::render::{OpenGLInitParams, RenderParam, RenderParamApiType};
    use std::ffi::c_void;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    struct InitParamsCtx {
        video: sdl2::VideoSubsystem,
        _flag: DropFlag,
    }

    fn get_proc_address(ctx: &InitParamsCtx, name: &str) -> *mut c_void {
        ctx.video.gl_get_proc_address(name) as *mut c_void
    }

    fn create_hidden_gl_context() -> (
        sdl2::video::Window,
        sdl2::VideoSubsystem,
        sdl2::video::GLContext,
    ) {
        let sdl = sdl2::init().unwrap();
        let video = sdl.video().unwrap();
        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(3, 3);
        gl_attr.set_context_flags().forward_compatible().set();
        let window = video
            .window("libmpv2 render context test", 64, 64)
            .opengl()
            .hidden()
            .build()
            .unwrap();
        let gl_context = window.gl_create_context().unwrap();
        (window, video, gl_context)
    }

    // Regression test for a use-after-free where `OpenGLInitParams` (and the
    // `get_proc_address` loader it carries) was freed synchronously right after
    // `mpv_render_context_create` returned, even though render_gl.h documents that mpv may
    // call `get_proc_address` lazily at any point during the render context's lifetime (e.g.
    // to resolve EGL extensions for hwdec interop).
    #[test]
    fn init_params_are_not_freed_before_render_context_is_dropped() {
        let (_window, video, _gl_context) = create_hidden_gl_context();

        let mpv = Mpv::with_initializer(|init| {
            init.set_property("vo", "libmpv")?;
            Ok(())
        })
        .unwrap();

        let dropped = Arc::new(AtomicBool::new(false));

        let render_context = mpv
            .create_render_context(vec![
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams {
                    get_proc_address,
                    ctx: InitParamsCtx {
                        video,
                        _flag: DropFlag(dropped.clone()),
                    },
                }),
            ])
            .expect("failed to create render context");

        assert!(
            !dropped.load(Ordering::SeqCst),
            "init params were freed before the render context was dropped"
        );

        drop(render_context);

        assert!(
            dropped.load(Ordering::SeqCst),
            "init params were never freed after the render context was dropped"
        );
    }

    // Regression test for a use-after-free race in `RenderContext::drop`'s cleanup order.
    //
    // mpv's internal render thread can invoke the update callback registered via
    // `set_update_callback` concurrently with `Drop`, right up until
    // `mpv_render_context_free()` returns. If the Rust-side closure backing that callback is
    // freed *before* `mpv_render_context_free()` runs, the render thread can read freed memory
    // through `ru_wrapper` -> the closure. This is a small race window (in a downstream user of
    // this crate it took two independent `coredumpctl` crash dumps with matching stack traces
    // to pin down), so a plain `cargo test` run won't reliably catch a regression here -- this
    // stress test is only deterministic under AddressSanitizer, which poisons freed memory
    // immediately instead of leaving it to accidentally still look valid.
    //
    // Run with:
    //   rustup component add rust-src --toolchain nightly
    //   RUSTFLAGS="-Z sanitizer=address" \
    //     cargo +nightly test -Z build-std --release --target x86_64-unknown-linux-gnu \
    //     render_context_free_races_update_callback -- --ignored --nocapture
    #[test]
    #[ignore = "only meaningful under ASAN, see comment above for the invocation"]
    fn render_context_free_races_update_callback_under_asan() {
        use std::sync::atomic::AtomicUsize;
        use std::time::{Duration, Instant};

        let deadline = Instant::now() + Duration::from_secs(15);
        let mut iterations = 0u32;
        let mut iterations_with_activity = 0u32;
        let total_fires = Arc::new(AtomicUsize::new(0));

        while Instant::now() < deadline {
            let (_window, video, _gl_context) = create_hidden_gl_context();

            let mpv = Mpv::with_initializer(|init| {
                init.set_property("vo", "libmpv")?;
                Ok(())
            })
            .unwrap();

            let mut render_context = mpv
                .create_render_context(vec![
                    RenderParam::ApiType(RenderParamApiType::OpenGl),
                    RenderParam::InitParams(OpenGLInitParams {
                        get_proc_address,
                        ctx: InitParamsCtx {
                            video,
                            _flag: DropFlag(Arc::new(AtomicBool::new(false))),
                        },
                    }),
                ])
                .expect("failed to create render context");

            // Heap-allocated state the callback touches on every invocation, so ASAN has
            // something to flag as heap-use-after-free if the render thread calls back into
            // this closure after Drop has already freed it.
            let fired = Arc::new(AtomicUsize::new(0));
            let fired_in_callback = fired.clone();
            let total_fires_in_callback = total_fires.clone();
            let padding = Box::new([0u8; 256]);
            render_context.set_update_callback(move || {
                let _ = padding[0];
                fired_in_callback.fetch_add(1, Ordering::SeqCst);
                total_fires_in_callback.fetch_add(1, Ordering::SeqCst);
            });

            mpv.command("loadfile", &["test-data/jellyfish.mp4", "replace"])
                .unwrap();

            // set_update_callback() raises one callback synchronously, on this thread, as part
            // of registering it -- that doesn't race anything. Wait for a *second* firing,
            // which can only come from mpv's internal decode/render thread delivering an
            // asynchronous redraw notification, then drop immediately: that's the actual race
            // window the fix closes.
            let wait_deadline = Instant::now() + Duration::from_millis(200);
            while fired.load(Ordering::SeqCst) < 2 && Instant::now() < wait_deadline {
                std::thread::yield_now();
            }
            if fired.load(Ordering::SeqCst) >= 2 {
                iterations_with_activity += 1;
            }

            drop(render_context);

            iterations += 1;
        }

        eprintln!(
            "completed {iterations} create/drop iterations ({iterations_with_activity} with \
             observed callback activity, {} total callback fires) without an ASAN report",
            total_fires.load(Ordering::SeqCst)
        );
    }
}
