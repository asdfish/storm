use {
    super::EVENT_SENDER,
    crate::{
        backend::windows::{WinapiError, WindowsBackendError},
        config::key::{InvisibleKey, Key, KeyKind, KeyModifier, KeyModifiers},
        state::{KeyIntercept, Event},
    },
    std::{borrow::Cow, num::NonZeroUsize, ptr::null_mut},
    widestring::ustr::U16Str,
    winapi::{
        ctypes::c_int,
        shared::minwindef::{LPARAM, LRESULT, WPARAM},
        um::winuser::{
            CallNextHookEx, GetKeyState, GetKeyboardState, KBDLLHOOKSTRUCT, ToUnicode, VK_CONTROL,
            VK_F1, VK_F24, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RWIN, VK_SHIFT, WM_KEYDOWN,
        },
    },
};

/// Returns Ok(None) for dead keys.
fn translate_key(key_diff: LPARAM) -> Result<Option<Key<'static>>, WindowsBackendError> {
    let key_diff = unsafe { (key_diff as *mut KBDLLHOOKSTRUCT).as_ref() }
        .ok_or(WindowsBackendError::NullKbdllhookstruct)?;

    let modifiers = [
        (KeyModifier::Alt, &[VK_MENU] as &[_]),
        (KeyModifier::Control, &[VK_CONTROL]),
        (KeyModifier::Shift, &[VK_SHIFT]),
        (KeyModifier::Super, &[VK_LWIN, VK_RWIN]),
    ]
    .into_iter()
    .map(|(modifier, virt_keys)| {
        (
            modifier,
            virt_keys
                .iter()
                .copied()
                .map(|virt_key| unsafe { GetKeyState(virt_key) })
                .any(|virt_key| virt_key & (1 << 15) != 0),
        )
    })
    .collect::<KeyModifiers>();

    match key_diff.vkCode as i32 {
        key @ VK_F1..=VK_F24 => Ok(Some(Key::new(
            modifiers,
            KeyKind::Invisible(InvisibleKey::F(
                (key + 1 - VK_F1)
                    .try_into()
                    .expect("internal error: the pattern should ensure this is valid"),
            )),
        ))),
        VK_PRIOR => Ok(Some(Key::new(
            modifiers,
            KeyKind::Invisible(InvisibleKey::PageUp),
        ))),
        VK_NEXT => Ok(Some(Key::new(
            modifiers,
            KeyKind::Invisible(InvisibleKey::PageDown),
        ))),
        _ => {
            let mut keyboard_state = [0; 256];
            WinapiError::from_return(unsafe { GetKeyboardState(keyboard_state.as_mut_ptr()) })?;

            let mut buffer = [0; 2];

            let Some(len) = (match unsafe {
                ToUnicode(
                    key_diff.vkCode,
                    key_diff.scanCode,
                    keyboard_state.as_ptr(),
                    buffer.as_mut_ptr(),
                    2,
                    0,
                )
            } {
                ..=0 => None,
                1 => Some(const { NonZeroUsize::new(1).unwrap() }),
                2.. => Some(const { NonZeroUsize::new(2).unwrap() }),
            }) else {
                return Ok(None);
            };

            Ok(Some(Key::new(
                modifiers,
                KeyKind::Visible(Cow::Owned(
                    U16Str::from_slice(&buffer[..len.get()]).to_string_lossy(),
                )),
            )))
        }
    }
}

pub unsafe extern "system" fn key_hook(
    code: c_int,
    event_ident: WPARAM,
    key_diff: LPARAM,
) -> LRESULT {
    let call_next_hook = || unsafe { CallNextHookEx(null_mut(), code, event_ident, key_diff) };

    if code < 0 {
        return call_next_hook();
    }

    if event_ident == WM_KEYDOWN as WPARAM {
        if let Some(sender) = EVENT_SENDER.read().as_ref() {
            let send = |event| {
                drop(sender.send(event));
            };

            let (tx, rx) = oneshot::channel();

            match translate_key(key_diff) {
                Ok(Some(key)) => {
                    send(Ok(Event::Key(tx, key)));

                    if matches!(rx.recv().unwrap_or_default(), KeyIntercept::Block) {
                        return 1;
                    }
                }
                Ok(None) => {}
                Err(err) => send(Err(err)),
            }
        }
    }

    call_next_hook()
}
