//! This crate provides a set of types to allow better sharing of the original std types like `Box` and `Vec`:
//!
//! | original type | share owner | reference type |
//! | --- | --- | --- |
//! | [`Box`] | [`SharedBox`] | [`SharedBoxRef`] |
//! | [`Vec`] | [`SharedVec`] | [`SharedVecRef`] |
//! | [`Vec`] | [`SharedVecMut`] | [`SharedVecPart`] |
//!
//! # Before using
//! One should consider using built-in types like [`std::sync::Arc`], [`std::rc::Rc`],
//! or other smart containers like `bytes::Bytes` or `arc-slice::ArcSlice` (which provide similar functionality of [`SharedVec`] and [`SharedVecMut`]).
//! These types use automatic resource management and are often easier to use.
//!
//! Also, sometimes rust built-in borrow checker is enough,
//! for example, [`std::thread::scope`] can be used to shared values to other threads
//! using vallina references without any runtime overhead.
//!
//! This crate use a "malloc-free" style manual resource management.
//! The borrowed **reference types** must be returned to the **share owner**,
//! otherwise a leak will occur,
//! and the **share owner** can never be converted back to the **original type** (use reference counting to enforce this).
//!
//! Users can enable **`panic-on-drop`** feature, so that whenever a leak happens, the thread panics to help users find the problem.
//!
//! # Pros
//! 1. Conversion between the original type and the shared type is copy free,
//!    while conversion between [`std::sync::Arc`] and [`Box`] involves new allocation and copy.
//! 2. No atomic Read-Modify-Write overhead, because only one thread can modify the reference counting.
//! 3. Despite the need to sent back the reference to the owner,
//!    in many cases, a `JoinHandle` is provide to easily do that.
//!
//!
//! # Feature flags
//! - **`panic-on-drop`** (_enabled by default_)
//!   When enabled, dropping **reference types** will panic,
//!   dropping **share owners** will panic when there are references not returned to it.
//! - **`do-not-panic-when-panicking`** (_enabled by default_)
//!   Prevent panic-on-drop when the thread is already panicking.
//!   This can prevent too much unrelated information being printed out when the thread is panicking due to other reasons.
//!
//!
//! # Special ZST handling
//! ZST (Zero-Sized Types) are types that have no size, such as `()` or `struct {}`.
//! ZST allocated on heap can have the same address,
//! so the ptr equality check of returning methods such as [`SharedBox::try_return`]
//! can't tell whether the [`SharedBoxRef`] has the same origin,
//! allowing [`SharedBoxRef`] created by one [`SharedBox`] to be returned to a different [`SharedBox`].
//!
//! This is totally fine in the sense that ZST has no data and do points to the same address.
//! However, this allows returning more [`SharedBoxRef`] than what the original [`SharedBox`] has created,
//! which will underflow the borrow count.
//!
//! So an underflow check is added for ZST,
//! giving back more [`SharedBoxRef`] will return an `Err` containing the [`SharedBoxRef`].
//!
//! User should be aware of the difference behavior of ZST compared to non-ZSTs:
//! 1. [`SharedBoxRef`] created by one [`SharedBox`] can be returned to a different [`SharedBox`],
//!    which is not allowed for non-ZSTs.
//!    However, for DST (Dynamically Sized Type), like `[T]` or `dyn Trait`, ptr equality also checks its metadata.
//!    So ZSTs with different metadata still can't be return to the same [`SharedBox`].
//! 2. Giving back more [`SharedBoxRef`] than what the original [`SharedBox`] has created will return an `Err` containing the [`SharedBoxRef`].
//! 3. For ZST [`SharedVec`], there is an additional length check,
//!    so [`SharedVecRef`] with different length can't be returned to the same [`SharedVec`].
//! 4. For ZST [`SharedVecMut`], returning [`SharedVecPart`] originated from other [`SharedVecMut`]
//!    can cause it to be in a state where its counter is 0,
//!    but doesn't have the same length as the original [`Vec`].
//!    So, a additional length check is added so that [`SharedVecMut`] of a different length
//!    can't be converted back to the original [`Vec`].
//!
//! The above model for handling ZST originated from discussions in
//! [a post in Rust user forum](https://users.rust-lang.org/t/built-a-crate-to-safely-share-box-and-vec-manually).

mod shared_box;
pub use shared_box::{SharedBox, SharedBoxRef};
mod shared_vec;
pub use shared_vec::{SharedVec, SharedVecMut, SharedVecPart, SharedVecRef};
mod scope;
pub use scope::{Scope, VecShareMut};
