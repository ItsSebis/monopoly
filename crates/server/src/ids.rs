use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use ulid::Ulid;

/// `docs/data-model.md`'s `"run_01hz..."` id shape: a ULID (sortable by
/// creation time) with a `run_` prefix.
pub fn new_run_id() -> String {
    format!("run_{}", Ulid::new().to_string().to_lowercase())
}

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("RFC3339 formatting of the current time never fails")
}
