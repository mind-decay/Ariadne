use crate::model::{ArchLayer, CanonicalPath, FsdLayer};

/// Detect whether a project uses Feature-Sliced Design.
/// Uses a 2-of-3 heuristic: if at least 2 of {features, entities, shared}
/// appear as top-level or src/-level directory segments, classify as FSD.
pub fn detect_fsd_project(paths: &[CanonicalPath]) -> bool {
    let mut found_features = false;
    let mut found_entities = false;
    let mut found_shared = false;

    for path in paths {
        let segments: Vec<&str> = path.as_str().split('/').collect();
        for (i, segment) in segments.iter().enumerate() {
            let lower = segment.to_ascii_lowercase();
            // Qualifies only at position 0 (root-level) or position 1 after "src"
            let qualifies = i == 0
                || (i == 1 && segments[0].eq_ignore_ascii_case("src"));
            if !qualifies {
                continue;
            }
            match lower.as_str() {
                "features" => found_features = true,
                "entities" => found_entities = true,
                "shared" => found_shared = true,
                _ => {}
            }
        }
        // Early exit if we already found 2
        let count = found_features as u8 + found_entities as u8 + found_shared as u8;
        if count >= 2 {
            return true;
        }
    }

    let count = found_features as u8 + found_entities as u8 + found_shared as u8;
    count >= 2
}

/// Infer the architectural layer from directory segments in the path.
///
/// When `is_fsd` is false, uses first-matching-segment strategy (original behavior).
/// When `is_fsd` is true, uses two-pass FSD classification:
///   1. Outermost-first scan for FSD layer segment
///   2. Innermost-first scan of remaining segments for ArchLayer
pub fn infer_arch_layer(path: &CanonicalPath, is_fsd: bool) -> (ArchLayer, Option<FsdLayer>) {
    let path_str = path.as_str();

    if !is_fsd {
        // Original first-match behavior
        for segment in directory_segments(path_str) {
            let lower = segment.to_ascii_lowercase();
            if let Some(layer) = match_layer(&lower) {
                return (layer, None);
            }
        }
        return (ArchLayer::Unknown, None);
    }

    // FSD mode: two-pass classification
    let segments: Vec<&str> = directory_segments(path_str).collect();

    // Pass 1: Find the outermost FSD layer segment
    let mut fsd_index = None;
    let mut fsd_layer = None;
    for (i, segment) in segments.iter().enumerate() {
        let lower = segment.to_ascii_lowercase();
        if let Some(fl) = match_fsd_layer(&lower) {
            fsd_index = Some(i);
            fsd_layer = Some(fl);
            break;
        }
    }

    match (fsd_index, fsd_layer) {
        (Some(idx), Some(fl)) => {
            // Pass 2: Scan segments AFTER the FSD layer, innermost-first (right-to-left)
            let after_fsd = &segments[idx + 1..];
            for segment in after_fsd.iter().rev() {
                let lower = segment.to_ascii_lowercase();
                if let Some(arch) = match_layer(&lower) {
                    return (arch, Some(fl));
                }
            }
            (ArchLayer::Unknown, Some(fl))
        }
        _ => {
            // No FSD layer found, fall back to first-match
            for segment in &segments {
                let lower = segment.to_ascii_lowercase();
                if let Some(layer) = match_layer(&lower) {
                    return (layer, None);
                }
            }
            (ArchLayer::Unknown, None)
        }
    }
}

/// Match a segment to an FSD layer name (case-insensitive).
fn match_fsd_layer(segment: &str) -> Option<FsdLayer> {
    match segment.to_ascii_lowercase().as_str() {
        "app" => Some(FsdLayer::App),
        "processes" => Some(FsdLayer::Processes),
        "pages" => Some(FsdLayer::Pages),
        "widgets" => Some(FsdLayer::Widgets),
        "features" => Some(FsdLayer::Features),
        "entities" => Some(FsdLayer::Entities),
        "shared" => Some(FsdLayer::Shared),
        _ => None,
    }
}

/// Extract directory segments from a path (everything except the last component).
fn directory_segments(path: &str) -> impl Iterator<Item = &str> {
    // Split on '/' and drop the last segment (the filename).
    let total = path.split('/').count();
    path.split('/').take(total.saturating_sub(1))
}

/// Match a lowercased directory segment to an architectural layer.
fn match_layer(segment: &str) -> Option<ArchLayer> {
    match segment {
        // Api (REST, GraphQL, MVC, Clean, Hexagonal, SvelteKit)
        "api" | "routes" | "endpoints" | "controllers" | "handlers" | "rest" | "graphql"
        | "presentation" | "adapters" | "interfaces" | "presenters" | "ports" | "params" => {
            Some(ArchLayer::Api)
        }
        // Service (DDD, Clean, CQRS, Event-Driven, Angular/NestJS)
        "services" | "service" | "domain" | "business" | "usecases" | "use-cases"
        | "interactors" | "middleware" | "application" | "commands" | "events" | "listeners"
        | "subscribers" | "guards" | "interceptors" | "filters" | "providers" => {
            Some(ArchLayer::Service)
        }
        // Data (DDD, Clean, Layered, CQRS, Rails/Django)
        "data" | "db" | "database" | "repository" | "repositories" | "models" | "model" | "dao"
        | "store" | "stores" | "schema" | "migration" | "migrations" | "entities"
        | "aggregates" | "value-objects" | "infrastructure" | "persistence" | "gateways"
        | "queries" | "serializers" => Some(ArchLayer::Data),
        // Util (Angular)
        "utils" | "util" | "helpers" | "lib" | "shared" | "common" | "pkg" | "pipes" => {
            Some(ArchLayer::Util)
        }
        // Component (MVVM, Django, Angular)
        "components" | "component" | "ui" | "views" | "pages" | "layouts" | "widgets"
        | "viewmodels" | "view-models" | "templates" | "forms" | "directives" => {
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
        let (arch, _fsd) = infer_arch_layer(&CanonicalPath::new(path), false);
        arch
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
    fn sveltekit() {
        assert_eq!(layer("src/params/slug.ts"), ArchLayer::Api);
        assert_eq!(layer("src/routes/api/users.ts"), ArchLayer::Api);
        assert_eq!(layer("src/lib/utils/format.ts"), ArchLayer::Util);
    }

    #[test]
    fn hexagonal_architecture() {
        assert_eq!(layer("src/ports/input.ts"), ArchLayer::Api);
        assert_eq!(layer("src/adapters/http.ts"), ArchLayer::Api);
    }

    #[test]
    fn cqrs() {
        assert_eq!(layer("src/commands/create_order.ts"), ArchLayer::Service);
        assert_eq!(layer("src/queries/get_orders.ts"), ArchLayer::Data);
    }

    #[test]
    fn event_driven() {
        assert_eq!(layer("src/events/order_created.ts"), ArchLayer::Service);
        assert_eq!(layer("src/listeners/email_sender.ts"), ArchLayer::Service);
        assert_eq!(layer("src/subscribers/audit_log.ts"), ArchLayer::Service);
    }

    #[test]
    fn mvvm() {
        assert_eq!(layer("src/viewmodels/user.ts"), ArchLayer::Component);
        assert_eq!(layer("src/view-models/user.ts"), ArchLayer::Component);
    }

    #[test]
    fn rails_django() {
        assert_eq!(layer("app/templates/index.html"), ArchLayer::Component);
        assert_eq!(layer("app/serializers/user.py"), ArchLayer::Data);
        assert_eq!(layer("app/forms/login.py"), ArchLayer::Component);
    }

    #[test]
    fn angular_nestjs() {
        assert_eq!(layer("src/guards/auth.guard.ts"), ArchLayer::Service);
        assert_eq!(layer("src/interceptors/logging.ts"), ArchLayer::Service);
        assert_eq!(layer("src/pipes/validation.pipe.ts"), ArchLayer::Util);
        assert_eq!(layer("src/directives/highlight.ts"), ArchLayer::Component);
        assert_eq!(layer("src/filters/exception.filter.ts"), ArchLayer::Service);
        assert_eq!(layer("src/providers/database.ts"), ArchLayer::Service);
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

    // ── FSD helpers ──────────────────────────────────────────────────

    fn fsd_layer(path: &str) -> (ArchLayer, Option<FsdLayer>) {
        infer_arch_layer(&CanonicalPath::new(path), true)
    }

    fn paths(strs: &[&str]) -> Vec<CanonicalPath> {
        strs.iter().map(|s| CanonicalPath::new(*s)).collect()
    }

    // ── 1. FSD Detection Tests ──────────────────────────────────────

    #[test]
    fn detect_fsd_features_and_entities() {
        assert!(detect_fsd_project(&paths(&["features/a.ts", "entities/b.ts"])));
    }

    #[test]
    fn detect_fsd_features_and_shared() {
        assert!(detect_fsd_project(&paths(&["features/a.ts", "shared/b.ts"])));
    }

    #[test]
    fn detect_fsd_entities_and_shared() {
        assert!(detect_fsd_project(&paths(&["entities/a.ts", "shared/b.ts"])));
    }

    #[test]
    fn detect_fsd_all_three() {
        assert!(detect_fsd_project(&paths(&[
            "features/a.ts",
            "entities/b.ts",
            "shared/c.ts"
        ])));
    }

    #[test]
    fn detect_fsd_only_one() {
        assert!(!detect_fsd_project(&paths(&[
            "features/a.ts",
            "src/utils/b.ts"
        ])));
    }

    #[test]
    fn detect_fsd_none() {
        assert!(!detect_fsd_project(&paths(&[
            "src/api/a.ts",
            "src/utils/b.ts"
        ])));
    }

    #[test]
    fn detect_fsd_empty() {
        assert!(!detect_fsd_project(&paths(&[])));
    }

    #[test]
    fn detect_fsd_under_src() {
        assert!(detect_fsd_project(&paths(&[
            "src/features/a.ts",
            "src/entities/b.ts"
        ])));
    }

    #[test]
    fn detect_fsd_deep_nested_not_counted() {
        // Only root-level or src/-level counts
        assert!(!detect_fsd_project(&paths(&[
            "lib/features/a.ts",
            "lib/entities/b.ts"
        ])));
    }

    // ── 2. FSD Layer Inference Tests ────────────────────────────────

    #[test]
    fn fsd_features_with_ui() {
        assert_eq!(
            fsd_layer("features/auth/ui/Button.tsx"),
            (ArchLayer::Component, Some(FsdLayer::Features))
        );
    }

    #[test]
    fn fsd_features_with_api() {
        assert_eq!(
            fsd_layer("features/auth/api/login.ts"),
            (ArchLayer::Api, Some(FsdLayer::Features))
        );
    }

    #[test]
    fn fsd_features_with_model() {
        assert_eq!(
            fsd_layer("features/auth/model/User.ts"),
            (ArchLayer::Data, Some(FsdLayer::Features))
        );
    }

    #[test]
    fn fsd_entities_with_model() {
        assert_eq!(
            fsd_layer("entities/user/model/types.ts"),
            (ArchLayer::Data, Some(FsdLayer::Entities))
        );
    }

    #[test]
    fn fsd_shared_with_lib() {
        assert_eq!(
            fsd_layer("shared/lib/utils.ts"),
            (ArchLayer::Util, Some(FsdLayer::Shared))
        );
    }

    #[test]
    fn fsd_shared_with_ui() {
        assert_eq!(
            fsd_layer("shared/ui/Button.tsx"),
            (ArchLayer::Component, Some(FsdLayer::Shared))
        );
    }

    #[test]
    fn fsd_shared_with_config() {
        assert_eq!(
            fsd_layer("shared/config/env.ts"),
            (ArchLayer::Config, Some(FsdLayer::Shared))
        );
    }

    #[test]
    fn fsd_pages_with_ui() {
        assert_eq!(
            fsd_layer("pages/home/ui/Page.tsx"),
            (ArchLayer::Component, Some(FsdLayer::Pages))
        );
    }

    #[test]
    fn fsd_widgets_with_ui() {
        assert_eq!(
            fsd_layer("widgets/header/ui/Header.tsx"),
            (ArchLayer::Component, Some(FsdLayer::Widgets))
        );
    }

    #[test]
    fn fsd_app_no_inner() {
        assert_eq!(
            fsd_layer("app/main.ts"),
            (ArchLayer::Unknown, Some(FsdLayer::App))
        );
    }

    #[test]
    fn fsd_processes_with_model() {
        assert_eq!(
            fsd_layer("processes/auth/model/state.ts"),
            (ArchLayer::Data, Some(FsdLayer::Processes))
        );
    }

    #[test]
    fn fsd_barrel_file() {
        assert_eq!(
            fsd_layer("features/auth/index.ts"),
            (ArchLayer::Unknown, Some(FsdLayer::Features))
        );
    }

    #[test]
    fn fsd_no_fsd_segment() {
        // Path has no FSD layer segment, falls back to first-match
        assert_eq!(
            fsd_layer("src/utils/format.ts"),
            (ArchLayer::Util, None)
        );
    }

    #[test]
    fn fsd_nested_fsd_outermost() {
        // Outermost FSD layer wins; inner "entities" is matched as ArchLayer::Data
        assert_eq!(
            fsd_layer("features/auth/entities/user.ts"),
            (ArchLayer::Data, Some(FsdLayer::Features))
        );
    }

    #[test]
    fn fsd_with_src_prefix() {
        assert_eq!(
            fsd_layer("src/features/auth/api/login.ts"),
            (ArchLayer::Api, Some(FsdLayer::Features))
        );
    }

    // ── 3. Non-FSD Backward Compatibility ───────────────────────────

    #[test]
    fn non_fsd_api_unchanged() {
        assert_eq!(
            infer_arch_layer(&CanonicalPath::new("src/api/auth.ts"), false),
            (ArchLayer::Api, None)
        );
    }

    #[test]
    fn non_fsd_entities_is_data() {
        assert_eq!(
            infer_arch_layer(&CanonicalPath::new("src/entities/user.ts"), false),
            (ArchLayer::Data, None)
        );
    }

    #[test]
    fn non_fsd_shared_is_util() {
        assert_eq!(
            infer_arch_layer(&CanonicalPath::new("src/shared/utils.ts"), false),
            (ArchLayer::Util, None)
        );
    }

    // ── 4. Edge Cases ───────────────────────────────────────────────

    #[test]
    fn ec04_case_insensitive() {
        // FSD layer and inner segment matching should be case-insensitive
        let (arch, fsd) = fsd_layer("Features/auth/UI/login.ts");
        assert_eq!(fsd, Some(FsdLayer::Features));
        assert_eq!(arch, ArchLayer::Component);
    }

    #[test]
    fn ec07_no_dir_segments() {
        // A bare filename has no directory segments, so no layer
        assert_eq!(fsd_layer("main.ts"), (ArchLayer::Unknown, None));
    }
}
