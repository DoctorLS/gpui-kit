//! Backup and single-patient files.
//!
//! The format is the browser edition's, so files move freely between the two:
//!
//! - a backup is `{app, kind: "backup", v, created, count, encrypted, …}` followed
//!   either by the plain `clinic` and `records`, or by an encrypted payload holding them;
//! - an encrypted payload is AES-256-GCM with a key derived by PBKDF2-SHA256 from the
//!   password (`kdf`, `iter`, `salt`, `iv`, `data`, all binary values base64), exactly
//!   what WebCrypto produces;
//! - a patient file is `{app, kind: "patient", v, id, created, updated, form, imgs}`, and
//!   the earliest exports were just `{v, form, imgs}`.
//!
//! Photos travel inside these files as `data:` URLs.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context as _, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::report::{Clinic, Report, new_id};

const APP: &str = "gastro-sq";
const VERSION: u32 = 2;
/// PBKDF2 rounds for new backups; the browser edition uses the same count.
const ITERATIONS: u32 = 310_000;
pub const MIN_PASSWORD_CHARS: usize = 8;

/// A record as files carry it: photos inline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortableRecord {
    pub id: String,
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub updated: i64,
    pub form: Report,
    #[serde(default)]
    pub imgs: Vec<PortablePhoto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortablePhoto {
    /// A `data:image/…;base64,…` URL.
    pub src: String,
    #[serde(default)]
    pub cap: String,
}

/// The contents of a backup once it is readable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Payload {
    #[serde(default)]
    pub clinic: Option<Clinic>,
    #[serde(default)]
    pub records: Vec<PortableRecord>,
}

/// An encrypted backup's ciphertext and the parameters needed to open it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sealed {
    pub kdf: String,
    pub iter: u32,
    pub salt: String,
    pub iv: String,
    pub data: String,
}

/// What a file chosen for import turned out to hold.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportFile {
    Backup(Payload),
    /// A backup that needs its password before it can be read.
    SealedBackup {
        sealed: Sealed,
        count: usize,
    },
    Patient(Box<PortableRecord>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrongPassword;

pub fn parse(text: &str) -> Result<ImportFile> {
    let value: Value = serde_json::from_str(text).context("the file is not JSON")?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("unexpected JSON"))?;
    match object.get("kind").and_then(Value::as_str) {
        Some("backup") => {
            if object.get("encrypted").and_then(Value::as_bool) == Some(true) {
                let sealed: Sealed = serde_json::from_value(value.clone())?;
                if sealed.kdf != "PBKDF2-SHA256" {
                    bail!("unsupported key derivation `{}`", sealed.kdf);
                }
                let count = object.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
                Ok(ImportFile::SealedBackup { sealed, count })
            } else {
                Ok(ImportFile::Backup(serde_json::from_value(value)?))
            }
        }
        _ if object.contains_key("form") => {
            let mut object = object.clone();
            // Early patient exports had no identity; give them one so they archive.
            if !object.get("id").is_some_and(Value::is_string) {
                object.insert("id".into(), Value::String(new_id()));
            }
            Ok(ImportFile::Patient(Box::new(serde_json::from_value(
                Value::Object(object),
            )?)))
        }
        _ => bail!("not a report or backup"),
    }
}

pub fn open(sealed: &Sealed, password: &str) -> Result<Payload, WrongPassword> {
    let decode = |s: &str| BASE64.decode(s).map_err(|_| WrongPassword);
    let salt = decode(&sealed.salt)?;
    let iv = decode(&sealed.iv)?;
    let data = decode(&sealed.data)?;
    if iv.len() != 12 {
        return Err(WrongPassword);
    }
    let cipher = Aes256Gcm::new(&derive_key(password, &salt, sealed.iter).into());
    let plain = cipher
        .decrypt(Nonce::from_slice(&iv), data.as_ref())
        .map_err(|_| WrongPassword)?;
    serde_json::from_slice(&plain).map_err(|_| WrongPassword)
}

/// Serializes a backup, encrypting it when a password is given.
pub fn write_backup(payload: &Payload, password: Option<&str>) -> Result<String> {
    let mut out = Map::new();
    out.insert("app".into(), APP.into());
    out.insert("kind".into(), "backup".into());
    out.insert("v".into(), VERSION.into());
    out.insert(
        "created".into(),
        chrono::Utc::now()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            .into(),
    );
    out.insert("count".into(), payload.records.len().into());
    match password {
        Some(password) => {
            out.insert("encrypted".into(), true.into());
            let sealed = seal(&serde_json::to_vec(payload)?, password)?;
            let Value::Object(sealed) = serde_json::to_value(sealed)? else {
                unreachable!("a struct serializes to an object");
            };
            out.extend(sealed);
        }
        None => {
            out.insert("encrypted".into(), false.into());
            let Value::Object(body) = serde_json::to_value(payload)? else {
                unreachable!("a struct serializes to an object");
            };
            out.extend(body);
        }
    }
    Ok(serde_json::to_string(&Value::Object(out))?)
}

pub fn write_patient(record: &PortableRecord) -> Result<String> {
    let mut value = json!({ "app": APP, "kind": "patient", "v": VERSION });
    let Value::Object(body) = serde_json::to_value(record)? else {
        unreachable!("a struct serializes to an object");
    };
    value.as_object_mut().expect("literal object").extend(body);
    Ok(serde_json::to_string(&value)?)
}

fn seal(plain: &[u8], password: &str) -> Result<Sealed> {
    let mut salt = [0u8; 16];
    let mut iv = [0u8; 12];
    getrandom::getrandom(&mut salt).map_err(|err| anyhow!("{err}"))?;
    getrandom::getrandom(&mut iv).map_err(|err| anyhow!("{err}"))?;
    let cipher = Aes256Gcm::new(&derive_key(password, &salt, ITERATIONS).into());
    let data = cipher
        .encrypt(Nonce::from_slice(&iv), plain)
        .map_err(|_| anyhow!("encryption failed"))?;
    Ok(Sealed {
        kdf: "PBKDF2-SHA256".into(),
        iter: ITERATIONS,
        salt: BASE64.encode(salt),
        iv: BASE64.encode(iv),
        data: BASE64.encode(data),
    })
}

fn derive_key(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

/// `data:image/png;base64,…` → (`png`, bytes).
pub fn decode_data_url(url: &str) -> Option<(String, Vec<u8>)> {
    let rest = url.strip_prefix("data:")?;
    let (meta, data) = rest.split_once(',')?;
    let mime = meta.strip_suffix(";base64")?;
    let extension = match mime {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        _ => return None,
    };
    Some((extension.into(), BASE64.decode(data).ok()?))
}

pub fn encode_data_url(extension: &str, bytes: &[u8]) -> String {
    let mime = match extension {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/jpeg",
    };
    format!("data:{mime};base64,{}", BASE64.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> Payload {
        Payload {
            clinic: Some(Clinic {
                c_name: "Klinika".into(),
                ..Clinic::default()
            }),
            records: vec![PortableRecord {
                id: "abc".into(),
                created: 1,
                updated: 2,
                form: Report {
                    pt_name: "Ardit Hoxha".into(),
                    ..Report::default()
                },
                imgs: vec![PortablePhoto {
                    src: encode_data_url("png", &[1, 2, 3]),
                    cap: "antrum".into(),
                }],
            }],
        }
    }

    // A test-sized iteration count: the real one makes the suite slow in debug builds.
    fn quick_seal(payload: &Payload, password: &str) -> Sealed {
        let salt = [7u8; 16];
        let iv = [9u8; 12];
        let cipher = Aes256Gcm::new(&derive_key(password, &salt, 1000).into());
        let data = cipher
            .encrypt(
                Nonce::from_slice(&iv),
                serde_json::to_vec(payload).unwrap().as_ref(),
            )
            .unwrap();
        Sealed {
            kdf: "PBKDF2-SHA256".into(),
            iter: 1000,
            salt: BASE64.encode(salt),
            iv: BASE64.encode(iv),
            data: BASE64.encode(data),
        }
    }

    #[test]
    fn test_plain_backup_round_trip() {
        let text = write_backup(&payload(), None).unwrap();
        assert_eq!(parse(&text).unwrap(), ImportFile::Backup(payload()));
    }

    #[test]
    fn test_sealed_backup_hides_names_and_checks_password() {
        let sealed = quick_seal(&payload(), "sekret123");
        assert!(!sealed.data.contains("Ardit"));
        assert_eq!(open(&sealed, "gabim1234"), Err(WrongPassword));
        assert_eq!(open(&sealed, "sekret123").unwrap(), payload());
    }

    #[test]
    fn test_sealed_backup_is_recognised() {
        let text = write_backup(&Payload::default(), Some("sekret123")).unwrap();
        let ImportFile::SealedBackup { sealed, count } = parse(&text).unwrap() else {
            panic!("expected a sealed backup");
        };
        assert_eq!(count, 0);
        assert_eq!(sealed.iter, ITERATIONS);
    }

    /// Ciphertext produced by WebCrypto in the browser edition, sealed with the
    /// password `sekret123`, 1000 PBKDF2 rounds, salt 16×0x07, IV 12×0x09.
    #[test]
    fn test_opens_webcrypto_output() {
        let sealed = Sealed {
            kdf: "PBKDF2-SHA256".into(),
            iter: 1000,
            salt: BASE64.encode([7u8; 16]),
            iv: BASE64.encode([9u8; 12]),
            data: "NTQt2RECNwjy9LEdaASZbjiUFZC5ij4N8sJ3z7RMeD/K5yngMGbrxwrT/2++dQBdWr/X0YU4+l5DFFiWDLSF6b/XZNjZV6GLLtRIckfOd+w2QTS4jCpWnInneDRYwY0jd6ycCu3XX45fe7ky3TJsuyvefckWM16U0R0Tn2ANhkjn5EXuOy5k8yLO22eWmX1NcBwig78=".into(),
        };
        let payload = open(&sealed, "sekret123").unwrap();
        assert_eq!(payload.clinic.unwrap().c_name, "Klinika");
        assert_eq!(payload.records[0].form.pt_name, "Ardit Hoxha");
        assert_eq!(payload.records[0].form.varices, "F1");
        assert_eq!(open(&sealed, "sekret124"), Err(WrongPassword));
    }

    #[test]
    fn test_patient_files() {
        let record = payload().records.remove(0);
        let text = write_patient(&record).unwrap();
        assert_eq!(parse(&text).unwrap(), ImportFile::Patient(Box::new(record)));

        let ImportFile::Patient(legacy) =
            parse(r#"{"v":2,"form":{"pt_name":"X"},"imgs":[]}"#).unwrap()
        else {
            panic!("expected a patient");
        };
        assert_eq!(legacy.form.pt_name, "X");
        assert!(!legacy.id.is_empty());

        assert!(parse(r#"{"hello":1}"#).is_err());
    }

    #[test]
    fn test_data_urls() {
        let url = encode_data_url("png", &[1, 2, 3]);
        assert_eq!(url, "data:image/png;base64,AQID");
        assert_eq!(decode_data_url(&url), Some(("png".into(), vec![1, 2, 3])));
        assert_eq!(decode_data_url("data:text/plain;base64,AQID"), None);
    }
}
