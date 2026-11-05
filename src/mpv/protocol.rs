use super::*;
use std::alloc::{self, Layout};
use std::ffi::c_char;
use std::mem;
use std::os::raw as ctype;
use std::panic;
use std::panic::RefUnwindSafe;
use std::slice;

/// Return a persistent `T` that is passed to all other `Stream*` functions, panic on errors.
pub type StreamOpen<T, U> = fn(&mut U, &str) -> T;
/// Do any necessary cleanup.
pub type StreamClose<T> = fn(Box<T>);
/// Seek to the given offset. Return the new offset, or either `MpvError::Generic` if seeking
/// failed or panic.
pub type StreamSeek<T> = fn(&mut T, i64) -> i64;
/// Target buffer with fixed capacity.
/// Return either the number of read bytes, `0` on EOF, or either `-1` or panic on error.
pub type StreamRead<T> = fn(&mut T, &mut [c_char]) -> i64;
/// Return the total size of the stream in bytes. Panic on error.
pub type StreamSize<T> = fn(&mut T) -> i64;

unsafe extern "C" fn open_wrapper<T, U>(
    user_data: *mut ctype::c_void,
    uri: *mut ctype::c_char,
    stream_cb_info: *mut libmpv2_sys::mpv_stream_cb_info,
) -> ctype::c_int
where
    T: RefUnwindSafe,
    U: RefUnwindSafe,
{
    // allocate memory for cookie
    let c_layout = Layout::from_size_align(mem::size_of::<T>(), mem::align_of::<T>()).unwrap();
    let new_cookie = unsafe { alloc::alloc(c_layout) as *mut T };

    let protocol_data = unsafe { &*(user_data as *mut InitProtocolData<T, U>) };

    // Make a clone of the protocol data
    let protocol_data_copy = ProtocolData {
        cookie: new_cookie,
        open_fn: protocol_data.open_fn,
        close_fn: protocol_data.close_fn,
        read_fn: protocol_data.read_fn,
        seek_fn: protocol_data.seek_fn,
        size_fn: protocol_data.size_fn,
    };

    let protocol_data_raw = Box::into_raw(Box::new(protocol_data_copy));

    unsafe {
        (*stream_cb_info).cookie = protocol_data_raw as *mut _;
        (*stream_cb_info).read_fn = Some(read_wrapper::<T, U>);
        (*stream_cb_info).seek_fn = Some(seek_wrapper::<T, U>);
        (*stream_cb_info).size_fn = Some(size_wrapper::<T, U>);
        (*stream_cb_info).close_fn = Some(close_wrapper::<T, U>);
    }

    let ret = panic::catch_unwind(|| unsafe {
        let uri = mpv_cstr_to_str!(uri as *const _).unwrap();

        let protocol_data = &mut *(user_data as *mut InitProtocolData<T, U>);

        // Call the users open fn and write the data to the new cookie
        ptr::write(
            (*protocol_data_raw).cookie,
            ((*protocol_data_raw).open_fn)(&mut (*protocol_data).user_data, uri),
        );
    });

    if ret.is_ok() {
        0
    } else {
        mpv_error::Generic as _
    }
}

unsafe extern "C" fn read_wrapper<T, U>(
    wrapper_cookie: *mut ctype::c_void,
    buf: *mut c_char,
    nbytes: u64,
) -> i64
where
    T: RefUnwindSafe,
    U: RefUnwindSafe,
{
    let data = wrapper_cookie as *mut ProtocolData<T, U>;

    let ret = panic::catch_unwind(|| unsafe {
        let slice = slice::from_raw_parts_mut(buf, nbytes as _);
        ((*data).read_fn)(&mut *(*data).cookie, slice)
    });
    if let Ok(ret) = ret { ret } else { -1 }
}

unsafe extern "C" fn seek_wrapper<T, U>(wrapper_cookie: *mut ctype::c_void, offset: i64) -> i64
where
    T: RefUnwindSafe,
    U: RefUnwindSafe,
{
    let data = wrapper_cookie as *mut ProtocolData<T, U>;

    if unsafe { (*data).seek_fn.is_none() } {
        return mpv_error::Unsupported as _;
    }

    let ret = panic::catch_unwind(|| unsafe {
        (*(*data).seek_fn.as_ref().unwrap())(&mut *(*data).cookie, offset)
    });
    if let Ok(ret) = ret {
        ret
    } else {
        mpv_error::Generic as _
    }
}

unsafe extern "C" fn size_wrapper<T, U>(wrapper_cookie: *mut ctype::c_void) -> i64
where
    T: RefUnwindSafe,
    U: RefUnwindSafe,
{
    let data = wrapper_cookie as *mut ProtocolData<T, U>;

    if unsafe { (*data).size_fn.is_none() } {
        return mpv_error::Unsupported as _;
    }

    let ret = panic::catch_unwind(|| unsafe {
        (*(*data).size_fn.as_ref().unwrap())(&mut *(*data).cookie)
    });
    if let Ok(ret) = ret {
        ret
    } else {
        mpv_error::Unsupported as _
    }
}

#[allow(unused_must_use)]
extern "C" fn close_wrapper<T, U>(wrapper_cookie: *mut ctype::c_void)
where
    T: RefUnwindSafe,
    U: RefUnwindSafe,
{
    // Free wrapper_cookie memory
    let data = unsafe { Box::from_raw(wrapper_cookie as *mut ProtocolData<T, U>) };

    // Free cookie memory
    panic::catch_unwind(|| unsafe { ((*data).close_fn)(Box::from_raw((*data).cookie)) });
}

struct InitProtocolData<T, U> {
    user_data: U,

    open_fn: StreamOpen<T, U>,
    close_fn: StreamClose<T>,
    read_fn: StreamRead<T>,
    seek_fn: Option<StreamSeek<T>>,
    size_fn: Option<StreamSize<T>>,
}

struct ProtocolData<T, U> {
    cookie: *mut T,

    open_fn: StreamOpen<T, U>,
    close_fn: StreamClose<T>,
    read_fn: StreamRead<T>,
    seek_fn: Option<StreamSeek<T>>,
    size_fn: Option<StreamSize<T>>,
}

/// `Protocol` holds all state used by a custom protocol.
pub struct Protocol<'parent, T: Sized + RefUnwindSafe, U: RefUnwindSafe> {
    mpv: &'parent Mpv,
    name: String,
    data: *mut InitProtocolData<T, U>,
}

unsafe impl<'parent, T: RefUnwindSafe, U: RefUnwindSafe> Send for Protocol<'parent, T, U> {}
unsafe impl<'parent, T: RefUnwindSafe, U: RefUnwindSafe> Sync for Protocol<'parent, T, U> {}

impl<'parent, T: RefUnwindSafe, U: RefUnwindSafe> Drop for Protocol<'parent, T, U> {
    fn drop(&mut self) {
        let _ = unsafe { Box::from_raw(self.data) };
    }
}

impl<'parent, T: RefUnwindSafe, U: RefUnwindSafe> Protocol<'parent, T, U> {
    /// `name` is the prefix of the protocol, e.g. `name://path`.
    ///
    /// `user_data` is data that will be passed to `open_fn`.
    ///
    /// # Safety
    /// Do not call libmpv functions in any supplied function.
    /// All panics of the provided functions are catched and can be used as generic error returns.
    pub unsafe fn new(
        mpv: &'parent Mpv,
        name: String,
        user_data: U,
        open_fn: StreamOpen<T, U>,
        close_fn: StreamClose<T>,
        read_fn: StreamRead<T>,
        seek_fn: Option<StreamSeek<T>>,
        size_fn: Option<StreamSize<T>>,
    ) -> Protocol<'parent, T, U> {
        let data = Box::into_raw(Box::new(InitProtocolData {
            user_data,

            open_fn,
            close_fn,
            read_fn,
            seek_fn,
            size_fn,
        }));

        Protocol { mpv, name, data }
    }

    /// This will register the `Protocol`, and invoke the given callbacks if an
    /// URI with the matching protocol prefix is opened.
    ///
    /// Will return `Err` if a `Protocol` with the same name is already
    /// registered
    pub fn register(&self) -> Result<()> {
        let name = CString::new(&self.name[..])?;
        unsafe {
            mpv_err(
                (),
                libmpv2_sys::mpv_stream_cb_add_ro(
                    self.mpv.ctx.as_ptr(),
                    name.as_ptr(),
                    self.data as *mut _,
                    Some(open_wrapper::<T, U>),
                ),
            )
        }
    }
}
