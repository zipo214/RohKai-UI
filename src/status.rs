//! Session-only transient status message primitive (CB-21).
//! Holds a single short message with an expiry time. Never persisted.

#[derive(Default)]
pub struct StatusMessage {
    text: Option<String>,
    expires_at: f64,
}

/// Default lifetime of a status message in seconds.
pub const STATUS_TTL: f64 = 1.5;

impl StatusMessage {
    /// Show `text`, expiring `STATUS_TTL` seconds after `now`.
    /// `now` is `ctx.input(|i| i.time)`.
    pub fn set(&mut self, text: impl Into<String>, now: f64) {
        self.text = Some(text.into());
        self.expires_at = now + STATUS_TTL;
    }

    /// The current message if one is set and `now` is before its expiry.
    pub fn current(&self, now: f64) -> Option<&str> {
        match &self.text {
            Some(t) if now < self.expires_at => Some(t.as_str()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_then_current_within_ttl() {
        let mut s = StatusMessage::default();
        s.set("Copied Button", 10.0);
        assert_eq!(s.current(10.5), Some("Copied Button"));
    }

    #[test]
    fn expires_after_ttl() {
        let mut s = StatusMessage::default();
        s.set("Pasted 4 widgets", 10.0);
        assert_eq!(s.current(11.0 + STATUS_TTL), None);
    }

    #[test]
    fn newer_message_replaces_older() {
        let mut s = StatusMessage::default();
        s.set("Copied Button", 10.0);
        s.set("Pasted 2 widgets", 10.2);
        assert_eq!(s.current(10.3), Some("Pasted 2 widgets"));
    }

    #[test]
    fn default_has_no_message() {
        let s = StatusMessage::default();
        assert_eq!(s.current(0.0), None);
    }
}
