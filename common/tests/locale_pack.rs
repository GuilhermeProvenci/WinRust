use std::{fs, path::Path, process::Command};

fn pack(locale: &str, pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut out = b"WRLANG\x01\0".to_vec();
    fn put(out: &mut Vec<u8>, s: &str) { out.extend_from_slice(&(s.len() as u32).to_le_bytes()); out.extend_from_slice(s.as_bytes()); }
    put(&mut out, locale);
    out.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
    for (key, value) in pairs { put(&mut out, key); put(&mut out, value); }
    out
}

fn setup(dir: &Path, valid: bool) {
    let shared = dir.join("program-data/WinRust/locales");
    let user = dir.join("app-data/WinRust");
    fs::create_dir_all(&shared).unwrap();
    fs::create_dir_all(&user).unwrap();
    fs::write(user.join("lang.txt"), "pt-BR").unwrap();
    let bytes = if valid { pack("pt-BR", &[("Hello", "Olá"), ("Balance {amount:.1}: {}", "Saldo {}: {amount:.1}"), ("Good {n}", "Erro {}"), ("Once {0} {0}", "Uma vez {0} {0}")]) } else { b"broken".to_vec() };
    fs::write(shared.join("pt-BR.wrlang"), bytes).unwrap();
}

#[test]
fn child_check() {
    let Ok(mode) = std::env::var("WINRUST_I18N_TEST") else { return };
    let amount = 2.5;
    let n = 8;
    if mode == "valid" {
        assert_eq!(winrust_common::locales::lookup_current("Hello"), Some("Olá"));
        assert_eq!(winrust_common::localized_format!("Balance {amount:.1}: {}", "items"), "Saldo items: 2.5");
    } else {
        assert_eq!(winrust_common::locales::lookup_current("Hello"), None);
        assert_eq!(winrust_common::localized_format!("Balance {amount:.1}: {}", "items"), "Balance 2.5: items");
    }
    assert_eq!(winrust_common::localized_format!("Good {n}"), "Good 8");
    let mut calls = 0;
    assert_eq!(winrust_common::localized_format!("Once {0} {0}", { calls += 1; calls }), "Once 1 1");
    assert_eq!(calls, 1);
    if mode == "valid" {
        winrust_common::set_language("en").unwrap();
        assert_eq!(winrust_common::language(), "en");
        winrust_common::set_language("pt-BR").unwrap();
        let preference = Path::new(&std::env::var_os("APPDATA").unwrap()).join("WinRust/lang.txt");
        assert_eq!(fs::read_to_string(&preference).unwrap(), "pt-BR");
        fs::write(&preference, "en").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert_eq!(winrust_common::language(), "en");
    }
}

#[test]
fn loads_valid_pack_and_falls_back_for_bad_data() {
    let dir = std::env::temp_dir().join(format!("winrust-pack-test-{}", std::process::id()));
    for valid in [true, false] {
        setup(&dir, valid);
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "child_check", "--nocapture"])
            .env("WINRUST_I18N_TEST", if valid { "valid" } else { "invalid" })
            .env("ProgramData", dir.join("program-data"))
            .env("APPDATA", dir.join("app-data"))
            .status().unwrap();
        assert!(status.success());
    }
    fs::remove_dir_all(dir).unwrap();
}
