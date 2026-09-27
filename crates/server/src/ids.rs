use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use ulid::Ulid;

/// `docs/data-model.md`'s `"run_01hz..."` id shape: a ULID (sortable by
/// creation time) with a `run_` prefix.
pub fn new_run_id() -> String {
    format!("run_{}", Ulid::new().to_string().to_lowercase())
}

/// Same shape as `new_run_id`, for Phase 9's interactive sessions — a
/// distinct prefix keeps the two id spaces visually distinguishable even
/// though they're never looked up in the same table.
pub fn new_session_id() -> String {
    format!("sess_{}", Ulid::new().to_string().to_lowercase())
}

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("RFC3339 formatting of the current time never fails")
}
