use {
    crate::{
        backend::{
            State,
            windows::{WinapiError, WindowsBackendError, WindowsWindow},
        },
        state::Event,
    },
    parking_lot::{RwLock, const_rwlock},
    std::{
        cell::Cell,
        collections::{HashMap, HashSet},
        mem,
        ptr::{NonNull, null_mut},
        sync::mpsc,
        thread,
    },
    winapi::{
        ctypes::c_int,
        shared::{
            minwindef::{LPARAM, LRESULT, WPARAM},
            windef::HHOOK__,
        },
        um::winuser::{
            CallNextHookEx, DispatchMessageW, GetMessageW, MSG, SetWindowsHookExW,
            TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
        },
    },
};

static EVENT_SENDER: RwLock<Option<mpsc::Sender<Event>>> = const_rwlock(None);

unsafe extern "system" fn key_hook(code: c_int, event_ident: WPARAM, info: LPARAM) -> LRESULT {
    let call_next_hook = || unsafe { CallNextHookEx(null_mut(), code, event_ident, info) };

    if code < 0 {
        return call_next_hook();
    }

    if let Some(sender) = EVENT_SENDER.read().as_ref() {
        sender
            .send(Event::Key(String::new()))
            .expect("internal error: EVENT_SENDER got disconnected");
    }
    //
    //if let Some(sender) = EVENT_SENDER
    //    .read()
    //    .as_ref()
    //    //.unwrap()
    //    //.send(Event::Key(String::new()))
    //{
    //    sender.send(Event::Key(String::new())).unwrap();
    //}

    return call_next_hook();
}

pub struct WindowsBackendState {
    key_hook: NonNull<HHOOK__>,
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
        event_sender: mpsc::Sender<Event>,
    ) -> Result<Self, WindowsBackendError> {
        {
            let mut event_sender_smuggler = EVENT_SENDER.write();
            if event_sender_smuggler.is_some() {
                return Err(WindowsBackendError::MultipleKeyboardHooks);
            } else {
                *event_sender_smuggler = Some(event_sender);
            }
        }

        Ok(Self {
            key_hook: WinapiError::from_return(unsafe {
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_hook), null_mut(), 0)
            })
            .map_err(<WinapiError as Into<WindowsBackendError>>::into)?,
        })
    }

    fn run(&self) {
        loop {
            let mut message = unsafe { mem::zeroed() };

            unsafe {
                GetMessageW(&mut message as *mut _, null_mut(), 0, 0);
            }
            unsafe {
                TranslateMessage(&message as *const _);
            }
            unsafe {
                DispatchMessageW(&message as *const _);
            }
        }
    }
}
