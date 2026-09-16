use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub profile: Option<String>,
    pub launch: bool,
    pub vanilla: bool,
    pub hide: bool,
}

pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if ch == ' ' || ch == '-' || ch == '_' {
            out.push('-');
        }
    }
    let out: String = out
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if out.is_empty() {
        "profile".to_string()
    } else {
        out
    }
}

pub fn parse_args(args: &[String]) -> LaunchRequest {
    let mut req = LaunchRequest::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--profile" => {
                if i + 1 < args.len() {
                    req.profile = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--launch" => req.launch = true,
            "--vanilla" => req.vanilla = true,
            "--hide" | "--hidden" | "--minimized" => req.hide = true,
            a if a.starts_with("ronmm://launch") => {
                if let Some(r) = parse_deep_link(a) {
                    req = r;
                }
            }
            a if a.starts_with("ronmm://") => {}
            a if a.starts_with("--profile=") => {
                req.profile = Some(a["--profile=".len()..].to_string());
            }
            _ => {}
        }
        i += 1;
    }
    req
}

pub fn parse_deep_link(url: &str) -> Option<LaunchRequest> {
    // ronmm://launch?profile=Foo&vanilla=1&hide=1
    let query = url.split_once('?').map(|x| x.1).unwrap_or("");
    let mut req = LaunchRequest {
        launch: true,
        hide: true,
        ..Default::default()
    };
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let (k, v) = (kv.next().unwrap_or(""), kv.next().unwrap_or(""));
        let v = url_decode(v);
        match k {
            "profile" => req.profile = Some(v),
            "vanilla" => req.vanilla = v == "1" || v == "true",
            "hide" => req.hide = v != "0" && v != "false",
            "launch" => req.launch = v != "0" && v != "false",
            _ => {}
        }
    }
    Some(req)
}

fn url_decode(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h: String = chars.by_ref().take(2).collect();
            if let Ok(b) = u8::from_str_radix(&h, 16) {
                out.push(b as char);
            }
        } else if c == '+' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_cli_flags() {
        let r = parse_args(&[
            "--profile".into(),
            "Foo".into(),
            "--launch".into(),
            "--hide".into(),
        ]);
        assert_eq!(r.profile.as_deref(), Some("Foo"));
        assert!(r.launch && r.hide);
    }
    #[test]
    fn parses_deep_link() {
        let r = parse_deep_link("ronmm://launch?profile=My%20Prof&vanilla=1").unwrap();
        assert_eq!(r.profile.as_deref(), Some("My Prof"));
        assert!(r.vanilla && r.launch);
    }
    #[test]
    fn slugifies() {
        assert_eq!(slugify("My Cool Profile!"), "my-cool-profile");
        assert_eq!(slugify("!!!"), "profile");
    }

    #[test]
    fn parses_inline_profile_and_alias_flags() {
        let r = parse_args(&[
            "--profile=Inline".into(),
            "--hidden".into(),
            "--minimized".into(),
            "--unknown".into(),
        ]);
        assert_eq!(r.profile.as_deref(), Some("Inline"));
        assert!(r.hide);
        assert!(!r.launch && !r.vanilla);
    }

    #[test]
    fn ignores_a_trailing_profile_flag_and_other_schemes() {
        let r = parse_args(&["--profile".into(), "ronmm://other".into()]);
        // The scheme is not a launch link, so nothing is applied.
        assert_eq!(r.profile, Some("ronmm://other".to_string()));
        assert!(!r.launch);

        let trailing = parse_args(&["--launch".into(), "--profile".into()]);
        assert!(trailing.launch);
        assert_eq!(trailing.profile, None);
    }

    #[test]
    fn deep_links_overwrite_earlier_flags() {
        let r = parse_args(&[
            "--launch".into(),
            "ronmm://launch?profile=Deep&hide=0&launch=0".into(),
        ]);
        assert_eq!(r.profile.as_deref(), Some("Deep"));
        assert!(!r.hide && !r.launch);
    }

    #[test]
    fn deep_link_query_values_are_decoded() {
        let r = parse_deep_link("ronmm://launch?profile=A+B%2Fc&vanilla=true").unwrap();
        assert_eq!(r.profile.as_deref(), Some("A B/c"));
        assert!(r.vanilla);

        // A link with no query still asks for a hidden launch.
        let bare = parse_deep_link("ronmm://launch").unwrap();
        assert!(bare.launch && bare.hide);
        assert_eq!(bare.profile, None);
    }
}
