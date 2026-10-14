//! Errors reported by native Markdown analysis.
use crate::unist::{Point, Position};
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    Point(Point),
    Position(Position),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub place: Option<Box<Place>>,
    pub reason: String,
    pub rule_id: Box<String>,
    pub source: Box<String>,
}
impl Message {
    pub(crate) fn unsupported(reason: &str) -> Self {
        Self {
            place: None,
            reason: reason.to_owned(),
            rule_id: Box::new("unsupported-syntax".into()),
            source: Box::new("pagemd".into()),
        }
    }
}
impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}
impl std::error::Error for Message {}
