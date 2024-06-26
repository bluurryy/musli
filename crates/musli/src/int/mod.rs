//! Traits and utilities for dealing with integers.
//!
//! In particular the [Signed] and [Unsigned] traits are defined in here which
//! allows for musli to work over signed and unsigned numbers generically.
//!
//! We also have a 7-bit [`continuation`] encoding, and [`zigzag`] encoding which
//! are based on these.

#![cfg(any(
    feature = "storage",
    feature = "wire",
    feature = "descriptive",
    feature = "value"
))]

pub(crate) mod continuation;
mod encoding;
mod traits;
pub(crate) mod zigzag;

pub(crate) use self::encoding::{
    decode_signed, decode_unsigned, decode_usize, encode_signed, encode_unsigned, encode_usize,
};
pub(crate) use self::traits::{Signed, Unsigned, UnsignedOps};

#[cfg(test)]
mod tests;
