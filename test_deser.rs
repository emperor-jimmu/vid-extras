use serde::{Deserialize, Deserializer};

fn deserialize_url<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct UrlHelper {
        #[serde(default)]
        url: String,
        #[serde(default)]
        webpage_url: String,
    }

    let helper = UrlHelper::deserialize(deserializer)?;
    Ok(if !helper.url.is_empty() {
        helper.url
    } else {
        helper.webpage_url
    })
}

#[derive(Debug, Deserialize)]
struct VideoCandidate {
    #[serde(deserialize_with = "deserialize_url")]
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub duration: f64,
}

fn main() {
    let json = r#"{"url": "https://www.youtube.com/watch?v=test", "webpage_url": "https://www.youtube.com/watch?v=test", "title": "Test Video", "duration": 300.0}"#;
    match serde_json::from_str::<VideoCandidate>(json) {
        Ok(candidate) => println!("Success: {:?}", candidate),
        Err(e) => println!("Error: {}", e),
    }
}
