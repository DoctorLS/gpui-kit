//! The main window: the patient archive beside the open report and its preview.
//!
//! The workspace owns the archive and every command that reaches outside one report:
//! saving, opening, printing, exporting, backup and restore. The open report itself
//! belongs to the [`ReportEditor`].

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, StyledExt as _, TitleBar,
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex,
    input::{Input, InputEvent, InputState},
    list::ListItem,
    menu::DropdownMenu as _,
    notification::Notification,
    resizable::{h_resizable, resizable_panel},
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::archive::{Archive, MergeSummary, Prefs};
use crate::backup::{self, ImportFile, Payload, PortableRecord};
use crate::dialogs::{BackupForm, ClinicForm, PasswordForm};
use crate::editor::{EditorEvent, ReportEditor};
use crate::narrative::{self, fmt_date};
use crate::preview::ReportPreview;
use crate::print;
use crate::report::{Clinic, Record, Report};

gpui_kit::actions!(
    gastroskopi,
    [
        NewReport,
        PrintReport,
        CopyText,
        SaveText,
        SavePatient,
        DeleteReport,
        NormalFindings,
        GenerateConclusion,
        BackupArchive,
        Import,
        ClinicSettings,
        FindPatient,
        Quit,
    ]
);

const CONTEXT: &str = "Workspace";
const BACKUP_REMINDER_DAYS: i64 = 7;

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-n", NewReport, Some(CONTEXT)),
        KeyBinding::new("secondary-p", PrintReport, Some(CONTEXT)),
        KeyBinding::new("secondary-shift-c", CopyText, Some(CONTEXT)),
        KeyBinding::new("secondary-s", SavePatient, Some(CONTEXT)),
        KeyBinding::new("secondary-b", BackupArchive, Some(CONTEXT)),
        KeyBinding::new("secondary-o", Import, Some(CONTEXT)),
        KeyBinding::new("secondary-,", ClinicSettings, Some(CONTEXT)),
        KeyBinding::new("secondary-f", FindPatient, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-q", Quit, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-q", Quit, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.set_menus(vec![
        Menu {
            name: "Gastroskopi".into(),
            items: vec![
                MenuItem::action("Cilësimet e klinikës…", ClinicSettings),
                MenuItem::separator(),
                MenuItem::action("Mbyll", Quit),
            ],
            disabled: false,
        },
        Menu {
            name: "Raporti".into(),
            items: vec![
                MenuItem::action("Raport i ri", NewReport),
                MenuItem::action("Printo / PDF…", PrintReport),
                MenuItem::separator(),
                MenuItem::action("Kopjo tekstin", CopyText),
                MenuItem::action("Ruaj tekstin…", SaveText),
                MenuItem::action("Ruaj pacientin…", SavePatient),
                MenuItem::separator(),
                MenuItem::action("Gjetje normale", NormalFindings),
                MenuItem::action("Gjenero konkluzionin", GenerateConclusion),
                MenuItem::separator(),
                MenuItem::action("Fshi raportin…", DeleteReport),
            ],
            disabled: false,
        },
        Menu {
            name: "Arkivi".into(),
            items: vec![
                MenuItem::action("Kërko pacient", FindPatient),
                MenuItem::separator(),
                MenuItem::action("Backup…", BackupArchive),
                MenuItem::action("Importo backup ose pacient…", Import),
            ],
            disabled: false,
        },
    ]);
}

/// Where the open report stands relative to the archive.
#[derive(Clone)]
enum SaveState {
    /// Nothing typed since the report was opened or saved.
    Clean,
    /// A save is scheduled.
    Pending,
    /// The report has no patient name, so it is not archived yet.
    Draft,
    Saved(SharedString),
    Failed(SharedString),
}

pub struct Workspace {
    archive: Archive,
    /// Every archived record, newest procedure first.
    records: Vec<Record>,
    clinic: Clinic,
    logo: Option<Arc<Image>>,
    prefs: Prefs,
    editor: Entity<ReportEditor>,
    search: Entity<InputState>,
    query: String,
    save_state: SaveState,
    save_task: Option<Task<()>>,
    busy: Option<SharedString>,
    focus_handle: FocusHandle,
    list_scroll: UniformListScrollHandle,
    form_scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(archive: Archive, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (mut records, unreadable) = archive.load_all();
        sort_records(&mut records);
        let clinic = archive.load_clinic();
        let prefs = archive.load_prefs();

        let first = new_record(&clinic);
        let editor = cx.new(|cx| ReportEditor::new(archive.clone(), first, window, cx));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Kërko pacient"));

        let subscriptions = vec![
            cx.subscribe_in(&editor, window, |this, _, event, _, cx| match event {
                EditorEvent::Changed => this.schedule_save(cx),
            }),
            cx.subscribe_in(&search, window, |this, state, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.query = state.read(cx).value().trim().to_lowercase();
                    cx.notify();
                }
            }),
        ];

        if !unreadable.is_empty() {
            let message = format!(
                "{} skedarë në arkiv nuk u lexuan dhe u anashkaluan. Ndodhen te {}.",
                unreadable.len(),
                archive.dir().join("records").display()
            );
            window.push_notification(Notification::warning(message).autohide(false), cx);
        }

        Self {
            logo: logo_image(&clinic),
            archive,
            records,
            clinic,
            prefs,
            editor,
            search,
            query: String::new(),
            save_state: SaveState::Clean,
            save_task: None,
            busy: None,
            focus_handle: cx.focus_handle(),
            list_scroll: UniformListScrollHandle::new(),
            form_scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    // MARK: Saving

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self.save_state = SaveState::Pending;
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(400))
                .await;
            _ = this.update(cx, |this, cx| this.save_now(cx));
        }));
        cx.notify();
    }

    /// Writes the open report to the archive if it has a patient name.
    fn save_now(&mut self, cx: &mut Context<Self>) {
        self.save_task = None;
        let mut record = self.editor.read(cx).record().clone();
        if record.form.pt_name.trim().is_empty() {
            self.save_state = if record.form.is_blank() {
                SaveState::Clean
            } else {
                SaveState::Draft
            };
            cx.notify();
            return;
        }
        let now = now_ms();
        if record.created == 0 {
            record.created = now;
        }
        record.updated = now;
        match self.archive.save(&record) {
            Ok(()) => {
                self.editor.update(cx, |editor, _| {
                    editor.mark_saved(record.created, record.updated)
                });
                self.save_state = SaveState::Saved(
                    format!("Ruajtur {}", chrono::Local::now().format("%H:%M")).into(),
                );
                upsert(&mut self.records, record);
            }
            Err(err) => {
                self.save_state = SaveState::Failed(format!("Ruajtja dështoi: {err}").into());
            }
        }
        cx.notify();
    }

    /// Saves any edit still waiting for its delay.
    fn flush(&mut self, cx: &mut Context<Self>) {
        if self.save_task.is_some() {
            self.save_now(cx);
        }
    }

    // MARK: Opening reports

    /// Runs `then` once the open report may be replaced, asking first when it holds
    /// unsaved work that has no patient name.
    fn leave_current(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        self.flush(cx);
        let draft = self.editor.read(cx).record().clone();
        let archived = self.records.iter().any(|r| r.id == draft.id);
        if archived || (draft.form.is_blank() && draft.photos.is_empty()) {
            if !archived {
                _ = self.archive.delete(&draft);
            }
            then(self, window, cx);
            return;
        }
        let workspace = cx.entity().downgrade();
        let then = std::rc::Rc::new(then);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let workspace = workspace.clone();
            let draft = draft.clone();
            let then = then.clone();
            alert
                .title("Të hidhet raporti pa emër?")
                .description("Raporti nuk ka emër pacienti, prandaj nuk është ruajtur në arkiv.")
                .show_cancel(true)
                .cancel_text("Anulo")
                .ok_text("Hidhe")
                .ok_variant(gpui_kit::component::button::ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    let then = then.clone();
                    let draft = draft.clone();
                    _ = workspace.update(cx, |this, cx| {
                        _ = this.archive.delete(&draft);
                        then(this, window, cx);
                    });
                    true
                })
        });
    }

    fn new_report(&mut self, _: &NewReport, window: &mut Window, cx: &mut Context<Self>) {
        // On the next tick, so a menu that dispatched the command has closed and
        // handed focus back before a dialog records where to return it.
        cx.defer_in(window, |this, window, cx| this.open_new_report(window, cx));
    }

    fn open_new_report(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_current(window, cx, |this, window, cx| {
            let record = new_record(&this.clinic);
            this.show(record, window, cx);
        });
    }

    fn open_record(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.read(cx).record().id == id {
            return;
        }
        let id = id.to_string();
        self.leave_current(window, cx, move |this, window, cx| {
            if let Some(record) = this.records.iter().find(|r| r.id == id).cloned() {
                this.show(record, window, cx);
            }
        });
    }

    fn show(&mut self, record: Record, window: &mut Window, cx: &mut Context<Self>) {
        self.save_task = None;
        self.save_state = SaveState::Clean;
        self.editor
            .update(cx, |editor, cx| editor.load(record, window, cx));
        self.form_scroll.set_offset(Point::default());
        self.editor.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    fn delete_report(&mut self, _: &DeleteReport, window: &mut Window, cx: &mut Context<Self>) {
        // On the next tick, so a menu that dispatched the command has closed and
        // handed focus back before a dialog records where to return it.
        cx.defer_in(window, |this, window, cx| {
            this.open_delete_report(window, cx)
        });
    }

    fn open_delete_report(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush(cx);
        let record = self.editor.read(cx).record().clone();
        if !self.records.iter().any(|r| r.id == record.id) {
            // Not archived: there is nothing to delete beyond clearing the form.
            self.open_new_report(window, cx);
            return;
        }
        let name = record.form.pt_name.trim().to_string();
        let date = fmt_date(&record.form.date);
        let workspace = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let workspace = workspace.clone();
            let record = record.clone();
            alert
                .title(format!("Të fshihet raporti i “{name}”?"))
                .description(format!(
                    "Raporti i datës {date} dhe fotot e tij hiqen nga arkivi. Ky veprim nuk mund të zhbëhet, përveçse nga një backup."
                ))
                .show_cancel(true)
                .cancel_text("Anulo")
                .ok_text("Fshi")
                .ok_variant(gpui_kit::component::button::ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    let record = record.clone();
                    _ = workspace.update(cx, |this, cx| this.delete_now(record, window, cx));
                    true
                })
        });
    }

    fn delete_now(&mut self, record: Record, window: &mut Window, cx: &mut Context<Self>) {
        match self.archive.delete(&record) {
            Ok(()) => {
                self.records.retain(|r| r.id != record.id);
                let next = new_record(&self.clinic);
                self.show(next, window, cx);
            }
            Err(err) => window.push_notification(
                Notification::error(format!("Raporti nuk u fshi: {err}")),
                cx,
            ),
        }
    }

    // MARK: Report commands

    fn normal_findings(&mut self, _: &NormalFindings, window: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |editor, cx| editor.apply_normal_findings(window, cx));
    }

    fn generate_conclusion(
        &mut self,
        _: &GenerateConclusion,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor
            .update(cx, |editor, cx| editor.generate_conclusion(window, cx));
    }

    fn report_text(&self, cx: &App) -> String {
        let record = self.editor.read(cx).record();
        let captions: Vec<String> = record.photos.iter().map(|p| p.cap.clone()).collect();
        narrative::plain_text(
            &narrative::build(&record.form),
            &record.form,
            &self.clinic,
            &captions,
        )
    }

    fn copy_text(&mut self, _: &CopyText, window: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(self.report_text(cx)));
        window.push_notification(Notification::success("Teksti i raportit u kopjua."), cx);
    }

    fn save_text(&mut self, _: &SaveText, window: &mut Window, cx: &mut Context<Self>) {
        let form = &self.editor.read(cx).record().form;
        let name = format!("{}.txt", print::file_stem(&form.pt_name, &form.date));
        let text = self.report_text(cx);
        self.save_file_as(name, text.into_bytes(), "Teksti u ruajt", window, cx);
    }

    fn save_patient(&mut self, _: &SavePatient, window: &mut Window, cx: &mut Context<Self>) {
        self.flush(cx);
        let record = self.editor.read(cx).record().clone();
        let name = format!(
            "{}.json",
            print::file_stem(&record.form.pt_name, &record.form.date)
        );
        let portable = self.archive.to_portable(&record);
        match backup::write_patient(&portable) {
            Ok(json) => self.save_file_as(name, json.into_bytes(), "Pacienti u ruajt", window, cx),
            Err(err) => window.push_notification(Notification::error(err.to_string()), cx),
        }
    }

    /// Asks where to save `bytes` and writes them there.
    fn save_file_as(
        &mut self,
        suggested: String,
        bytes: Vec<u8>,
        done: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory = default_save_dir();
        let path = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn_in(window, async move |_, cx| {
            let Ok(Ok(Some(path))) = path.await else {
                return;
            };
            let result = std::fs::write(&path, bytes);
            _ = cx.update(|window, cx| {
                let note = match result {
                    Ok(()) => Notification::success(format!("{done}: {}", path.display())),
                    Err(err) => Notification::error(format!("Skedari nuk u ruajt: {err}")),
                };
                window.push_notification(note, cx);
            });
        })
        .detach();
    }

    fn print_report(&mut self, _: &PrintReport, window: &mut Window, cx: &mut Context<Self>) {
        // On the next tick, so a menu that dispatched the command has closed and
        // handed focus back before a dialog records where to return it.
        cx.defer_in(window, |this, window, cx| {
            this.open_print_report(window, cx)
        });
    }

    fn open_print_report(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush(cx);
        let missing = narrative::missing_for_print(&self.editor.read(cx).record().form);
        if missing.is_empty() {
            self.print_now(window, cx);
            return;
        }
        let workspace = cx.entity().downgrade();
        let list = missing.join(", ");
        window.open_alert_dialog(cx, move |alert, _, _| {
            let workspace = workspace.clone();
            alert
                .title("Raporti nuk është i plotë")
                .description(format!("Mungojnë: {list}."))
                .show_cancel(true)
                .cancel_text("Anulo")
                .ok_text("Printo gjithsesi")
                .on_ok(move |_, window, cx| {
                    _ = workspace.update(cx, |this, cx| this.print_now(window, cx));
                    true
                })
        });
    }

    /// Opens the report in the browser, whose print dialog also saves a PDF.
    fn print_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let record = self.editor.read(cx).record().clone();
        let photos = self.archive.to_portable(&record).imgs;
        let page = print::html(
            &narrative::build(&record.form),
            &self.clinic,
            &photos,
            &record.form.endo,
        );
        let dir = std::env::temp_dir().join("gastroskopi-print");
        let path = dir.join(format!(
            "{}.html",
            print::file_stem(&record.form.pt_name, &record.form.date)
        ));
        match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, page)) {
            Ok(()) => cx.open_with_system(&path),
            Err(err) => window.push_notification(
                Notification::error(format!("Faqja e printimit nuk u krijua: {err}")),
                cx,
            ),
        }
    }

    // MARK: Backup and import

    fn backup_archive(&mut self, _: &BackupArchive, window: &mut Window, cx: &mut Context<Self>) {
        // On the next tick, so a menu that dispatched the command has closed and
        // handed focus back before a dialog records where to return it.
        cx.defer_in(window, |this, window, cx| {
            this.open_backup_archive(window, cx)
        });
    }

    fn open_backup_archive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush(cx);
        if self.records.is_empty() {
            window.push_notification(
                Notification::info("Arkivi është bosh. Nuk ka asgjë për backup."),
                cx,
            );
            return;
        }
        let form = cx.new(|cx| BackupForm::new(window, cx));
        let focus = Cell::new(Some(form.read(cx).initial_focus(cx)));
        let workspace = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, cx| {
            if let Some(focus) = focus.take() {
                focus.focus(window, cx);
            }
            let form = form.clone();
            let workspace = workspace.clone();
            dialog
                .title("Backup i arkivit")
                .w(px(480.))
                .child(form.clone())
                .on_ok(move |_, window, cx| {
                    let Ok(password) = form.update(cx, |form, cx| form.confirm(cx)) else {
                        return false;
                    };
                    _ = workspace.update(cx, |this, cx| this.write_backup(password, window, cx));
                    true
                })
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().child(Button::new("cancel").label("Anulo")))
                        .child(
                            DialogAction::new()
                                .child(Button::new("backup").primary().label("Ruaj backup-in…")),
                        ),
                )
        });
    }

    fn write_backup(
        &mut self,
        password: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M");
        let suffix = if password.is_some() {
            "_enkriptuar"
        } else {
            ""
        };
        let suggested = format!("gastroskopi_backup_{stamp}{suffix}.json");
        let path = cx.prompt_for_new_path(&default_save_dir(), Some(&suggested));
        let archive = self.archive.clone();
        let records = self.records.clone();
        let clinic = self.clinic.clone();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = path.await else {
                return;
            };
            _ = this.update(cx, |this, cx| {
                this.busy = Some("Po ruhet backup-i…".into());
                cx.notify();
            });
            let count = records.len();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let payload = archive.backup_payload(&records, &clinic);
                    let text = backup::write_backup(&payload, password.as_deref())?;
                    std::fs::write(&path, text)?;
                    anyhow::Ok(path)
                })
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                this.busy = None;
                match result {
                    Ok(path) => {
                        this.prefs.last_backup = now_ms();
                        _ = this.archive.save_prefs(&this.prefs);
                        window.push_notification(
                            Notification::success(format!(
                                "Backup-i me {count} pacientë u ruajt te {}. Mbaje edhe një kopje në USB.",
                                path.display()
                            )),
                            cx,
                        );
                    }
                    Err(err) => window.push_notification(
                        Notification::error(format!("Backup-i nuk u ruajt: {err}")),
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn import_file(&mut self, _: &Import, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Importo".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            _ = this.update_in(cx, |this, window, cx| this.import_path(path, window, cx));
        })
        .detach();
    }

    /// Reads a backup or patient file and brings its records into the archive.
    fn import_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let parsed = cx
                .background_executor()
                .spawn(async move { backup::parse(&std::fs::read_to_string(&path)?) })
                .await;
            _ = this.update_in(cx, |this, window, cx| match parsed {
                Ok(ImportFile::Backup(payload)) => this.restore(payload, window, cx),
                Ok(ImportFile::SealedBackup { sealed, count }) => {
                    this.ask_backup_password(sealed, count, window, cx)
                }
                Ok(ImportFile::Patient(record)) => this.import_patient(*record, window, cx),
                Err(_) => window.push_notification(
                    Notification::error("Skedari nuk është një raport ose backup i vlefshëm."),
                    cx,
                ),
            });
        })
        .detach();
    }

    fn ask_backup_password(
        &mut self,
        sealed: backup::Sealed,
        count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let form = cx.new(|cx| PasswordForm::new(count, window, cx));
        let focus = Cell::new(Some(form.read(cx).initial_focus(cx)));
        let workspace = cx.entity().downgrade();
        let sealed = Arc::new(sealed);
        window.open_dialog(cx, move |dialog, window, cx| {
            if let Some(focus) = focus.take() {
                focus.focus(window, cx);
            }
            let opening = form.read(cx).is_opening();
            let form = form.clone();
            let workspace = workspace.clone();
            let sealed = sealed.clone();
            dialog
                .title("Hap backup-in e enkriptuar")
                .w(px(420.))
                .child(form.clone())
                .on_ok(move |_, window, cx| {
                    if form.read(cx).is_opening() {
                        return false;
                    }
                    let password = form.read(cx).password(cx);
                    form.update(cx, |form, cx| form.set_opening(true, cx));
                    let sealed = sealed.clone();
                    let form = form.clone();
                    let workspace = workspace.clone();
                    window
                        .spawn(cx, async move |cx| {
                            let opened = cx
                                .background_executor()
                                .spawn(async move { backup::open(&sealed, &password) })
                                .await;
                            _ = cx.update(|window, cx| match opened {
                                Ok(payload) => {
                                    window.close_dialog(cx);
                                    _ = workspace
                                        .update(cx, |this, cx| this.restore(payload, window, cx));
                                }
                                Err(_) => form.update(cx, |form, cx| form.set_wrong_password(cx)),
                            });
                        })
                        .detach();
                    // The dialog closes once the password has been checked.
                    false
                })
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().child(Button::new("cancel").label("Anulo")))
                        .child(
                            DialogAction::new().child(
                                Button::new("open")
                                    .primary()
                                    .label("Hap")
                                    .loading(opening)
                                    .disabled(opening),
                            ),
                        ),
                )
        });
    }

    fn restore(&mut self, payload: Payload, window: &mut Window, cx: &mut Context<Self>) {
        self.flush(cx);
        self.busy = Some("Po rikthehet backup-i…".into());
        cx.notify();
        let archive = self.archive.clone();
        let existing = self.records.clone();
        let Payload { clinic, records } = payload;
        cx.spawn_in(window, async move |this, cx| {
            let merged = cx
                .background_executor()
                .spawn(async move { archive.merge(&existing, records) })
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                this.busy = None;
                match merged {
                    Ok((summary, stored)) => {
                        this.adopt_clinic(clinic, cx);
                        this.adopt_records(stored, window, cx);
                        window.push_notification(
                            Notification::success(restore_message(summary))
                                .title("Rikthimi përfundoi"),
                            cx,
                        );
                    }
                    Err(err) => window.push_notification(
                        Notification::error(format!("Rikthimi dështoi: {err}")),
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn import_patient(
        &mut self,
        record: PortableRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.flush(cx);
        let id = record.id.clone();
        match self.archive.merge(&self.records, vec![record]) {
            Ok((summary, stored)) => {
                self.adopt_records(stored, window, cx);
                if summary.unchanged > 0 {
                    window.push_notification(
                        Notification::info("Kopja në arkiv është më e re ose e njëjtë. Ajo u hap."),
                        cx,
                    );
                }
                self.open_record(&id, window, cx);
            }
            Err(err) => window.push_notification(
                Notification::error(format!("Pacienti nuk u importua: {err}")),
                cx,
            ),
        }
    }

    /// A restored backup brings its clinic details only when none are set here.
    fn adopt_clinic(&mut self, clinic: Option<Clinic>, cx: &mut Context<Self>) {
        let Some(clinic) = clinic else {
            return;
        };
        if self.clinic.c_name.is_empty() && self.clinic.c_addr.is_empty() {
            self.set_clinic(clinic, cx);
        }
    }

    fn adopt_records(&mut self, stored: Vec<Record>, window: &mut Window, cx: &mut Context<Self>) {
        let open_id = self.editor.read(cx).record().id.clone();
        let mut reopen = None;
        for record in stored {
            if record.id == open_id {
                reopen = Some(record.clone());
            }
            upsert(&mut self.records, record);
        }
        if let Some(record) = reopen {
            self.show(record, window, cx);
        }
    }

    // MARK: Clinic

    fn clinic_settings(&mut self, _: &ClinicSettings, window: &mut Window, cx: &mut Context<Self>) {
        // On the next tick, so a menu that dispatched the command has closed and
        // handed focus back before a dialog records where to return it.
        cx.defer_in(window, |this, window, cx| {
            this.open_clinic_settings(window, cx)
        });
    }

    fn open_clinic_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let form = cx.new(|cx| ClinicForm::new(&self.clinic, window, cx));
        let focus = Cell::new(Some(form.read(cx).initial_focus(cx)));
        let workspace = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, cx| {
            if let Some(focus) = focus.take() {
                focus.focus(window, cx);
            }
            let form = form.clone();
            let workspace = workspace.clone();
            dialog
                .title("Cilësimet e klinikës")
                .w(px(560.))
                .child(form.clone())
                .on_ok(move |_, window, cx| {
                    let clinic = form.read(cx).clinic(cx);
                    _ = workspace.update(cx, |this, cx| {
                        if let Err(err) = this.archive.save_clinic(&clinic) {
                            window.push_notification(
                                Notification::error(format!("Cilësimet nuk u ruajtën: {err}")),
                                cx,
                            );
                        }
                        this.set_clinic(clinic, cx);
                    });
                    true
                })
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().child(Button::new("cancel").label("Anulo")))
                        .child(
                            DialogAction::new().child(Button::new("save").primary().label("Ruaj")),
                        ),
                )
        });
    }

    fn set_clinic(&mut self, clinic: Clinic, cx: &mut Context<Self>) {
        _ = self.archive.save_clinic(&clinic);
        self.logo = logo_image(&clinic);
        self.clinic = clinic;
        cx.notify();
    }

    fn find_patient(&mut self, _: &FindPatient, window: &mut Window, cx: &mut Context<Self>) {
        self.search.focus_handle(cx).focus(window, cx);
    }

    // MARK: Rendering

    fn visible_records(&self) -> Vec<usize> {
        self.records
            .iter()
            .enumerate()
            .filter(|(_, record)| matches_query(record, &self.query))
            .map(|(ix, _)| ix)
            .collect()
    }

    fn backup_status(&self) -> (SharedString, bool) {
        if self.prefs.last_backup == 0 {
            return ("Backup: asnjëherë".into(), true);
        }
        let days = (now_ms() - self.prefs.last_backup) / 86_400_000;
        let text = match days {
            0 => "Backup: sot".to_string(),
            1 => "Backup: dje".to_string(),
            days => format!("Backup: {days} ditë më parë"),
        };
        (text.into(), days >= BACKUP_REMINDER_DAYS)
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.visible_records();
        let count = visible.len();
        let open_id = self.editor.read(cx).record().id.clone();
        let (backup_text, backup_due) = self.backup_status();
        let theme = cx.theme();

        v_flex()
            .size_full()
            .bg(theme.sidebar)
            .text_color(theme.sidebar_foreground)
            .child(
                v_flex()
                    .gap_2()
                    .p_3()
                    .child(
                        Button::new("new-report")
                            .w_full()
                            .icon(IconName::Plus)
                            .label("Raport i ri")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.new_report(&NewReport, window, cx)
                            })),
                    )
                    .child(
                        Input::new(&self.search)
                            .prefix(IconName::Search)
                            .cleanable(true)
                            .small(),
                    ),
            )
            .child(
                h_flex()
                    .px_3()
                    .pb_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(if self.query.is_empty() {
                        format!("{} pacientë", self.records.len())
                    } else {
                        format!("{count} nga {} pacientë", self.records.len())
                    }),
            )
            .child(div().flex_1().min_h_0().map(|this| {
                if count == 0 {
                    this.p_3()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(if self.query.is_empty() {
                            "Arkivi është bosh. Raportet me emër pacienti ruhen këtu automatikisht."
                        } else {
                            "Asnjë pacient nuk përputhet me kërkimin."
                        })
                } else {
                    this.child(
                        uniform_list(
                            "archive",
                            count,
                            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                range
                                    .filter_map(|ix| visible.get(ix).copied())
                                    .map(|ix| this.render_archive_row(ix, &open_id, cx))
                                    .collect::<Vec<_>>()
                            }),
                        )
                        .size_full()
                        .px_2()
                        .track_scroll(&self.list_scroll),
                    )
                }
            }))
            .child(
                h_flex()
                    .gap_2()
                    .p_3()
                    .border_t_1()
                    .border_color(theme.sidebar_border)
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .text_xs()
                            .text_color(if backup_due {
                                theme.danger
                            } else {
                                theme.muted_foreground
                            })
                            .when(backup_due, |this| this.child(IconName::TriangleAlert))
                            .child(div().truncate().child(backup_text)),
                    )
                    .child(
                        Button::new("backup")
                            .small()
                            .label("Backup…")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.backup_archive(&BackupArchive, window, cx)
                            })),
                    ),
            )
    }

    fn render_archive_row(&self, ix: usize, open_id: &str, cx: &mut Context<Self>) -> AnyElement {
        let record = &self.records[ix];
        let form = &record.form;
        let first_diagnosis = form.concl.lines().map(str::trim).find(|l| !l.is_empty());
        let meta = [
            fmt_date(&form.date),
            (!form.pt_id.trim().is_empty())
                .then(|| format!("#{}", form.pt_id.trim()))
                .unwrap_or_default(),
            first_diagnosis.map(str::to_string).unwrap_or_default(),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        let id = record.id.clone();
        let name = form.pt_name.trim().to_string();
        ListItem::new(SharedString::from(format!("record-{}", record.id)))
            .w_full()
            .overflow_hidden()
            .rounded(cx.theme().radius)
            .selected(record.id == open_id)
            .accessibility_label(name.clone())
            .child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .py_0p5()
                    .child(div().truncate().font_medium().child(name))
                    .child(
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(meta),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.open_record(&id, window, cx)))
            .into_any_element()
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (status, failed) = match &self.save_state {
            SaveState::Clean => (SharedString::default(), false),
            SaveState::Pending => ("Duke ruajtur…".into(), false),
            SaveState::Draft => (
                "Draft – shkruaj emrin e pacientit për ta ruajtur".into(),
                false,
            ),
            SaveState::Saved(text) => (text.clone(), false),
            SaveState::Failed(text) => (text.clone(), true),
        };
        let status = self.busy.clone().unwrap_or(status);
        let theme = cx.theme();

        h_flex()
            .gap_2()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                Button::new("normal-findings")
                    .ghost()
                    .icon(IconName::Check)
                    .label("Gjetje normale")
                    .tooltip("Plotëson të gjitha organet si normale")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.normal_findings(&NormalFindings, window, cx)
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(if failed {
                        theme.danger
                    } else {
                        theme.muted_foreground
                    })
                    .child(status),
            )
            .child(
                Button::new("copy-text")
                    .icon(IconName::Copy)
                    .label("Kopjo tekstin")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.copy_text(&CopyText, window, cx)),
                    ),
            )
            .child(
                Button::new("more")
                    .icon(IconName::Ellipsis)
                    .tooltip("Më shumë veprime")
                    .dropdown_menu(|menu, _, _| {
                        menu.menu("Ruaj tekstin…", Box::new(SaveText))
                            .menu("Ruaj pacientin…", Box::new(SavePatient))
                            .menu("Importo backup ose pacient…", Box::new(Import))
                            .separator()
                            .menu("Cilësimet e klinikës…", Box::new(ClinicSettings))
                            .separator()
                            .menu("Fshi raportin…", Box::new(DeleteReport))
                    }),
            )
            .child(
                Button::new("print")
                    .primary()
                    .icon(IconName::File)
                    .label("Printo / PDF…")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.print_report(&PrintReport, window, cx)
                    })),
            )
    }

    fn render_preview(&self, cx: &App) -> ReportPreview {
        let record = self.editor.read(cx).record();
        ReportPreview::new(
            narrative::build(&record.form),
            self.clinic.clone(),
            self.logo.clone(),
            record
                .photos
                .iter()
                .map(|photo| (self.archive.photo_path(&photo.file), photo.cap.clone()))
                .collect(),
            record.form.endo.clone(),
        )
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = {
            let name = self
                .editor
                .read(cx)
                .record()
                .form
                .pt_name
                .trim()
                .to_string();
            if name.is_empty() {
                "Raport Gastroskopie".to_string()
            } else {
                format!("Raport Gastroskopie — {name}")
            }
        };

        v_flex()
            .id("workspace")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::new_report))
            .on_action(cx.listener(Self::print_report))
            .on_action(cx.listener(Self::copy_text))
            .on_action(cx.listener(Self::save_text))
            .on_action(cx.listener(Self::save_patient))
            .on_action(cx.listener(Self::delete_report))
            .on_action(cx.listener(Self::normal_findings))
            .on_action(cx.listener(Self::generate_conclusion))
            .on_action(cx.listener(Self::backup_archive))
            .on_action(cx.listener(Self::import_file))
            .on_action(cx.listener(Self::clinic_settings))
            .on_action(cx.listener(Self::find_patient))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(TitleBar::new().child(div().text_sm().child(title)))
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("workspace-split")
                        .child(
                            resizable_panel()
                                .size(px(272.))
                                .size_range(px(220.)..px(420.))
                                .child(self.render_sidebar(cx)),
                        )
                        .child(resizable_panel().child(self.render_report(cx))),
                ),
            )
    }
}

impl Workspace {
    /// The open report: its toolbar above the form and the preview side by side.
    fn render_report(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let form = div()
            .id("form-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.form_scroll)
            .vertical_scrollbar(&self.form_scroll)
            .child(self.editor.clone());
        let preview = div()
            .id("preview-scroll")
            .size_full()
            .bg(cx.theme().muted)
            .overflow_y_scrollbar()
            .child(div().p_4().child(self.render_preview(cx)));

        v_flex()
            .size_full()
            .border_l_1()
            .border_color(cx.theme().border)
            .child(self.render_toolbar(cx))
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("report-split")
                        .child(
                            resizable_panel()
                                .size_range(px(480.)..Pixels::MAX)
                                .child(form),
                        )
                        .child(
                            resizable_panel()
                                .size(px(520.))
                                .size_range(px(360.)..Pixels::MAX)
                                .child(preview),
                        ),
                ),
            )
    }
}

fn new_record(clinic: &Clinic) -> Record {
    Record::new(Report {
        date: chrono::Local::now().date_naive().to_string(),
        endo: clinic.c_endo.clone(),
        scope: clinic.c_scope.clone(),
        ..Report::default()
    })
}

fn sort_records(records: &mut [Record]) {
    records.sort_by(|a, b| {
        b.form
            .date
            .cmp(&a.form.date)
            .then(b.updated.cmp(&a.updated))
    });
}

fn upsert(records: &mut Vec<Record>, record: Record) {
    match records.iter_mut().find(|r| r.id == record.id) {
        Some(existing) => *existing = record,
        None => records.push(record),
    }
    sort_records(records);
}

fn matches_query(record: &Record, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let form = &record.form;
    [
        form.pt_name.as_str(),
        form.pt_id.as_str(),
        form.date.as_str(),
        &fmt_date(&form.date),
        form.concl.as_str(),
    ]
    .iter()
    .any(|field| field.to_lowercase().contains(query))
}

fn restore_message(summary: MergeSummary) -> String {
    format!(
        "Të rinj: {}. Të përditësuar: {}. Të pandryshuar: {} (kopja në kompjuter është më e re ose e njëjtë).",
        summary.added, summary.updated, summary.unchanged
    )
}

fn logo_image(clinic: &Clinic) -> Option<Arc<Image>> {
    let (extension, bytes) = backup::decode_data_url(&clinic.c_logo)?;
    let format = match extension.as_str() {
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::Webp,
        "gif" => ImageFormat::Gif,
        "bmp" => ImageFormat::Bmp,
        _ => ImageFormat::Jpeg,
    };
    Some(Arc::new(Image::from_bytes(format, bytes)))
}

fn default_save_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| Path::new(".").to_path_buf())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    // Explicit imports: `gpui_kit::*` would shadow the built-in `#[test]`.
    use super::{matches_query, sort_records};
    use crate::report::{Record, Report};

    #[test]
    fn test_search_matches_name_number_date_and_diagnosis() {
        let record = Record::new(Report {
            pt_name: "Ardit Hoxha".into(),
            pt_id: "A-17".into(),
            date: "2026-10-11".into(),
            concl: "Gastrit erozive".into(),
            ..Report::default()
        });
        for query in ["ardit", "a-17", "11.10.2026", "2026-10", "erozive", ""] {
            assert!(matches_query(&record, query), "{query}");
        }
        assert!(!matches_query(&record, "blerta"));
    }

    #[test]
    fn test_records_sort_by_procedure_then_edit() {
        let mut records = vec![
            Record {
                updated: 1,
                ..Record::new(Report {
                    date: "2026-01-01".into(),
                    ..Report::default()
                })
            },
            Record {
                updated: 1,
                ..Record::new(Report {
                    date: "2026-03-01".into(),
                    ..Report::default()
                })
            },
            Record {
                updated: 9,
                ..Record::new(Report {
                    date: "2026-01-01".into(),
                    ..Report::default()
                })
            },
        ];
        sort_records(&mut records);
        let order: Vec<_> = records
            .iter()
            .map(|r| (r.form.date.as_str(), r.updated))
            .collect();
        assert_eq!(
            order,
            [("2026-03-01", 1), ("2026-01-01", 9), ("2026-01-01", 1)]
        );
    }
}
