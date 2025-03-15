use {
    crate::{
        backend::{
            State,
            windows::{WindowsBackendError, WindowsWindow},
        },
        state::Event,
    },
    parking_lot::{RwLock, const_rwlock},
    std::{
        cell::Cell,
        collections::{HashMap, HashSet},
        ptr::{NonNull, null_mut},
        sync::mpsc::Sender,
    },
    winapi::{
        ctypes::c_int,
        shared::{
            minwindef::{LPARAM, LRESULT, WPARAM},
            windef::HHOOK__,
        },
        um::winuser::{CallNextHookEx, UnhookWindowsHookEx}
    },
};

static EVENT_SENDER: RwLock<Option<Sender<Event>>> = const_rwlock(None);

pub struct WindowsBackendState {
    key_hook: NonNull<HHOOK__>,
}
impl WindowsBackendState {
    /// # Panics
    ///
    /// Will panic if the user does any key presses while [EVENT_SENDER] is [None].
    fn set_key_hook() -> Result<NonNull<HHOOK__>, WindowsBackendError> {
        extern "system" fn key_hook(code: c_int, event_ident: WPARAM, info: LPARAM) -> LRESULT {
            if code < 0 {
                unsafe { CallNextHookEx(null_mut(), code, event_ident, info) }
            } else {
                todo!()
            }
        }

        todo!()
    }
}
impl Drop for WindowsBackendState {
    fn drop(&mut self) {
        unsafe {
            UnhookWindowsHookEx(self.key_hook.as_ptr());
        }
        *EVENT_SENDER.write() = None;
    }
}
impl State<WindowsWindow, WindowsBackendError> for WindowsBackendState {
    fn new(
        _: &mut HashMap<u8, HashSet<WindowsWindow>>,
        event_sender: Sender<Event>,
    ) -> Result<Self, WindowsBackendError> {
        let mut event_sender_smuggler = EVENT_SENDER.write();
        if event_sender_smuggler.is_some() {
            return Err(WindowsBackendError::MultipleKeyboardHooks);
        } else {
            *event_sender_smuggler = Some(event_sender);
        }
        drop(event_sender_smuggler);

        Ok(Self {
            key_hook: Self::set_key_hook()?,
        })
    }
}
