//! Domain layer for weedforge.
//!
//! This module contains pure domain logic with no infrastructure dependencies.
//! It defines:
//!
//! - [`FileId`]: The file identifier entity
//! - [`DomainError`]: Domain-level errors
//! - Port traits ([`MasterPort`], [`VolumePort`]): Interfaces for infrastructure

pub mod entities;
pub mod errors;
pub mod ports;

// Re-export main types
pub use entities::FileId;
pub use errors::{DomainError, DomainResult};
pub use ports::{
    AssignOptions, AssignResult, LookupResult, MasterPort, UploadResult, VolumeLocation, VolumePort,
};
