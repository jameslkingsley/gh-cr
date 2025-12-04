use chrono::{DateTime, Utc};

pub trait RenderableComment {
    fn author(&self) -> &str;
    fn created_at(&self) -> DateTime<Utc>;
    fn body(&self) -> &str;
}
