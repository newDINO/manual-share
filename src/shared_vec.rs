//! # Manually shared vector
//!
//! `SharedVec` is the `Vec`-based counterpart to `SharedBox`.
//! It owns the original allocation and lets you create multiple immutable `SharedVecRef`
//! values that can be sent to other threads while keeping a single shared owner.
//!
//! The API is similar to `SharedBox`:
//! - use `SharedVec::from_vec` to create a shared vector from a `Vec`
//! - use `SharedVec::borrow` to create a `SharedVecRef`
//! - use `SharedVec::try_return` to give a borrowed reference back
//! - use `SharedVec::try_into_vec` to recover the original `Vec` once no references remain
//!
//! ```
//! use std::thread;
//! use manual_share::SharedVec;
//!
//! let values = vec![1, 2, 3];
//! let mut shared = SharedVec::from_vec(values);
//!
//! let shared_ref = shared.borrow();
//! let handle = thread::spawn(move || {
//!     assert_eq!(shared_ref.as_slice(), &[1, 2, 3]);
//!     shared_ref
//! });
//!
//! let shared_ref = shared.borrow();
//! let handle2 = thread::spawn(move || {
//!     assert_eq!(shared_ref.as_slice(), &[1, 2, 3]);
//!     shared_ref
//! });
//!
//! shared.try_return(handle.join().unwrap()).unwrap();
//! shared.try_return(handle2.join().unwrap()).unwrap();
//!
//! let values = shared.try_into_vec().unwrap();
//! assert_eq!(values, vec![1, 2, 3]);
//! ```
//!

/// A structure owning the original `Vec` which can be used to create multiple immutable
/// `SharedVecRef` values to send to other threads.
/// It uses a counter to record how many references have been created and not returned yet.
///
/// Dropping `SharedVec` without returning all `SharedVecRef` values leaks the underlying allocation.
/// When the `panic-on-drop` feature is enabled, it will panic:
/// ```should_panic
/// let r = {
///     let mut values = manual_share::SharedVec::from_vec(vec![0]);
///     values.borrow()
/// };
/// println!("{:?}", r.as_slice());
/// ```
///
/// Once all `SharedVecRef` values have been returned, `SharedVec` can be converted back into
/// a `Vec` and its allocation will be released when dropped:
/// ```
/// let mut values = manual_share::SharedVec::from_vec(vec![0]);
/// let reference = values.borrow();
/// values.try_return(reference).unwrap();
/// let values = values.try_into_vec().unwrap();
/// assert_eq!(values, vec![0]);
/// ```
#[derive(Debug)]
pub struct SharedVec<T> {
    borrow_count: usize,
    ptr: *mut T,
    len: usize,
    cap: usize,
}
impl<T> SharedVec<T> {
    /// Create a `SharedVec` by consuming a `Vec`.
    pub fn from_vec(vec: Vec<T>) -> Self {
        let (ptr, len, cap) = vec.into_raw_parts();
        Self {
            borrow_count: 0,
            ptr,
            len,
            cap,
        }
    }
    /// Create a `SharedVecRef` and increase the borrow count.
    ///
    /// ```
    /// let mut values = manual_share::SharedVec::from_vec(vec![1, 2, 3]);
    /// let reference = values.borrow();
    /// assert_eq!(reference.as_slice(), &[1, 2, 3]);
    /// values.try_return(reference).unwrap();
    /// ```
    ///
    /// # panics
    /// Panics when borrow count overflows `usize`.
    pub fn borrow(&mut self) -> SharedVecRef<T> {
        self.borrow_count = self.borrow_count.checked_add(1).unwrap();
        SharedVecRef {
            ptr: self.ptr,
            len: self.len,
        }
    }
    /// Try to return back a `SharedVecRef`.
    /// Returns `Err` if the `SharedVecRef` does not originate from the same `SharedVec`.
    ///
    /// ```
    /// let mut first = manual_share::SharedVec::from_vec(vec![8]);
    /// let first_ref = first.borrow();
    /// first.try_return(first_ref).unwrap();
    ///
    /// let mut second = manual_share::SharedVec::from_vec(vec![9]);
    /// let second_ref = second.borrow();
    /// let err = first.try_return(second_ref).unwrap_err();
    ///
    /// assert_eq!(err.as_slice(), &[9]);
    /// second.try_return(err).unwrap();
    /// ```
    pub fn try_return(&mut self, reference: SharedVecRef<T>) -> Result<(), SharedVecRef<T>> {
        if !core::ptr::eq(self.ptr, reference.ptr) {
            return Err(reference);
        }

        if size_of::<T>() == 0 {
            if self.len != reference.len {
                return Err(reference);
            }

            if let Some(new_count) = self.borrow_count.checked_sub(1) {
                self.borrow_count = new_count;
                let _ = core::mem::ManuallyDrop::new(reference);
                Ok(())
            } else {
                Err(reference)
            }
        } else {
            self.borrow_count -= 1;
            let _ = core::mem::ManuallyDrop::new(reference);
            Ok(())
        }
    }
    /// Try to convert `Self` into a `Vec` once all borrowed references are returned.
    ///
    /// ```
    /// let mut values = manual_share::SharedVec::from_vec(vec![0]);
    /// let reference = values.borrow();
    ///
    /// // Try to convert to Vec without returning all SharedVecRef returns Err.
    /// let mut values = values.try_into_vec().unwrap_err();
    ///
    /// values.try_return(reference).unwrap();
    /// let values = values.try_into_vec().unwrap();
    ///
    /// assert_eq!(values, vec![0]);
    /// ```
    pub fn try_into_vec(self) -> Result<Vec<T>, Self> {
        if self.borrow_count > 0 {
            Err(self)
        } else {
            let r = core::mem::ManuallyDrop::new(self);
            Ok(unsafe { Vec::from_raw_parts(r.ptr, r.len, r.cap) })
        }
    }
    /// Directly get a slice to the values inside the `SharedVec`.
    /// This use rust built-in lifetime check to ensure the slice is valid as long as the `SharedVec` is alive,
    /// and has no runtime overhead.
    pub fn get(&self) -> &[T] {
        // SAFETY:
        // The pointer is valid as long as the SharedVec is alive.
        // All other references can only get immutable reference.
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

unsafe impl<T: Send> Send for SharedVec<T> {}
unsafe impl<T: Sync> Sync for SharedVec<T> {}

impl<T> Drop for SharedVec<T> {
    fn drop(&mut self) {
        #[cfg(feature = "panic-on-drop")]
        {
            // Let user deal with other panics.
            #[cfg(feature = "do-not-panic-when-panicking")]
            if std::thread::panicking() {
                return;
            }

            if self.borrow_count > 0 {
                panic!("Dropping a SharedVec without giving back all SharedVecRef")
            }
        }
        // Only drops when there are no outstanding SharedBoxRef values to prevent use-after-free.
        if self.borrow_count == 0 {
            unsafe {
                drop(Vec::from_raw_parts(self.ptr, self.len, self.cap));
            }
        }
    }
}

/// A reference to `SharedVec` that can be sent to other threads.
///
/// Dropping a `SharedVecRef` leaks the heap allocation it points to.
/// When the `panic-on-drop` feature is enabled, dropping it will panic:
/// ```should_panic
/// let mut values = manual_share::SharedVec::from_vec(vec![1]);
/// values.borrow();
///
/// // forget SharedVecMut to make sure the panic is not caused by dropping it first.
/// std::mem::forget(values);
///
/// // panic here due to dropping ShareVecRef
/// ```
///
/// Use `SharedVec::try_return` to consume it without causing panic.
/// ```
/// let mut values = manual_share::SharedVec::from_vec(vec![1]);
/// let reference = values.borrow();
/// values.try_return(reference).unwrap();
/// ```
#[derive(Debug)]
pub struct SharedVecRef<T> {
    ptr: *const T,
    len: usize,
}

impl<T> SharedVecRef<T> {
    /// View the referenced data as a slice.
    ///
    /// ```
    /// let mut values = manual_share::SharedVec::from_vec(vec![1, 2, 3]);
    /// let reference = values.borrow();
    /// assert_eq!(reference.as_slice(), &[1, 2, 3]);
    /// values.try_return(reference).unwrap();
    /// ```
    pub fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<T> Drop for SharedVecRef<T> {
    fn drop(&mut self) {
        #[cfg(feature = "panic-on-drop")]
        {
            // Let user deal with other panics.
            #[cfg(feature = "do-not-panic-when-panicking")]
            if std::thread::panicking() {
                return;
            }

            panic!("Dropping a SharedVecRef without returning it to the SharedVec")
        }
    }
}

unsafe impl<T: Sync> Send for SharedVecRef<T> {}
unsafe impl<T: Sync> Sync for SharedVecRef<T> {}

/// A container of a `Vec` allocation that can be split into multiple `SharedVecPart` values.
///
/// This type is useful when a single `Vec` needs to be partitioned into multiple independently
/// owned segments that still refer to the same underlying allocation.
/// ```
/// let mut values = manual_share::SharedVecMut::from_vec(vec![1, 2, 3, 4]);
/// let mut part = values.split_off(2).unwrap();
///
/// let join_handle = std::thread::spawn(|| {
///     part.as_slice_mut().iter_mut().for_each(|v| *v += 1);
///     part
/// });
///
/// let part = join_handle.join().unwrap();
///
/// assert_eq!(part.as_slice(), &[4, 5]);
/// assert!(values.try_unsplit_off(part).is_ok());
/// ```
///
/// Dropping the `SharedVecMut` without returning all `SharedVecPart` values leaks the underlying allocation.
/// When the `panic-on-drop` feature is enabled, it will panic:
/// ```should_panic
/// let r = {
///     let mut values = manual_share::SharedVecMut::from_vec(vec![0]);
///     values.split_off(1).unwrap()
///
///     // panic here because the part was not returned to the original SharedVecMut.
/// };
/// println!("{:?}", r.as_slice());
/// ```
#[derive(Debug)]
pub struct SharedVecMut<T> {
    borrow_count: usize,

    ptr: *mut T,
    len: usize,
    cap: usize,

    remain_start: usize,
    remain_len: usize,
}

impl<T> SharedVecMut<T> {
    /// Create a `SharedVecMut` by consuming a `Vec`.
    pub fn from_vec(vec: Vec<T>) -> Self {
        let (ptr, len, cap) = vec.into_raw_parts();
        Self {
            borrow_count: 0,
            ptr,
            len,
            cap,
            remain_start: 0,
            remain_len: len,
        }
    }
    /// Split off the suffix of the vector starting at `at`.
    /// This method is similar to `bytes::BytesMut::split_off`.
    ///
    /// Returns None when:
    /// 1. `at` is greater than the length of the vector.
    /// 2. `borrow_count` overflows `usize`.
    ///
    /// If successful, the returned part will contain [at, len) and self will contain [0, at).
    ///
    /// Here is an example of splitting a `SharedVecMut` into 3 parts:
    /// ```
    /// let mut values = manual_share::SharedVecMut::from_vec(vec![1, 2, 3]);
    ///
    /// let part1 = values.split_off(2).unwrap();
    /// let part2 = values.split_off(1).unwrap();
    /// let part3 = values.split_off(0).unwrap();
    ///
    /// assert_eq!(part1.as_slice(), &[3]);
    /// assert_eq!(part2.as_slice(), &[2]);
    /// assert_eq!(part3.as_slice(), &[1]);
    ///
    /// values.try_unsplit_off(part3).unwrap();
    /// values.try_unsplit_off(part2).unwrap();
    /// values.try_unsplit_off(part1).unwrap();
    /// ```
    ///
    pub fn split_off(&mut self, at: usize) -> Option<SharedVecPart<T>> {
        if at > self.remain_len {
            return None;
        }
        self.borrow_count = self.borrow_count.checked_add(1)?;

        let last_len = self.remain_len;
        self.remain_len = at;

        Some(SharedVecPart {
            ptr: self.ptr,
            start: self.remain_start + at,
            len: last_len - at,
        })
    }
    /// Split off the prefix of the vector ending at `at`.
    /// This method is similar to `bytes::BytesMut::split_to`.
    ///
    /// Returns None when:
    /// 1. `at` is greater than the length of the vector.
    /// 2. `borrow_count` overflows `usize`.
    ///
    /// If successful, the returned part will contain [0, at) and self will contain [at, len).
    ///
    /// Here is an example of splitting a `SharedVecMut` into 3 parts:
    /// ```
    /// let mut values = manual_share::SharedVecMut::from_vec(vec![1, 2, 3]);
    ///
    /// let part1 = values.split_to(1).unwrap();
    /// let part2 = values.split_to(1).unwrap();
    /// let part3 = values.split_to(1).unwrap();
    ///
    /// assert_eq!(part1.as_slice(), &[1]);
    /// assert_eq!(part2.as_slice(), &[2]);
    /// assert_eq!(part3.as_slice(), &[3]);
    ///
    /// values.try_unsplit_to(part3).unwrap();
    /// values.try_unsplit_to(part2).unwrap();
    /// values.try_unsplit_to(part1).unwrap();
    /// ```
    pub fn split_to(&mut self, at: usize) -> Option<SharedVecPart<T>> {
        if at > self.remain_len {
            return None;
        }
        self.borrow_count = self.borrow_count.checked_add(1)?;

        let last_start = self.remain_start;
        self.remain_start += at;
        self.remain_len -= at;

        Some(SharedVecPart {
            ptr: self.ptr,
            start: last_start,
            len: at,
        })
    }
    /// Try to unsplit a part that was previously split off with `split_off`.
    pub fn try_unsplit_off(&mut self, part: SharedVecPart<T>) -> Result<(), SharedVecPart<T>> {
        if !core::ptr::eq(self.ptr, part.ptr) {
            return Err(part);
        }
        if part.start != self.remain_start + self.remain_len {
            return Err(part);
        }

        self.remain_len += part.len;

        self.consume_part(part)
    }
    /// Try to unsplit a part that was previously split off with `split_to`.
    pub fn try_unsplit_to(&mut self, part: SharedVecPart<T>) -> Result<(), SharedVecPart<T>> {
        if !core::ptr::eq(self.ptr, part.ptr) {
            return Err(part);
        }
        if self.remain_start != part.start + part.len {
            return Err(part);
        }

        self.remain_start = part.start;
        self.remain_len += part.len;

        self.consume_part(part)
    }
    fn consume_part(&mut self, part: SharedVecPart<T>) -> Result<(), SharedVecPart<T>> {
        if size_of::<T>() == 0 {
            // ZST types can have multiple allocations to the same address, so we need to check for overflow.
            if let Some(new_count) = self.borrow_count.checked_sub(1) {
                self.borrow_count = new_count;
                let _ = core::mem::ManuallyDrop::new(part);
                Ok(())
            } else {
                Err(part)
            }
        } else {
            self.borrow_count -= 1;
            let _ = core::mem::ManuallyDrop::new(part);
            Ok(())
        }
    }
    fn can_convert_back(&self) -> bool {
        self.borrow_count == 0
            && self.remain_start == 0
            && if size_of::<T>() == 0 {
                self.remain_len == self.len
            } else {
                true
            }
    }
    /// Try to convert the mutable view back into a `Vec` when no parts remain outstanding.
    pub fn try_into_vec(self) -> Result<Vec<T>, Self> {
        if self.can_convert_back() {
            let r = core::mem::ManuallyDrop::new(self);
            let vec = unsafe { Vec::from_raw_parts(r.ptr, r.remain_len, r.cap) };

            Ok(vec)
        } else {
            Err(self)
        }
    }
    /// Directly get a slice of the remaining part of the `SharedVecMut`.
    /// ```
    /// let mut values = manual_share::SharedVecMut::from_vec(vec![1, 2, 3]);
    ///
    /// let part1 = values.split_to(1).unwrap();
    /// let part2 = values.split_off(1).unwrap();
    ///
    /// assert_eq!(values.as_slice(), &[2]);
    /// assert_eq!(part1.as_slice(), &[1]);
    /// assert_eq!(part2.as_slice(), &[3]);
    ///
    /// values.try_unsplit_off(part2).unwrap();
    /// values.try_unsplit_to(part1).unwrap();
    /// assert_eq!(values.as_slice(), &[1, 2, 3]);
    /// ```
    ///
    /// Further splitting is no longer possible as long as the returned slice is held alive:
    /// ```compile_fail
    /// let mut values = manual_share::SharedVecMut::from_vec(vec![1, 2, 3]);
    /// let slice = values.as_slice();
    /// let part = values.split_off(1).unwrap();
    ///
    /// println!("{:?}", slice);
    /// ```
    pub fn as_slice(&self) -> &[T] {
        // SAFETY:
        // The pointer is valid as long as the SharedVecMut is alive.
        // SharedVecPart cannot point to the same or overlapping region as self.
        // Also, splitting methods can't be called when the returned slice is alive.
        unsafe { core::slice::from_raw_parts(self.ptr.add(self.remain_start), self.remain_len) }
    }
    /// Directly get a mutable slice of the remaining part of the `SharedVecMut`.
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        // SAFETY:
        // The pointer is valid as long as the SharedVecMut is alive.
        // SharedVecPart cannot point to the same or overlapping region as self.
        // Also, splitting methods can't be called when the returned slice is alive.
        unsafe { core::slice::from_raw_parts_mut(self.ptr.add(self.remain_start), self.remain_len) }
    }
}

unsafe impl<T: Send> Send for SharedVecMut<T> {}
unsafe impl<T: Sync> Sync for SharedVecMut<T> {}

impl<T> Drop for SharedVecMut<T> {
    fn drop(&mut self) {
        #[cfg(feature = "panic-on-drop")]
        {
            // Let user deal with other panics.
            #[cfg(feature = "do-not-panic-when-panicking")]
            if std::thread::panicking() {
                return;
            }

            if self.borrow_count > 0 {
                panic!("Dropping a SharedVecMut without giving back all SharedVecRef")
            }
        }

        if self.can_convert_back() {
            unsafe {
                drop(Vec::from_raw_parts(self.ptr, self.len, self.cap));
            }
        }
    }
}

/// A slice-like view into a segment of a `SharedVecMut` allocation.
///
/// It can be read as a slice or mutated in place while the underlying allocation is still owned
/// by the original `SharedVecMut`.
///
/// Dropping a `SharedVecPart` leaks the underlying allocation.
/// When the **`panic-on-drop`** feature is enabled, dropping it will panic:
/// ```should_panic
/// let mut values = manual_share::SharedVecMut::from_vec(vec![1, 2, 3, 4]);
/// let mut part = values.split_off(2).unwrap();
///
/// // forget SharedVecMut to make sure the panic is not caused by dropping it first.
/// std::mem::forget(values);
///
/// // panic here due to dropping ShareVecPart
/// ```
#[derive(Debug)]
pub struct SharedVecPart<T> {
    ptr: *mut T,
    start: usize,
    len: usize,
}

impl<T> SharedVecPart<T> {
    /// View the part as an immutable slice.
    pub fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.ptr.add(self.start), self.len) }
    }
    /// View the part as a mutable slice.
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.add(self.start), self.len) }
    }
}

impl<T> Drop for SharedVecPart<T> {
    fn drop(&mut self) {
        #[cfg(feature = "panic-on-drop")]
        {
            // Let user deal with other panics.
            #[cfg(feature = "do-not-panic-when-panicking")]
            if std::thread::panicking() {
                return;
            }

            panic!("Dropping a SharedVecPart without returning it to the SharedVecMut")
        }
    }
}

unsafe impl<T: Send> Send for SharedVecPart<T> {}
unsafe impl<T: Sync> Sync for SharedVecPart<T> {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn zst() {
        let mut b1: SharedVec<()> = SharedVec::from_vec(Vec::new());
        let mut b2: SharedVec<()> = SharedVec::from_vec(Vec::new());

        let r11 = b1.borrow();
        let r12 = b1.borrow();

        let r2 = b2.borrow();

        b1.try_return(r2).unwrap();

        b1.try_return(r11).unwrap();
        let r12 = b1.try_return(r12).unwrap_err();

        b2.try_return(r12).unwrap();
    }

    #[test]
    fn mut_zst() {
        let mut b1: SharedVecMut<()> = SharedVecMut::from_vec(Vec::new());
        let mut b2: SharedVecMut<()> = SharedVecMut::from_vec(Vec::new());

        let r11 = b1.split_off(0).unwrap();
        let r12 = b1.split_off(0).unwrap();

        let r2 = b2.split_off(0).unwrap();

        b1.try_unsplit_off(r2).unwrap();

        b1.try_unsplit_off(r11).unwrap();
        let r12 = b1.try_unsplit_off(r12).unwrap_err();

        b2.try_unsplit_off(r12).unwrap();
    }
}
