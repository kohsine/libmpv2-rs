macro_rules! mpv_cstr_to_str {
    ($cstr: expr) => {
        std::ffi::CStr::from_ptr($cstr)
            .to_str()
            .map_err(Error::from)
    };
}

mod errors;

/// Event handling
pub mod events;
pub mod protocol;
/// Custom rendering
#[cfg(feature = "render")]
pub mod render;

pub use self::errors::*;
use super::*;

use std::{
    ffi::CString,
    mem::MaybeUninit,
    ops::Deref,
    ptr::{self, NonNull},
};

fn mpv_err<T>(ret: T, err: ctype::c_int) -> Result<T> {
    if err == 0 {
        Ok(ret)
    } else {
        Err(Error::Raw(err))
    }
}

/// This trait describes which types are allowed to be passed to getter mpv APIs.
pub unsafe trait GetData: Sized {
    #[doc(hidden)]
    fn get_from_c_void<T, F: FnMut(*mut ctype::c_void) -> Result<T>>(mut fun: F) -> Result<Self> {
        let mut val = MaybeUninit::uninit();
        let _ = fun(val.as_mut_ptr() as *mut _)?;
        Ok(unsafe { val.assume_init() })
    }
    fn get_format() -> Format;
}

/// This trait describes which types are allowed to be passed to setter mpv APIs.
pub unsafe trait SetData: Sized {
    #[doc(hidden)]
    fn call_as_c_void<T, F: FnMut(*mut ctype::c_void) -> Result<T>>(
        mut self,
        mut fun: F,
    ) -> Result<T> {
        fun(&mut self as *mut Self as _)
    }
    fn get_format() -> Format;
}

unsafe impl GetData for f64 {
    fn get_format() -> Format {
        Format::Double
    }
}

unsafe impl SetData for f64 {
    fn get_format() -> Format {
        Format::Double
    }
}

unsafe impl GetData for i64 {
    fn get_format() -> Format {
        Format::Int64
    }
}

unsafe impl SetData for i64 {
    fn get_format() -> Format {
        Format::Int64
    }
}

unsafe impl GetData for bool {
    fn get_format() -> Format {
        Format::Flag
    }
}

unsafe impl SetData for bool {
    fn call_as_c_void<T, F: FnMut(*mut ctype::c_void) -> Result<T>>(self, mut fun: F) -> Result<T> {
        let mut cpy: i64 = if self { 1 } else { 0 };
        fun(&mut cpy as *mut i64 as *mut _)
    }

    fn get_format() -> Format {
        Format::Flag
    }
}

unsafe impl GetData for String {
    fn get_from_c_void<T, F: FnMut(*mut ctype::c_void) -> Result<T>>(mut fun: F) -> Result<String> {
        let ptr = &mut ptr::null();
        fun(ptr as *mut *const ctype::c_char as _)?;

        let ret = unsafe { mpv_cstr_to_str!(*ptr) }?.to_owned();
        unsafe { libmpv2_sys::mpv_free(*ptr as *mut _) };
        Ok(ret)
    }

    fn get_format() -> Format {
        Format::String
    }
}

unsafe impl SetData for String {
    fn call_as_c_void<T, F: FnMut(*mut ctype::c_void) -> Result<T>>(self, mut fun: F) -> Result<T> {
        let string = CString::new(self)?;
        fun((&mut string.as_ptr()) as *mut *const ctype::c_char as *mut _)
    }

    fn get_format() -> Format {
        Format::String
    }
}

/// Wrapper around an `&str` returned by mpv, that properly deallocates it with mpv's allocator.
#[derive(Debug, Hash, Eq, PartialEq)]
pub struct MpvStr<'a>(&'a str);
impl<'a> Deref for MpvStr<'a> {
    type Target = str;

    fn deref(&self) -> &str {
        self.0
    }
}
impl<'a> Drop for MpvStr<'a> {
    fn drop(&mut self) {
        unsafe { libmpv2_sys::mpv_free(self.0.as_ptr() as *mut u8 as _) };
    }
}

unsafe impl<'a> GetData for MpvStr<'a> {
    fn get_from_c_void<T, F: FnMut(*mut ctype::c_void) -> Result<T>>(
        mut fun: F,
    ) -> Result<MpvStr<'a>> {
        let ptr = &mut ptr::null();
        let _ = fun(ptr as *mut *const ctype::c_char as _)?;

        Ok(MpvStr(unsafe { mpv_cstr_to_str!(*ptr) }?))
    }

    fn get_format() -> Format {
        Format::String
    }
}

unsafe impl<'a> SetData for &'a str {
    fn call_as_c_void<T, F: FnMut(*mut ctype::c_void) -> Result<T>>(self, mut fun: F) -> Result<T> {
        let string = CString::new(self)?;
        fun((&mut string.as_ptr()) as *mut *const ctype::c_char as *mut _)
    }

    fn get_format() -> Format {
        Format::String
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
/// Subset of `mpv_format` used by the public API.
pub enum Format {
    String,
    Flag,
    Int64,
    Double,
    Node,
}

impl Format {
    fn as_mpv_format(&self) -> MpvFormat {
        match *self {
            Format::String => mpv_format::String,
            Format::Flag => mpv_format::Flag,
            Format::Int64 => mpv_format::Int64,
            Format::Double => mpv_format::Double,
            Format::Node => mpv_format::Node,
        }
    }
}

/// Context passed to the `initializer` of `Mpv::with_initialzer`.
pub struct MpvInitializer {
    ctx: *mut libmpv2_sys::mpv_handle,
}

impl MpvInitializer {
    /// Set a property to a given value. Properties are essentially variables which
    /// can be queried or set at runtime. For example, writing to the pause property
    /// will actually pause or unpause playback.
    ///
    /// Will return `Err` If the format `T` doesn't match with the internal format of
    /// the property and it also fails to convert to `T` or if setting properties not
    /// backed by options.
    pub fn set_property<T: SetData>(&self, name: &str, data: T) -> Result<()> {
        let name = CString::new(name)?;
        let format = T::get_format().as_mpv_format() as _;
        data.call_as_c_void(|ptr| {
            mpv_err((), unsafe {
                libmpv2_sys::mpv_set_property(self.ctx, name.as_ptr(), format, ptr)
            })
        })
    }

    /// Set the value of an option
    pub fn set_option<T: SetData>(&self, name: &str, data: T) -> Result<()> {
        let name = CString::new(name)?;
        let format = T::get_format().as_mpv_format() as _;
        data.call_as_c_void(|ptr| {
            mpv_err((), unsafe {
                libmpv2_sys::mpv_set_option(self.ctx, name.as_ptr(), format, ptr)
            })
        })
    }

    /// Load a config file. This loads and parses the file, and sets every entry in
    /// the config file's default section as if MpvInitializer::set_option() is called.
    ///
    /// The filename should be an absolute path. If it isn't, the actual path used
    /// is unspecified.
    ///
    /// Will return `Err` if the file wasn't found or if a fatal error happens when
    /// parsing a config file.
    pub fn load_config(&self, path: &str) -> Result<()> {
        let file = CString::new(path)?;
        mpv_err((), unsafe {
            libmpv2_sys::mpv_load_config_file(self.ctx, file.as_ptr())
        })
    }
}

/// The central mpv context.
pub struct Mpv {
    /// The handle to the mpv core
    pub ctx: NonNull<libmpv2_sys::mpv_handle>,
    wakeup_callback_cleanup: Option<Box<dyn FnOnce()>>,
}

unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

impl Drop for Mpv {
    fn drop(&mut self) {
        if let Some(wakeup_callback_cleanup) = self.wakeup_callback_cleanup.take() {
            wakeup_callback_cleanup();
        }

        unsafe {
            libmpv2_sys::mpv_destroy(self.ctx.as_ptr());
        }
    }
}

impl Mpv {
    /// Create and initialize a new `Mpv` instance with default options. Use
    /// [`with_initializer`](Mpv::with_initializer) instead if you want to
    /// set options.
    ///
    /// Unlike the command line player, this will have initial settings suitable
    /// for embedding in applications. The following settings are different:
    /// - stdin/stdout/stderr and the terminal will never be accessed. This is
    ///   equivalent to setting the --no-terminal option.
    ///   (Technically, this also suppresses C signal handling.)
    /// - No config files will be loaded. This is roughly equivalent to using
    ///   --config=no. You can re-enable this option, which will make libmpv
    ///   load config files during mpv_initialize(). If you do this, you are
    ///   strongly encouraged to set the "config-dir" option too.
    ///   (Otherwise it will load the mpv command line player's config.)
    ///
    /// For example:
    /// ```
    /// # use libmpv2::Mpv;
    /// Mpv::with_initializer(|init| {
    ///     init.set_option("config-dir", "test-data")?;
    ///     init.set_option("config", "mpv.conf")?;
    ///     Ok(())
    /// });
    /// ```
    ///
    /// - Idle mode is enabled, which means the playback core will enter idle
    ///   mode if there are no more files to play on the internal playlist,
    ///   instead of exiting. This is equivalent to the --idle option.
    /// - Disable parts of input handling.
    /// - Most of the different settings can be viewed with the command line
    ///   player by running "mpv --show-profile=libmpv".
    ///
    /// All this assumes that API users want a mpv instance that is strictly
    /// isolated from the command line player's configuration, user settings,
    /// and so on. You can re-enable disabled features by setting the
    /// appropriate options.
    ///
    /// The mpv command line parser is not available through this API, but you
    /// can set individual options with
    /// [`set_property`](Mpv::set_property). Files for playback must be
    /// loaded with [`command`](Mpv::command) or others.
    ///
    /// Will return `Err` if out of memory or LC_NUMERIC is not set to "C".
    pub fn new() -> Result<Mpv> {
        Mpv::with_initializer(|_| Ok(()))
    }

    /// Create and initialize a new `Mpv` instance with options set by the
    /// `initializer`.
    ///
    /// The following options can't be set after initialization:
    /// 1. options which are only read at initialization time:
    ///     - config
    ///     - config-dir
    ///     - input-conf
    ///     - load-scripts
    ///     - script
    ///     - player-operation-mode
    ///     - input-app-events (OSX)
    /// 2. all encoding mode options
    ///
    /// Will return `Err` if out of memory or LC_NUMERIC is not set to "C" or
    /// `initializer` fails.
    pub fn with_initializer<F: FnOnce(MpvInitializer) -> Result<()>>(
        initializer: F,
    ) -> Result<Mpv> {
        let api_version = unsafe { libmpv2_sys::mpv_client_api_version() };
        if crate::MPV_CLIENT_API_MAJOR != api_version >> 16 {
            return Err(Error::VersionMismatch {
                linked: crate::MPV_CLIENT_API_VERSION,
                loaded: api_version,
            });
        }

        let ctx = unsafe { libmpv2_sys::mpv_create() };
        if ctx.is_null() {
            return Err(Error::Null);
        }

        initializer(MpvInitializer { ctx })?;
        mpv_err((), unsafe { libmpv2_sys::mpv_initialize(ctx) }).map_err(|err| {
            unsafe { libmpv2_sys::mpv_terminate_destroy(ctx) };
            err
        })?;

        let ctx = unsafe { NonNull::new_unchecked(ctx) };

        Ok(Mpv {
            ctx,
            wakeup_callback_cleanup: None,
        })
    }

    /// Create a new client handle connected to the same player core as `self`. This
    /// context has its own event queue, its own [`enable_event`](Mpv::enable_event)
    /// and [`disable_event`](Mpv::disable_event) states, its own
    /// mpv_request_log_messages() state (unimplemented), its own set of observed
    /// properties, and its own state for asynchronous operations. Otherwise,
    /// everything is shared.
    ///
    /// The core will live as long as there is at least 1 handle referencing
    /// it.
    ///
    /// # Arguments
    ///
    /// * `name` - The client name. This will be returned by mpv_client_name()
    ///            (unimplemented). If the name is already in use, or contains
    ///            non-alphanumeric characters (other than '_'), the name is
    ///            modified to fit. If `None`, an arbitrary name is automatically
    ///            chosen.
    pub fn create_client(&self, name: Option<&str>) -> Result<Mpv> {
        let mpv_handle = unsafe {
            libmpv2_sys::mpv_create_client(
                self.ctx.as_ptr(),
                if let Some(name) = name {
                    CString::new(name)?.as_ptr()
                } else {
                    ptr::null()
                },
            )
        };

        let ctx = unsafe { NonNull::new_unchecked(mpv_handle) };

        Ok(Mpv {
            ctx,
            wakeup_callback_cleanup: None,
        })
    }

    /// Send a command to the player. Commands are the same as those used in
    /// input.conf.
    pub fn command(&self, name: &str, args: &[&str]) -> Result<()> {
        let mut cstr_args: Vec<CString> = Vec::with_capacity(args.len() + 1);
        cstr_args.push(CString::new(name)?);

        for arg in args {
            cstr_args.push(CString::new(*arg)?);
        }

        let mut ptrs: Vec<_> = cstr_args.iter().map(|cstr| cstr.as_ptr()).collect();
        ptrs.push(std::ptr::null());

        mpv_err((), unsafe {
            libmpv2_sys::mpv_command(self.ctx.as_ptr(), ptrs.as_mut_ptr())
        })
    }

    /// Same as [`Mpv::command`], but run the command asynchronously.
    /// 
    /// Commands are executed asynchronously. You will receive a
    /// `MPV_EVENT_COMMAND_REPLY` event.
    /// 
    /// The only case when you do not receive an event is when the function call
    /// itself fails. This happens only if parsing the command itself (or otherwise
    /// validating it) fails, i.e. the return code of the API call is not 0 or
    /// positive.
    pub fn command_async(&self, name: &str, userdata: u64, args: &[&str]) -> Result<()> {
        let mut cstr_args: Vec<CString> = Vec::with_capacity(args.len() + 1);
        cstr_args.push(CString::new(name)?);

        for arg in args {
            cstr_args.push(CString::new(*arg)?);
        }

        let mut ptrs: Vec<_> = cstr_args.iter().map(|cstr| cstr.as_ptr()).collect();
        ptrs.push(std::ptr::null());

        mpv_err((), unsafe {
            libmpv2_sys::mpv_command_async(self.ctx.as_ptr(), userdata, ptrs.as_mut_ptr())
        })
    }

    /// Set a property to a given value. Properties are essentially variables which
    /// can be queried or set at runtime. For example, writing to the pause property
    /// will actually pause or unpause playback.
    ///
    /// Will return `Err` If the format `T` doesn't match with the internal format of
    /// the property and it also fails to convert to `T`.
    pub fn set_property<T: SetData>(&self, name: &str, data: T) -> Result<()> {
        let name = CString::new(name)?;
        let format = T::get_format().as_mpv_format() as _;
        data.call_as_c_void(|ptr| {
            mpv_err((), unsafe {
                libmpv2_sys::mpv_set_property(self.ctx.as_ptr(), name.as_ptr(), format, ptr)
            })
        })
    }

    /// Read the value of the given property. Tries to convert the value to the given
    /// format `T` if it does not match.
    ///
    /// Will return `Err` If the format `T` doesn't match with the internal format of
    /// the property and it also fails to convert to `T`.
    pub fn get_property<T: GetData>(&self, name: &str) -> Result<T> {
        let name = CString::new(name)?;

        let format = T::get_format().as_mpv_format() as _;
        T::get_from_c_void(|ptr| {
            mpv_err((), unsafe {
                libmpv2_sys::mpv_get_property(self.ctx.as_ptr(), name.as_ptr(), format, ptr)
            })
        })
    }

    /// Return the internal time in nanoseconds. This has an arbitrary start
    /// offset, but will never wrap or go backwards.
    ///
    /// Note that this is always the real time, and doesn't necessarily have to
    /// do with playback time. For example, playback could go faster or slower
    /// due to playback speed, or due to playback being paused. Use the
    /// "time-pos" property instead to get the playback status.
    ///
    /// Unlike other libmpv APIs, this can be called at absolutely any time
    /// (even within wakeup callbacks), as long as the context is valid.
    ///
    /// Safe to be called from mpv render API threads.
    pub fn get_time_ns(&self) -> i64 {
        unsafe { libmpv2_sys::mpv_get_time_ns(self.ctx.as_ptr()) }
    }

    /// Same as get_time_ns but in microseconds.
    pub fn get_time_us(&self) -> i64 {
        unsafe { libmpv2_sys::mpv_get_time_us(self.ctx.as_ptr()) }
    }
}
