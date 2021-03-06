// Copyright 2020 Hyperledger Ursa Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! The errors that are generated by this crate
//!
//! Uses a kind enum for the error type

use std::{
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
};

/// A specialized [`Result`] type for Sharing operations.
pub type SharingResult<T> = Result<T, SharingError>;

/// The error type for Sharing operations.
#[derive(Copy, Clone, Debug)]
pub enum SharingError {
    /// Deserializing less than 4 bytes for a shamir share
    ShareSecretMinSize,
    /// Secret sharing limit param is less than threshold param
    ShareLimitLessThanThreshold,
    /// Secret sharing threshold param is less than 2
    ShareMinThreshold,
    /// Secret sharing value to be split is not within the field
    ShareInvalidSecret,
    /// Secret share identifier is bad
    ShareInvalidIdentifier,
    /// More than one secret share identifier is duplicated when recombining
    ShareDuplicateIdentifier,
    /// The secret share value is corrupted or invalid
    ShareInvalidValue,
    /// Deserializing less than the minimum size for a pedersen verifier
    PedersenVerifierMinSize(usize, usize),
    /// The blinding factor share value is corrupted or invalid
    PedersenBlindShareInvalid,
    /// Deserializing an invalid ECC point
    InvalidPoint,
}

impl Display for SharingError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        use SharingError::*;

        match *self {
            ShareSecretMinSize => write!(f, "ShamirShare requires at least 4 bytes"),
            ShareLimitLessThanThreshold => write!(f, "Limit cannot be less than the threshold"),
            ShareMinThreshold => write!(f, "Threshold must be at least 2"),
            ShareInvalidSecret => write!(f, "Can't split secret"),
            ShareInvalidIdentifier => write!(f, "Share must have a non-zero identifier"),
            ShareDuplicateIdentifier => write!(
                f,
                "Duplicate shares cannot be used to reconstruct the secret"
            ),
            ShareInvalidValue => write!(f, "Share is not valid"),
            PedersenVerifierMinSize(expected, found) => write!(
                f,
                "Minimum length not satisfied: expected {}, found {}",
                expected, found
            ),
            PedersenBlindShareInvalid => write!(f, "Blind share is not valid"),
            InvalidPoint => write!(f, "Invalid curve point"),
        }
    }
}

impl Error for SharingError {}
