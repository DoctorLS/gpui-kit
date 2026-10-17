# Gastroskopi

A desktop application for writing, archiving and printing gastroscopy
(esophagogastroduodenoscopy) reports. The interface and the report text are in
Albanian.

It is the native edition of a single-file browser tool. The same form fields,
standard classifications (LA, Prague, Forrest, Hill, Paris, Sydney) and report
sentences carry over. The archive lives in files on disk instead of browser
storage.

```sh
cargo run -p gastroskopi
```

## What it does

- **Patient archive.** Every report with a patient name is saved as you type.
  The sidebar lists the archive, newest procedure first. Search covers name,
  file number, date and diagnosis.
- **Structured form with live preview.** The A4 report updates beside the form.
  "Gjetje normale" fills every organ as normal. The conclusion can be proposed
  from the findings.
- **Printing and PDF.** "Printo / PDF…" opens the formatted report in the system
  browser and starts its print dialog, where *Save as PDF* is also available.
- **Copy and export.** Copy the report as plain text, or save it as `.txt`, or
  as a patient `.json` file that includes its photos.
- **Photos.** Photos are downscaled to 1600 px, stored with the report and
  printed with captions.
- **Backup and restore.** One file holds every patient, photo and the clinic
  details. It can be encrypted with a password (AES-256-GCM, key derived by
  PBKDF2-SHA256). A restore merges: a patient who exists in both places keeps
  the more recently edited copy. The sidebar shows how long ago the last backup
  was made and warns after seven days.

## Compatibility with the browser edition

Backups and patient files use the browser edition's format, so they move in
both directions. Open a backup made in the browser with
**Importo backup ose pacient…** (Ctrl/Cmd+O), with its password if it was
encrypted, and every patient comes across with their photos.

## Where the data lives

| Platform | Folder |
| --- | --- |
| Linux | `~/.local/share/gastroskopi` |
| macOS | `~/Library/Application Support/gastroskopi` |
| Windows | `%APPDATA%\gastroskopi` |

Set `GASTROSKOPI_DATA_DIR` to use another folder. Each report is one JSON file
under `records/`, written to a temporary file first and then renamed into
place, so a crash cannot leave half a report behind. Keep regular backups
anyway: a failed disk takes this folder with it.

## Keyboard

| Command | Shortcut |
| --- | --- |
| New report | Ctrl/Cmd+N |
| Print / PDF | Ctrl/Cmd+P |
| Copy report text | Ctrl/Cmd+Shift+C |
| Save patient file | Ctrl/Cmd+S |
| Backup | Ctrl/Cmd+B |
| Import backup or patient | Ctrl/Cmd+O |
| Clinic settings | Ctrl/Cmd+, |
| Find patient | Ctrl/Cmd+F |
