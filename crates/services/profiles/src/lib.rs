//! Profile CRUD, avatar storage, and `.aivtprofile` import/export —
//! backs the pestaña Identidad and the profile switcher (sección 9-10 del
//! brief).

mod error;
mod export;
mod service;

pub use aivt_database::{NewProfile, Profile, ProfileIdentity, ProfilePersonality, ProfileUpdate};
pub use error::ProfileError;
pub use service::ProfileService;
