pub mod case_sensitivity;
pub mod filetype;
pub mod framework;
pub mod java_framework;
pub mod js_framework;
pub mod layer;
pub mod workspace;

pub use case_sensitivity::{find_case_insensitive, is_case_insensitive};
pub use filetype::detect_file_type;
pub use java_framework::{JavaFrameworkHints, detect_java_framework};
pub use js_framework::{JsFrameworkHints, RouteConvention, detect_js_framework};
pub use layer::{detect_fsd_project, infer_arch_layer};
pub use workspace::{detect_rust_crate_name, detect_workspace};
