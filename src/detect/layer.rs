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
        "api" | "routes" | "endpoints" | "controllers" | "handlers" | "rest" | "graphql"
        | "presentation" | "adapters" | "interfaces" | "presenters" => Some(ArchLayer::Api),
        // Service
        "services" | "service" | "domain" | "business" | "usecases" | "use-cases"
        | "interactors" | "middleware" | "application" => Some(ArchLayer::Service),
        // Data
        "data" | "db" | "database" | "repository" | "repositories" | "models" | "model"
        | "dao" | "store" | "stores" | "schema" | "migration" | "migrations"
        | "entities" | "aggregates" | "value-objects" | "infrastructure" | "persistence"
        | "gateways" => Some(ArchLayer::Data),
        // Util
        "utils" | "util" | "helpers" | "lib" | "shared" | "common" | "pkg" => {
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
        // Clean / Layered architecture
        assert_eq!(layer("src/presentation/views.ts"), ArchLayer::Api);
        assert_eq!(layer("src/adapters/http.ts"), ArchLayer::Api);
        assert_eq!(layer("src/interfaces/rest.ts"), ArchLayer::Api);
        assert_eq!(layer("src/presenters/user.ts"), ArchLayer::Api);
    }

    #[test]
    fn service_layer() {
        assert_eq!(layer("src/services/auth.ts"), ArchLayer::Service);
        assert_eq!(layer("src/domain/user.ts"), ArchLayer::Service);
        assert_eq!(layer("src/usecases/login.ts"), ArchLayer::Service);
        assert_eq!(layer("src/middleware/cors.ts"), ArchLayer::Service);
        // Clean / DDD
        assert_eq!(layer("src/application/commands.ts"), ArchLayer::Service);
    }

    #[test]
    fn data_layer() {
        assert_eq!(layer("src/data/user.ts"), ArchLayer::Data);
        assert_eq!(layer("src/repository/auth.ts"), ArchLayer::Data);
        assert_eq!(layer("src/models/user.py"), ArchLayer::Data);
        assert_eq!(layer("src/model/user.ts"), ArchLayer::Data);
        assert_eq!(layer("src/db/connection.ts"), ArchLayer::Data);
        assert_eq!(layer("src/schema/user.graphql"), ArchLayer::Data);
        assert_eq!(layer("src/migrations/001_init.sql"), ArchLayer::Data);
        // DDD / Clean / Layered architecture
        assert_eq!(layer("src/entities/user.ts"), ArchLayer::Data);
        assert_eq!(layer("src/aggregates/order.ts"), ArchLayer::Data);
        assert_eq!(layer("src/value-objects/money.ts"), ArchLayer::Data);
        assert_eq!(layer("src/infrastructure/database.ts"), ArchLayer::Data);
        assert_eq!(layer("src/persistence/user_repo.ts"), ArchLayer::Data);
        assert_eq!(layer("src/gateways/payment.ts"), ArchLayer::Data);
    }

    #[test]
    fn util_layer() {
        assert_eq!(layer("src/utils/format.ts"), ArchLayer::Util);
        assert_eq!(layer("src/helpers/string.ts"), ArchLayer::Util);
        assert_eq!(layer("src/lib/auth.ts"), ArchLayer::Util);
        assert_eq!(layer("pkg/util/hash.go"), ArchLayer::Util);
    }

    #[test]
    fn internal_is_not_util() {
        // Go's internal/ is an access modifier, not an architectural layer.
        // Sub-directories within internal/ should be matched by their own names.
        assert_eq!(layer("internal/handlers/auth.go"), ArchLayer::Api);
        assert_eq!(layer("internal/service/auth.go"), ArchLayer::Service);
        assert_eq!(layer("internal/repo/user.go"), ArchLayer::Unknown);
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
