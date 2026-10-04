use ruvoraq::prelude::*;

#[derive(Clone, Serialize)]
pub struct Student {
    pub id: u64,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateStudent {
    pub name: String,
}

impl Validate for CreateStudent {
    fn validate(&self) -> Result<()> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(invalid("name", "Name is required"));
        }
        if name.chars().count() > 80 {
            return Err(invalid("name", "Name must be at most 80 characters"));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchStudent {
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct StudentFilter {
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub min_id: Option<u64>,
}

fn default_limit() -> usize {
    10
}
