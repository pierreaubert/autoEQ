//! AutoEQ - Automatic Equalization for Speakers and Headphones
//!
//! This crate provides automatic equalization functionality using measurement data
//! from Spinorama.org. It integrates multiple specialized crates:
//! 
//! - `autoeq_iir`: IIR filter implementations
//! - `autoeq_de`: Differential Evolution optimizer  
//! - `autoeq_cea2034`: CEA2034 preference scoring
//! - `autoeq_testfunctions`: Optimization test functions

// Re-export external crate functionality
pub use autoeq_iir as iir;
pub use autoeq_de as de;
pub use autoeq_cea2034 as cea2034;

// Re-export Curve from CEA2034 crate to ensure type compatibility
pub use autoeq_cea2034::Curve;

/// Common CLI argument definitions shared across binaries
pub mod cli;
/// Constants used throughout the library
pub mod constants;
/// Constraint functions for optimization
pub mod constraints;
/// Loss functions for optimization
pub mod loss;
/// Optimization algorithms and objective functions
pub mod optim;
/// Plotting and visualization functions
pub mod plot;
/// Data reading and parsing functions
pub mod read;
/// Shared workflow steps used by binaries
pub mod workflow;

// Re-export commonly used items
pub use cli::*;
pub use loss::{LossType, ScoreLossData};
pub use optim::*;
pub use plot::*;
pub use read::*;
pub use workflow::*;
