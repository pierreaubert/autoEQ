// ============================================================================
// Helper Functions
// ============================================================================

use std::path::PathBuf;

use super::errors::{CamillaError, CamillaResult};

/// Find the CamillaDSP binary in the system PATH or bundled location
pub fn find_camilladsp_binary() -> CamillaResult<PathBuf> {
    // Try bundled binary first (Tauri sidecar)
    // In production, the sidecar is in the same directory as the executable
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        let bundled_name = if cfg!(windows) {
            "camilladsp.exe"
        } else {
            "camilladsp"
        };

        let bundled_path = exe_dir.join(bundled_name);
        if bundled_path.exists() {
            println!("[CamillaDSP] Using bundled binary: {:?}", bundled_path);
            return Ok(bundled_path);
        }

        // Check for Tauri sidecar in triplet subdirectory (e.g., camilladsp-aarch64-apple-darwin/camilladsp)
        let triplet = if cfg!(target_os = "windows") {
            if cfg!(target_arch = "x86_64") {
                "x86_64-pc-windows-msvc"
            } else {
                "aarch64-pc-windows-msvc"
            }
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "aarch64-apple-darwin"
            } else {
                "x86_64-apple-darwin"
            }
        } else if cfg!(target_os = "linux") {
            if cfg!(target_arch = "aarch64") {
                "aarch64-unknown-linux-gnu"
            } else {
                "x86_64-unknown-linux-gnu"
            }
        } else {
            ""
        };

        if !triplet.is_empty() {
            let bundled_path_triplet = exe_dir
                .join(format!("camilladsp-{}", triplet))
                .join(bundled_name);
            if bundled_path_triplet.exists() {
                println!(
                    "[CamillaDSP] Using bundled sidecar binary: {:?}",
                    bundled_path_triplet
                );
                return Ok(bundled_path_triplet);
            }
        }
    }

    // Try to find in PATH
    if let Ok(path) = which::which("camilladsp") {
        println!("[CamillaDSP] Using system binary from PATH: {:?}", path);
        return Ok(path);
    }

    // Check common installation locations
    let common_paths = vec![
        PathBuf::from("/usr/local/bin/camilladsp"),
        PathBuf::from("/usr/bin/camilladsp"),
        PathBuf::from("/opt/homebrew/bin/camilladsp"),
    ];

    for path in common_paths {
        if path.exists() {
            println!("[CamillaDSP] Using system binary: {:?}", path);
            return Ok(path);
        }
    }

    Err(CamillaError::BinaryNotFound(
        "CamillaDSP binary not found. Looked for:\n\
         1. Bundled binary (next to executable)\n\
         2. System PATH\n\
         3. Common locations (/usr/local/bin, /usr/bin, /opt/homebrew/bin)\n\
         \n\
         Please install CamillaDSP from https://github.com/HEnquist/camilladsp"
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_filter_params_validation() {
        // Valid filter
        let filter = crate::filters::FilterParams::new(1000.0, 1.0, 3.0);
        assert!(filter.validate().is_ok());

        // Invalid frequency
        let filter = crate::filters::FilterParams::new(0.0, 1.0, 3.0);
        assert!(filter.validate().is_err());

        // Invalid Q
        let filter = crate::filters::FilterParams::new(1000.0, 0.0, 3.0);
        assert!(filter.validate().is_err());
    }

    #[test]
    fn test_loudness_compensation_validation() {
        // Valid loudness compensation
        let lc = crate::loudness_compensation::LoudnessCompensation {
            low_boost: 5.0,
            high_boost: 3.0,
            reference_level: -20.0,
            attenuate_mid: true,
        };
        assert!(lc.validate().is_ok());

        // Invalid low boost
        let lc = crate::loudness_compensation::LoudnessCompensation {
            low_boost: 25.0,
            high_boost: 3.0,
            reference_level: -20.0,
            attenuate_mid: true,
        };
        assert!(lc.validate().is_err());

        // Invalid reference level
        let lc = crate::loudness_compensation::LoudnessCompensation {
            low_boost: 5.0,
            high_boost: 3.0,
            reference_level: 30.0,
            attenuate_mid: true,
        };
        assert!(lc.validate().is_err());
    }
}
