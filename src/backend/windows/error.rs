use {
    raii::Guard,
    std::{
        error::Error as StdError,
        fmt::{self, Display, Formatter},
        num::{NonZeroI8, NonZeroUsize},
        ptr::{NonNull, null_mut},
    },
    widestring::ucstr::U16CStr,
    winapi::{
        ctypes::c_void,
        shared::minwindef::DWORD,
        um::{
            errhandlingapi::GetLastError,
            winbase::{
                FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM,
                FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW, LocalFree,
            },
            winnt::{LANG_NEUTRAL, LPWSTR, MAKELANGID, SUBLANG_DEFAULT, WCHAR},
        },
    },
};

#[derive(Debug)]
#[repr(transparent)]
/// Contains the error code similar to errno.
///
/// Error code 0 is for when operations return successfully, however it relies on the callee using
/// `SetLastError` which is unreliable, so this cannot be represented with a NonZero<DWORD>.
pub struct WindowsError(DWORD);

impl WindowsError {
    #[must_use]
    /// Returns `Some(Self)` if `GetLastError` returns a non zero value.
    pub fn new() -> Option<Self> {
        match unsafe { GetLastError() } {
            0 => None,
            err => Some(Self(err)),
        }
    }

    #[must_use]
    /// [Self::new] without checking for zero.
    pub fn new_unchecked() -> Self {
        Self(unsafe { GetLastError() })
    }
}

impl Display for WindowsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut str = Guard::<*mut WCHAR, _>::new(null_mut(), |ptr| {
            if !ptr.is_null() {
                // SAFETY: null pointer check above
                unsafe {
                    LocalFree(*ptr as *mut c_void);
                }
            }
        });

        // SAFETY: There is no way to make this safe since windows is unsafe.
        let Some(len) = unsafe {
            FormatMessageW(
                FORMAT_MESSAGE_ALLOCATE_BUFFER
                    | FORMAT_MESSAGE_IGNORE_INSERTS
                    | FORMAT_MESSAGE_FROM_SYSTEM,
                null_mut(),
                self.0,
                MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT).into(),
                str.as_mut() as *mut *mut WCHAR as LPWSTR,
                0,
                null_mut(),
            )
        }
        .try_into()
        .ok()
        .and_then(NonZeroUsize::new) else {
            return write!(f, "failed to get error message: received message with an impossible length");
        };

        // SAFETY: There is no way to make this safe since windows is unsafe.
        match unsafe { U16CStr::from_ptr(*str.as_ref() as *const _, len.get()) } {
            Ok(msg) => write!(f, "{}", msg.display()),
            Err(err) => write!(f, "failed to get error message: {}", err),
        }
    }
}
impl StdError for WindowsError {}
