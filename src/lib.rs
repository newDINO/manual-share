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

pub mod shared_box;
pub mod shared_vec;
pub use shared_box::*;
pub use shared_vec::*;
