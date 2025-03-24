use {
    crate::{
        backend::{
            State,
            windows::{WinapiError, WindowsBackendError, WindowsWindow},
        },
        error,
        state::{Event, EventSender, Storm},
    },
    parking_lot::{RwLock, const_rwlock},
    std::{
        collections::HashMap,
        mem,
        ptr::{NonNull, null_mut},
        sync::atomic::AtomicPtr,
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

static EVENT_SENDER: RwLock<Option<EventSender<WindowsWindow, WindowsBackendError>>> =
    const_rwlock(None);

pub struct WindowsBackendState {
    event_sender: EventSender<WindowsWindow, WindowsBackendError>,
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
            let _ = state
                .backend_state
                .event_sender
                .send(Ok(Event::AddWindow {
                    workspace: state.workspace,
                    window: foreground_window,
                }));
        }
    }

    fn new(
        _: &mut HashMap<u8, Vec<WindowsWindow>>,
        event_sender: EventSender<WindowsWindow, WindowsBackendError>,
    ) -> Result<Self, WindowsBackendError> {
        {
            let mut event_sender_smuggler = EVENT_SENDER.write();
            if event_sender_smuggler.is_some() {
                return Err(WindowsBackendError::MultipleKeyboardHooks);
            } else {
                *event_sender_smuggler = Some(EventSender::clone(&event_sender));
            }
        }

        let (tx, rx) = oneshot::channel();

        thread::spawn(move || {
            // the hook must be set on the same thread as the message sending
            let _ = tx.send(
                WinapiError::from_return(unsafe {
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_hook::key_hook), null_mut(), 0)
                })
                .map(NonNull::as_ptr)
                .map(AtomicPtr::new),
            );

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
