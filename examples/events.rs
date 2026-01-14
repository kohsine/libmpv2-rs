use libmpv2::{events::*, *};
use serde_json::Value;
use std::{env, thread, time::Duration};

const VIDEO_URL: &str = "https://www.youtube.com/watch?v=VLnWf1sQkjY";

fn main() -> Result<()> {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| String::from(VIDEO_URL));

    // Create an `Mpv` and set some properties.
    let mpv = Mpv::with_initializer(|init| {
        init.set_property("vo", "null")?;
        Ok(())
    })
    .unwrap();
    mpv.set_property("volume", 15)?;

    let mut mpv_client = mpv.create_client(None)?;

    mpv_client.disable_deprecated_events()?;
    mpv_client.observe_property("volume", Format::Int64, 0)?;
    mpv_client.observe_property("demuxer-cache-state", Format::String, 0)?;

    crossbeam::scope(|scope| {
        scope.spawn(|_| {
            mpv.command("loadfile", &[&path, "append-play"]).unwrap();

            thread::sleep(Duration::from_secs(3));

            mpv.set_property("volume", 25).unwrap();

            thread::sleep(Duration::from_secs(5));

            // Trigger `Event::EndFile`.
            mpv.command("playlist-next", &["force"]).unwrap();
        });
        scope.spawn(|_| {
            loop {
                let ev = mpv_client.wait_event(600.).unwrap_or(Err(Error::Null));

                match ev {
                    Ok(Event::EndFile(r)) => {
                        println!("Exiting! Reason: {:?}", r);
                        break;
                    }

                    Ok(Event::PropertyChange {
                        name: "demuxer-cache-state",
                        change: PropertyData::Str(r),
                        ..
                    }) => {
                        let ranges = seekable_ranges(r);
                        println!("Seekable ranges updated: {:?}", ranges);
                    }
                    Ok(e) => println!("Event triggered: {:?}", e),
                    Err(e) => println!("Event errored: {:?}", e),
                }
            }
        });
    })
    .unwrap();
    Ok(())
}

fn seekable_ranges(demuxer_cache_state: &str) -> Vec<(f64, f64)> {
    let mut res = Vec::new();

    let v: Value = serde_json::from_str(demuxer_cache_state).unwrap();
    let ranges = v["seekable-ranges"].as_array().unwrap();

    for val in ranges {
        let range = val.as_object().unwrap();
        let start = range.get("start").unwrap().as_f64().unwrap();
        let end = range.get("end").unwrap().as_f64().unwrap();
        res.push((start, end));
    }
    res
}
