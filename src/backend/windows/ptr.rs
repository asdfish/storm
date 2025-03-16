use winapi::{ctypes::c_void, um::winbase::LocalFree};

#[repr(transparent)]
/// The pointer must be allocated with [winapi::um::winbase::LocalAlloc].
pub struct LocalPtr<T>(pub *mut T);
impl<T> Drop for LocalPtr<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0 as *mut c_void);
            }
        }
    }
}
