use ruvoraq::prelude::*;

#[schema]
#[derive(Serialize)]
pub struct Receipt {
    pub id: usize,
    pub state: &'static str,
}
