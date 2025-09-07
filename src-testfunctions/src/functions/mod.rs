//! Test function implementations organized by category
//!
//! This module contains all the optimization test functions organized into logical groups:
//! - `unimodal`: Single-optimum functions (bowl-shaped, plate-shaped, etc.)
//! - `multimodal`: Multi-optimum functions (many local minima, valley-shaped, etc.)
//! - `constrained`: Functions with constraints
//! - `composite`: Hybrid and composite functions
//! - `modern`: Recent benchmark functions from CEC and other competitions

pub mod unimodal;
pub mod multimodal;
pub mod constrained;
pub mod composite;
pub mod modern;

// Re-export all functions for easy access
pub use unimodal::*;
pub use multimodal::*;
pub use constrained::*;
pub use composite::*;
pub use modern::*;
