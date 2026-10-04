use ruvoraq_config::Env;
use std::{
    fs, io,
    path::PathBuf,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ruvoraq-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, text: &str) -> PathBuf {
        let path = self.0.join(".env");
        fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn required_optional_defaults_and_strict_typed_parsing() {
    let env = Env::from_values([
        ("COUNT", "42"),
        ("ENABLED", "true"),
        ("TOKEN", "  exact value  "),
        ("EMPTY", ""),
        ("BAD", "secret-value"),
    ]);
    assert_eq!(env.get::<u16>("COUNT").unwrap(), 42);
    assert!(env.get::<bool>("ENABLED").unwrap());
    assert_eq!(env.get::<String>("TOKEN").unwrap(), "  exact value  ");
    assert_eq!(env.get::<String>("EMPTY").unwrap(), "");
    assert_eq!(env.optional::<u16>("MISSING").unwrap(), None);
    assert_eq!(env.get_or("MISSING", 12u16).unwrap(), 12);
    assert_eq!(
        env.get::<String>("MISSING").unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    let error = env.get_or("BAD", 12u16).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("BAD"));
    assert!(!error.to_string().contains("secret-value"));
    assert!(env.get::<u16>("EMPTY").is_err());
}
#[test]
fn values_and_debug_never_leak_in_parse_errors() {
    struct Leaky;
    impl FromStr for Leaky {
        type Err = String;
        fn from_str(value: &str) -> Result<Self, Self::Err> {
            Err(format!("secret: {value}"))
        }
    }
    let env = Env::from_values([("PASSWORD", "do-not-print-this")]);
    assert!(
        !env.get::<Leaky>("PASSWORD")
            .err()
            .unwrap()
            .to_string()
            .contains("do-not-print-this")
    );
    let debug = format!("{env:?}");
    assert!(!debug.contains("PASSWORD"));
    assert!(!debug.contains("do-not-print-this"));
}
#[test]
fn invalid_variable_names_are_rejected_without_echoing_them() {
    let env = Env::from_values(Vec::<(String, String)>::new());
    for name in ["", "1BAD", "bad-name", "BAD=VALUE", "BAD\nSECRET"] {
        let error = env.optional::<String>(name).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(!error.to_string().contains("SECRET"));
    }
}
#[test]
fn dotenv_quotes_comments_export_and_process_precedence_work_without_mutation() {
    let temp = Temp::new();
    let path=temp.write("# comment\nexport NAME=\"Hello world\"\nCOUNT=21 # comment\nTOKEN='a # b $literal'\nENABLED=false\nMULTILINE=\"line1\\nline2\"\n");
    let before: std::collections::HashMap<_, _> = std::env::vars_os().collect();
    let env = Env::from_values([("COUNT", "42")])
        .with_file(&path)
        .unwrap();
    assert_eq!(env.get::<String>("NAME").unwrap(), "Hello world");
    assert_eq!(env.get::<u16>("COUNT").unwrap(), 42);
    assert_eq!(env.get::<String>("TOKEN").unwrap(), "a # b $literal");
    assert!(!env.get::<bool>("ENABLED").unwrap());
    assert_eq!(env.get::<String>("MULTILINE").unwrap(), "line1\nline2");
    assert!(
        before == std::env::vars_os().collect(),
        "dotenv loading changed the process environment"
    );
    assert_eq!(env.clone().get::<u16>("COUNT").unwrap(), 42);
}
#[test]
fn malformed_duplicate_and_non_utf8_files_return_redacted_errors() {
    let temp = Temp::new();
    for text in [
        "PASSWORD=\"top-secret",
        "PASSWORD=top-secret\nPASSWORD=other-secret\n",
    ] {
        let error = Env::from_values(Vec::<(String, String)>::new())
            .with_file(temp.write(text))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!error.to_string().contains("top-secret"));
        assert!(!error.to_string().contains("other-secret"));
    }
    let path = temp.write("");
    fs::write(&path, [0xff, 0xfe]).unwrap();
    let error = Env::from_file(path).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}
#[test]
fn optional_local_loading_never_searches_parent_directories() {
    let temp = Temp::new();
    let key = format!("RUVORAQ_TEST_LOCAL_{}", std::process::id());
    temp.write(&format!("{key}=local\n"));
    let child = temp.0.join("child");
    fs::create_dir(&child).unwrap();
    assert_eq!(
        Env::load_from(&child)
            .unwrap()
            .optional::<String>(&key)
            .unwrap(),
        None
    );
    assert_eq!(
        Env::load_from(&temp.0)
            .unwrap()
            .get::<String>(&key)
            .unwrap(),
        "local"
    );
    assert_eq!(
        Env::from_file(child.join("missing.env"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
}
#[test]
fn captured_process_environment_takes_precedence_over_file() {
    let temp = Temp::new();
    let (key, value) = std::env::vars().find(|(k, _)| k == "PATH").unwrap();
    temp.write("PATH=file-shadow\n");
    assert_eq!(
        Env::from_file(temp.0.join(".env"))
            .unwrap()
            .get::<String>(&key)
            .unwrap(),
        value
    );
}
#[cfg(unix)]
#[test]
fn invalid_utf8_process_value_does_not_fall_back_to_a_file_or_default() {
    use std::os::unix::ffi::OsStringExt;
    let env = Env::from_values([(
        std::ffi::OsString::from("TOKEN"),
        std::ffi::OsString::from_vec(vec![0xff]),
    )]);
    let error = env.get_or("TOKEN", "default".to_owned()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("UTF-8"));
}
