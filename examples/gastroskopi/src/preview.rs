//! The live report preview beside the form.
//!
//! It shows the same [`ReportDoc`] the printed page and the text copy are made from,
//! drawn with the window theme so it stays readable in light and dark mode.

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::narrative::{ORGAN_SECTIONS, ReportDoc, dot};
use crate::report::Clinic;

#[derive(IntoElement)]
pub struct ReportPreview {
    doc: ReportDoc,
    clinic: Clinic,
    logo: Option<Arc<Image>>,
    /// Photo files and captions, in report order.
    photos: Vec<(PathBuf, String)>,
    signer: String,
}

impl ReportPreview {
    pub fn new(
        doc: ReportDoc,
        clinic: Clinic,
        logo: Option<Arc<Image>>,
        photos: Vec<(PathBuf, String)>,
        signer: String,
    ) -> Self {
        Self {
            doc,
            clinic,
            logo,
            photos,
            signer,
        }
    }
}

fn heading(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_xs()
        .font_semibold()
        .text_color(cx.theme().primary)
        .child(text.into().to_uppercase())
}

fn empty(text: &'static str, cx: &App) -> Div {
    div()
        .italic()
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

impl RenderOnce for ReportPreview {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let clinic_name = if self.clinic.c_name.trim().is_empty() {
            "Emri i klinikës".to_string()
        } else {
            self.clinic.c_name.trim().to_string()
        };

        // The document title sits on its own row so a long clinic name and a logo
        // keep their width in a narrow pane.
        let header = v_flex()
            .gap_2()
            .pb_2()
            .border_b_2()
            .border_color(theme.primary)
            .child(
                h_flex()
                    .gap_3()
                    .when_some(self.logo, |this, logo| {
                        this.child(
                            img(logo)
                                .flex_none()
                                .h_10()
                                .w_24()
                                .object_fit(ObjectFit::Contain),
                        )
                    })
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().font_bold().child(clinic_name))
                            .when(!self.clinic.c_addr.trim().is_empty(), |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(self.clinic.c_addr.trim().to_string()),
                                )
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_bold()
                            .text_color(theme.primary)
                            .child("EZOFAGOGASTRODUODENOSKOPI"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Raport endoskopik"),
                    ),
            );

        // Labels sit above their values so the table stays legible in a narrow pane.
        let meta = div()
            .grid()
            .grid_cols(2)
            .border_t_1()
            .border_l_1()
            .border_color(theme.border)
            .children(self.doc.meta.iter().map(|(key, value)| {
                v_flex()
                    .min_w_0()
                    .px_2()
                    .py_1()
                    .border_r_1()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(*key),
                    )
                    .child(
                        div()
                            .when(*key == "Pacienti", |this| this.font_semibold())
                            .map(|this| {
                                if value.is_empty() {
                                    this.text_color(theme.muted_foreground).child("—")
                                } else {
                                    this.child(value.clone())
                                }
                            }),
                    )
            }));

        let sections = self
            .doc
            .sections
            .iter()
            .filter(|(title, body)| !body.is_empty() || ORGAN_SECTIONS.contains(title))
            .map(|(title, body)| {
                v_flex()
                    .gap_0p5()
                    .child(heading(*title, cx))
                    .child(if body.is_empty() {
                        empty("Pa të dhëna.", cx)
                    } else {
                        div().child(body.clone())
                    })
            });

        let photos = (!self.photos.is_empty()).then(|| {
            v_flex()
                .gap_1()
                .child(heading("Fotodokumentimi", cx))
                .child(div().grid().grid_cols(4).gap_2().children(
                    self.photos.iter().enumerate().map(|(ix, (path, caption))| {
                        v_flex()
                            .gap_0p5()
                            .child(
                                img(path.clone())
                                    .w_full()
                                    .h_20()
                                    .rounded(theme.radius)
                                    .object_fit(ObjectFit::Cover),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_center()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{}. {}", ix + 1, caption.trim())),
                            )
                    }),
                ))
        });

        let conclusion = v_flex()
            .gap_1()
            .p_3()
            .rounded(theme.radius)
            .bg(theme.muted)
            .child(heading("Konkluzioni", cx))
            .map(|this| {
                if self.doc.concl.is_empty() {
                    this.child(empty("Shkruaj ose gjenero konkluzionin.", cx))
                } else {
                    this.children(self.doc.concl.iter().enumerate().map(|(ix, line)| {
                        div()
                            .font_semibold()
                            .child(format!("{}. {}", ix + 1, dot(line)))
                    }))
                }
            });

        let recommendations = (!self.doc.recs.is_empty()).then(|| {
            v_flex()
                .gap_0p5()
                .child(heading("Rekomandime", cx))
                .children(
                    self.doc
                        .recs
                        .iter()
                        .map(|line| div().child(format!("• {}", dot(line)))),
                )
        });

        let signer = if self.signer.trim().is_empty() {
            "Mjeku endoskopist".to_string()
        } else {
            self.signer.trim().to_string()
        };

        v_flex()
            .gap_3()
            .p_6()
            .text_sm()
            .bg(theme.background)
            .text_color(theme.foreground)
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius_lg)
            .child(header)
            .child(meta)
            .children(sections)
            .children(photos)
            .child(conclusion)
            .children(recommendations)
            .child(
                h_flex().justify_end().pt_6().child(
                    v_flex()
                        .w_56()
                        .items_center()
                        .pt_1()
                        .border_t_1()
                        .border_color(theme.foreground)
                        .child(signer)
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("Nënshkrimi dhe vula"),
                        ),
                ),
            )
    }
}
