use crate::events::{Event, PropertyData};
use crate::*;

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
}
