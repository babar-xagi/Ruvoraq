use ruvoraq::prelude::*;

#[schema]
#[derive(Clone, Serialize)]
pub struct Student {
    pub id: u64,
    pub name: String,
}

#[schema]
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

#[schema]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchStudent {
    pub name: Option<String>,
}

#[schema]
#[derive(Deserialize)]
pub struct StudentFilter {
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub min_id: Option<u64>,
}

fn default_limit() -> usize {
    10
}
