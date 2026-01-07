mod detect;
mod registry;

pub use detect::{detect_project_root, Project};
pub use registry::{ProjectEntry, ProjectRegistry};
