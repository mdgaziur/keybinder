use libc::{c_char, c_void};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::ptr;
use std::sync::Once;

#[link(name = "keybinder-3.0")]
extern "C" {
    fn keybinder_init();
    fn keybinder_bind(
        keystring: *const c_char,
        handler: unsafe extern "C" fn(*const c_char, *mut c_void),
        user_data: *mut c_void,
    ) -> bool;
    fn keybinder_get_current_event_time() -> u32;
    fn keybinder_set_use_cooked_accelerators(use_cooked: bool);
    fn keybinder_unbind(keystring: *const c_char);
    fn keybinder_supported() -> bool;
}

static INIT: Once = Once::new();

struct Payload<T> {
    user_handler: fn(String, &T),
    user_data: T,
}

/// # Safety:
///
/// KeyBinder::bind() puts valid data after calling Box::leak() (to prevent use after free)
/// in data_ptr hence dereferencing it shouldn't cause
/// any problems. right?
unsafe extern "C" fn handler_impl<T>(c_keystring: *const c_char, data: *mut c_void) {
    let keystring = CStr::from_ptr(c_keystring).to_str().unwrap();
    let payload = ptr::NonNull::new(data as *mut Payload<T>).unwrap().as_mut();

    (payload.user_handler)(keystring.to_string(), &payload.user_data)
}

pub struct KeyBinder<T> {
    data_ptrs: HashMap<String, *mut c_void>,
    _marker: PhantomData<T>,
}

impl<T> KeyBinder<T> {
    pub fn new(use_cooked: bool) -> Result<Self, ()> {
        if !unsafe { keybinder_supported() } {
            return Err(());
        }

        INIT.call_once(|| unsafe { keybinder_init() });

        unsafe {
            keybinder_set_use_cooked_accelerators(use_cooked);
        }

        Ok(Self {
            data_ptrs: HashMap::new(),
            _marker: PhantomData,
        })
    }

    pub fn bind(&mut self, keystring: &str, user_handler: fn(String, &T), user_data: T) -> bool {
        // To make sure the keystring is not already bound.
        // It'll not do anything if the keystring isn't bound.
        self.unbind(keystring);

        let c_keystring = CString::new(keystring).unwrap();

        // Put the data in heap and immediately leak it so that when it's passed to
        // handler, it's valid. If we don't leak it, the data will drop after this scope ends.
        // This would result in use after free.
        let payload_ptr = Box::leak(Box::new(Payload {
            user_data,
            user_handler,
        })) as *const _ as *mut c_void;

        self.data_ptrs.insert(keystring.to_string(), payload_ptr);

        // Handler properly handles the data and payload_ptr is valid unless the keystring is unbinded.
        // To prevent use after free, the drop implementation unbinds the keystring and frees the data_ptr.
        unsafe { keybinder_bind(c_keystring.as_ptr(), handler_impl::<T>, payload_ptr) }
    }

    pub fn unbind(&mut self, keystring: &str) {
        if self.data_ptrs.contains_key(keystring) {
            // SAFETY: Two `keystring` can't have the save data_ptr. This prevents use after free.
            //         Also, the data is alloc'd by KeyBinder::bind() and never dealloc'd unless
            //         the user unbinds it. In that case, KeyBinder::unbind() removes the data_ptr from
            //         the hashmap.
            unsafe {
                Self::unbind_impl(keystring, *self.data_ptrs.get(keystring).unwrap());
            }

            self.data_ptrs.remove(keystring).unwrap();
        }
    }

    /// # Safety:
    /// Caller has to make sure that data isn't freed twice and the data_ptr is valid
    unsafe fn unbind_impl(keystring: &str, data_ptr: *mut c_void) {
        let c_keystring = CString::new(keystring).unwrap();
        
        // TODO: check if it's still leaking or not
        let _ = Box::<Payload<T>>::from_raw(data_ptr as *mut Payload<T>);

        keybinder_unbind(c_keystring.as_ptr());
    }
}

pub fn get_current_event_time() -> u32 {
    unsafe { keybinder_get_current_event_time() }
}

impl<T> Drop for KeyBinder<T> {
    fn drop(&mut self) {
        for keystring in self.data_ptrs.keys() {
            // SAFETY: Two `keystring` can't have the save data_ptr. This prevents use after free.
            //         Also, the data is alloc'd by KeyBinder::bind() and never dealloc'd unless
            //         the user unbinds it. In that case, KeyBinder::unbind() removes the data_ptr from
            //         the hashmap.
            unsafe {
                Self::unbind_impl(keystring, *self.data_ptrs.get(keystring).unwrap());
            }
        }
    }
}
