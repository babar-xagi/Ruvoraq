use ruvoraq::prelude::*;

use super::{models::Receipt, services::BillingService};

#[get("/billing")]
async fn summary(service: Inject<BillingService>) -> Value {
    json!({"accepted": service.count()})
}

#[post("/billing/requests", status = 202)]
async fn accept_request(service: Inject<BillingService>) -> Reply<Receipt> {
    accepted(service.accept())
}

#[get("/billing/health", status = 200)]
async fn health() -> Reply<Value> {
    ok(json!({"status": "ready"}))
}
