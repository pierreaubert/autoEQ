use std::path::PathBuf;
use std::{error::Error, io};

use autoeq::read;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::fs;

const BASE_URL: &str = "https://api.spinorama.org";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::new();

    let speakers: Vec<String> = fetch_json(&client, &format!("{}/v1/speakers", BASE_URL)).await?;
    println!("Found {} speakers", speakers.len());

    for speaker in speakers {
        if let Err(e) = process_speaker(&client, &speaker).await {
            eprintln!("[WARN] Skipping speaker '{}': {}", speaker, e);
        }
    }

    Ok(())
}

async fn process_speaker(client: &Client, speaker: &str) -> Result<(), Box<dyn Error>> {
    let enc_speaker = urlencoding::encode(speaker);

    // 1. versions
    let versions_url = format!("{}/v1/speaker/{}/versions", BASE_URL, enc_speaker);
    let versions: Vec<String> = fetch_json(client, &versions_url).await?;
    if versions.is_empty() {
        return Ok(());
    }
    let version = &versions[0];

    // 2. measurements list for the first version
    let enc_version = urlencoding::encode(version);
    let measurements_url = format!(
        "{}/v1/speaker/{}/version/{}/measurements",
        BASE_URL, enc_speaker, enc_version
    );
    let measurements: Vec<String> = fetch_json(client, &measurements_url).await?;

    // 3. if CEA2034 present, download CEA2034 and Estimated In-Room Response and metadata
    if measurements.iter().any(|m| m == "CEA2034") {
        let dir = read::data_dir_for(speaker);
        fs::create_dir_all(&dir).await?;

        // metadata
        let metadata_url = format!("{}/v1/speaker/{}/metadata", BASE_URL, enc_speaker);
        let metadata: Value = fetch_json(client, &metadata_url).await?;
        write_json(&dir.join("metadata.json"), &metadata).await?;

        // Measurements: leverage shared cache-aware fetcher, which also saves to disk
        let _ = read::fetch_measurement_plot_data(speaker, version, "CEA2034").await?;
        let _ = read::fetch_measurement_plot_data(speaker, version, "Estimated In-Room Response")
            .await?;

        println!(
            "Saved CEA2034, Estimated In-Room Response and metadata for '{}' (version '{}')",
            speaker, version
        );
    }

    Ok(())
}

async fn fetch_json<T: DeserializeOwned>(client: &Client, url: &str) -> Result<T, Box<dyn Error>> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        let err = io::Error::new(
            io::ErrorKind::Other,
            format!("HTTP {} for {}", resp.status(), url),
        );
        return Err(Box::new(err));
    }
    let val = resp.json::<T>().await?;
    Ok(val)
}

async fn write_json(path: &PathBuf, value: &Value) -> Result<(), Box<dyn Error>> {
    let data = serde_json::to_vec_pretty(value)?;
    fs::write(path, data).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use autoeq_env::DATA_CACHED;
    use autoeq::read;

    #[test]
    fn sanitize_replaces_forbidden() {
        assert_eq!(read::sanitize_dir_name("A/B\\C|D?E*F: G"), "A_B_C_D_E_F_ G");
    }

    #[test]
    fn data_dir_builds_expected_path() {
        let p = read::data_dir_for("KEF LS50 Meta");
        let expected = std::path::Path::new(DATA_CACHED).join("KEF LS50 Meta");
        assert!(p.ends_with(expected));
    }
}
