//! | original type | share owner | reference type |
//! | --- | --- | --- |
//! | `Box` | `SharedBox` | `SharedBoxRef` |
//! | `Vec` | `SharedVec` | `SharedVecRef` |
//! | `Vec` | `SharedVecMut` | `SharedVecPart` |
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
