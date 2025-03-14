use {
    raii::Guard,
    std::{
        error::Error as StdError,
        fmt::{self, Display, Formatter},
        num::{
            NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128, NonZeroIsize, NonZeroU8,
            NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128, NonZeroUsize, TryFromIntError,
        },
        ptr::{NonNull, null_mut},
    },
    widestring::ucstr::U16CStr,
    winapi::{
        ctypes::{c_int, c_void},
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

/// Marks a return type that can indicate a [WinapiError].
pub trait ReturnError: Sized {
    type Into;
    fn attempt_into(self) -> Option<Self::Into>;
}
macro_rules! impl_return_error_for_number {
    ($num:ty, $non_zero:ty) => {
        impl ReturnError for $num {
            type Into = $non_zero;
            fn attempt_into(self) -> Option<$non_zero> {
                <$non_zero>::new(self)
            }
        }
    };
}
impl_return_error_for_number!(i8, NonZeroI8);
impl_return_error_for_number!(i16, NonZeroI16);
impl_return_error_for_number!(i32, NonZeroI32);
impl_return_error_for_number!(i64, NonZeroI64);
impl_return_error_for_number!(i128, NonZeroI128);
impl_return_error_for_number!(isize, NonZeroIsize);
impl_return_error_for_number!(u8, NonZeroU8);
impl_return_error_for_number!(u16, NonZeroU16);
impl_return_error_for_number!(u32, NonZeroU32);
impl_return_error_for_number!(u64, NonZeroU64);
impl_return_error_for_number!(u128, NonZeroU128);
impl_return_error_for_number!(usize, NonZeroUsize);
impl<T> ReturnError for *mut T {
    type Into = NonNull<T>;
    fn attempt_into(self) -> Option<NonNull<T>> {
        NonNull::new(self)
    }
}

#[derive(Debug)]
#[repr(transparent)]
/// Errors from windows api functions. For all possible errors in the windows backend, see
/// [WindowsBackendError].
///
/// Contains the error code similar to errno.
///
/// Error code 0 is for when operations return successfully, however it relies on the callee using
/// `SetLastError` which is unreliable, so this cannot be represented with a NonZero<DWORD>.
pub struct WinapiError(DWORD);

impl WinapiError {
    pub fn from_return<T>(value: T) -> Result<<T as ReturnError>::Into, Self>
    where
        T: ReturnError,
    {
        value.attempt_into().ok_or_else(|| Self::new_unchecked())
    }

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

impl Display for WinapiError {
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
            return write!(
                f,
                "failed to get error message: received message with an impossible length"
            );
        };

        // SAFETY: There is no way to make this safe since windows is unsafe.
        match unsafe { U16CStr::from_ptr(*str.as_ref() as *const _, len.get()) } {
            Ok(msg) => write!(f, "{}", msg.display()),
            Err(err) => write!(f, "failed to get error message: {}", err),
        }
    }
}
impl StdError for WinapiError {}

#[derive(Debug)]
/// All possible errors in this backend.
pub enum WindowsBackendError {
    TryFromInt(TryFromIntError),
    Winapi(WinapiError),
}
impl From<TryFromIntError> for WindowsBackendError {
    fn from(error: TryFromIntError) -> Self {
        Self::TryFromInt(error)
    }
}
impl From<WinapiError> for WindowsBackendError {
    fn from(error: WinapiError) -> Self {
        Self::Winapi(error)
    }
}
impl Display for WindowsBackendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TryFromInt(error) => write!(f, "{}", error),
            Self::Winapi(error) => write!(f, "{}", error),
        }
    }
}
impl StdError for WindowsBackendError {}
