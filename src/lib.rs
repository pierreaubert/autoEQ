//! AutoEQ - A library for audio equalization and filter optimization
//!
//! This library provides functionality for:
//! - Computing audio equalization parameters
//! - Optimizing IIR filters to match target frequency responses
//! - Reading and processing audio measurement data
//! - Generating CEA2034 metrics and preference scores
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

#![warn(missing_docs)]

use ndarray::Array1;

/// A struct to hold frequency and SPL data.
#[derive(Debug, Clone)]
pub struct Curve {
    /// Frequency points in Hz
    pub freq: Array1<f64>,
    /// Sound Pressure Level in dB
    pub spl: Array1<f64>,
}

/// Common CLI argument definitions shared across binaries
pub mod cli;
/// IIR filter implementations and utilities
pub mod iir;
/// Loss functions for optimization
pub mod loss;
/// Optimization algorithms and objective functions
pub mod optim;
/// Plotting and visualization functions
pub mod plot;
/// Data reading and parsing functions
pub mod read;
/// Audio quality scoring functions
pub mod score;

// Re-export commonly used items
pub use cli::*;
pub use iir::*;
pub use loss::{LossType, ScoreLossData};
pub use optim::*;
pub use plot::*;
pub use read::*;
pub use score::*;
