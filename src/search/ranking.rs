//! Custom ranking and boosting

/// Boost factors for different result types
pub struct BoostFactors {
    /// Boost for symbol name matches
    pub symbol_name: f32,
    /// Boost for exact matches
    pub exact_match: f32,
    /// Penalty for test files
    pub test_penalty: f32,
    /// Penalty for generated files
    pub generated_penalty: f32,
}

impl Default for BoostFactors {
    fn default() -> Self {
        Self {
            symbol_name: 3.0,
            exact_match: 2.0,
            test_penalty: 0.5,
            generated_penalty: 0.3,
        }
    }
}

/// Check if a path looks like a test file
pub fn is_test_file(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();

    // Check path components
    if path_str.contains("/test/")
        || path_str.contains("/tests/")
        || path_str.contains("/__tests__/")
        || path_str.contains("/spec/")
        || path_str.contains("/__mocks__/")
    {
        return true;
    }

    // Check filename patterns
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        let name_lower = name.to_lowercase();
        if name_lower.ends_with("_test.go")
            || name_lower.ends_with("_test.rs")
            || name_lower.ends_with(".test.ts")
            || name_lower.ends_with(".test.tsx")
            || name_lower.ends_with(".test.js")
            || name_lower.ends_with(".test.jsx")
            || name_lower.ends_with(".spec.ts")
            || name_lower.ends_with(".spec.tsx")
            || name_lower.ends_with(".spec.js")
            || name_lower.ends_with(".spec.jsx")
            || name_lower.starts_with("test_")
            || name_lower == "conftest.py"
        {
            return true;
        }
    }

    false
}

/// Check if a path looks like a generated file
pub fn is_generated_file(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();

    // Check path components
    if path_str.contains("/generated/")
        || path_str.contains("/gen/")
        || path_str.contains("/dist/")
        || path_str.contains("/build/")
        || path_str.contains("/.next/")
        || path_str.contains("/node_modules/")
    {
        return true;
    }

    // Check filename patterns
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        let name_lower = name.to_lowercase();
        if name_lower.ends_with(".min.js")
            || name_lower.ends_with(".min.css")
            || name_lower.ends_with(".generated.ts")
            || name_lower.ends_with(".g.dart")
            || name_lower.ends_with(".pb.go")
            || name_lower.contains(".bundle.")
        {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_is_test_file() {
        assert!(is_test_file(Path::new("src/__tests__/foo.test.ts")));
        assert!(is_test_file(Path::new("tests/unit/bar.rs")));
        assert!(is_test_file(Path::new("pkg/auth/auth_test.go")));
        assert!(!is_test_file(Path::new("src/auth/handler.ts")));
    }

    #[test]
    fn test_is_generated_file() {
        assert!(is_generated_file(Path::new("dist/bundle.js")));
        assert!(is_generated_file(Path::new("src/api.min.js")));
        assert!(is_generated_file(Path::new("node_modules/foo/index.js")));
        assert!(!is_generated_file(Path::new("src/auth/handler.ts")));
    }
}
