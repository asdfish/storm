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
        sync::{Arc, atomic::AtomicPtr, mpsc},
        thread,
    },
    winapi::{
        ctypes::c_int,
        shared::{
            minwindef::{LPARAM, LRESULT, WPARAM},
            windef::HHOOK__,
        },
        um::winuser::{
            CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, MSG, SetWindowsHookExW,
            TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN,
        },
    },
};

mod key_hook;

static EVENT_SENDER: RwLock<Option<mpsc::Sender<Result<Event, WindowsBackendError>>>> =
    const_rwlock(None);

#[repr(transparent)]
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
        event_sender: mpsc::Sender<Result<Event, WindowsBackendError>>,
    ) -> Result<Self, WindowsBackendError> {
        {
            let mut event_sender_smuggler = EVENT_SENDER.write();
            if event_sender_smuggler.is_some() {
                return Err(WindowsBackendError::MultipleKeyboardHooks);
            } else {
                *event_sender_smuggler = Some(event_sender);
            }
        }

        let (tx, rx) = mpsc::sync_channel(1);

        thread::spawn(move || {
            // the hook must be set on the same thread as the message sending
            tx.send(
                WinapiError::from_return(unsafe {
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_hook::key_hook), null_mut(), 0)
                })
                .map(NonNull::as_ptr)
                .map(AtomicPtr::new),
            )
            .expect("internal error: `rx` should not be dropped yet");

            let mut msg = unsafe { mem::zeroed() };
            loop {
                unsafe {
                    GetMessageW(&mut msg as *mut _, null_mut(), 0, 0);
                }
                unsafe {
                    TranslateMessage(&msg as *const _);
                }
                unsafe {
                    DispatchMessageW(&msg as *const _);
                }
            }
        });

        Ok(Self {
            key_hook: rx
                .recv()
                .expect("internal error: `tx` should not be dropped")
                .map(AtomicPtr::into_inner)
                .map(NonNull::new)
                .map(|ptr| {
                    ptr.expect(
                        "internal error: [WinapiError::from_return] should filter null pointers",
                    )
                })?,
        })
    }
}
