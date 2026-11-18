use std::rc::Rc;

use repose_core::prelude::*;
use repose_core::text::FontWeight;
use repose_material::Icon;
use repose_ui::*;

use crate::app::{App, Route};
use crate::icons::Symbols;
use crate::ui::common::{
    card, empty_state, format_bytes, format_duration, recording_row, scroll_page, section_title,
};

pub fn screen(app: &Rc<App>) -> View {
    let library = app.library.get();
    let stats = app.store.borrow().stats();
    let mut recent: Vec<_> = library.recordings.iter().cloned().collect();
    recent.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    recent.truncate(6);

    let settings = app.settings.get();
    let device = settings
        .device
        .clone()
        .or_else(|| {
            app.devices
                .get()
                .iter()
                .find(|d| d.is_default)
                .map(|d| d.name.clone())
        })
        .unwrap_or_else(|| "Default microphone".to_string());

    let mut content: Vec<View> = vec![
        Row(Modifier::new()
            .fill_max_width()
            .height(Dp(74.0))
            .align_items(AlignItems::CENTER))
        .child((
            Spacer(),
            crate::ui::common::icon_button(
                Symbols::search,
                {
                    let app = app.clone();
                    move || app.nav.push(Route::Library)
                },
                None,
            ),
            crate::ui::common::icon_button(
                Symbols::settings,
                {
                    let app = app.clone();
                    move || app.nav.push(Route::Settings)
                },
                None,
            ),
        )),
        record_hero(app, &device),
        card(
            Row(Modifier::new()
                .fill_max_width()
                .padding(Dp(18.0))
                .gap(Dp(16.0))
                .align_items(AlignItems::CENTER))
            .child((
                stat("Recordings", stats.recordings.to_string()),
                divider(),
                stat("Total audio", format_duration(stats.duration_ms)),
                divider(),
                stat("On disk", format_bytes(stats.bytes)),
            )),
        ),
    ];

    content.push(section_title(
        "Recent",
        Some(crate::ui::common::chip("All recordings", false, {
            let app = app.clone();
            move || app.nav.push(Route::Library)
        })),
    ));

    if recent.is_empty() {
        content.push(card(empty_state(
            Symbols::mic,
            "Nothing recorded yet",
            "Your first voice note is one tap away.",
        )));
    } else {
        let rows: Vec<View> = recent
            .iter()
            .map(|recording| recording_row(app, recording))
            .collect();
        content.push(Column(Modifier::new().fill_max_width().gap(Dp(8.0))).child(rows));
    }

    scroll_page("aareco:home", Dp(18.0), content)
}

fn record_hero(app: &Rc<App>, device: &str) -> View {
    let th = theme();
    let app = app.clone();
    Box(Modifier::new()
        .fill_max_width()
        .background(th.surface_container)
        .border(Dp(1.0), th.outline_variant.with_alpha_f32(0.55), Dp(28.0))
        .clip_rounded(Dp(28.0)))
    .child(
        Column(
            Modifier::new()
                .fill_max_width()
                .padding(Dp(26.0))
                .align_items(AlignItems::CENTER)
                .gap(Dp(14.0)),
        )
        .child((
            Box(Modifier::new()
                .size(Dp(176.0), Dp(176.0))
                .background(th.primary.with_alpha_f32(0.08))
                .clip_rounded(Dp(999.0)))
            .child(
                Box(Modifier::new()
                    .fill_max_size()
                    .align_items(AlignItems::CENTER)
                    .justify_content(JustifyContent::CENTER))
                .child(
                    Box(Modifier::new()
                        .size(Dp(132.0), Dp(132.0))
                        .background(th.primary)
                        .clip_rounded(Dp(999.0))
                        .clickable()
                        .on_click(move || app.start_recording()))
                    .child(
                        Box(Modifier::new()
                            .fill_max_size()
                            .align_items(AlignItems::CENTER)
                            .justify_content(JustifyContent::CENTER))
                        .child((Icon(Symbols::mic).size(Sp(46.0)).color(th.on_primary),)),
                    ),
                ),
            ),
            Text("Start a recording".to_string())
                .size(Sp(18.0))
                .font_weight(FontWeight(600))
                .color(th.on_surface),
            Row(Modifier::new().align_items(AlignItems::CENTER).gap(Dp(6.0))).child((
                Icon(Symbols::graphic_eq).size(Sp(14.0)).color(th.secondary),
                Text(device)
                    .size(Sp(12.0))
                    .color(th.on_surface_variant)
                    .single_line(),
            )),
        )),
    )
}

fn stat(label: &str, value: String) -> View {
    let th = theme();
    Column(Modifier::new().flex_grow(1.0).gap(Dp(3.0))).child((
        Text(value)
            .size(Sp(18.0))
            .font_weight(FontWeight(600))
            .color(th.on_surface)
            .single_line(),
        Text(label.to_string())
            .size(Sp(11.0))
            .color(th.on_surface_variant)
            .single_line(),
    ))
}

fn divider() -> View {
    let th = theme();
    Box(Modifier::new()
        .width(Dp(1.0))
        .height(Dp(28.0))
        .background(th.outline_variant.with_alpha_f32(0.6)))
}
