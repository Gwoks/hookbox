//! Rule path patterns (`/exact`, `/users/:id`, `/v1/*`) compiled into a
//! segment matcher — no regex dependency, so a crafted pattern cannot cause
//! regex blowup. `:name` segments are captured for `{{request.path.<name>}}`
//! templating (§5.7).

use std::collections::BTreeMap;

/// A compiled path: a sequence of segment matchers plus a trailing-wildcard flag.
#[derive(Debug, Clone)]
pub struct CompiledPath {
    segments: Vec<Segment>,
    /// Trailing `/*` — match this prefix followed by anything (or nothing).
    wildcard: bool,
    /// Pure catch-all (`/*`, `*`, `/**`) — matches any path.
    catch_all: bool,
}

#[derive(Debug, Clone)]
enum Segment {
    Literal(String),
    /// `:name` — captures one non-empty, non-slash segment under `name`.
    Param(String),
}

/// Compile a match path (`/users/:id`, `/v1/*`, `/exact`) into a `CompiledPath`.
/// Mirrors `matcher.py::compile_path` semantics.
pub fn compile_path(path: &str) -> CompiledPath {
    let mut p = if path.is_empty() {
        "/*".to_string()
    } else {
        path.to_string()
    };
    if !p.starts_with('/') {
        p = format!("/{p}");
    }
    if p == "/*" || p == "*" || p == "/**" {
        return CompiledPath {
            segments: Vec::new(),
            wildcard: false,
            catch_all: true,
        };
    }
    let mut wildcard = false;
    if let Some(stripped) = p.strip_suffix("/*") {
        wildcard = true;
        p = stripped.to_string();
    }
    let segments: Vec<Segment> = p
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|seg| {
            if seg.len() > 1 && seg.starts_with(':') {
                Segment::Param(seg[1..].to_string())
            } else {
                Segment::Literal(seg.to_string())
            }
        })
        .collect();
    CompiledPath {
        segments,
        wildcard,
        catch_all: false,
    }
}

impl CompiledPath {
    /// Match `path` against this compiled pattern; return captured params
    /// (`:name` -> value) on a match, `None` otherwise.
    pub fn match_path(&self, path: &str) -> Option<BTreeMap<String, String>> {
        if self.catch_all {
            return Some(BTreeMap::new());
        }
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let n = self.segments.len();
        if self.wildcard {
            // Prefix must match; anything (incl. nothing) may follow.
            if parts.len() < n {
                return None;
            }
        } else if parts.len() != n {
            return None;
        }
        let mut params = BTreeMap::new();
        for (i, seg) in self.segments.iter().enumerate() {
            match seg {
                Segment::Literal(lit) => {
                    if parts[i] != lit {
                        return None;
                    }
                }
                Segment::Param(name) => {
                    params.insert(name.clone(), parts[i].to_string());
                }
            }
        }
        Some(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_exact_param_and_wildcard() {
        assert!(compile_path("/users").match_path("/users").is_some());
        assert!(compile_path("/users").match_path("/users/1").is_none());
        let p = compile_path("/users/:id");
        let caps = p.match_path("/users/42").unwrap();
        assert_eq!(caps.get("id").map(String::as_str), Some("42"));
        assert!(p.match_path("/users").is_none());
        let w = compile_path("/v1/*");
        assert!(w.match_path("/v1").is_some());
        assert!(w.match_path("/v1/a/b/c").is_some());
        assert!(w.match_path("/v2/a").is_none());
        assert!(compile_path("/*").match_path("/anything/here").is_some());
    }
}
