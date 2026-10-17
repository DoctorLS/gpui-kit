//! The report form: one control per field of the open [`Record`].
//!
//! The record is the source of truth. Controls report edits back into it and emit
//! [`EditorEvent::Changed`]; loading a record pushes its values into the controls
//! without echoing them back as edits.

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, StyledExt as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    date_picker::{DatePicker, DatePickerEvent, DatePickerState},
    form::{Field, v_form},
    group_box::{GroupBox, GroupBoxVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    menu::{DropdownMenu as _, PopupMenuItem},
    select::{Select, SelectEvent, SelectItem, SelectState},
    switch::Switch,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::archive::Archive;
use crate::narrative::{self, parse_date};
use crate::report::{self, Choice, Photo, Record, Report};

pub enum EditorEvent {
    Changed,
}

/// A select option: the stored value and its label.
#[derive(Clone)]
pub struct ChoiceItem {
    value: SharedString,
    title: SharedString,
}

impl SelectItem for ChoiceItem {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &SharedString {
        &self.value
    }
}

type ChoiceState = SelectState<Vec<ChoiceItem>>;

const TEXT_KEYS: &[(&str, &str)] = &[
    ("pt_name", "p.sh. Ardit Hoxha"),
    ("pt_dob", "dd.mm.vvvv"),
    ("pt_id", ""),
    ("referrer", "p.sh. Dr. … / QKMF …"),
    ("time", "hh:mm"),
    ("endo", ""),
    ("assistant", ""),
    ("scope", "p.sh. Olympus GIF-H190"),
    ("ind_free", "p.sh. histori familjare për karcinomë gastrike"),
    ("propofol", ""),
    ("midazolam", ""),
    ("fentanyl", ""),
    ("o2", ""),
    ("sed_other", "p.sh. butilskopolaminë 20 mg i.v."),
    ("zline", ""),
    ("hh_cm", ""),
    ("bar_c", ""),
    ("bar_m", ""),
    ("s_ul_mm", ""),
    ("pol_n", ""),
    ("pol_mm", ""),
    ("d_ul_mm", ""),
    ("bx_other", "p.sh. 4× Barrett (Seattle), 4× D2 për celiaki"),
    ("int_free", "p.sh. adrenalinë 1:10.000, 6 ml; 2 klipe"),
    ("comp_free", ""),
];

const AREA_KEYS: &[(&str, &str)] = &[
    ("e_free", "p.sh. erozione lineare në 36–38 cm …"),
    ("s_free", ""),
    ("d_free", ""),
    ("concl", "Një diagnozë për rresht"),
    ("recs", "Një rekomandim për rresht"),
];

const SELECT_KEYS: &[(&str, &[Choice])] = &[
    ("pt_sex", report::SEX),
    ("asa", report::ASA),
    ("sed_type", report::SEDATION),
    ("monitor", report::MONITORING),
    ("tolerance", report::TOLERANCE),
    ("la", report::LA_GRADE),
    ("hh", report::YES_NO),
    ("var", report::VARICES),
    ("retro", report::RETROFLEXION),
    ("hill", report::HILL),
    ("s_content", report::GASTRIC_CONTENT),
    ("gastro", report::GASTROPATHY),
    ("s_ul_loc", report::GASTRIC_ULCER_SITE),
    ("s_ul_f", report::FORREST),
    ("pol_loc", report::POLYP_SITE),
    ("pol_paris", report::PARIS),
    ("extent", report::EXTENT),
    ("bulb", report::BULB),
    ("d_ul_loc", report::DUODENAL_ULCER_SITE),
    ("d_ul_f", report::FORREST),
    ("hp_test", report::HP_TEST),
    ("hp_res", report::HP_RESULT),
    ("comp", report::COMPLICATIONS),
];

pub struct ReportEditor {
    archive: Archive,
    record: Record,
    inputs: Vec<(&'static str, Entity<InputState>)>,
    areas: Vec<(&'static str, Entity<TextareaState>)>,
    selects: Vec<(&'static str, Entity<ChoiceState>)>,
    date: Entity<DatePickerState>,
    /// Caption inputs by photo file name, in photo order.
    captions: Vec<(String, Entity<InputState>)>,
    adding_photos: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<EditorEvent> for ReportEditor {}

impl ReportEditor {
    pub fn new(
        archive: Archive,
        record: Record,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        let inputs = TEXT_KEYS
            .iter()
            .map(|&(key, placeholder)| {
                let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
                subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this, state, event, _, cx| {
                        if matches!(event, InputEvent::Change) {
                            let value = state.read(cx).value().to_string();
                            this.on_text_changed(key, value, cx);
                        }
                    },
                ));
                (key, state)
            })
            .collect();

        let areas = AREA_KEYS
            .iter()
            .map(|&(key, placeholder)| {
                let state = cx.new(|cx| {
                    TextareaState::new(window, cx)
                        .auto_grow(2, 12)
                        .placeholder(placeholder)
                });
                subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this, state, event, _, cx| {
                        if matches!(event, InputEvent::Change) {
                            let value = state.read(cx).value().to_string();
                            this.on_text_changed(key, value, cx);
                        }
                    },
                ));
                (key, state)
            })
            .collect();

        let selects = SELECT_KEYS
            .iter()
            .map(|&(key, choices)| {
                let items = choices
                    .iter()
                    .map(|&(value, title)| ChoiceItem {
                        value: value.into(),
                        title: title.into(),
                    })
                    .collect();
                let state = cx.new(|cx| SelectState::new(items, None, window, cx));
                subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this, _, event: &SelectEvent<Vec<ChoiceItem>>, _, cx| {
                        let SelectEvent::Confirm(value) = event;
                        let value = value.as_ref().map(|v| v.to_string()).unwrap_or_default();
                        this.on_text_changed(key, value, cx);
                    },
                ));
                (key, state)
            })
            .collect();

        let date = cx.new(|cx| DatePickerState::new(window, cx).date_format("%d.%m.%Y"));
        subscriptions.push(cx.subscribe_in(
            &date,
            window,
            |this, _, event: &DatePickerEvent, _, cx| {
                let DatePickerEvent::Change(value) = event;
                let iso = value
                    .start()
                    .map(|d| d.date().to_string())
                    .unwrap_or_default();
                this.on_text_changed("date", iso, cx);
            },
        ));

        let mut editor = Self {
            archive,
            record: Record::new(Report::default()),
            inputs,
            areas,
            selects,
            date,
            captions: Vec::new(),
            adding_photos: false,
            _subscriptions: subscriptions,
        };
        editor.load(record, window, cx);
        editor
    }

    pub fn record(&self) -> &Record {
        &self.record
    }

    /// Records when the open report was stored, without touching its contents.
    pub fn mark_saved(&mut self, created: i64, updated: i64) {
        self.record.created = created;
        self.record.updated = updated;
    }

    /// Shows `record` in the form.
    pub fn load(&mut self, record: Record, window: &mut Window, cx: &mut Context<Self>) {
        self.record = record;
        for (key, state) in &self.inputs {
            let value = match *key {
                // People type and read birth dates day first; the record keeps ISO.
                "pt_dob" => narrative::fmt_date(&self.record.form.pt_dob),
                key => self.record.form.text(key).to_string(),
            };
            state.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        for (key, state) in &self.areas {
            let value = self.record.form.text(key).to_string();
            state.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        for (key, state) in &self.selects {
            let value = SharedString::from(self.record.form.text(key).to_string());
            state.update(cx, |state, cx| state.set_selected_value(&value, window, cx));
        }
        let date = parse_date(&self.record.form.date);
        self.date.update(cx, |state, cx| {
            state.set_date(calendar_date(date), window, cx)
        });
        let photos = self.record.photos.clone();
        self.captions = photos
            .iter()
            .map(|photo| self.caption_input(photo, window, cx))
            .collect();
        cx.notify();
    }

    fn caption_input(
        &mut self,
        photo: &Photo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (String, Entity<InputState>) {
        let file = photo.file.clone();
        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Përshkrimi")
                .default_value(photo.cap.clone())
        });
        self._subscriptions.push(cx.subscribe_in(&state, window, {
            let file = file.clone();
            move |this, state, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let caption = state.read(cx).value().to_string();
                    if let Some(photo) = this.record.photos.iter_mut().find(|p| p.file == file) {
                        photo.cap = caption;
                        cx.emit(EditorEvent::Changed);
                        cx.notify();
                    }
                }
            }
        }));
        (file, state)
    }

    fn on_text_changed(&mut self, key: &'static str, value: String, cx: &mut Context<Self>) {
        let value = match key {
            "pt_dob" => chrono::NaiveDate::parse_from_str(value.trim(), "%d.%m.%Y")
                .map(|date| date.to_string())
                .unwrap_or(value),
            _ => value,
        };
        let field = self.record.form.text_mut(key);
        if *field != value {
            *field = value;
            self.changed(cx);
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    /// Changes a field from code and shows the new value in its control.
    fn set_text(&mut self, key: &str, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        *self.record.form.text_mut(key) = value.to_string();
        if let Some((_, state)) = self.areas.iter().find(|(k, _)| *k == key) {
            state.update(cx, |state, cx| {
                state.set_value(value.to_string(), window, cx)
            });
        }
        if let Some((_, state)) = self.selects.iter().find(|(k, _)| *k == key) {
            let value = SharedString::from(value.to_string());
            state.update(cx, |state, cx| state.set_selected_value(&value, window, cx));
        }
    }

    fn set_flag(&mut self, key: &str, value: bool, cx: &mut Context<Self>) {
        *self.record.form.flag_mut(key) = value;
        self.changed(cx);
    }

    fn toggle_choice(&mut self, key: &str, choice: &str, checked: bool, cx: &mut Context<Self>) {
        let list = self.record.form.list_mut(key);
        list.retain(|item| item != choice);
        if checked {
            list.push(choice.to_string());
        }
        self.changed(cx);
    }

    /// Marks every organ normal and fills the usual answers that are still empty.
    pub fn apply_normal_findings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let form = &mut self.record.form;
        form.e_normal = true;
        form.s_normal = true;
        form.d_normal = true;
        let defaults = [
            ("retro", "u krye"),
            ("tolerance", "e mirë"),
            ("comp", "asnjë"),
        ];
        for (key, value) in defaults {
            if self.record.form.text(key).is_empty() {
                self.set_text(key, value, window, cx);
            }
        }
        self.set_text("extent", "D2", window, cx);
        if self.record.form.concl.trim().is_empty() {
            let conclusion = narrative::auto_conclusion(&self.record.form);
            self.set_text("concl", &conclusion, window, cx);
        }
        self.changed(cx);
    }

    /// Writes the conclusion proposed by the findings, asking before replacing one.
    pub fn generate_conclusion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.record.form.concl.trim().is_empty() {
            self.replace_conclusion(window, cx);
            return;
        }
        let editor = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let editor = editor.clone();
            alert
                .title("Të zëvendësohet konkluzioni?")
                .description("Konkluzioni aktual zëvendësohet me atë që propozojnë gjetjet.")
                .show_cancel(true)
                .cancel_text("Anulo")
                .ok_text("Zëvendëso")
                .on_ok(move |_, window, cx| {
                    _ = editor.update(cx, |editor, cx| editor.replace_conclusion(window, cx));
                    true
                })
        });
    }

    fn replace_conclusion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let conclusion = narrative::auto_conclusion(&self.record.form);
        self.set_text("concl", &conclusion, window, cx);
        self.changed(cx);
    }

    fn add_recommendation(&mut self, line: &str, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.record.form.recs.trim();
        if current.lines().any(|existing| existing.trim() == line) {
            return;
        }
        let recs = if current.is_empty() {
            line.to_string()
        } else {
            format!("{current}\n{line}")
        };
        self.set_text("recs", &recs, window, cx);
        self.changed(cx);
    }

    fn add_photos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Shto".into()),
        });
        let archive = self.archive.clone();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            _ = this.update(cx, |this, cx| {
                this.adding_photos = true;
                cx.notify();
            });
            let results = cx
                .background_executor()
                .spawn(async move {
                    paths
                        .iter()
                        .map(|path| archive.add_photo(path))
                        .collect::<Vec<_>>()
                })
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                this.adding_photos = false;
                let mut failed = 0;
                for result in results {
                    match result {
                        Ok(file) => {
                            let photo = Photo {
                                file,
                                cap: String::new(),
                            };
                            let caption = this.caption_input(&photo, window, cx);
                            this.captions.push(caption);
                            this.record.photos.push(photo);
                        }
                        Err(_) => failed += 1,
                    }
                }
                if failed > 0 {
                    window.push_notification(
                        gpui_kit::component::notification::Notification::error(format!(
                            "{failed} skedarë nuk u hapën si foto."
                        )),
                        cx,
                    );
                }
                this.changed(cx);
            });
        })
        .detach();
    }

    fn remove_photo(&mut self, file: &str, cx: &mut Context<Self>) {
        self.record.photos.retain(|photo| photo.file != file);
        self.captions.retain(|(f, _)| f != file);
        _ = std::fs::remove_file(self.archive.photo_path(file));
        self.changed(cx);
    }

    fn input(&self, key: &str) -> &Entity<InputState> {
        &self
            .inputs
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap_or_else(|| unreachable!("no input for `{key}`"))
            .1
    }

    fn area(&self, key: &str) -> &Entity<TextareaState> {
        &self
            .areas
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap_or_else(|| unreachable!("no text area for `{key}`"))
            .1
    }

    fn select(&self, key: &str) -> &Entity<ChoiceState> {
        &self
            .selects
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap_or_else(|| unreachable!("no select for `{key}`"))
            .1
    }

    fn text_field(&self, label: &'static str, key: &str) -> Field {
        Field::new().label(label).child(Input::new(self.input(key)))
    }

    fn area_field(&self, label: &'static str, key: &str) -> Field {
        Field::new()
            .label(label)
            .child(Textarea::new(self.area(key)))
    }

    fn select_field(&self, label: &'static str, key: &str) -> Field {
        Field::new()
            .label(label)
            .child(Select::new(self.select(key)).accessibility_label(label))
    }

    fn choices(
        &self,
        key: &'static str,
        options: &'static [&'static str],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.record.form.list(key);
        h_flex()
            .flex_wrap()
            .gap_x_4()
            .gap_y_2()
            .children(options.iter().enumerate().map(|(ix, &option)| {
                Checkbox::new((key, ix))
                    .label(option)
                    .checked(selected.iter().any(|s| s == option))
                    .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                        this.toggle_choice(key, option, *checked, cx);
                    }))
            }))
    }

    fn flag(&self, key: &'static str, label: &'static str, cx: &mut Context<Self>) -> Checkbox {
        let checked = *self.record.form.flag(key);
        Checkbox::new(key)
            .label(label)
            .checked(checked)
            .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                this.set_flag(key, *checked, cx);
            }))
    }

    fn normal_switch(
        &self,
        key: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> Switch {
        let checked = *self.record.form.flag(key);
        Switch::new(key)
            .label(label)
            .checked(checked)
            .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                this.set_flag(key, *checked, cx);
            }))
    }

    fn section(
        &self,
        id: &'static str,
        title: &'static str,
        summary: String,
        cx: &App,
    ) -> GroupBox {
        GroupBox::new().id(id).outline().title(
            h_flex()
                .w_full()
                .gap_2()
                .child(div().font_semibold().child(title))
                .when(!summary.is_empty(), |this| {
                    this.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(summary),
                    )
                }),
        )
    }

    fn subheading(text: &'static str, cx: &App) -> Div {
        div()
            .text_sm()
            .font_semibold()
            .text_color(cx.theme().muted_foreground)
            .child(text)
    }

    fn render_patient(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let r = &self.record.form;
        let summary = [r.pt_name.trim().to_string(), narrative::fmt_date(&r.date)]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        self.section("patient", "Pacienti dhe procedura", summary, cx)
            .child(
                v_form()
                    .columns(4)
                    .child(
                        self.text_field("Emri dhe mbiemri", "pt_name")
                            .required(true)
                            .col_span(2),
                    )
                    .child(self.text_field("Datëlindja", "pt_dob"))
                    .child(self.select_field("Gjinia", "pt_sex"))
                    .child(self.text_field("Nr. i dosjes", "pt_id"))
                    .child(self.text_field("Referuar nga", "referrer").col_span(3))
                    .child(
                        Field::new()
                            .label("Data e procedurës")
                            .required(true)
                            .col_span(2)
                            .child(DatePicker::new(&self.date)),
                    )
                    .child(self.text_field("Ora", "time"))
                    .child(self.select_field("ASA", "asa"))
                    .child(self.text_field("Mjeku endoskopist", "endo").col_span(2))
                    .child(self.text_field("Asistenti/ja", "assistant").col_span(2))
                    .child(self.text_field("Aparati", "scope").col_span(2)),
            )
            .child(Self::subheading("Indikacioni", cx))
            .child(self.choices("ind", report::INDICATIONS, cx))
            .child(v_form().child(self.text_field("Tjetër", "ind_free")))
            .child(self.flag("consent", "Pëlqimi i informuar u mor me shkrim", cx))
    }

    fn render_sedation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        self.section(
            "sedation",
            "Sedacioni dhe toleranca",
            self.record.form.sed_type.clone(),
            cx,
        )
        .child(
            v_form()
                .columns(4)
                .child(self.select_field("Lloji", "sed_type").col_span(4))
                .child(self.text_field("Propofol (mg)", "propofol"))
                .child(self.text_field("Midazolam (mg)", "midazolam"))
                .child(self.text_field("Fentanil (µg)", "fentanyl"))
                .child(self.text_field("O₂ (l/min)", "o2"))
                .child(self.text_field("Tjetër", "sed_other").col_span(2))
                .child(self.select_field("Monitorimi", "monitor"))
                .child(self.select_field("Toleranca", "tolerance")),
        )
    }

    fn render_esophagus(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let normal = self.record.form.e_normal;
        self.section(
            "esophagus",
            "Ezofagu",
            narrative::section_summary(&self.record.form, "e"),
            cx,
        )
        .child(self.normal_switch("e_normal", "Ezofag normal", cx))
        .child(
            v_form()
                .columns(4)
                .child(
                    self.text_field("Linja Z (cm nga harqet dentare)", "zline")
                        .col_span(2),
                )
                .when(!normal, |form| {
                    form.child(
                        self.select_field("Ezofagit refluksi (LA)", "la")
                            .col_span(2),
                    )
                    .child(self.select_field("Hernie hiatale", "hh"))
                    .child(self.text_field("Gjatësia e hernies (cm)", "hh_cm"))
                    .child(self.text_field("Barrett – Praga C (cm)", "bar_c"))
                    .child(self.text_field("Barrett – Praga M (cm)", "bar_m"))
                    .child(self.select_field("Varice ezofageale", "var").col_span(4))
                }),
        )
        .when(!normal, |this| {
            this.child(self.flag(
                "var_red",
                "Varice me shenja të kuqe (red wale / cherry-red spots)",
                cx,
            ))
            .child(Self::subheading("Gjetje të tjera", cx))
            .child(self.choices("e_other", report::ESOPHAGUS_OTHER, cx))
        })
        .child(v_form().child(self.area_field("Përshkrim shtesë", "e_free")))
    }

    fn render_stomach(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let normal = self.record.form.s_normal;
        self.section(
            "stomach",
            "Stomaku",
            narrative::section_summary(&self.record.form, "s"),
            cx,
        )
        .child(self.normal_switch("s_normal", "Stomak normal", cx))
        .child(
            v_form()
                .columns(4)
                .child(self.select_field("Retrofleksioni", "retro").col_span(2))
                .child(self.select_field("Valvula GE (Hill)", "hill").col_span(2))
                .when(!normal, |form| {
                    form.child(self.select_field("Përmbajtja", "s_content").col_span(2))
                        .child(self.select_field("Gastropatia", "gastro").col_span(2))
                }),
        )
        .when(!normal, |this| {
            this.child(Self::subheading("Ulçera gastrike", cx))
                .child(
                    v_form()
                        .columns(4)
                        .child(self.select_field("Lokalizimi", "s_ul_loc"))
                        .child(self.text_field("Madhësia (mm)", "s_ul_mm"))
                        .child(self.select_field("Forrest", "s_ul_f").col_span(2)),
                )
                .child(Self::subheading("Polipe", cx))
                .child(
                    v_form()
                        .columns(4)
                        .child(self.text_field("Numri", "pol_n"))
                        .child(self.text_field("Madhësia max (mm)", "pol_mm"))
                        .child(self.select_field("Lokalizimi", "pol_loc"))
                        .child(self.select_field("Paris", "pol_paris")),
                )
                .child(Self::subheading("Gjetje të tjera", cx))
                .child(self.choices("s_other", report::STOMACH_OTHER, cx))
        })
        .child(v_form().child(self.area_field("Përshkrim shtesë", "s_free")))
    }

    fn render_duodenum(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let normal = self.record.form.d_normal;
        self.section(
            "duodenum",
            "Duodeni",
            narrative::section_summary(&self.record.form, "d"),
            cx,
        )
        .child(self.normal_switch("d_normal", "Duoden normal", cx))
        .child(
            v_form()
                .columns(4)
                .child(
                    self.select_field("Ekzaminimi deri në", "extent")
                        .col_span(2),
                )
                .when(!normal, |form| {
                    form.child(self.select_field("Bulbi", "bulb").col_span(2))
                }),
        )
        .when(!normal, |this| {
            this.child(Self::subheading("Ulçera duodenale", cx))
                .child(
                    v_form()
                        .columns(4)
                        .child(self.select_field("Lokalizimi", "d_ul_loc"))
                        .child(self.text_field("Madhësia (mm)", "d_ul_mm"))
                        .child(self.select_field("Forrest", "d_ul_f").col_span(2)),
                )
                .child(Self::subheading("Gjetje të tjera", cx))
                .child(self.choices("d_other", report::DUODENUM_OTHER, cx))
        })
        .child(v_form().child(self.area_field("Përshkrim shtesë", "d_free")))
    }

    fn render_biopsy(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let r = &self.record.form;
        let summary = [
            (!r.sydney.is_empty()).then(|| "Sydney".to_string()),
            (!r.bx_other.trim().is_empty()).then(|| "biopsi".to_string()),
            (!r.interventions.is_empty()).then(|| format!("{} ndërhyrje", r.interventions.len())),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
        self.section("biopsy", "Biopsi, ndërhyrje, komplikime", summary, cx)
            .child(Self::subheading("Biopsi sipas protokollit Sydney", cx))
            .child(self.choices("sydney", report::SYDNEY, cx))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("A1/A2: antrum (kurvatura e vogël/e madhe) · IA: angulus · B1/B2: korpus (kurvatura e vogël/e madhe)"),
            )
            .child(
                v_form()
                    .columns(4)
                    .child(self.text_field("Biopsi të tjera", "bx_other").col_span(4))
                    .child(self.select_field("Testi H. pylori", "hp_test").col_span(2))
                    .child(self.select_field("Rezultati HP", "hp_res").col_span(2)),
            )
            .child(Self::subheading("Ndërhyrje terapeutike", cx))
            .child(self.choices("int", report::INTERVENTIONS, cx))
            .child(
                v_form()
                    .columns(4)
                    .child(self.text_field("Detaje", "int_free").col_span(4))
                    .child(self.select_field("Komplikime", "comp").col_span(2))
                    .child(self.text_field("Shënim", "comp_free").col_span(2)),
            )
    }

    fn render_photos(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.record.photos.len();
        let summary = if count == 0 {
            String::new()
        } else {
            format!("{count} foto")
        };
        self.section("photos", "Fotodokumentimi", summary, cx)
            .child(
                h_flex().child(
                    Button::new("add-photos")
                        .icon(IconName::Plus)
                        .label("Shto foto…")
                        .loading(self.adding_photos)
                        .disabled(self.adding_photos)
                        .on_click(cx.listener(|this, _, window, cx| this.add_photos(window, cx))),
                ),
            )
            .when(count > 0, |this| {
                this.child(
                    div().grid().grid_cols(3).gap_3().children(
                        self.captions
                            .iter()
                            .enumerate()
                            .map(|(ix, (file, caption))| {
                                let remove = file.clone();
                                v_flex()
                                    .gap_1()
                                    .child(
                                        img(self.archive.photo_path(file))
                                            .w_full()
                                            .h_32()
                                            .rounded(cx.theme().radius)
                                            .object_fit(ObjectFit::Cover),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .child(div().text_sm().child(format!("{}.", ix + 1)))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .child(Input::new(caption).small()),
                                            )
                                            .child(
                                                Button::new(("remove-photo", ix))
                                                    .ghost()
                                                    .small()
                                                    .icon(IconName::Close)
                                                    .tooltip("Hiq foton")
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.remove_photo(&remove, cx)
                                                        },
                                                    )),
                                            ),
                                    )
                            }),
                    ),
                )
            })
    }

    fn render_conclusion(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = cx.entity().downgrade();
        self.section(
            "conclusion",
            "Konkluzioni dhe rekomandimet",
            String::new(),
            cx,
        )
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("generate-conclusion")
                        .icon(IconName::Redo)
                        .label("Gjenero nga gjetjet")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.generate_conclusion(window, cx)),
                        ),
                )
                .child(
                    Button::new("add-recommendation")
                        .icon(IconName::Plus)
                        .label("Shto rekomandim")
                        .dropdown_caret(true)
                        .dropdown_menu(move |menu, _, _| {
                            report::RECOMMENDATIONS.iter().fold(menu, |menu, &line| {
                                let editor = editor.clone();
                                menu.item(PopupMenuItem::new(line).on_click(
                                    move |_, window, cx| {
                                        _ = editor.update(cx, |editor, cx| {
                                            editor.add_recommendation(line, window, cx)
                                        });
                                    },
                                ))
                            })
                        }),
                ),
        )
        .child(
            v_form()
                .child(self.area_field("Konkluzioni", "concl"))
                .child(self.area_field("Rekomandimet", "recs")),
        )
    }
}

fn calendar_date(date: Option<chrono::NaiveDate>) -> gpui_kit::component::calendar::Date {
    gpui_kit::component::calendar::Date::Single(date)
}

impl Render for ReportEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_4()
            .p_4()
            .child(self.render_patient(cx))
            .child(self.render_sedation(cx))
            .child(self.render_esophagus(cx))
            .child(self.render_stomach(cx))
            .child(self.render_duodenum(cx))
            .child(self.render_biopsy(cx))
            .child(self.render_photos(cx))
            .child(self.render_conclusion(cx))
    }
}

impl Focusable for ReportEditor {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input("pt_name").focus_handle(cx)
    }
}
