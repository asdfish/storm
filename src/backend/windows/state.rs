use {
    crate::{
        backend::{
            State,
            windows::{WinapiError, WindowsBackendError, WindowsWindow},
        },
        error,
        state::{Event, Storm},
    },
    parking_lot::{RwLock, const_rwlock},
    std::{
        collections::HashMap,
        mem,
        ptr::{NonNull, null_mut},
        sync::{atomic::AtomicPtr, mpsc},
        thread,
    },
    winapi::{
        shared::windef::HHOOK__,
        um::winuser::{
            DispatchMessageW, GetForegroundWindow, GetMessageW, SetWindowsHookExW,
            TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
        },
    },
};

mod key_hook;

static EVENT_SENDER: RwLock<Option<mpsc::Sender<Result<Event<WindowsWindow>, WindowsBackendError>>>> =
    const_rwlock(None);

pub struct WindowsBackendState {
    event_sender: mpsc::Sender<Result<Event<WindowsWindow>, WindowsBackendError>>,
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
    fn each_event(state: &mut Storm<Self, WindowsWindow, WindowsBackendError>) {
        if let Ok(foreground_window) = WindowsWindow::try_from(unsafe { GetForegroundWindow() }) {
            state.backend_state.event_sender.send(Ok(Event::AddWindow(state.workspace, foreground_window)))
                .expect(error::CLOSED_CHANNEL);
        }
    }

    fn new(
        _: &mut HashMap<u8, Vec<WindowsWindow>>,
        event_sender: mpsc::Sender<Result<Event<WindowsWindow>, WindowsBackendError>>,
    ) -> Result<Self, WindowsBackendError> {
        {
            let mut event_sender_smuggler = EVENT_SENDER.write();
            if event_sender_smuggler.is_some() {
                return Err(WindowsBackendError::MultipleKeyboardHooks);
            } else {
                *event_sender_smuggler = Some(mpsc::Sender::clone(&event_sender));
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
                .expect(error::CLOSED_CHANNEL);

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
            event_sender,
            key_hook: rx
                .recv()
                .expect(error::CLOSED_CHANNEL)
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
