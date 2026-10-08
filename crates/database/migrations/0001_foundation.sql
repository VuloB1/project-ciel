-- Key/value store for app-level preferences (Settings > General and beyond).
-- `value` holds a JSON-encoded scalar or object so a single table can back
-- settings of any shape without a migration per new preference.
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
