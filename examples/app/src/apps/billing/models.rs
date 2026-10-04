use ruvoraq::prelude::*;

#[derive(Serialize)]
pub struct Receipt {
    pub id: usize,
    pub state: &'static str,
}
