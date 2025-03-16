use {
    super::EVENT_SENDER,
    crate::{
        backend::{
            State,
            windows::{WinapiError, WindowsBackendError},
        },
        error,
        state::{Event, Modifier},
    },
    enum_map::EnumMap,
    std::{num::NonZeroUsize, ptr::null_mut},
    widestring::ustr::U16Str,
    winapi::{
        ctypes::c_int,
        shared::minwindef::{LPARAM, LRESULT, WPARAM},
        um::winuser::{
            CallNextHookEx, GetKeyState, GetKeyboardState, KBDLLHOOKSTRUCT, ToUnicode, WM_KEYDOWN, VK_CONTROL,
            VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN,
        },
    },
};

/// Returns Ok(None) for dead keys.
fn translate_key(key_diff: LPARAM) -> Result<Option<(EnumMap<Modifier, ()>, String)>, WindowsBackendError> {
    let key_diff = unsafe { (key_diff as *mut KBDLLHOOKSTRUCT).as_ref() }
        .ok_or(WindowsBackendError::NullKbdllhookstruct)?;

    let modifiers = [
        (Modifier::Alt, &[VK_MENU] as &[_]),
        (Modifier::Control, &[VK_CONTROL]),
        (Modifier::Shift, &[VK_SHIFT]),
        (Modifier::Super, &[VK_LWIN, VK_RWIN]),
    ]
        .into_iter()
        .map(|(modifier, virt_keys)| {
            (
                modifier,
                virt_keys.iter()
                    .map(|virt_key| unsafe { GetKeyState(*virt_key) })
                    .inspect(|state| {
                        println!("{:?} {:b}", modifier, state);
                    })
                    .any(|virt_key| virt_key.signum() == 1)
            )
        })
        .filter_map(|(modifier, pressed)| pressed.then_some((modifier, ())))
        .collect::<EnumMap<Modifier, ()>>();

    //[
    //    VK_MENU,
    //    VK_SHIFT,
    //]
    //    .into_iter()
    //    .for_each(|vk_key| unsafe { GetKeyState(vk_key); });

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
        1 => Some(NonZeroUsize::new(1).unwrap()),
        2.. => Some(NonZeroUsize::new(2).unwrap()),
    }) else {
        return Ok(None);
    };

    Ok(Some(
        (modifiers, U16Str::from_slice(&buffer[..len.get()]).to_string_lossy())
    ))
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

    if event_ident == WM_KEYDOWN.try_into().expect("internal error: `WM_KEYDOWN` should be comparable with the second parameter of a `LowlevelKeyboardProc`") {
        if let Some(sender) = EVENT_SENDER.read().as_ref() {
            let send = |event|
                sender.send(event)
                    .expect(error::CLOSED_CHANNEL);

            match translate_key(key_diff) {
                Ok(Some((modifiers, text))) => send(Ok(Event::Key(modifiers, text))),
                Ok(None) => {},
                Err(err) => send(Err(err)),
            }
        }
    }

    return call_next_hook();
}
