use ruvoraq::prelude::*;

use super::{models::Receipt, services::BillingService};

#[get("/billing")]
async fn summary(service: Inject<BillingService>) -> Value {
    json!({"accepted": service.count()})
}

#[post("/billing/requests")]
async fn accept_request(service: Inject<BillingService>) -> Reply<Receipt> {
    accepted(service.accept())
}

#[get("/billing/health")]
async fn health() -> Reply<Value> {
    ok(json!({"status": "ready"}))
}
