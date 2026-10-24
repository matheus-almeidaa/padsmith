use padsmith_core::Profile;
use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::MetadataExt,
};

pub fn matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let value = value.to_lowercase();
    let (p, v) = (pattern.as_bytes(), value.as_bytes());
    let (mut i, mut j, mut star, mut retry) = (0, 0, None, 0);
    while j < v.len() {
        if i < p.len() && (p[i] == b'?' || p[i] == v[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == b'*' {
            star = Some(i);
            i += 1;
            retry = j;
        } else if let Some(s) = star {
            retry += 1;
            j = retry;
            i = s + 1;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == b'*' {
        i += 1;
    }
    i == p.len()
}
pub fn choose<P: Borrow<Profile>>(
    profiles: &BTreeMap<String, P>,
    processes: &[String],
    device: &str,
) -> Option<String> {
    profiles
        .values()
        .map(Borrow::borrow)
        .filter(|p| {
            !p.processes.is_empty()
                && (p.devices.is_empty()
                    || p.devices.iter().any(|pattern| matches(pattern, device)))
                && p.processes
                    .iter()
                    .any(|pattern| processes.iter().any(|name| matches(pattern, name)))
        })
        .max_by(|a, b| a.priority.cmp(&b.priority).then_with(|| b.id.cmp(&a.id)))
        .map(|p| p.id.clone())
}
fn basename(s: &str) -> &str {
    s.rsplit(['/', '\\']).next().unwrap_or(s)
}
pub fn names_from_cmdline(command: &[u8]) -> Vec<String> {
    command
        .split(|c| *c == 0)
        .filter_map(|part| std::str::from_utf8(part).ok())
        .enumerate()
        .filter(|(i, s)| *i == 0 || s.to_ascii_lowercase().ends_with(".exe"))
        .map(|(_, s)| basename(s).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
/// A low-frequency process census; this never runs on the input actor.
pub fn snapshot() -> std::io::Result<Vec<String>> {
    let mut names = BTreeSet::new();
    // SAFETY: getuid has no preconditions.
    let uid = unsafe { libc::getuid() };
    for entry in fs::read_dir("/proc")?.flatten() {
        if !entry
            .file_name()
            .as_encoded_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            continue;
        }
        let path = entry.path();
        if !entry.metadata().is_ok_and(|m| m.uid() == uid) {
            continue;
        }
        if let Ok(exe) = fs::read_link(path.join("exe"))
            && let Some(name) = exe.file_name()
        {
            names.insert(name.to_string_lossy().into_owned());
        }
        if let Ok(command) = fs::read(path.join("cmdline")) {
            names.extend(names_from_cmdline(&command));
        }
    }
    Ok(names.into_iter().collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proton_names_and_deterministic_priority() {
        let names =
            names_from_cmdline(b"/bin/wine64\0C:\\Games\\Example Game\\GAME.EXE\0--other\0");
        assert_eq!(names, vec!["wine64", "GAME.EXE"]);
        let a = Profile {
            id: "a".into(),
            processes: vec!["game.exe".into()],
            ..Default::default()
        };
        let b = Profile {
            id: "b".into(),
            processes: vec!["*.exe".into()],
            priority: 2,
            devices: vec!["054c:*".into()],
            ..Default::default()
        };
        let profiles = BTreeMap::from([("a".into(), a), ("b".into(), b)]);
        assert_eq!(
            choose(&profiles, &names, "054c:09cc:serial"),
            Some("b".into())
        );
        assert_eq!(
            choose(&profiles, &names, "045e:028e:other"),
            Some("a".into())
        );
        assert_eq!(choose(&profiles, &[], "054c:09cc:serial"), None);
        assert!(!matches("game.exe", "not-game.exe"));
        assert!(matches("g?me.*", "GAME.EXE"));
    }
}
