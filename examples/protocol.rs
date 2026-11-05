use std::{
    ffi::c_char,
    fs::File,
    io::{Read, Seek, SeekFrom},
    mem, thread,
    time::Duration,
};

use libmpv2::{Mpv, protocol::Protocol};

fn main() {
    let mpv = Mpv::new().unwrap();
    mpv.set_property("volume", 25).unwrap();

    let protocol = unsafe {
        Protocol::new(
            &mpv,
            "filereader".into(),
            (),
            open,
            close,
            read,
            Some(seek),
            Some(size),
        )
    };

    protocol.register().unwrap();

    mpv.command(
        "loadfile",
        &[
            &format!("filereader://{}", "test-data/jellyfish.mp4"),
            "append-play",
        ],
    )
    .unwrap();

    mpv.command(
        "loadfile",
        &[
            &format!("filereader://{}", "test-data/speech_12kbps_mb.wav"),
            "append",
        ],
    )
    .unwrap();

    mpv.command(
        "loadfile",
        &[
            &format!("filereader://{}", "test-data/jellyfish.mp4"),
            "append",
        ],
    )
    .unwrap();

    thread::sleep(Duration::from_secs(10));

    mpv.command("seek", &["15"]).unwrap();

    thread::sleep(Duration::from_secs(10));

    mpv.command("seek", &["15"]).unwrap();

    thread::sleep(Duration::from_secs(5));
}

fn open(_: &mut (), uri: &str) -> File {
    // Open the file, and strip the `filereader://` part
    let ret = File::open(&uri[13..]).unwrap();

    println!("Opened file[{}], ready for orders o7", &uri[13..]);
    ret
}

fn close(_: Box<File>) {
    println!("Closing file, bye bye~~");
}

fn read(cookie: &mut File, buf: &mut [c_char]) -> i64 {
    unsafe {
        let forbidden_magic = mem::transmute::<&mut [c_char], &mut [u8]>(buf);

        cookie.read(forbidden_magic).unwrap() as _
    }
}

fn seek(cookie: &mut File, offset: i64) -> i64 {
    println!("Seeking to byte {}", offset);
    cookie.seek(SeekFrom::Start(offset as u64)).unwrap() as _
}

fn size(cookie: &mut File) -> i64 {
    cookie.metadata().unwrap().len() as _
}
