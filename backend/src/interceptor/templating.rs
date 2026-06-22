//! Sandboxed response-templating engine — PORT of `app/interceptor/templating.py`
//! (§5.7, AC-16..19, AC-S5).
//!
//! A hand-written single-pass scanner over the closed §5.7 tag set. There is NO
//! eval/exec, NO general template engine, and NO format-string over user text:
//! the inner expression is tokenized and matched against a fixed handler set, so
//! SSTI is structurally impossible. `{{7*7}}`, `{{config}}`, `{{''.__class__}}`,
//! `{{self}}` are all *unknown tags* → returned verbatim, executing nothing.
//!
//! DoS bounds: a template longer than `TEMPLATE_MAX_SIZE` is returned unrendered;
//! at most `TEMPLATE_MAX_TAGS` substitutions are performed (further tags are left
//! literal). Unknown / malformed tags are left literal and never error the path.

use std::collections::BTreeMap;

use chrono::Utc;
use rand::Rng;
use uuid::Uuid;

use crate::helpers::jsonpath_lite;

const OPEN: &str = "{{";
const CLOSE: &str = "}}";

/// Everything a template tag may read. All values are already-extracted plain
/// data — the engine performs no attribute access on user objects.
#[derive(Debug, Default, Clone)]
pub struct TemplateContext {
    pub method: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>, // lower-cased names
    pub path_params: BTreeMap<String, String>,
    pub body: String,
    pub state: BTreeMap<String, String>,
}

impl TemplateContext {
    fn header(&self, name: &str) -> String {
        self.headers.get(&name.to_ascii_lowercase()).cloned().unwrap_or_default()
    }
}

/// Tokenize the part after a verb into args. Single-quoted literals and bare
/// (non-space) tokens are supported. `None` on a malformed (unterminated) quote.
fn tokenize_args(s: &str) -> Option<Vec<String>> {
    let mut args = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '\'' {
            // find closing quote
            let mut j = i + 1;
            while j < n && chars[j] != '\'' {
                j += 1;
            }
            if j >= n {
                return None; // unterminated
            }
            args.push(chars[i + 1..j].iter().collect());
            i = j + 1;
        } else {
            let mut j = i;
            while j < n && !chars[j].is_whitespace() {
                j += 1;
            }
            args.push(chars[i..j].iter().collect());
            i = j;
        }
    }
    Some(args)
}

fn t_now(args: &[String]) -> Option<String> {
    let fmt = args.first().map(String::as_str).unwrap_or("iso");
    let now = Utc::now();
    match fmt {
        "iso" => Some(now.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)),
        "unix" => Some(now.timestamp().to_string()),
        "epoch_ms" => Some(now.timestamp_millis().to_string()),
        _ => None,
    }
}

fn t_random(args: &[String]) -> Option<String> {
    let kind = args.first()?.as_str();
    match kind {
        "uuid" => Some(Uuid::new_v4().to_string()),
        "int" => {
            if args.len() != 3 {
                return None;
            }
            let (mut lo, mut hi): (i64, i64) = (args[1].parse().ok()?, args[2].parse().ok()?);
            if lo > hi {
                std::mem::swap(&mut lo, &mut hi);
            }
            let mut rng = rand::thread_rng();
            Some(rng.gen_range(lo..=hi).to_string())
        }
        "hex" => {
            if args.len() != 2 {
                return None;
            }
            let length: usize = args[1].parse().ok()?;
            if length == 0 || length > 4096 {
                return None;
            }
            let nbytes = (length + 1) / 2;
            let mut rng = rand::thread_rng();
            let mut buf = vec![0u8; nbytes];
            rng.fill(&mut buf[..]);
            Some(hex::encode(buf)[..length].to_string())
        }
        _ => None,
    }
}

fn t_request(ctx: &TemplateContext, expr: &str) -> Option<String> {
    let rest = expr.strip_prefix("request.")?;
    match rest {
        "method" => Some(ctx.method.clone()),
        "path" => Some(ctx.path.clone()),
        "body" => Some(ctx.body.clone()),
        _ => {
            if let Some(k) = rest.strip_prefix("query.") {
                Some(ctx.query.get(k).cloned().unwrap_or_default())
            } else if let Some(k) = rest.strip_prefix("path.") {
                Some(ctx.path_params.get(k).cloned().unwrap_or_default())
            } else if let Some(k) = rest.strip_prefix("header.") {
                Some(ctx.header(k))
            } else if let Some(jp) = rest.strip_prefix("body.") {
                Some(jsonpath_lite(&ctx.body, jp).unwrap_or_default())
            } else {
                None
            }
        }
    }
}

fn t_state(ctx: &TemplateContext, expr: &str) -> Option<String> {
    let key = expr.strip_prefix("state.")?;
    if key.is_empty() {
        return None;
    }
    Some(ctx.state.get(key).cloned().unwrap_or_default())
}

/// Resolve one tag's inner text (already stripped of `{{`/`}}`). `None` for an
/// unknown/malformed tag (caller leaves the raw `{{...}}` literal). Never panics.
fn resolve_tag(inner: &str, ctx: &TemplateContext) -> Option<String> {
    let expr = inner.trim();
    if expr.is_empty() {
        return None;
    }
    // request.* / state.* are dotted families with no whitespace verb form.
    if expr.starts_with("request.") {
        if expr.contains(char::is_whitespace) {
            return None;
        }
        return t_request(ctx, expr);
    }
    if expr.starts_with("state.") {
        if expr.contains(char::is_whitespace) {
            return None;
        }
        return t_state(ctx, expr);
    }
    // verb-style tags: "now", "now 'iso'", "random 'uuid'", "random 'int' 1 9"
    let mut split = expr.splitn(2, char::is_whitespace);
    let verb = split.next().unwrap_or("");
    let arg_str = split.next().unwrap_or("");
    match verb {
        "now" => tokenize_args(arg_str).and_then(|a| t_now(&a)),
        "random" => tokenize_args(arg_str).and_then(|a| t_random(&a)),
        _ => None,
    }
}

/// Render `template` against `ctx` with the §5.7 sandboxed grammar. Single pass,
/// left-to-right. Bounded by `template_max_size` / `template_max_tags`.
pub fn render(template: &str, ctx: &TemplateContext, max_size: usize, max_tags: usize) -> String {
    if template.is_empty() {
        return String::new();
    }
    if template.len() > max_size {
        return template.to_string();
    }
    let mut out = String::with_capacity(template.len());
    let mut i = 0;
    let bytes = template.as_bytes();
    let n = bytes.len();
    let mut tags_rendered = 0usize;

    while i < n {
        match template[i..].find(OPEN) {
            None => {
                out.push_str(&template[i..]);
                break;
            }
            Some(rel_start) => {
                let start = i + rel_start;
                out.push_str(&template[i..start]);
                let after_open = start + OPEN.len();
                let end = match template[after_open..].find(CLOSE) {
                    None => {
                        // no closing braces — rest is literal.
                        out.push_str(&template[start..]);
                        break;
                    }
                    Some(rel_end) => after_open + rel_end,
                };
                let inner = &template[after_open..end];
                let raw = &template[start..end + CLOSE.len()];

                if tags_rendered >= max_tags {
                    out.push_str(raw);
                    i = end + CLOSE.len();
                    continue;
                }
                // A nested "{{" inside the span: emit the opening braces literally
                // and resume just after them so a following valid tag still scans.
                if inner.contains(OPEN) {
                    out.push_str(OPEN);
                    i = after_open;
                    continue;
                }
                match resolve_tag(inner, ctx) {
                    Some(resolved) => {
                        out.push_str(&resolved);
                        tags_rendered += 1;
                    }
                    None => out.push_str(raw), // unknown/malformed -> literal
                }
                i = end + CLOSE.len();
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> TemplateContext {
        let mut c = TemplateContext {
            method: "POST".into(),
            path: "/users/42".into(),
            body: r#"{"user":{"name":"Ada"}}"#.into(),
            ..Default::default()
        };
        c.query.insert("q".into(), "search".into());
        c.path_params.insert("id".into(), "42".into());
        c.headers.insert("x-test".into(), "hdr".into());
        c.state.insert("count".into(), "7".into());
        c
    }

    fn r(t: &str) -> String {
        render(t, &ctx(), 256_000, 500)
    }

    #[test]
    fn ssti_probes_returned_verbatim() {
        for probe in ["{{7*7}}", "{{config}}", "{{''.__class__.__mro__}}", "{{self}}"] {
            assert_eq!(r(probe), probe, "probe must be left literal");
        }
    }

    #[test]
    fn request_and_state_tags() {
        assert_eq!(r("{{request.method}}"), "POST");
        assert_eq!(r("{{request.path}}"), "/users/42");
        assert_eq!(r("{{request.query.q}}"), "search");
        assert_eq!(r("{{request.path.id}}"), "42");
        assert_eq!(r("{{request.header.x-test}}"), "hdr");
        assert_eq!(r("{{request.body.user.name}}"), "Ada");
        assert_eq!(r("{{state.count}}"), "7");
        assert_eq!(r("{{state.missing}}"), ""); // present-but-empty
    }

    #[test]
    fn now_and_random_tags() {
        assert!(r("{{now 'unix'}}").parse::<i64>().is_ok());
        let u = r("{{random 'uuid'}}");
        assert_eq!(u.len(), 36);
        let h = r("{{random 'hex' 8}}");
        assert_eq!(h.len(), 8);
        let n: i64 = r("{{random 'int' 5 5}}").parse().unwrap();
        assert_eq!(n, 5);
        // malformed -> literal
        assert_eq!(r("{{now 'bogus'}}"), "{{now 'bogus'}}");
        assert_eq!(r("{{random}}"), "{{random}}");
    }

    #[test]
    fn size_and_tag_caps() {
        let big = "x".repeat(20);
        assert_eq!(render("{{request.method}}", &ctx(), 5, 500), "{{request.method}}"); // over size -> unrendered
        let _ = big;
        // tag cap: only the first tag renders, the rest stay literal.
        let out = render("{{request.method}}{{request.method}}", &ctx(), 256_000, 1);
        assert_eq!(out, "POST{{request.method}}");
    }

    #[test]
    fn mixed_literal_and_unterminated() {
        assert_eq!(r("hello {{request.path}} world"), "hello /users/42 world");
        assert_eq!(r("dangling {{ no close"), "dangling {{ no close");
    }
}
