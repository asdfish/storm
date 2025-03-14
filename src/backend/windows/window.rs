use {std::ptr::NonNull, winapi::shared::windef::HWND__};

#[repr(transparent)]
pub struct WindowsWindow(NonNull<HWND__>);
