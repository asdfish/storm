use {
    crate::{
        backend::{
            State,
            windows::{WinapiError, WindowsBackendError, WindowsWindow},
        },
        state::Event,
    },
    parking_lot::{Condvar, Mutex, RwLock, const_rwlock},
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

        let package = Arc::new((Mutex::new(None), Condvar::new()));

        let thread_package = Arc::clone(&package);
        thread::spawn(move || {
            {
                let (tx, notification) = &*thread_package;
                *tx.lock() = Some(
                    WinapiError::from_return(unsafe {
                        SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_hook), null_mut(), 0)
                    })
                    .map(NonNull::as_ptr)
                    .map(AtomicPtr::new),
                );
                notification.notify_one();
                drop(thread_package);
            }

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

        {
            let (rx, notification) = &*package;
            notification.wait(&mut rx.lock());
        }

        Ok(Self {
            key_hook: Mutex::into_inner(
                Arc::into_inner(package)
                    .expect("all references should have been dropped")
                    .0,
            )
            .expect("`notification` should only wake after `rx` is [Some]")
            .map(AtomicPtr::into_inner)
            .map(NonNull::new)
            .map(|ptr| {
                ptr.expect("internal error: [WinapiError::from_return] should filter null pointers")
            })?,
        })
    }
}
