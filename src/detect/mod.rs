pub mod case_sensitivity;
pub mod filetype;
pub mod layer;
pub mod workspace;

pub use case_sensitivity::{find_case_insensitive, is_case_insensitive};
pub use filetype::detect_file_type;
pub use layer::{detect_fsd_project, infer_arch_layer};
pub use workspace::detect_workspace;
