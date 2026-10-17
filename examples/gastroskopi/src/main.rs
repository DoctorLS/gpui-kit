//! Gastroskopi: write, archive and print gastroscopy reports.
//!
//! The native edition of the single-file browser tool. Reports, photos and clinic
//! details live in the platform's data directory instead of browser storage, and
//! backups and patient files stay interchangeable with the browser edition.

mod archive;
mod backup;
mod dialogs;
mod editor;
mod narrative;
mod preview;
mod print;
mod report;
mod workspace;

use gpui_kit::component::TitleBar;
use gpui_kit::*;

use crate::archive::Archive;
use crate::workspace::Workspace;

fn main() {
    let archive = match Archive::open_default() {
        Ok(archive) => archive,
        Err(err) => {
            eprintln!("The archive folder could not be opened: {err:#}");
            std::process::exit(1);
        }
    };

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            workspace::init(cx);

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1440.), px(900.)), cx)),
                window_min_size: Some(size(px(1024.), px(640.))),
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                window.set_window_title("Raport Gastroskopie");
                let workspace = cx.new(|cx| Workspace::new(archive, window, cx));
                workspace.focus_handle(cx).focus(window, cx);
                workspace
            })
            .expect("failed to open the main window");
            cx.activate(true);
        });
}
