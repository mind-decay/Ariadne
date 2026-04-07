//! Java framework detection from tree-sitter Java AST.
//!
//! Detects common Java framework patterns (Spring Boot, Jakarta EE, Android,
//! Micronaut, Quarkus) by walking the parsed tree-sitter tree for annotations
//! and base class names.

/// Hints about which Java framework patterns are present in a source file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JavaFrameworkHints {
    pub is_spring_controller: bool,
    pub is_spring_service: bool,
    pub is_spring_repository: bool,
    pub is_spring_component: bool,
    pub is_spring_configuration: bool,
    pub is_spring_boot_application: bool,
    pub is_jakarta_jaxrs: bool,
    pub is_jakarta_cdi: bool,
    pub is_jakarta_jpa_entity: bool,
    pub is_android_activity: bool,
    pub is_android_fragment: bool,
    pub is_android_service: bool,
    pub is_micronaut_controller: bool,
    pub is_micronaut_singleton: bool,
    pub is_quarkus_resource: bool,
    pub is_quarkus_application_scoped: bool,
}

/// Detect Java framework patterns from a parsed Java tree-sitter tree.
pub fn detect_java_framework(tree: &tree_sitter::Tree, source: &[u8]) -> JavaFrameworkHints {
    let mut hints = JavaFrameworkHints::default();
    walk_node(tree.root_node(), source, &mut hints);
    hints
}

fn walk_node(node: tree_sitter::Node, source: &[u8], hints: &mut JavaFrameworkHints) {
    if node.kind() == "class_declaration" {
        detect_class_patterns(&node, source, hints);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(child, source, hints);
    }
}

/// Detect framework patterns from a class declaration node.
fn detect_class_patterns(
    node: &tree_sitter::Node,
    source: &[u8],
    hints: &mut JavaFrameworkHints,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "modifiers" => {
                // Check annotations within modifiers
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    match mod_child.kind() {
                        "marker_annotation" | "annotation" => {
                            if let Some(name) = extract_annotation_name(&mod_child, source) {
                                match_annotation(&name, hints);
                            }
                        }
                        _ => {}
                    }
                }
            }
            "superclass" => {
                // tree-sitter-java superclass node contains the superclass type
                let mut sc_cursor = child.walk();
                for sc_child in child.children(&mut sc_cursor) {
                    if sc_child.kind() == "type_identifier" {
                        let class_name = sc_child.utf8_text(source).unwrap_or("");
                        match_superclass(class_name, hints);
                    }
                }
            }
            _ => {}
        }
    }

    // Also check annotations that may be siblings before the class_declaration
    let mut sibling = node.prev_sibling();
    while let Some(sib) = sibling {
        match sib.kind() {
            "marker_annotation" | "annotation" => {
                if let Some(name) = extract_annotation_name(&sib, source) {
                    match_annotation(&name, hints);
                }
                sibling = sib.prev_sibling();
            }
            _ => break,
        }
    }
}

/// Extract the annotation name from a marker_annotation or annotation node.
fn extract_annotation_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    // In tree-sitter-java, marker_annotation has a `name` field child (identifier or scoped_identifier).
    // annotation also has a `name` field.
    if let Some(name_node) = node.child_by_field_name("name") {
        // For scoped identifiers like @org.springframework.stereotype.Service,
        // get just the last segment (the simple name).
        let text = name_node.utf8_text(source).ok()?;
        let simple_name = text.rsplit('.').next().unwrap_or(text);
        return Some(simple_name.to_string());
    }

    // Fallback: look for first identifier child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return child.utf8_text(source).ok().map(|s| s.to_string());
        }
    }

    None
}

/// Match an annotation name against known Java framework annotations.
fn match_annotation(name: &str, hints: &mut JavaFrameworkHints) {
    match name {
        "RestController" => {
            hints.is_spring_controller = true;
        }
        "Controller" => {
            // Over-approximation (D-133): set both Spring and Micronaut
            hints.is_spring_controller = true;
            hints.is_micronaut_controller = true;
        }
        "Service" => {
            hints.is_spring_service = true;
        }
        "Repository" => {
            hints.is_spring_repository = true;
        }
        "Component" => {
            hints.is_spring_component = true;
        }
        "Configuration" => {
            hints.is_spring_configuration = true;
        }
        "SpringBootApplication" => {
            hints.is_spring_boot_application = true;
        }
        "Path" => {
            hints.is_jakarta_jaxrs = true;
        }
        "Stateless" | "RequestScoped" | "SessionScoped" => {
            hints.is_jakarta_cdi = true;
        }
        "Singleton" => {
            hints.is_jakarta_cdi = true;
            hints.is_micronaut_singleton = true;
        }
        "ApplicationScoped" => {
            hints.is_jakarta_cdi = true;
            hints.is_quarkus_application_scoped = true;
        }
        "Entity" => {
            hints.is_jakarta_jpa_entity = true;
        }
        _ => {}
    }
}

/// Match a superclass name against known Android framework base classes.
fn match_superclass(name: &str, hints: &mut JavaFrameworkHints) {
    match name {
        "Activity" | "AppCompatActivity" | "FragmentActivity" | "ListActivity" => {
            hints.is_android_activity = true;
        }
        "Fragment" | "DialogFragment" | "ListFragment" => {
            hints.is_android_fragment = true;
        }
        "Service" => {
            hints.is_android_service = true;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_detect(source: &str) -> JavaFrameworkHints {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(tree_sitter_java::LANGUAGE))
            .unwrap();
        let tree = parser.parse(source.as_bytes(), None).unwrap();
        detect_java_framework(&tree, source.as_bytes())
    }

    #[test]
    fn test_detect_spring_controller() {
        let source = r#"
import org.springframework.web.bind.annotation.RestController;

@RestController
public class UserController {
    public String getUser() { return "user"; }
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_spring_controller);
    }

    #[test]
    fn test_detect_spring_service() {
        let source = r#"
import org.springframework.stereotype.Service;

@Service
public class UserService {
    public void process() {}
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_spring_service);
    }

    #[test]
    fn test_detect_android_activity() {
        let source = r#"
import android.os.Bundle;

public class MainActivity extends AppCompatActivity {
    protected void onCreate(Bundle savedInstanceState) {}
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_android_activity);
    }

    #[test]
    fn test_detect_jakarta_jaxrs() {
        let source = r#"
import javax.ws.rs.Path;

@Path("/api/users")
public class UserResource {
    public String getUsers() { return "[]"; }
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_jakarta_jaxrs);
    }

    #[test]
    fn test_detect_jakarta_entity() {
        let source = r#"
import javax.persistence.Entity;

@Entity
public class User {
    private Long id;
    private String name;
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_jakarta_jpa_entity);
    }

    #[test]
    fn test_detect_spring_boot_application() {
        let source = r#"
import org.springframework.boot.autoconfigure.SpringBootApplication;

@SpringBootApplication
public class Application {
    public static void main(String[] args) {}
}
"#;
        let hints = parse_and_detect(source);
        assert!(hints.is_spring_boot_application);
    }

    #[test]
    fn test_empty_class_no_hints() {
        let source = r#"
public class PlainClass {
    private int value;
    public int getValue() { return value; }
}
"#;
        let hints = parse_and_detect(source);
        assert_eq!(hints, JavaFrameworkHints::default());
    }
}
