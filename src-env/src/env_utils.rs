//! Environment variable utilities for AutoEQ
//!
//! This module provides utilities for handling environment variables,
//! particularly the AUTOEQ_DIR variable that points to the AutoEQ project root.

use crate::constants::DATA_GENERATED;
use std::env;
use std::path::PathBuf;

/// Error type for environment variable issues
#[derive(Debug, thiserror::Error)]
pub enum EnvError {
    #[error(
        "AUTOEQ_DIR environment variable is not set. Please set it to the AutoEQ project root directory (e.g., export AUTOEQ_DIR=/path/to/autoeq)"
    )]
    AutoeqDirNotSet,

    #[error("AUTOEQ_DIR points to a non-existent directory: {0}")]
    AutoeqDirNotFound(PathBuf),

    #[error("Failed to create data_generated directory: {0}")]
    DataGeneratedCreationFailed(std::io::Error),
}

/// Get the AUTOEQ_DIR environment variable and validate it exists
///
/// # Returns
///
/// Returns the path to the AutoEQ project root directory.
///
/// # Errors
///
/// Returns an error if:
/// - AUTOEQ_DIR is not set
/// - AUTOEQ_DIR points to a non-existent directory
///
/// # Example
///
/// ```no_run
/// use autoeq_env::env_utils::get_autoeq_dir;
///
/// let autoeq_dir = get_autoeq_dir()?;
/// println!("AutoEQ directory: {}", autoeq_dir.display());
/// # Ok::<(), autoeq_env::env_utils::EnvError>(())
/// ```
pub fn get_autoeq_dir() -> Result<PathBuf, EnvError> {
    let autoeq_dir = env::var("AUTOEQ_DIR").map_err(|_| EnvError::AutoeqDirNotSet)?;

    let path = PathBuf::from(autoeq_dir);

    if !path.exists() {
        return Err(EnvError::AutoeqDirNotFound(path));
    }

    Ok(path)
}

/// Get the path to the data_generated directory, creating it if necessary
///
/// This function:
/// 1. Gets the AUTOEQ_DIR from environment
/// 2. Constructs the path to data_generated
/// 3. Creates the directory if it doesn't exist
///
/// # Returns
///
/// Returns the path to the data_generated directory.
///
/// # Errors
///
/// Returns an error if:
/// - AUTOEQ_DIR is not set or invalid
/// - Cannot create the data_generated directory
///
/// # Example
///
/// ```no_run
/// use autoeq_env::env_utils::get_data_generated_dir;
///
/// let data_dir = get_data_generated_dir()?;
/// println!("Data directory: {}", data_dir.display());
/// # Ok::<(), autoeq_env::env_utils::EnvError>(())
/// ```
pub fn get_data_generated_dir() -> Result<PathBuf, EnvError> {
    let autoeq_dir = get_autoeq_dir()?;
    let data_generated = autoeq_dir.join(DATA_GENERATED);

    // Create the directory if it doesn't exist
    if !data_generated.exists() {
        std::fs::create_dir_all(&data_generated).map_err(EnvError::DataGeneratedCreationFailed)?;
    }

    Ok(data_generated)
}

/// Get the path to the records subdirectory within data_generated
///
/// This is a convenience function for the common case of writing
/// optimization records.
///
/// # Returns
///
/// Returns the path to the data_generated/records directory.
///
/// # Errors
///
/// Returns an error if:
/// - AUTOEQ_DIR is not set or invalid
/// - Cannot create the directories
///
/// # Example
///
/// ```no_run
/// use autoeq_env::env_utils::get_records_dir;
///
/// let records_dir = get_records_dir()?;
/// println!("Records directory: {}", records_dir.display());
/// # Ok::<(), autoeq_env::env_utils::EnvError>(())
/// ```
pub fn get_records_dir() -> Result<PathBuf, EnvError> {
    let data_generated = get_data_generated_dir()?;
    let records_dir = data_generated.join("records");

    // Create the records directory if it doesn't exist
    if !records_dir.exists() {
        std::fs::create_dir_all(&records_dir).map_err(EnvError::DataGeneratedCreationFailed)?;
    }

    Ok(records_dir)
}

/// Check if AUTOEQ_DIR is properly configured and print helpful information
///
/// This function is useful for diagnostic purposes and can be called
/// at the start of applications to provide clear error messages.
///
/// # Example
///
/// ```no_run
/// use autoeq_env::env_utils::check_autoeq_env;
///
/// // At the start of your application
/// if let Err(e) = check_autoeq_env() {
///     eprintln!("Environment setup error: {}", e);
///     eprintln!("Please set AUTOEQ_DIR to your AutoEQ project root directory.");
///     eprintln!("Example: export AUTOEQ_DIR=/path/to/your/autoeq/project");
///     std::process::exit(1);
/// }
/// ```
pub fn check_autoeq_env() -> Result<(), EnvError> {
    let autoeq_dir = get_autoeq_dir()?;
    let data_generated = get_data_generated_dir()?;

    println!("✓ AUTOEQ_DIR: {}", autoeq_dir.display());
    println!("✓ Data directory: {}", data_generated.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_autoeq_dir_not_set() {
        // Temporarily remove AUTOEQ_DIR if it exists
        let original = env::var("AUTOEQ_DIR").ok();
        env::remove_var("AUTOEQ_DIR");

        let result = get_autoeq_dir();
        assert!(matches!(result, Err(EnvError::AutoeqDirNotSet)));

        // Restore original value if it existed
        if let Some(value) = original {
            env::set_var("AUTOEQ_DIR", value);
        }
    }

    #[test]
    fn test_autoeq_dir_nonexistent() {
        let original = env::var("AUTOEQ_DIR").ok();
        env::set_var("AUTOEQ_DIR", "/this/path/should/not/exist");

        let result = get_autoeq_dir();
        assert!(matches!(result, Err(EnvError::AutoeqDirNotFound(_))));

        // Restore original value
        if let Some(value) = original {
            env::set_var("AUTOEQ_DIR", value);
        } else {
            env::remove_var("AUTOEQ_DIR");
        }
    }
}
