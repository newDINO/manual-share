//! # Manually shared box
//! ```
//! use std::thread;
//! use manual_share::SharedBox;
//!
//! let b = Box::new(13);
//! let mut b = SharedBox::from_box(b);
//!
//! let br = b.borrow();
//! let j1 = thread::spawn(move || {
//!     println!("{}", br.get());
//!     br
//! });
//!
//! let br = b.borrow();
//! let j2 = thread::spawn(move || {
//!     println!("{}", br.get() + 1);
//!     br
//! });
//!
//! b.try_return(j1.join().unwrap()).unwrap();
//! b.try_return(j2.join().unwrap()).unwrap();
//!
//! let b = b.try_into_box().unwrap();
//! println!("{:?}", b);
//! ```

/// A structure owning the original `Box`
/// which can be used to create multiple `SharedBoxRef` to send to other thread.
/// It uses a counter to record the number of `SharedBoxRef` that has been created and not given back.
///
/// Dropping `SharedBox` without returning all `SharedBoxRef` that it has created leaks its heap memory.
/// When **`panic-on-drop`** feature is enabled, it will panic:
/// ```should_panic
/// let r = {
///     let mut b = manual_share::SharedBox::new(0);
///     b.borrow()
///
///     // dropping SharedBox, panic here
/// };
/// println!("{}", r.get());
/// ```
///
/// When all `SharedBoxRef` have been returned back to `SharedBox`,
/// it will free and properly release its heap memory when `SharedBox` is dropped.
/// ```
/// let mut b = manual_share::SharedBox::new(0);
/// let r = b.borrow();
/// b.try_return(r).unwrap();
///
/// // Dropping SharedBox here will free the heap memory it owns. No panic will occur.
/// ```
#[derive(Debug)]
pub struct SharedBox<T> {
    borrow_count: usize,
    ptr: *mut T,
}

impl<T> SharedBox<T> {
    /// Create a `SharedBox` by creating a `Box` first,
    /// then convert it to a `SharedBox`.
    pub fn new(value: T) -> Self {
        let b = Box::new(value);
        Self::from_box(b)
    }

    /// Create a `SharedBox` by consuming a `Box`.
    pub fn from_box(unique: Box<T>) -> Self {
        Self {
            borrow_count: 0,
            ptr: Box::into_raw(unique),
        }
    }

    /// Create a `SharedBoxRef` and increase the borrow count.
    pub fn borrow(&mut self) -> SharedBoxRef<T> {
        // No overflow check is needed because creating more than `usize::MAX` SharedBoxRef is impossible.
        self.borrow_count += 1;
        SharedBoxRef { ptr: self.ptr }
    }

    /// Try to return back the `SharedBoxRef`.
    /// Returns `Err` if the `SharedBoxRef` does not originate from the same `SharedBox`.
    ///
    /// Decrease the borrow count if not error occurs.
    ///
    /// ```
    /// use manual_share::SharedBox;
    ///
    /// let mut b1 = SharedBox::from_box(Box::new(8));
    /// let r1 = b1.borrow();
    /// b1.try_return(r1).unwrap();
    ///
    /// let mut b2 = SharedBox::from_box(Box::new(9));
    /// let r2 = b2.borrow();
    ///
    /// // Giving SharedBoxRef to the wrong SharedBox returns Err.
    /// let r2 = b1.try_return(r2).unwrap_err();
    ///
    /// b2.try_return(r2).unwrap();
    /// ```
    pub fn try_return(&mut self, reference: SharedBoxRef<T>) -> Result<(), SharedBoxRef<T>> {
        if !core::ptr::eq(self.ptr, reference.ptr) {
            return Err(reference);
        }

        self.borrow_count -= 1;
        let _ = core::mem::ManuallyDrop::new(reference);
        Ok(())
    }

    /// Try to convert `Self` into a `Box` if all borrowed `SharedBoxRef` has been given back.
    ///
    /// ```
    /// use manual_share::SharedBox;
    ///
    /// let b = Box::new(0);
    /// let mut b = SharedBox::from_box(b);
    ///
    /// let r = b.borrow();
    ///
    /// // Try to convert to Box without returning all SharedBoxRef returns Err.
    /// let mut b = b.try_into_box().unwrap_err();
    ///
    /// b.try_return(r).unwrap();
    ///
    /// let b = b.try_into_box().unwrap();
    /// assert_eq!(b, Box::new(0));
    /// ```
    pub fn try_into_box(self) -> Result<Box<T>, Self> {
        if self.borrow_count > 0 {
            Err(self)
        } else {
            let r = core::mem::ManuallyDrop::new(self);
            Ok(unsafe { Box::from_raw(r.ptr) })
        }
    }
    /// Directly get a reference to the value inside the `SharedBox`.
    /// This use rust built-in lifetime check to ensure the reference is valid as long as the `SharedBox` is alive,
    /// and has no runtime overhead.
    pub fn get(&self) -> &T {
        // SAFETY:
        // The pointer is valid as long as the SharedBox is alive.
        // All other references can only get immutable reference.
        unsafe { &*self.ptr }
    }
}

unsafe impl<T: Send> Send for SharedBox<T> {}
unsafe impl<T: Sync> Sync for SharedBox<T> {}

impl<T> Drop for SharedBox<T> {
    fn drop(&mut self) {
        #[cfg(feature = "panic-on-drop")]
        {
            // Let user deal with other panics.
            #[cfg(feature = "do-not-panic-when-panicking")]
            if std::thread::panicking() {
                return;
            }

            if self.borrow_count > 0 {
                panic!("Dropping a SharedBox without giving back all SharedBoxRef")
            }
        }
        // Only drops when there are no outstanding SharedBoxRef values to prevent use-after-free.
        if self.borrow_count == 0 {
            unsafe {
                drop(Box::from_raw(self.ptr));
            }
        }
    }
}

/// A Reference to `SharedBox` that can be sent to other threads.
///
/// Dropping a `SharedBoxRef` leaks the heap memory it points to.
/// When **`panic-on-drop`** feature is enabled, dropping it will panic:
/// ```should_panic
/// let mut b = manual_share::SharedBox::new(1);
/// b.borrow();
///
/// // forget SharedBox to make sure the panic is not caused by dropping it first.
/// std::mem::forget(b);
///
/// // panic here due to dropping SharedBoxRef
/// ```
///
/// Use `SharedBox::try_return` to consume it without causing panic.
/// ```
/// let mut b = manual_share::SharedBox::new(1);
/// let r = b.borrow();
/// b.try_return(r).unwrap();
/// ```
#[derive(Debug)]
pub struct SharedBoxRef<T> {
    ptr: *const T,
}

/// `SharedBoxRef` is like `&T`, which only requires `T: Sync` to implement `Send`.
unsafe impl<T: Sync> Send for SharedBoxRef<T> {}
unsafe impl<T: Sync> Sync for SharedBoxRef<T> {}

impl<T> SharedBoxRef<T> {
    /// Example usage:
    /// ```
    /// let mut b = manual_share::SharedBox::new(42);
    /// let r = b.borrow();
    /// let value = *r.get();
    /// assert_eq!(value, 42);
    ///
    /// b.try_return(r).unwrap();
    /// ```
    ///
    /// The reference got from this method has the same lifetime of the `SharedBoxRef`,
    /// which means it will be invalidated after `SharedBoxRef` is given back to `SharedBox`:
    /// ```compile_fail
    /// let mut b = manual_share::SharedBox::new(42);
    /// let br = b.borrow();
    /// let r = br.get();
    ///
    /// b.try_return(br).unwrap();
    ///
    /// // r is no longer valid here.
    /// println!("{}", r);
    /// ```
    pub fn get(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T> Drop for SharedBoxRef<T> {
    fn drop(&mut self) {
        #[cfg(feature = "panic-on-drop")]
        {
            #[cfg(feature = "do-not-panic-when-panicking")]
            // Let user deal with other panics.
            if std::thread::panicking() {
                return;
            }

            panic!("SharedBoxRef should not be dropped. Use SharedBox::try_return to consume it.");
        }
    }
}
