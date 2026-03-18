use crate::model::{ArchLayer, CanonicalPath};

/// Infer the architectural layer from directory segments in the path.
/// First matching segment wins; matching is case-insensitive.
pub fn infer_arch_layer(path: &CanonicalPath) -> ArchLayer {
    let path_str = path.as_str();

    // Check each directory segment (exclude the filename itself)
    for segment in directory_segments(path_str) {
        let lower = segment.to_ascii_lowercase();
        if let Some(layer) = match_layer(&lower) {
            return layer;
        }
    }

    ArchLayer::Unknown
}

/// Extract directory segments from a path (everything except the last component).
fn directory_segments(path: &str) -> impl Iterator<Item = &str> {
    let parts: Vec<&str> = path.split('/').collect();
    // All segments except the last one (which is the filename)
    let dir_parts = if parts.len() > 1 {
        &parts[..parts.len() - 1]
    } else {
        &[]
    };
    dir_parts.to_vec().into_iter()
}

/// Match a lowercased directory segment to an architectural layer.
fn match_layer(segment: &str) -> Option<ArchLayer> {
    match segment {
        // Api
        "api" | "routes" | "endpoints" | "controllers" | "handlers" | "rest" | "graphql" => {
            Some(ArchLayer::Api)
        }
        // Service
        "services" | "service" | "domain" | "business" | "usecases" | "use-cases"
        | "interactors" => Some(ArchLayer::Service),
        // Data
        "data" | "db" | "database" | "repository" | "repositories" | "models" | "dao"
        | "store" | "stores" => Some(ArchLayer::Data),
        // Util
        "utils" | "util" | "helpers" | "lib" | "shared" | "common" | "pkg" | "internal" => {
            Some(ArchLayer::Util)
        }
        // Component
        "components" | "component" | "ui" | "views" | "pages" | "layouts" | "widgets" => {
            Some(ArchLayer::Component)
        }
        // Hook
        "hooks" | "composables" => Some(ArchLayer::Hook),
        // Config
        "config" | "configuration" | "settings" | "env" => Some(ArchLayer::Config),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(path: &str) -> ArchLayer {
        infer_arch_layer(&CanonicalPath::new(path))
    }

    #[test]
    fn api_layer() {
        assert_eq!(layer("src/api/auth.ts"), ArchLayer::Api);
        assert_eq!(layer("src/controllers/user.ts"), ArchLayer::Api);
        assert_eq!(layer("src/routes/index.ts"), ArchLayer::Api);
        assert_eq!(layer("src/handlers/login.go"), ArchLayer::Api);
    }

    #[test]
    fn service_layer() {
        assert_eq!(layer("src/services/auth.ts"), ArchLayer::Service);
        assert_eq!(layer("src/domain/user.ts"), ArchLayer::Service);
        assert_eq!(layer("src/usecases/login.ts"), ArchLayer::Service);
    }

    #[test]
    fn data_layer() {
        assert_eq!(layer("src/data/user.ts"), ArchLayer::Data);
        assert_eq!(layer("src/repository/auth.ts"), ArchLayer::Data);
        assert_eq!(layer("src/models/user.py"), ArchLayer::Data);
        assert_eq!(layer("src/db/connection.ts"), ArchLayer::Data);
    }

    #[test]
    fn util_layer() {
        assert_eq!(layer("src/utils/format.ts"), ArchLayer::Util);
        assert_eq!(layer("src/helpers/string.ts"), ArchLayer::Util);
        assert_eq!(layer("src/lib/auth.ts"), ArchLayer::Util);
        assert_eq!(layer("pkg/util/hash.go"), ArchLayer::Util);
    }

    #[test]
    fn component_layer() {
        assert_eq!(layer("src/components/Button.tsx"), ArchLayer::Component);
        assert_eq!(layer("src/ui/Modal.tsx"), ArchLayer::Component);
        assert_eq!(layer("src/pages/Home.tsx"), ArchLayer::Component);
    }

    #[test]
    fn hook_layer() {
        assert_eq!(layer("src/hooks/useAuth.ts"), ArchLayer::Hook);
        assert_eq!(layer("src/composables/useUser.ts"), ArchLayer::Hook);
    }

    #[test]
    fn config_layer() {
        assert_eq!(layer("src/config/database.ts"), ArchLayer::Config);
        assert_eq!(layer("src/settings/app.py"), ArchLayer::Config);
    }

    #[test]
    fn unknown_layer() {
        assert_eq!(layer("src/auth/login.ts"), ArchLayer::Unknown);
        assert_eq!(layer("main.rs"), ArchLayer::Unknown);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(layer("src/API/auth.ts"), ArchLayer::Api);
        assert_eq!(layer("src/Services/auth.ts"), ArchLayer::Service);
        assert_eq!(layer("src/Utils/format.ts"), ArchLayer::Util);
    }

    #[test]
    fn first_match_wins() {
        // api comes before utils in the path — api wins
        assert_eq!(layer("src/api/utils/helper.ts"), ArchLayer::Api);
    }
}
