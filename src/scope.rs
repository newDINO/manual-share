//! A convenient trait built on existing API.

use crate::{SharedBox, SharedVec, SharedVecMut};

/// # Examples:
/// ```
/// use manual_share::Scope;
///
/// let mut v = vec![1];
///
/// v.scope(|v| {
///     let v_ref = v.borrow();
///     assert_eq!(v[0], 1);
///     v.try_return(v_ref).unwrap();
/// });
/// ```
///
/// Using tuples:
/// ```
/// use manual_share::{Scope, VecShareMut};
///
/// let mut v1 = vec![1];
/// let mut v2 = vec![2];
/// let mut v3 = vec![3];
///
/// (&mut v1, &mut v2, &mut VecShareMut(&mut v3)).scope(|(v1, v2, v3)| {
///     let v1_ref = v1.borrow();
///     let v2_ref = v2.borrow();
///     let mut v3_ref = v3.split_off(0).unwrap();
///
///     v1_ref.iter().zip(v2_ref.iter()).zip(v3_ref.iter_mut()).for_each(|((x1, x2), x3)| *x3 = x1 + x2);
///
///     v1.try_return(v1_ref).unwrap();
///     v2.try_return(v2_ref).unwrap();
///     v3.try_unsplit_off(v3_ref).unwrap();
/// });
///
/// assert_eq!(v3, [3]);
///
/// ```
pub trait Scope {
    type Shared;

    /// Create a Shared representation of Self.
    /// Afterwards, self usually becomes Self::default()
    fn create_shared(&mut self) -> Self::Shared;

    /// Can panic when conversion failed.
    fn set_from_shared(&mut self, shared: Self::Shared);

    /// If `Shared` failed to convert back to `Self`,
    /// `Self` usually becomes `Self::default()`,
    /// and the underlying memory of `Shared` leaks.
    fn scope<T>(&mut self, f: impl FnOnce(&mut Self::Shared) -> T) -> T {
        let mut shared = self.create_shared();

        let ret = f(&mut shared);

        self.set_from_shared(shared);

        ret
    }

    /// If `Shared` failed to convert back to `Self`,
    /// or the future is dropped before [`Self::set_from_shared`] is called,
    /// `Self` usually becomes `Self::default()`,
    /// and the underlying memory of `Shared` may leaks.
    fn scope_async<T>(
        &mut self,
        f: impl AsyncFnOnce(&mut Self::Shared) -> T,
    ) -> impl Future<Output = T> {
        async {
            let mut shared = self.create_shared();

            let ret = f(&mut shared).await;

            self.set_from_shared(shared);

            ret
        }
    }
}

impl<T> Scope for Vec<T> {
    type Shared = SharedVec<T>;
    fn create_shared(&mut self) -> Self::Shared {
        let v = core::mem::take(self);
        SharedVec::from_vec(v)
    }
    fn set_from_shared(&mut self, shared: Self::Shared) {
        *self = if let Ok(v) = shared.try_into_vec() {
            v
        } else {
            panic!("Failed to convert SharedVec to Vec");
        }
    }
}

impl<T: Default + ?Sized> Scope for Box<T> {
    type Shared = SharedBox<T>;
    fn create_shared(&mut self) -> Self::Shared {
        let b = core::mem::take(self);
        SharedBox::from_box(b)
    }
    fn set_from_shared(&mut self, shared: Self::Shared) {
        *self = if let Ok(b) = shared.try_into_box() {
            b
        } else {
            panic!("Failed to convert SharedBox to Box");
        }
    }
}

pub struct VecShareMut<'a, T>(pub &'a mut Vec<T>);

impl<'a, T> Scope for VecShareMut<'a, T> {
    type Shared = SharedVecMut<T>;
    fn create_shared(&mut self) -> Self::Shared {
        let v = core::mem::take(self.0);
        SharedVecMut::from_vec(v)
    }
    fn set_from_shared(&mut self, shared: Self::Shared) {
        *self.0 = if let Ok(v) = shared.try_into_vec() {
            v
        } else {
            panic!("Failed to convert SharedVecMut to Vec");
        };
    }
}

macro_rules! impl_for_tuple {
    (
        $($types:ident),*
        ;
        $($numbers:tt),*
    ) => {
        impl< $( $types: Scope ),* > Scope for ( $(&mut $types),* ,) {
            type Shared = ( $( $types::Shared ),* ,);
            fn create_shared(&mut self) -> Self::Shared {
                ($( self.$numbers.create_shared() ),* ,)
            }
            fn set_from_shared(&mut self, shared: Self::Shared) {
                $(
                    self.$numbers.set_from_shared(shared.$numbers)
                );*;
            }
        }
    };
}

impl_for_tuple!(T1; 0);
impl_for_tuple!(T1, T2; 0, 1);
impl_for_tuple!(T1, T2, T3; 0, 1, 2);
impl_for_tuple!(T1, T2, T3, T4; 0, 1, 2, 3);
impl_for_tuple!(T1, T2, T3, T4, T5; 0, 1, 2, 3, 4);
impl_for_tuple!(T1, T2, T3, T4, T5, T6; 0, 1, 2, 3, 4, 5);
impl_for_tuple!(T1, T2, T3, T4, T5, T6, T7; 0, 1, 2, 3, 4, 5, 6);
impl_for_tuple!(T1, T2, T3, T4, T5, T6, T7, T8; 0, 1, 2, 3, 4, 5, 6, 7);
