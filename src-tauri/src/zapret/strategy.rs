use crate::paths;

/// Creates the user list files winws references but that ship empty/absent in
/// the release. Mirrors `:load_user_lists` in service.bat — without these,
/// winws fails with "cannot access ipset file ..." and exits immediately.
pub fn ensure_user_lists() -> std::io::Result<()> {
    let lists = paths::zapret_lists_dir();
    std::fs::create_dir_all(&lists)?;
    for (name, default) in [
        ("ipset-exclude-user.txt", "203.0.113.113/32"),
        ("list-general-user.txt", "domain.example.abc"),
        ("list-exclude-user.txt", "domain.example.abc"),
    ] {
        let path = lists.join(name);
        if !path.exists() {
            std::fs::write(&path, format!("{default}\r\n"))?;
        }
    }
    Ok(())
}

/// Game filter state, mirroring `service.bat` (`utils\game_filter.enabled`):
/// missing file -> disabled (ports collapse to dummy "12"), otherwise the
/// file's first line selects "all" | "tcp" | "udp".
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameFilter {
    /// off | all | tcp | udp
    pub mode: String,
    pub tcp: String,
    pub udp: String,
}

pub fn game_filter() -> GameFilter {
    let flag = paths::zapret_utils_dir().join("game_filter.enabled");
    let mode = std::fs::read_to_string(&flag)
        .ok()
        .and_then(|t| t.lines().next().map(|l| l.trim().to_lowercase()))
        .unwrap_or_default();

    match mode.as_str() {
        "all" => GameFilter {
            mode: "all".into(),
            tcp: "1024-65535".into(),
            udp: "1024-65535".into(),
        },
        "tcp" => GameFilter {
            mode: "tcp".into(),
            tcp: "1024-65535".into(),
            udp: "12".into(),
        },
        "udp" => GameFilter {
            mode: "udp".into(),
            tcp: "12".into(),
            udp: "1024-65535".into(),
        },
        _ if flag.exists() => GameFilter {
            mode: "udp".into(),
            tcp: "12".into(),
            udp: "1024-65535".into(),
        },
        _ => GameFilter {
            mode: "off".into(),
            tcp: "12".into(),
            udp: "12".into(),
        },
    }
}

pub fn set_game_filter_mode(mode: &str) -> Result<(), String> {
    let utils = paths::zapret_utils_dir();
    std::fs::create_dir_all(&utils).map_err(|e| e.to_string())?;
    let flag = utils.join("game_filter.enabled");
    match mode {
        "off" => {
            if flag.exists() {
                std::fs::remove_file(&flag).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        "all" | "tcp" | "udp" => std::fs::write(&flag, mode).map_err(|e| e.to_string()),
        _ => Err(format!("unknown game filter mode: {mode}")),
    }
}

/// Lists strategy bats from the zapret release dir, excluding service*.bat.
/// Sorted naturally (digit runs padded), same as Flowseal's picker.
#[tauri::command]
pub fn list_strategies() -> Result<Vec<String>, String> {
    let dir = paths::zapret_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.to_lowercase().ends_with(".bat") && !n.to_lowercase().starts_with("service"))
        .collect();
    names.sort_by_key(|n| natural_key(n));
    Ok(names)
}

fn natural_key(name: &str) -> String {
    let mut out = String::new();
    let mut digits = String::new();
    for ch in name.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            if !digits.is_empty() {
                out.push_str(&format!("{:0>8}", digits));
                digits.clear();
            }
            out.push(ch);
        }
    }
    if !digits.is_empty() {
        out.push_str(&format!("{:0>8}", digits));
    }
    out
}

/// Extracts the winws.exe argument list from a strategy .bat file.
///
/// The bats follow a fixed layout:
///   start "zapret: ..." /min "%BIN%winws.exe" --arg1 ... ^
///   --arg2 ... ^
///   --argN
/// We join the caret-continued lines, take everything after `winws.exe"`,
/// tokenize quote-aware and substitute %BIN%/%LISTS%/%GameFilter*% variables.
pub fn winws_args(bat_name: &str) -> Result<Vec<String>, String> {
    let path = paths::zapret_dir().join(bat_name);
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    let bin = format!("{}\\", paths::zapret_bin_dir().display());
    let lists = format!("{}\\", paths::zapret_lists_dir().display());
    let root = format!("{}\\", paths::zapret_dir().display());
    parse_winws_args(&content, &bin, &lists, &root, &game_filter())
}

/// Pure parsing core, separated from the filesystem for testability.
fn parse_winws_args(
    content: &str,
    bin: &str,
    lists: &str,
    root: &str,
    gf: &GameFilter,
) -> Result<Vec<String>, String> {
    let lines: Vec<&str> = content.lines().collect();
    let start_idx = lines
        .iter()
        .position(|l| l.to_lowercase().contains("winws.exe"))
        .ok_or("winws.exe launch line not found in strategy file")?;

    // Join continuation lines (trailing ^)
    let mut joined = String::new();
    let mut idx = start_idx;
    loop {
        let line = lines.get(idx).unwrap_or(&"").trim_end();
        let trimmed = line.trim_end_matches('^').trim_end();
        joined.push_str(trimmed);
        joined.push(' ');
        if !line.ends_with('^') || idx + 1 >= lines.len() {
            break;
        }
        idx += 1;
    }

    // Everything after `winws.exe"` is the argument list.
    let lower = joined.to_lowercase();
    let pos = lower.find("winws.exe").ok_or("winws.exe not found")?;
    let mut rest = &joined[pos + "winws.exe".len()..];
    rest = rest.strip_prefix('"').unwrap_or(rest);

    let tokens = tokenize(rest);
    let args: Vec<String> = tokens
        .into_iter()
        .map(|t| {
            t.replace("%BIN%", bin)
                .replace("%LISTS%", lists)
                .replace("%~dp0", root)
                .replace("%GameFilterTCP%", &gf.tcp)
                .replace("%GameFilterUDP%", &gf.udp)
                .replace(
                    "%GameFilter%",
                    if gf.mode == "off" { "12" } else { "1024-65535" },
                )
        })
        .collect();

    if args.is_empty() {
        return Err("strategy produced an empty argument list".into());
    }
    Ok(args)
}

/// Splits a command tail into tokens, honouring double quotes (quotes are
/// stripped, their content kept verbatim including spaces).
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in input.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Rebuilds the argument string used for `sc create ... binPath=`, with
/// path-like values wrapped in escaped quotes — same format service.bat uses.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn service_args_string(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if let Some(eq) = arg.find('=') {
                let (key, value) = arg.split_at(eq + 1);
                if value.contains('\\') || value.contains(' ') {
                    return format!("{key}\"{value}\"");
                }
            } else if arg.contains(' ') {
                return format!("\"{arg}\"");
            }
            arg.clone()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real fragment of Flowseal's general.bat (1.9.9a), shortened.
    const GENERAL_BAT: &str = "@echo off\r\nchcp 65001 > nul\r\n\r\ncd /d \"%~dp0\"\r\nset \"BIN=%~dp0bin\\\"\r\nset \"LISTS=%~dp0lists\\\"\r\n\r\nstart \"zapret: %~n0\" /min \"%BIN%winws.exe\" --wf-tcp=80,443,%GameFilterTCP% --wf-udp=443,%GameFilterUDP% ^\r\n--filter-udp=443 --hostlist=\"%LISTS%list-general.txt\" --dpi-desync=fake --dpi-desync-repeats=6 --dpi-desync-fake-quic=\"%BIN%quic_initial_www_google_com.bin\" --new ^\r\n--filter-tcp=80,443 --hostlist=\"%LISTS%list-general-user.txt\" --dpi-desync=multisplit --dpi-desync-split-pos=1\r\n";

    #[test]
    fn parses_general_bat() {
        let gf = GameFilter {
            mode: "off".into(),
            tcp: "12".into(),
            udp: "12".into(),
        };
        let args = parse_winws_args(
            GENERAL_BAT,
            "C:\\EasyZapret\\zapret\\bin\\",
            "C:\\EasyZapret\\zapret\\lists\\",
            "C:\\EasyZapret\\zapret\\",
            &gf,
        )
        .unwrap();

        assert_eq!(args[0], "--wf-tcp=80,443,12");
        assert_eq!(args[1], "--wf-udp=443,12");
        assert!(args
            .contains(&"--hostlist=C:\\EasyZapret\\zapret\\lists\\list-general.txt".to_string()));
        assert!(args.contains(
            &"--dpi-desync-fake-quic=C:\\EasyZapret\\zapret\\bin\\quic_initial_www_google_com.bin"
                .to_string()
        ));
        // caret continuation tokens must not leak into args
        assert!(!args.iter().any(|a| a == "^"));
        // both --new sections are preserved
        assert_eq!(args.iter().filter(|a| *a == "--new").count(), 1);
        assert_eq!(args.last().unwrap(), "--dpi-desync-split-pos=1");
    }

    #[test]
    fn game_filter_substitution() {
        let gf = GameFilter {
            mode: "all".into(),
            tcp: "1024-65535".into(),
            udp: "1024-65535".into(),
        };
        let args = parse_winws_args(GENERAL_BAT, "B\\", "L\\", "R\\", &gf).unwrap();
        assert_eq!(args[0], "--wf-tcp=80,443,1024-65535");
        assert_eq!(args[1], "--wf-udp=443,1024-65535");
    }

    #[test]
    fn service_args_quoting() {
        let args = vec![
            "--wf-tcp=80,443".to_string(),
            "--hostlist=C:\\EasyZapret\\zapret\\lists\\list-general.txt".to_string(),
        ];
        assert_eq!(
            service_args_string(&args),
            "--wf-tcp=80,443 --hostlist=\"C:\\EasyZapret\\zapret\\lists\\list-general.txt\""
        );
    }

    #[test]
    fn natural_sort_pads_digits() {
        let mut names = [
            "general (ALT10).bat".to_string(),
            "general (ALT2).bat".to_string(),
            "general (ALT).bat".to_string(),
            "general.bat".to_string(),
        ];
        names.sort_by_key(|n| natural_key(n));
        assert_eq!(names[0], "general (ALT).bat");
        assert_eq!(names[1], "general (ALT2).bat");
        assert_eq!(names[2], "general (ALT10).bat");
    }
}
