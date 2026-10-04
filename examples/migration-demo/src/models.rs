use ruvoraq::prelude::*;

#[schema]
#[derive(Serialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
}

impl From<(i64, String)> for Task {
    fn from((id, title): (i64, String)) -> Self {
        Self { id, title }
    }
}

#[schema]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTask {
    pub title: String,
}

impl Validate for CreateTask {
    fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() || self.title.trim().chars().count() > 120 {
            return Err(invalid("title", "Title must contain 1–120 characters"));
        }
        Ok(())
    }
}
