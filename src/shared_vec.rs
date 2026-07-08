//! # Manually shared vector

/// A structure owning the original `Vec`,
/// it can be used to create immutable `SharedVecRef` which can be sent to other threads.
///
/// ```
/// use std::thread;
/// use manual_share::SharedVec;
///
/// let v = vec![1, 2, 3];
/// let mut sv = SharedVec::from_vec(v);
///
/// let sv_ref = sv.borrow();
/// let j1 = thread::spawn(move || {
///    println!("{:?}", sv_ref.as_slice());
///    sv_ref
/// });
/// let sv_ref = sv.borrow();
/// let j2 = thread::spawn(move || {
///    println!("{:?}", sv_ref.as_slice());
///    sv_ref
/// });
///
/// sv.try_return(j1.join().unwrap()).unwrap();
/// sv.try_return(j2.join().unwrap()).unwrap();
///
/// let v = sv.try_into_vec().unwrap();
/// println!("{:?}", v);
/// ```
#[derive(Debug)]
pub struct SharedVec<T> {
    borrow_count: usize,
    ptr: *mut T,
    len: usize,
    cap: usize,
}
impl<T> SharedVec<T> {
    pub fn from_vec(vec: Vec<T>) -> Self {
        let (ptr, len, cap) = vec.into_raw_parts();
        Self {
            borrow_count: 0,
            ptr,
            len,
            cap,
        }
    }
    pub fn borrow(&mut self) -> SharedVecRef<T> {
        self.borrow_count += 1;
        SharedVecRef {
            ptr: self.ptr,
            len: self.len,
        }
    }
    pub fn try_return(&mut self, reference: SharedVecRef<T>) -> Result<(), SharedVecRef<T>> {
        if !core::ptr::eq(self.ptr, reference.ptr) {
            return Err(reference);
        }

        self.borrow_count -= 1;
        let _ = core::mem::ManuallyDrop::new(reference);
        Ok(())
    }
    pub fn try_into_vec(self) -> Result<Vec<T>, Self> {
        if self.borrow_count > 0 {
            Err(self)
        } else {
            let r = core::mem::ManuallyDrop::new(self);
            Ok(unsafe { Vec::from_raw_parts(r.ptr, r.len, r.cap) })
        }
    }
}

unsafe impl<T: Send> Send for SharedVec<T> {}

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
        unsafe {
            drop(Vec::from_raw_parts(self.ptr, self.len, self.cap));
        }
    }
}

#[derive(Debug)]
pub struct SharedVecRef<T> {
    ptr: *const T,
    len: usize,
}

impl<T> SharedVecRef<T> {
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

pub struct SharedVecMut<T> {
    borrow_count: usize,

    ptr: *mut T,
    cap: usize,

    start: usize,
    len: usize,
}

impl<T> SharedVecMut<T> {
    pub fn from_vec(vec: Vec<T>) -> Self {
        let (ptr, len, cap) = vec.into_raw_parts();
        Self {
            borrow_count: 0,
            ptr,
            cap,
            start: 0,
            len,
        }
    }
    pub fn split_off(&mut self, at: usize) -> Option<SharedVecPart<T>> {
        if at > self.len {
            return None;
        }
        self.borrow_count += 1;

        let last_len = self.len;
        self.len = at;

        Some(SharedVecPart {
            ptr: self.ptr,
            start: self.start + at,
            len: last_len - at,
        })
    }
    pub fn split_to(&mut self, at: usize) -> Option<SharedVecPart<T>> {
        if at > self.len {
            return None;
        }
        self.borrow_count += 1;

        let last_start = self.start;
        self.start += at;
        self.len -= at;

        Some(SharedVecPart {
            ptr: self.ptr,
            start: last_start,
            len: at,
        })
    }
    pub fn try_unsplit_off(&mut self, part: SharedVecPart<T>) -> Result<(), SharedVecPart<T>> {
        if !core::ptr::eq(self.ptr, part.ptr) {
            return Err(part);
        }
        if part.start != self.start + self.len {
            return Err(part);
        }

        self.borrow_count -= 1;
        self.len += part.len;

        let _ = core::mem::ManuallyDrop::new(part);

        Ok(())
    }
    pub fn try_unsplit_to(&mut self, part: SharedVecPart<T>) -> Result<(), SharedVecPart<T>> {
        if !core::ptr::eq(self.ptr, part.ptr) {
            return Err(part);
        }
        if self.start != part.start + part.len {
            return Err(part);
        }

        self.borrow_count -= 1;
        self.start = part.start;
        self.len += part.len;

        let _ = core::mem::ManuallyDrop::new(part);

        Ok(())
    }
    pub fn try_into_vec(self) -> Result<Vec<T>, Self> {
        if self.borrow_count > 0 {
            return Err(self);
        }

        if self.start != 0 {
            return Err(self);
        }

        let r = core::mem::ManuallyDrop::new(self);
        let vec = unsafe { Vec::from_raw_parts(r.ptr, r.len, r.cap) };

        Ok(vec)
    }
}

unsafe impl<T: Send> Send for SharedVecMut<T> {}

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
        unsafe {
            drop(Vec::from_raw_parts(self.ptr, self.len, self.cap));
        }
    }
}

pub struct SharedVecPart<T> {
    ptr: *mut T,
    start: usize,
    len: usize,
}

impl<T> SharedVecPart<T> {
    pub fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.ptr.add(self.start), self.len) }
    }
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
