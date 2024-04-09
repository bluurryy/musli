//! [<img alt="github" src="https://img.shields.io/badge/github-udoprog/musli-8da0cb?style=for-the-badge&logo=github" height="20">](https://github.com/udoprog/musli)
//! [<img alt="crates.io" src="https://img.shields.io/crates/v/musli-utils.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/musli-utils)
//! [<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-musli--utils-66c2a5?style=for-the-badge&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/musli-utils)
//!
//! Common utilities shared by [Müsli] encodings.
//!
//! The [Reader] and [Writer] traits are defined in here which determined the
//! types that can be used in collaboration with [Müsli].
//!
//! Please refer to <https://docs.rs/musli> for documentation.
//!
//! [Müsli]: <https://docs.rs/musli>
//! [Reader]: https://docs.rs/musli-utils/latest/musli-utils/reader/trait.Reader.html
//! [Writer]: https://docs.rs/musli-utils/latest/musli-utils/writer/trait.Writer.html

#![deny(missing_docs)]
#![no_std]
#![cfg_attr(doc_cfg, feature(doc_cfg))]

#[cfg_attr(test, macro_use)]
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod context;

pub mod fixed;
#[doc(inline)]
pub use self::fixed::FixedBytes;

#[macro_use]
pub mod options;
#[doc(inline)]
pub use self::options::Options;

pub mod buf;

pub mod reader;
#[doc(inline)]
pub use self::reader::Reader;

pub mod wrap;

pub mod writer;
#[doc(inline)]
pub use self::writer::Writer;

pub mod int;

#[doc(hidden)]
pub mod str;

#[doc(inline)]
pub use musli_allocator as allocator;

#[macro_use]
mod macros;

#[cfg_attr(feature = "std", path = "system/std.rs")]
#[cfg_attr(not(feature = "std"), path = "system/no_std.rs")]
mod system;