//! Vehicles, versioned mileage rates, trips, the ledger postings behind claims, reports and exports.

mod common;

use axum::http::{StatusCode, header};
use common::{TestApp, TestUser};
use serde_json::{Value, json};
use uuid::Uuid;

/// A business (not GST-registered) with a vehicle whose rate is $1.04/km from 1 April 2025 and
/// $1.17/km from 1 April 2026.
struct Fleet {
    app: TestApp,
    user: TestUser,
    business: Uuid,
    vehicle: String,
    rate: String,
}

impl Fleet {
    async fn new(prefix: &str) -> Self {
        let app = TestApp::new().await;
        let user = app.register(prefix).await;
        let business = app
            .create_business(&user, &format!("{prefix} Studio"))
            .await;
        let mut fleet = Self {
            app,
            user,
            business,
            vehicle: String::new(),
            rate: String::new(),
        };
        let vehicle = fleet
            .post(
                "/mileage/vehicles",
                json!({ "name": "Corolla", "registration": "abc123", "make": "Toyota", "model": "Corolla", "year": 2019,
                        "vehicle_type": "car", "fuel_type": "hybrid",
                        "new_rate": { "name": "Kilometre rate", "effective_from": "2025-04-01", "rate_per_km": "1.04" } }),
            )
            .await;
        assert_eq!(vehicle.status, StatusCode::CREATED, "{}", vehicle.body);
        assert_eq!(
            vehicle.body["is_default"], true,
            "the first vehicle is the default"
        );
        assert_eq!(vehicle.body["registration"], "ABC123");
        assert_eq!(vehicle.body["default_rate_name"], "Kilometre rate");
        fleet.vehicle = vehicle.body["id"].as_str().expect("id").to_owned();
        fleet.rate = vehicle.body["default_rate_id"]
            .as_str()
            .expect("rate")
            .to_owned();
        let version = fleet
            .post(
                &format!("/mileage/rates/{}/versions", fleet.rate),
                json!({ "effective_from": "2026-04-01", "rate_per_km": "1.17" }),
            )
            .await;
        assert_eq!(version.status, StatusCode::CREATED, "{}", version.body);
        fleet
    }

    fn path(&self, rest: &str) -> String {
        format!("/api/v1/businesses/{}{rest}", self.business)
    }

    async fn get(&self, rest: &str) -> common::TestResponse {
        self.app.get(&self.path(rest), &self.user.auth()).await
    }

    async fn post(&self, rest: &str, body: Value) -> common::TestResponse {
        self.app
            .post(&self.path(rest), &self.user.auth(), body)
            .await
    }

    async fn put(&self, rest: &str, body: Value) -> common::TestResponse {
        self.app
            .put(&self.path(rest), &self.user.auth(), body)
            .await
    }

    async fn trip(&self, body: Value) -> Value {
        let mut body = body;
        body["vehicle_id"] = json!(self.vehicle);
        let response = self.post("/mileage/trips", body).await;
        assert_eq!(response.status, StatusCode::CREATED, "{}", response.body);
        response.body
    }

    async fn assert_ledger_balances(&self) {
        let mut tx = self
            .app
            .state
            .db
            .begin_business(self.user.id, self.business)
            .await
            .expect("tx");
        let (debits, credits): (i64, i64) = sqlx::query_as(
            "SELECT coalesce(sum(debit), 0)::bigint, coalesce(sum(credit), 0)::bigint FROM journal_entry_lines",
        )
        .fetch_one(&mut *tx)
        .await
        .expect("sum");
        assert_eq!(debits, credits, "ledger out of balance");
    }
}

fn journal(trip: &Value) -> &Vec<Value> {
    trip["journal"].as_array().expect("journal")
}

#[tokio::test]
async fn rates_keep_dated_versions_and_protect_what_uses_them() {
    let f = Fleet::new("mileage-rates").await;
    let rates = f.get("/mileage/rates").await;
    assert_eq!(rates.status, StatusCode::OK);
    let rate = &rates.body[0];
    assert_eq!(rate["name"], "Kilometre rate");
    assert_eq!(rate["vehicle_count"], 1);
    let versions: Vec<(&str, &str)> = rate["versions"]
        .as_array()
        .expect("versions")
        .iter()
        .map(|v| {
            (
                v["effective_from"].as_str().expect("date"),
                v["rate_per_km"].as_str().expect("rate"),
            )
        })
        .collect();
    assert_eq!(
        versions,
        [("2025-04-01", "1.0400"), ("2026-04-01", "1.1700")]
    );

    let duplicate = f
        .post(
            &format!("/mileage/rates/{}/versions", f.rate),
            json!({ "effective_from": "2026-04-01", "rate_per_km": "1.20" }),
        )
        .await;
    assert!(
        duplicate.field_error("effective_from").is_some(),
        "{}",
        duplicate.body
    );
    let too_high = f
        .post(
            &format!("/mileage/rates/{}/versions", f.rate),
            json!({ "effective_from": "2027-04-01", "rate_per_km": "250" }),
        )
        .await;
    assert!(too_high.field_error("rate_per_km").is_some());

    let archive = f
        .post(
            &format!("/mileage/rates/{}/archive", f.rate),
            json!({ "archived": true }),
        )
        .await;
    assert_eq!(
        archive.status,
        StatusCode::CONFLICT,
        "a vehicle's default rate can't be archived"
    );

    // A rate keeps at least one version.
    let solo_rate = f
        .post(
            "/mileage/rates",
            json!({ "name": "My own rate", "effective_from": "2026-01-01", "rate_per_km": "0.90" }),
        )
        .await;
    assert_eq!(solo_rate.status, StatusCode::CREATED, "{}", solo_rate.body);
    let solo_id = solo_rate.body["id"].as_str().expect("id");
    let only = solo_rate.body["versions"][0]["id"]
        .as_str()
        .expect("version");
    let delete = f
        .app
        .delete(
            &f.path(&format!("/mileage/rates/{solo_id}/versions/{only}")),
            &f.user.auth(),
            None,
        )
        .await;
    assert_eq!(delete.status, StatusCode::CONFLICT);
    let same_name = f
        .post(
            "/mileage/rates",
            json!({ "name": "my own RATE", "effective_from": "2026-01-01", "rate_per_km": "1" }),
        )
        .await;
    assert!(same_name.field_error("name").is_some());

    // A second default vehicle replaces the first.
    let van = f
        .post(
            "/mileage/vehicles",
            json!({ "name": "Van", "vehicle_type": "van", "fuel_type": "diesel", "default_rate_id": solo_id, "is_default": true }),
        )
        .await;
    assert_eq!(van.status, StatusCode::CREATED, "{}", van.body);
    let vehicles = f.get("/mileage/vehicles").await;
    let defaults: Vec<&str> = vehicles
        .body
        .as_array()
        .expect("vehicles")
        .iter()
        .filter(|v| v["is_default"] == true)
        .map(|v| v["name"].as_str().expect("name"))
        .collect();
    assert_eq!(defaults, ["Van"]);
}

#[tokio::test]
async fn trip_claims_use_the_rate_in_force_and_post_to_the_ledger() {
    let f = Fleet::new("mileage-claims").await;

    // 42.5 km from the odometer at $1.04 (before the April 2026 version) = $44.20.
    let first = f
        .trip(json!({ "trip_date": "2026-03-15", "purpose": "Client workshop", "start_location": "Home",
                       "destination": "Kererū Café", "start_odometer": "10000.0", "end_odometer": "10042.5" }))
        .await;
    assert_eq!(first["distance_km"], "42.5");
    assert_eq!(first["rate_per_km"], "1.0400");
    assert_eq!(first["claim_amount"], 4_420);
    assert_eq!(first["rate_overridden"], false);
    assert_eq!(first["rate_name"], "Kilometre rate");
    let entries = journal(&first);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["description"], "Mileage: Client workshop");
    assert_eq!(entries[0]["amount"], 4_420);

    // 100 km, 60% business, at $1.17 = 60 km → $70.20; 40 km personal.
    let second = f
        .trip(json!({ "trip_date": "2026-05-01", "purpose": "Site visits", "distance_km": "100", "business_percentage": "60" }))
        .await;
    assert_eq!(second["business_distance_km"], "60.0");
    assert_eq!(second["personal_distance_km"], "40.0");
    assert_eq!(second["claim_amount"], 7_020);

    // A rate entered for one trip is recorded as such.
    let third = f
        .trip(json!({ "trip_date": "2026-05-02", "purpose": "Supplies", "distance_km": "10", "rate_per_km": "0.85" }))
        .await;
    assert_eq!(third["claim_amount"], 850);
    assert_eq!(third["rate_overridden"], true);
    assert!(third["rate_id"].is_null());

    // A personal trip is logged but claims nothing.
    let personal = f
        .trip(json!({ "trip_date": "2026-05-03", "purpose": "Weekend", "distance_km": "25", "business_percentage": "0" }))
        .await;
    assert_eq!(personal["claim_amount"], 0);
    assert!(journal(&personal).is_empty());

    // Editing reverses the old entry and posts the new one.
    let first_id = first["id"].as_str().expect("id");
    let edited = f
        .put(
            &format!("/mileage/trips/{first_id}"),
            json!({ "vehicle_id": f.vehicle, "trip_date": "2026-03-15", "purpose": "Client workshop",
                    "start_location": "Home", "destination": "Kererū Café",
                    "start_odometer": "10000.0", "end_odometer": "10050.0" }),
        )
        .await;
    assert_eq!(edited.status, StatusCode::OK, "{}", edited.body);
    assert_eq!(edited.body["claim_amount"], 5_200);
    let entries = journal(&edited.body);
    assert_eq!(entries.len(), 3, "original, reversal and correction");
    assert!(entries.iter().any(|e| !e["reverses_entry_id"].is_null()));

    // Deleting reverses the claim; the trip no longer counts.
    let third_id = third["id"].as_str().expect("id");
    let deleted = f
        .app
        .delete(
            &f.path(&format!("/mileage/trips/{third_id}")),
            &f.user.auth(),
            Some(json!({ "reason": "Duplicate" })),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let after = f.get(&format!("/mileage/trips/{third_id}")).await;
    assert_eq!(after.body["voided"], true);
    assert_eq!(journal(&after.body).len(), 2);
    f.assert_ledger_balances().await;

    // Claims are business expenses in the ledger: $52.00 + $70.20.
    let dashboard = f
        .get("/dashboard?from=2026-01-01&to=2026-12-31&basis=accrual")
        .await;
    assert_eq!(dashboard.status, StatusCode::OK, "{}", dashboard.body);
    assert_eq!(dashboard.body["totals"]["expenses"], 12_220);
    let categories = dashboard.body["expenses_by_category"]
        .as_array()
        .expect("categories");
    assert!(
        categories
            .iter()
            .any(|c| c["name"] == "Mileage claims" && c["amount"] == 12_220)
    );
    assert_eq!(dashboard.body["mileage"]["trips"], 3);
    assert_eq!(dashboard.body["mileage"]["claim_amount"], 12_220);

    // The report: totals, every month, and the vehicle breakdown.
    let summary = f
        .get("/mileage/summary?from=2026-01-01&to=2026-12-31")
        .await;
    assert_eq!(summary.status, StatusCode::OK, "{}", summary.body);
    let totals = &summary.body["totals"];
    assert_eq!(totals["trips"], 3);
    assert_eq!(totals["distance_km"], "175.0");
    assert_eq!(totals["business_distance_km"], "110.0");
    assert_eq!(totals["personal_distance_km"], "65.0");
    assert_eq!(totals["claim_amount"], 12_220);
    let monthly = summary.body["monthly"].as_array().expect("monthly");
    assert_eq!(monthly.len(), 12);
    assert_eq!(monthly[4]["month"], "2026-05-01");
    assert_eq!(monthly[4]["trips"], 2);
    assert_eq!(monthly[4]["claim_amount"], 7_020);
    assert_eq!(summary.body["vehicles"][0]["vehicle_name"], "Corolla");

    let trips = f.get("/mileage/trips?from=2026-05-01&to=2026-05-31").await;
    assert_eq!(trips.body["total"], 2);
    assert_eq!(trips.body["totals"]["claim_amount"], 7_020);
    let searched = f.get("/mileage/trips?search=kerer").await;
    assert_eq!(searched.body["total"], 1);

    // CSV for spreadsheets: BOM, header, one row per trip (oldest first), money in dollars.
    let csv = f.get("/mileage/export?from=2026-01-01&to=2026-12-31").await;
    assert_eq!(csv.status, StatusCode::OK);
    assert!(
        csv.headers[header::CONTENT_TYPE]
            .to_str()
            .expect("type")
            .starts_with("text/csv")
    );
    let text = String::from_utf8(csv.bytes.clone()).expect("utf-8");
    assert!(text.starts_with('\u{feff}'));
    let lines: Vec<&str> = text.trim_end().split("\r\n").collect();
    assert_eq!(lines.len(), 4, "header and three trips:\n{text}");
    assert!(lines[0].contains("Business distance (km)"));
    assert!(
        lines[1].starts_with("2026-03-15,Corolla,ABC123,Home,Kererū Café,Client workshop,10000.0,10050.0,50.0,100,50.0,0.0,Kilometre rate,1.04,52.00"),
        "{}",
        lines[1]
    );
}

#[tokio::test]
async fn trip_errors_name_the_field_to_correct() {
    let f = Fleet::new("mileage-errors").await;
    let calc = |body: Value| {
        let mut body = body;
        body["vehicle_id"] = json!(f.vehicle);
        f.post("/mileage/calculate", body)
    };

    let ok = calc(
        json!({ "trip_date": "2026-06-01", "distance_km": "12.35", "business_percentage": "50" }),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.body);
    // 12.35 → 12.4 km; 50% = 6.2 km at $1.17 = $7.254 → $7.25.
    assert_eq!(ok.body["distance_km"], "12.4");
    assert_eq!(ok.body["business_distance_km"], "6.2");
    assert_eq!(ok.body["claim_amount"], 725);
    assert_eq!(ok.body["rate_effective_from"], "2026-04-01");

    let cases = [
        (
            json!({ "trip_date": "2026-06-01", "start_odometer": "100" }),
            "end_odometer",
        ),
        (
            json!({ "trip_date": "2026-06-01", "start_odometer": "100", "end_odometer": "90" }),
            "end_odometer",
        ),
        (json!({ "trip_date": "2026-06-01" }), "distance_km"),
        (
            json!({ "trip_date": "2026-06-01", "start_odometer": "100", "end_odometer": "150", "distance_km": "40" }),
            "distance_km",
        ),
        (
            json!({ "trip_date": "2026-06-01", "distance_km": "10", "business_percentage": "150" }),
            "business_percentage",
        ),
        // Before the first version of the vehicle's rate.
        (
            json!({ "trip_date": "2025-01-01", "distance_km": "10" }),
            "rate_id",
        ),
    ];
    for (body, field) in cases {
        let response = calc(body.clone()).await;
        assert_eq!(
            response.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{body}: {}",
            response.body
        );
        assert!(
            response.field_error(field).is_some(),
            "{body}: expected an error on {field}, got {}",
            response.body
        );
    }

    let missing_purpose = f
        .post(
            "/mileage/trips",
            json!({ "vehicle_id": f.vehicle, "trip_date": "2026-06-01", "distance_km": "10" }),
        )
        .await;
    assert!(missing_purpose.field_error("purpose").is_some());

    // A vehicle without a default rate needs a rate for each trip.
    let bike = f
        .post(
            "/mileage/vehicles",
            json!({ "name": "Scooter", "vehicle_type": "motorcycle", "fuel_type": "electric" }),
        )
        .await;
    let bike_id = bike.body["id"].as_str().expect("id");
    let no_rate = f
        .post(
            "/mileage/calculate",
            json!({ "vehicle_id": bike_id, "trip_date": "2026-06-01", "distance_km": "10" }),
        )
        .await;
    assert!(no_rate.field_error("rate_id").is_some(), "{}", no_rate.body);

    // Archived vehicles can't take new trips.
    f.post(
        &format!("/mileage/vehicles/{bike_id}/archive"),
        json!({ "archived": true }),
    )
    .await;
    let archived = f
        .post(
            "/mileage/calculate",
            json!({ "vehicle_id": bike_id, "trip_date": "2026-06-01", "distance_km": "10", "rate_per_km": "1" }),
        )
        .await;
    assert!(archived.field_error("vehicle_id").is_some());

    let trips = f.get("/mileage/trips").await;
    assert_eq!(trips.body["total"], 0, "calculations never save");
}

#[tokio::test]
async fn editing_a_trip_previews_with_its_archived_vehicle() {
    let f = Fleet::new("mileage-edit-archived").await;
    let van = f
        .post(
            "/mileage/vehicles",
            json!({ "name": "Van", "vehicle_type": "van", "fuel_type": "diesel" }),
        )
        .await;
    let van_id = van.body["id"].as_str().expect("id").to_owned();
    let input = json!({ "vehicle_id": van_id, "trip_date": "2026-06-01", "purpose": "Site visit",
                        "distance_km": "20", "business_percentage": "100", "rate_per_km": "1" });
    let trip = f.post("/mileage/trips", input.clone()).await;
    assert_eq!(trip.status, StatusCode::CREATED, "{}", trip.body);
    let trip_id = trip.body["id"].as_str().expect("id").to_owned();
    f.post(
        &format!("/mileage/vehicles/{van_id}/archive"),
        json!({ "archived": true }),
    )
    .await;

    // A new trip can't use the archived van, but the trip already recorded with it can be edited,
    // and its preview agrees with what saving does.
    let new_trip = f.post("/mileage/calculate", input.clone()).await;
    assert!(new_trip.field_error("vehicle_id").is_some());
    let edit = f
        .post(
            &format!("/mileage/calculate?trip_id={trip_id}"),
            input.clone(),
        )
        .await;
    assert_eq!(edit.status, StatusCode::OK, "{}", edit.body);
    assert_eq!(edit.body["claim_amount"], 2_000);
    let saved = f
        .put(&format!("/mileage/trips/{trip_id}"), input.clone())
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);

    let unknown = f
        .post(
            &format!("/mileage/calculate?trip_id={}", Uuid::now_v7()),
            input,
        )
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn expense_categories_never_take_the_mileage_account_code() {
    let f = Fleet::new("mileage-codes").await;
    // The 16 default categories use 5000–5150; custom ones continue in tens up to 5990.
    for i in 1..=84 {
        let category = f
            .post(
                "/expense-categories",
                json!({ "name": format!("Category {i}") }),
            )
            .await;
        assert_eq!(category.status, StatusCode::CREATED, "{}", category.body);
    }
    // The next would have been 6000 (Mileage claims): it takes the first free code instead.
    let next = f
        .post("/expense-categories", json!({ "name": "One more" }))
        .await;
    assert_eq!(next.status, StatusCode::CREATED, "{}", next.body);
    assert_eq!(next.body["account_code"], "5001");

    // Claims still post to Mileage claims.
    let trip = f
        .post(
            "/mileage/trips",
            json!({ "vehicle_id": f.vehicle, "trip_date": "2026-06-01", "purpose": "Client meeting",
                    "distance_km": "10", "business_percentage": "100" }),
        )
        .await;
    assert_eq!(trip.status, StatusCode::CREATED, "{}", trip.body);
}

#[tokio::test]
async fn mileage_is_private_to_its_business() {
    let f = Fleet::new("mileage-tenant").await;
    let trip = f
        .trip(json!({ "trip_date": "2026-06-01", "purpose": "Meeting", "distance_km": "20" }))
        .await;
    let trip_id = trip["id"].as_str().expect("id");

    let intruder = f.app.register("mileage-intruder").await;
    for path in [
        "/mileage/vehicles".to_owned(),
        "/mileage/rates".to_owned(),
        "/mileage/trips".to_owned(),
        format!("/mileage/trips/{trip_id}"),
        "/mileage/summary".to_owned(),
        "/mileage/export".to_owned(),
    ] {
        assert_eq!(
            f.app.get(&f.path(&path), &intruder.auth()).await.status,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }

    // In their own business, the intruder can't use the victim's vehicle or rate.
    let own = f.app.create_business(&intruder, "Intruder Ltd").await;
    let forged_trip = f
        .app
        .post(
            &format!("/api/v1/businesses/{own}/mileage/trips"),
            &intruder.auth(),
            json!({ "vehicle_id": f.vehicle, "trip_date": "2026-06-01", "purpose": "x", "distance_km": "5", "rate_per_km": "1" }),
        )
        .await;
    assert!(
        forged_trip.field_error("vehicle_id").is_some(),
        "{}",
        forged_trip.body
    );
    let forged_vehicle = f
        .app
        .post(
            &format!("/api/v1/businesses/{own}/mileage/vehicles"),
            &intruder.auth(),
            json!({ "name": "Mine", "vehicle_type": "car", "fuel_type": "petrol", "default_rate_id": f.rate }),
        )
        .await;
    assert!(
        forged_vehicle.field_error("default_rate_id").is_some(),
        "{}",
        forged_vehicle.body
    );

    // The account export includes mileage records.
    let export = f.app.get("/api/v1/users/me/export", &f.user.auth()).await;
    let records = &export.body["businesses"][0]["records"];
    for table in [
        "vehicles",
        "mileage_rates",
        "mileage_rate_versions",
        "mileage_trips",
    ] {
        assert!(
            records[table]
                .as_array()
                .is_some_and(|rows| !rows.is_empty()),
            "{table} missing from the export"
        );
    }
}
