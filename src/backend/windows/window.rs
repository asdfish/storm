use {
    crate::backend::{
        Rect, Window,
        windows::{WinapiError, WindowsBackendError},
    },
    std::{
        mem,
        error::Error as StdError,
        fmt::{self, Display, Formatter},
        num::{NonZeroUsize, TryFromIntError},
        sync::atomic::{AtomicPtr, Ordering},
    },
    widestring::ustring::U16String,
    winapi::{
        shared::{
            minwindef::{FALSE, TRUE},
            windef::{HWND, HWND__, LPRECT, RECT},
        },
        um::{
            winnt::{LONG, WCHAR},
            winuser::{
                EnableWindow, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsWindow,
                IsWindowEnabled, IsWindowVisible, MoveWindow, SW_MINIMIZE, SW_SHOW,
                ShowWindowAsync,
            },
        },
    },
};

#[repr(transparent)]
pub struct WindowsWindow(AtomicPtr<HWND__>);
impl WindowsWindow {
    pub fn as_ptr(&self) -> HWND {
        self.0.load(Ordering::SeqCst)
    }
}
impl TryFrom<HWND> for WindowsWindow {
    type Error = NullHwndError;

    fn try_from(handle: HWND) -> Result<Self, NullHwndError> {
        if handle.is_null() {
            Err(NullHwndError)
        } else {
            Ok(Self(AtomicPtr::new(handle)))
        }
    }
}
#[derive(Debug)]
pub struct NullHwndError;
impl Display for NullHwndError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "internal error: [WindowsWindow] must not contain null pointers")
    }
}
impl StdError for NullHwndError {}

impl Window for WindowsWindow {
    type Error = WindowsBackendError;
    type String = U16String;

    fn is_alive(&self) -> bool {
        // SAFETY: pointer is not null
        unsafe { IsWindow(self.as_ptr()) == TRUE }
    }
    fn is_focused(&self) -> bool {
        unsafe { IsWindowEnabled(self.as_ptr()) == TRUE }
    }
    fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.as_ptr()) == TRUE }
    }

    fn move_to(&self, to: Rect) -> Result<(), WindowsBackendError> {
        // SAFETY: Self is not null.
        WinapiError::from_return(unsafe {
            MoveWindow(
                self.as_ptr(),
                to.x.into(),
                to.y.into(),
                to.width.into(),
                to.height.into(),
                TRUE,
            )
        })
        .map(drop)
        .map_err(<WinapiError as Into<WindowsBackendError>>::into)
    }
    fn position(&self) -> Result<Rect, WindowsBackendError> {
        // SAFETY: The rect is initialized with [GetWindowRect].
        let mut rect: RECT = unsafe { mem::zeroed() };

        WinapiError::from_return(unsafe {
            GetWindowRect(self.as_ptr(), &mut rect as *mut _ as LPRECT)
        })?;
        rect.try_into()
            .map_err(<TryFromIntError as Into<WindowsBackendError>>::into)
    }

    fn title(&self) -> Result<U16String, WindowsBackendError> {
        let length: NonZeroUsize =
            WinapiError::from_return(unsafe { GetWindowTextLengthW(self.as_ptr()) })?
                .try_into()?;

        let mut str: Box<[WCHAR]> = vec![0; length.get()].into_boxed_slice();

        WinapiError::from_return(unsafe {
            GetWindowTextW(self.as_ptr(), str.as_mut_ptr(), length.get().try_into().expect("internal error: the length was created with a [DWORD], so it should also be converted back into one"))
        })?;

        Ok(U16String::from(Vec::from(str)))
    }

    fn set_focus(&mut self, focused: bool) -> Result<(), WindowsBackendError> {
        WinapiError::from_return(unsafe {
            EnableWindow(
                self.as_ptr(),
                match focused {
                    true => TRUE,
                    false => FALSE,
                },
            )
        })
        .map(drop)
        .map_err(<WinapiError as Into<WindowsBackendError>>::into)
    }
    fn set_visibility(&mut self, visible: bool) -> Result<(), WindowsBackendError> {
        WinapiError::from_return(unsafe {
            ShowWindowAsync(
                self.as_ptr(),
                match visible {
                    true => SW_SHOW,
                    false => SW_MINIMIZE,
                },
            )
        })
        .map(drop)
        .map_err(<WinapiError as Into<WindowsBackendError>>::into)
    }
}

impl From<Rect> for RECT {
    fn from(rect: Rect) -> Self {
        Self {
            left: rect.x.into(),
            top: rect.y.into(),
            right: <i16 as Into<i32>>::into(rect.x) + <u16 as Into<i32>>::into(rect.width),
            bottom: <i16 as Into<i32>>::into(rect.y) + <u16 as Into<i32>>::into(rect.height),
        }
    }
}
impl TryFrom<RECT> for Rect {
    type Error = TryFromIntError;

    fn try_from(rect: RECT) -> Result<Self, TryFromIntError> {
        let x: i16 = rect.left.try_into()?;
        let y: i16 = rect.top.try_into()?;

        Ok(Self {
            x,
            y,
            width: (<LONG as TryInto<i16>>::try_into(rect.right)? - x).try_into()?,
            height: (<LONG as TryInto<i16>>::try_into(rect.bottom)? - y).try_into()?,
        })
    }
}
