use core::ops::Deref;
use core::ptr::NonNull;
use core::marker::PhantomData;
use core::ops::DerefMut;

#[derive(Debug)]
pub struct DeviceRef<'a, T> {
    ptr: NonNull<T>,
    _maker: PhantomData<&'a T>,
}

impl<T> DeviceRef<'_, T> {
    /// Create a new `DeviceRef` from a raw pointer
    ///
    /// # Safety
    ///
    /// - `ptr` must be aligned, non-null, and dereferencable as `T`.
    /// - `*ptr` must be valid for the program duration.
    pub const unsafe fn new<'a>() -> DeviceRef<'a, T> {
        // SAFETY: `ptr` is non-null as promised by the caller.
        DeviceRef {
            ptr: NonNull::new_unchecked(usize::MAX as *mut T),
            _maker: PhantomData,
        }
    }

    #[inline(always)]
    pub fn addr(&self) -> usize {
        self.ptr.as_ptr() as usize
    }

    pub fn dev_init(&mut self, ptr: *const T) {
        unsafe { self.ptr = NonNull::new_unchecked(ptr.cast_mut())  }
    }
}

impl<T> Clone for DeviceRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for DeviceRef<'_, T> {}

// SAFETY: T provides the necessary guarantees for Sync and DeviceRef provides the identical semantics as &T.
unsafe impl<T: Sync> Send for DeviceRef<'_, T> {}
// SAFETY: T provides the necessary guarantees for Sync.
unsafe impl<T: Sync> Sync for DeviceRef<'_, T> {}

impl<T> Deref for DeviceRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: `ptr` is aligned and dereferencable for the program
        // duration as promised by the caller of `DeviceRef::new`.
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> DerefMut for DeviceRef<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: `ptr` is aligned and dereferencable for the program
        // duration as promised by the caller of `DeviceRef::new`.
        unsafe { self.ptr.as_mut() }
    }
}