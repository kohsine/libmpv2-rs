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
