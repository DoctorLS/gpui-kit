//! The patient archive on disk.
//!
//! ```text
//! <data dir>/gastroskopi/
//! ├── records/<id>.json   one report each, photos by file name
//! ├── photos/<file>       downscaled JPEG, or the original bytes of an imported image
//! ├── clinic.json
//! └── prefs.json          when the last backup was made
//! ```
//!
//! Unlike the browser edition's storage, clearing a browser or moving the program
//! file cannot touch these files. Every write goes to a temporary file first and is
//! then renamed over the old one, so a crash never leaves half a report behind.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::backup::{self, Payload, PortablePhoto, PortableRecord};
use crate::report::{Clinic, Photo, Record, new_id};

/// Longest side of a stored photo; the browser edition used the same limit.
const PHOTO_MAX_SIDE: u32 = 1600;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// Milliseconds since the Unix epoch, `0` when there has never been one.
    pub last_backup: i64,
}

/// How a restore changed the archive.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MergeSummary {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
}

#[derive(Clone)]
pub struct Archive {
    dir: PathBuf,
}

impl Archive {
    /// The archive in the platform's data directory, or in `GASTROSKOPI_DATA_DIR`.
    pub fn open_default() -> Result<Self> {
        let dir = match std::env::var_os("GASTROSKOPI_DATA_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => dirs::data_dir()
                .context("the system has no data directory")?
                .join("gastroskopi"),
        };
        Self::open(dir)
    }

    pub fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let archive = Self { dir: dir.into() };
        fs::create_dir_all(archive.records_dir())?;
        fs::create_dir_all(archive.photos_dir())?;
        Ok(archive)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn records_dir(&self) -> PathBuf {
        self.dir.join("records")
    }

    fn photos_dir(&self) -> PathBuf {
        self.dir.join("photos")
    }

    fn record_path(&self, id: &str) -> PathBuf {
        // IDs come from files people import; keep them from naming another path.
        let safe: String = id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        self.records_dir().join(format!("{safe}.json"))
    }

    pub fn photo_path(&self, file: &str) -> PathBuf {
        self.photos_dir()
            .join(Path::new(file).file_name().unwrap_or_default())
    }

    /// Every stored record. A file that cannot be read is skipped and reported.
    pub fn load_all(&self) -> (Vec<Record>, Vec<PathBuf>) {
        let mut records = Vec::new();
        let mut unreadable = Vec::new();
        let Ok(entries) = fs::read_dir(self.records_dir()) else {
            return (records, unreadable);
        };
        for path in entries.flatten().map(|entry| entry.path()) {
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            match fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok())
            {
                Some(record) => records.push(record),
                None => unreadable.push(path),
            }
        }
        (records, unreadable)
    }

    pub fn save(&self, record: &Record) -> Result<()> {
        write_atomic(
            &self.record_path(&record.id),
            &serde_json::to_vec_pretty(record)?,
        )
    }

    /// Removes the record and the photos it owns.
    pub fn delete(&self, record: &Record) -> Result<()> {
        for photo in &record.photos {
            let _ = fs::remove_file(self.photo_path(&photo.file));
        }
        match fs::remove_file(self.record_path(&record.id)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        }
    }

    /// Stores a photo chosen from disk, downscaled, and returns its file name.
    pub fn add_photo(&self, source: &Path) -> Result<String> {
        let bytes = fs::read(source)?;
        let image = image::load_from_memory(&bytes)
            .with_context(|| format!("{} is not an image", source.display()))?;
        let image = if image.width().max(image.height()) > PHOTO_MAX_SIDE {
            image.resize(
                PHOTO_MAX_SIDE,
                PHOTO_MAX_SIDE,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            image
        };
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut Cursor::new(&mut jpeg), 85)
            .encode_image(&image.to_rgb8())?;
        let file = format!("{}.jpg", new_id());
        write_atomic(&self.photo_path(&file), &jpeg)?;
        Ok(file)
    }

    /// Reads an image for the clinic logo and returns it as a `data:` URL.
    pub fn logo_data_url(source: &Path) -> Result<String> {
        let bytes = fs::read(source)?;
        let image = image::load_from_memory(&bytes)?;
        let image = image.resize(480, 480, image::imageops::FilterType::Lanczos3);
        let mut png = Vec::new();
        image.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)?;
        Ok(backup::encode_data_url("png", &png))
    }

    pub fn load_clinic(&self) -> Clinic {
        read_json(&self.dir.join("clinic.json")).unwrap_or_default()
    }

    pub fn save_clinic(&self, clinic: &Clinic) -> Result<()> {
        write_atomic(
            &self.dir.join("clinic.json"),
            &serde_json::to_vec_pretty(clinic)?,
        )
    }

    pub fn load_prefs(&self) -> Prefs {
        read_json(&self.dir.join("prefs.json")).unwrap_or_default()
    }

    pub fn save_prefs(&self, prefs: &Prefs) -> Result<()> {
        write_atomic(
            &self.dir.join("prefs.json"),
            &serde_json::to_vec_pretty(prefs)?,
        )
    }

    /// A record with its photos inline, ready for a backup or patient file.
    pub fn to_portable(&self, record: &Record) -> PortableRecord {
        PortableRecord {
            id: record.id.clone(),
            created: record.created,
            updated: record.updated,
            form: record.form.clone(),
            imgs: record
                .photos
                .iter()
                .filter_map(|photo| {
                    let bytes = fs::read(self.photo_path(&photo.file)).ok()?;
                    let extension = Path::new(&photo.file)
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("jpg");
                    Some(PortablePhoto {
                        src: backup::encode_data_url(extension, &bytes),
                        cap: photo.cap.clone(),
                    })
                })
                .collect(),
        }
    }

    /// Writes an imported record's photos to disk and returns the stored record.
    pub fn store_portable(&self, portable: PortableRecord) -> Result<Record> {
        let mut photos = Vec::new();
        for img in portable.imgs {
            let Some((extension, bytes)) = backup::decode_data_url(&img.src) else {
                continue;
            };
            let file = format!("{}.{extension}", new_id());
            write_atomic(&self.photo_path(&file), &bytes)?;
            photos.push(Photo { file, cap: img.cap });
        }
        let record = Record {
            id: portable.id,
            created: portable.created,
            updated: portable.updated,
            form: portable.form,
            photos,
        };
        self.save(&record)?;
        Ok(record)
    }

    pub fn backup_payload(&self, records: &[Record], clinic: &Clinic) -> Payload {
        Payload {
            clinic: Some(clinic.clone()),
            records: records
                .iter()
                .map(|record| self.to_portable(record))
                .collect(),
        }
    }

    /// Adds a backup's records: unknown ones are added, and a known one is replaced
    /// only when the backup's copy was edited more recently.
    pub fn merge(
        &self,
        existing: &[Record],
        incoming: Vec<PortableRecord>,
    ) -> Result<(MergeSummary, Vec<Record>)> {
        let mut summary = MergeSummary::default();
        let mut stored = Vec::new();
        for portable in incoming {
            if portable.id.is_empty() {
                continue;
            }
            match existing.iter().find(|record| record.id == portable.id) {
                Some(current) if portable.updated <= current.updated => summary.unchanged += 1,
                Some(current) => {
                    let replaced = current.clone();
                    stored.push(self.store_portable(portable)?);
                    for photo in &replaced.photos {
                        let _ = fs::remove_file(self.photo_path(&photo.file));
                    }
                    summary.updated += 1;
                }
                None => {
                    stored.push(self.store_portable(portable)?);
                    summary.added += 1;
                }
            }
        }
        Ok((summary, stored))
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Report;

    fn temp_archive(name: &str) -> Archive {
        let dir = std::env::temp_dir().join(format!("gastroskopi-test-{name}-{}", new_id()));
        Archive::open(dir).unwrap()
    }

    fn record(id: &str, updated: i64, name: &str) -> Record {
        Record {
            id: id.into(),
            created: 1,
            updated,
            form: Report {
                pt_name: name.into(),
                ..Report::default()
            },
            photos: Vec::new(),
        }
    }

    #[test]
    fn test_save_load_delete() {
        let archive = temp_archive("crud");
        archive.save(&record("a", 1, "A")).unwrap();
        archive.save(&record("b", 1, "B")).unwrap();
        fs::write(archive.records_dir().join("broken.json"), "{").unwrap();

        let (mut records, unreadable) = archive.load_all();
        records.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].form.pt_name, "B");
        assert_eq!(unreadable.len(), 1);

        archive.delete(&records[0]).unwrap();
        assert_eq!(archive.load_all().0.len(), 1);
        fs::remove_dir_all(archive.dir()).unwrap();
    }

    #[test]
    fn test_record_ids_cannot_escape() {
        let archive = temp_archive("escape");
        assert_eq!(
            archive.record_path("../../etc/passwd"),
            archive.records_dir().join("etcpasswd.json")
        );
        assert_eq!(
            archive.photo_path("../x.jpg"),
            archive.photos_dir().join("x.jpg")
        );
        fs::remove_dir_all(archive.dir()).unwrap();
    }

    #[test]
    fn test_merge_keeps_the_newer_copy() {
        let archive = temp_archive("merge");
        let existing = vec![record("a", 10, "Old A"), record("b", 10, "Local B")];
        let incoming = vec![
            archive.to_portable(&record("a", 20, "New A")),
            archive.to_portable(&record("b", 5, "Stale B")),
            archive.to_portable(&record("c", 1, "C")),
        ];
        let (summary, stored) = archive.merge(&existing, incoming).unwrap();
        assert_eq!(
            summary,
            MergeSummary {
                added: 1,
                updated: 1,
                unchanged: 1
            }
        );
        let names: Vec<_> = stored.iter().map(|r| r.form.pt_name.as_str()).collect();
        assert_eq!(names, ["New A", "C"]);
        fs::remove_dir_all(archive.dir()).unwrap();
    }

    #[test]
    fn test_photos_travel_inline() {
        let archive = temp_archive("photos");
        let source = archive.dir().join("source.png");
        image::RgbImage::from_pixel(2000, 1000, image::Rgb([200, 40, 40]))
            .save(&source)
            .unwrap();
        let file = archive.add_photo(&source).unwrap();
        let stored = image::open(archive.photo_path(&file)).unwrap();
        assert_eq!((stored.width(), stored.height()), (1600, 800));

        let mut original = record("p", 1, "P");
        original.photos.push(Photo {
            file,
            cap: "antrum".into(),
        });
        let portable = archive.to_portable(&original);
        assert!(portable.imgs[0].src.starts_with("data:image/jpeg;base64,"));

        let restored = archive.store_portable(portable).unwrap();
        assert_eq!(restored.photos[0].cap, "antrum");
        assert_ne!(restored.photos[0].file, original.photos[0].file);
        assert!(archive.photo_path(&restored.photos[0].file).exists());
        fs::remove_dir_all(archive.dir()).unwrap();
    }
}
