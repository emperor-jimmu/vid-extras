use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub event: String,
    pub current: usize,
    pub total: usize,
    pub title: String,
    pub year: Option<u16>,
    pub phase: Option<String>,
    pub downloads: Option<usize>,
    pub conversions: Option<usize>,
    pub discovered: Option<usize>,
    pub success: Option<bool>,
    pub error: Option<String>,
}

impl ProgressEvent {
    pub fn new(
        event: &str,
        current: usize,
        total: usize,
        title: String,
        year: Option<u16>,
    ) -> Self {
        Self {
            event: event.to_string(),
            current,
            total,
            title,
            year,
            phase: None,
            downloads: None,
            conversions: None,
            discovered: None,
            success: None,
            error: None,
        }
    }

    pub fn emit(&self) {
        println!("{}", serde_json::to_string(self).unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_event_serialization() {
        let event = ProgressEvent::new("started", 1, 10, "The Matrix".to_string(), Some(1999));
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"started\""));
        assert!(json.contains("\"current\":1"));
        assert!(json.contains("\"total\":10"));
        assert!(json.contains("\"title\":\"The Matrix\""));
        assert!(json.contains("\"year\":1999"));
    }

    #[test]
    fn test_progress_event_with_phase() {
        let mut event = ProgressEvent::new("started", 1, 10, "The Matrix".to_string(), Some(1999));
        event.phase = Some("discovery".to_string());
        event.discovered = Some(15);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"phase\":\"discovery\""));
        assert!(json.contains("\"discovered\":15"));
    }

    #[test]
    fn test_progress_event_completed() {
        let mut event =
            ProgressEvent::new("completed", 1, 10, "The Matrix".to_string(), Some(1999));
        event.success = Some(true);
        event.downloads = Some(5);
        event.conversions = Some(4);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"downloads\":5"));
        assert!(json.contains("\"conversions\":4"));
    }

    #[test]
    fn test_progress_event_with_error() {
        let mut event =
            ProgressEvent::new("completed", 1, 10, "The Matrix".to_string(), Some(1999));
        event.success = Some(false);
        event.error = Some("Network timeout".to_string());
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"Network timeout\""));
    }
}
