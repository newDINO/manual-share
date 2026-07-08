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
//! so the ptr equallity check of returning methods such as [`SharedBox::try_return`]
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
//! User should be aware of the two difference of ZST compared to non-ZSTs:
//! 1. [`SharedBoxRef`] created by one [`SharedBox`] can be returned to a different [`SharedBox`],
//!    which is not allowed for non-ZSTs.
//! 2. Giving back more [`SharedBoxRef`] than what the original [`SharedBox`] has created will return an `Err` containing the [`SharedBoxRef`].
//!
//! The above behavior is also applied to [`SharedVec`] and [`SharedVecMut`].
//!
//! The above model for handling ZST originated from discussions in
//! [a post in Rust user forum](https://users.rust-lang.org/t/built-a-crate-to-safely-share-box-and-vec-manually/141138/5).

pub mod shared_box;
pub mod shared_vec;
pub use shared_box::*;
pub use shared_vec::*;
