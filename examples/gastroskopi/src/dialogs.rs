//! Form bodies for the backup, password and clinic dialogs.
//!
//! Each owns its inputs for the lifetime of one dialog opening; the workspace reads
//! them when the dialog is confirmed.

use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::Button,
    form::{Field, v_form},
    h_flex,
    input::{Input, InputState, Textarea, TextareaState},
    switch::Switch,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::archive::Archive;
use crate::backup::MIN_PASSWORD_CHARS;
use crate::report::Clinic;

fn error_line(error: &Option<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .min_h_5()
        .text_sm()
        .text_color(cx.theme().danger)
        .when_some(error.clone(), |this, error| this.child(error))
}

fn masked_input(
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .masked(true)
            .placeholder(placeholder)
    })
}

/// Chooses whether a backup is encrypted, and with which password.
pub struct BackupForm {
    encrypt: bool,
    password: Entity<InputState>,
    repeat: Entity<InputState>,
    error: Option<SharedString>,
}

impl BackupForm {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            encrypt: true,
            password: masked_input("Fjalëkalimi", window, cx),
            repeat: masked_input("Përsërit fjalëkalimin", window, cx),
            error: None,
        }
    }

    pub fn initial_focus(&self, cx: &App) -> FocusHandle {
        self.password.focus_handle(cx)
    }

    /// The password to encrypt with, `Ok(None)` for a plain backup, or an error shown
    /// in the form.
    pub fn confirm(&mut self, cx: &mut Context<Self>) -> Result<Option<String>, ()> {
        if !self.encrypt {
            return Ok(None);
        }
        let password = self.password.read(cx).value().to_string();
        let repeat = self.repeat.read(cx).value().to_string();
        self.error = if password.chars().count() < MIN_PASSWORD_CHARS {
            Some(
                format!("Fjalëkalimi duhet të ketë të paktën {MIN_PASSWORD_CHARS} karaktere.")
                    .into(),
            )
        } else if password != repeat {
            Some("Fjalëkalimet nuk përputhen.".into())
        } else {
            None
        };
        cx.notify();
        match self.error {
            Some(_) => Err(()),
            None => Ok(Some(password)),
        }
    }
}

impl Render for BackupForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Ruhet një skedar me të gjithë pacientët, fotot dhe cilësimet e klinikës. Ruaje në iCloud Drive ose OneDrive dhe një herë në muaj edhe në një disk USB."),
            )
            .child(
                Switch::new("encrypt-backup")
                    .label("Enkripto me fjalëkalim")
                    .checked(self.encrypt)
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        this.encrypt = *checked;
                        this.error = None;
                        cx.notify();
                    })),
            )
            .when(self.encrypt, |this| {
                this.child(
                    v_form()
                        .child(Field::new().label("Fjalëkalimi").child(Input::new(&self.password).mask_toggle()))
                        .child(Field::new().label("Përsërit fjalëkalimin").child(Input::new(&self.repeat))),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().warning)
                        .child("Nëse e harron fjalëkalimin, backup-i nuk mund të hapet nga askush. Ruaje në një vend të sigurt."),
                )
            })
            .child(error_line(&self.error, cx))
    }
}

/// Asks for the password of an encrypted backup being restored.
pub struct PasswordForm {
    password: Entity<InputState>,
    count: usize,
    error: Option<SharedString>,
    opening: bool,
}

impl PasswordForm {
    pub fn new(count: usize, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            password: masked_input("Fjalëkalimi", window, cx),
            count,
            error: None,
            opening: false,
        }
    }

    pub fn initial_focus(&self, cx: &App) -> FocusHandle {
        self.password.focus_handle(cx)
    }

    pub fn password(&self, cx: &App) -> String {
        self.password.read(cx).value().to_string()
    }

    pub fn is_opening(&self) -> bool {
        self.opening
    }

    pub fn set_opening(&mut self, opening: bool, cx: &mut Context<Self>) {
        self.opening = opening;
        if opening {
            self.error = None;
        }
        cx.notify();
    }

    pub fn set_wrong_password(&mut self, cx: &mut Context<Self>) {
        self.opening = false;
        self.error = Some("Fjalëkalim i gabuar.".into());
        cx.notify();
    }
}

impl Render for PasswordForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let description = if self.count > 0 {
            format!(
                "Backup-i përmban {} pacientë dhe është i enkriptuar.",
                self.count
            )
        } else {
            "Backup-i është i enkriptuar.".to_string()
        };
        v_flex()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(description),
            )
            .child(
                v_form().child(
                    Field::new().label("Fjalëkalimi").child(
                        Input::new(&self.password)
                            .mask_toggle()
                            .disabled(self.opening),
                    ),
                ),
            )
            .child(error_line(&self.error, cx))
    }
}

/// Edits the clinic details printed on every report.
pub struct ClinicForm {
    name: Entity<InputState>,
    address: Entity<TextareaState>,
    endo: Entity<InputState>,
    scope: Entity<InputState>,
    logo: String,
    error: Option<SharedString>,
}

impl ClinicForm {
    pub fn new(clinic: &Clinic, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |value: &str,
                     placeholder: &'static str,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            let value = value.to_string();
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .default_value(value)
            })
        };
        let address = clinic.c_addr.clone();
        Self {
            name: input(
                &clinic.c_name,
                "p.sh. Klinika e Gastroenterologjisë",
                window,
                cx,
            ),
            address: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(2, 5)
                    .default_value(address)
            }),
            endo: input(&clinic.c_endo, "", window, cx),
            scope: input(&clinic.c_scope, "p.sh. Olympus GIF-H190", window, cx),
            logo: clinic.c_logo.clone(),
            error: None,
        }
    }

    pub fn initial_focus(&self, cx: &App) -> FocusHandle {
        self.name.focus_handle(cx)
    }

    pub fn clinic(&self, cx: &App) -> Clinic {
        Clinic {
            c_name: self.name.read(cx).value().trim().to_string(),
            c_addr: self.address.read(cx).value().trim().to_string(),
            c_endo: self.endo.read(cx).value().trim().to_string(),
            c_scope: self.scope.read(cx).value().trim().to_string(),
            c_logo: self.logo.clone(),
        }
    }

    fn choose_logo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Zgjidh".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let logo = cx
                .background_executor()
                .spawn(async move { Archive::logo_data_url(&path) })
                .await;
            _ = this.update(cx, |this, cx| {
                match logo {
                    Ok(logo) => {
                        this.logo = logo;
                        this.error = None;
                    }
                    Err(_) => this.error = Some("Skedari nuk është një imazh.".into()),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for ClinicForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_logo = !self.logo.is_empty();
        v_flex()
            .gap_3()
            .child(
                v_form()
                    .columns(2)
                    .child(
                        Field::new()
                            .label("Emri i klinikës")
                            .col_span(2)
                            .child(Input::new(&self.name)),
                    )
                    .child(
                        Field::new()
                            .label("Adresa / tel. / email")
                            .col_span(2)
                            .child(Textarea::new(&self.address)),
                    )
                    .child(
                        Field::new()
                            .label("Mjeku")
                            .description("Plotësohet në çdo raport të ri.")
                            .child(Input::new(&self.endo)),
                    )
                    .child(
                        Field::new()
                            .label("Aparati")
                            .description("Plotësohet në çdo raport të ri.")
                            .child(Input::new(&self.scope)),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(div().text_sm().child(if has_logo {
                        "Logoja shtohet në krye të raportit."
                    } else {
                        "Pa logo."
                    }))
                    .child(div().flex_1())
                    .child(
                        Button::new("choose-logo")
                            .small()
                            .icon(IconName::Folder)
                            .label(if has_logo {
                                "Ndrysho logon…"
                            } else {
                                "Ngarko logon…"
                            })
                            .on_click(
                                cx.listener(|this, _, window, cx| this.choose_logo(window, cx)),
                            ),
                    )
                    .when(has_logo, |this| {
                        this.child(
                            Button::new("remove-logo")
                                .small()
                                .label("Hiq logon")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.logo.clear();
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(error_line(&self.error, cx))
    }
}
