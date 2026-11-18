use std::rc::Rc;

use aareco_store::Library;
use repose_core::modifier::PaddingValues;
use repose_core::prelude::*;
use repose_core::text::FontWeight;
use repose_material::Icon;
use repose_ui::*;

use crate::app::{App, Route};
use crate::icons::Symbols;
use crate::ui::common::{card, empty_state, scroll_page, section_title, top_bar};

pub fn screen(app: &Rc<App>) -> View {
    let library = app.library.get();
    let inbox = library
        .recordings
        .iter()
        .filter(|recording| recording.topic_id.is_none())
        .count();

    let mut rows: Vec<View> = vec![inbox_row(app, inbox)];
    append_rows(app, &library, None, 0, &mut rows);

    let mut content: Vec<View> = vec![
        top_bar(
            "Topics",
            Some(format!(
                "{} topic(s) · {} recording(s)",
                library.topics.len(),
                library.recordings.len()
            )),
            None,
            vec![crate::ui::common::icon_button(
                Symbols::add,
                {
                    let app = app.clone();
                    move || app.open_new_topic(None)
                },
                None,
            )],
        ),
        section_title("All topics", None),
        Column(Modifier::new().fill_max_width().gap(Dp(6.0))).child(rows),
    ];

    if library.topics.is_empty() {
        content.push(card(empty_state(
            Symbols::folder,
            "No topics yet",
            "Topics are folders for your recordings. Create one, or keep everything in the Inbox.",
        )));
    }

    scroll_page("aareco:topics", Dp(16.0), content)
}

fn append_rows(
    app: &Rc<App>,
    library: &Library,
    parent: Option<&str>,
    depth: usize,
    rows: &mut Vec<View>,
) {
    for topic in library.children_of(parent) {
        rows.push(topic_row(app, library, topic, depth + 1));
        append_rows(app, library, Some(&topic.id), depth + 1, rows);
    }
}

fn topic_row(app: &Rc<App>, library: &Library, topic: &aareco_store::Topic, depth: usize) -> View {
    let th = theme();
    let count = library.recording_count(&topic.id, true);
    let id = topic.id.clone();
    Box(Modifier::new()
        .fill_max_width()
        .background(th.surface_container)
        .border(Dp(1.0), th.outline_variant.with_alpha_f32(0.35), Dp(14.0))
        .clip_rounded(Dp(14.0))
        .clickable()
        .on_click({
            let app = app.clone();
            move || app.nav.push(Route::Topic { id: id.clone() })
        })
        .padding_values(PaddingValues {
            left: Dp(14.0 + depth as f32 * 14.0),
            right: Dp(14.0),
            top: Dp(12.0),
            bottom: Dp(12.0),
        }))
    .child(
        Row(Modifier::new()
            .fill_max_width()
            .align_items(AlignItems::CENTER)
            .gap(Dp(10.0)))
        .child((
            Icon(if depth > 1 {
                Symbols::folder_open
            } else {
                Symbols::folder
            })
            .size(Sp(18.0))
            .color(if depth > 1 {
                th.on_surface_variant
            } else {
                th.tertiary
            }),
            Column(Modifier::new().flex_grow(1.0).gap(Dp(2.0))).child((
                Text(topic.name.clone())
                    .size(Sp(14.0))
                    .font_weight(FontWeight(500))
                    .color(th.on_surface)
                    .single_line(),
                Text(format!(
                    "{count} recording(s){}",
                    if topic.parent_id.is_some() {
                        " · nested"
                    } else {
                        ""
                    }
                ))
                .size(Sp(11.0))
                .color(th.on_surface_variant)
                .single_line(),
            )),
            crate::ui::common::icon_button(
                Symbols::create_new_folder,
                {
                    let app = app.clone();
                    let id = topic.id.clone();
                    move || app.open_new_topic(Some(id.clone()))
                },
                None,
            ),
            Icon(Symbols::chevron_right)
                .size(Sp(18.0))
                .color(th.on_surface_variant),
        )),
    )
}

fn inbox_row(app: &Rc<App>, count: usize) -> View {
    let th = theme();
    Box(Modifier::new()
        .fill_max_width()
        .background(th.secondary_container.with_alpha_f32(0.35))
        .border(Dp(1.0), th.secondary_container, Dp(14.0))
        .clip_rounded(Dp(14.0))
        .clickable()
        .on_click({
            let app = app.clone();
            move || app.nav.push(Route::Inbox)
        })
        .padding(Dp(14.0)))
    .child(
        Row(Modifier::new()
            .fill_max_width()
            .align_items(AlignItems::CENTER)
            .gap(Dp(10.0)))
        .child((
            Box(Modifier::new()
                .size(Dp(34.0), Dp(34.0))
                .background(th.secondary_container)
                .clip_rounded(Dp(10.0)))
            .child(
                Box(Modifier::new()
                    .fill_max_size()
                    .align_items(AlignItems::CENTER)
                    .justify_content(JustifyContent::CENTER))
                .child(Icon(Symbols::label).size(Sp(18.0)).color(th.secondary)),
            ),
            Column(Modifier::new().flex_grow(1.0).gap(Dp(2.0))).child((
                Text("Inbox".to_string())
                    .size(Sp(14.0))
                    .font_weight(FontWeight(600))
                    .color(th.on_surface),
                Text(format!("{count} unsorted recording(s)"))
                    .size(Sp(11.0))
                    .color(th.on_surface_variant),
            )),
            Icon(Symbols::chevron_right)
                .size(Sp(18.0))
                .color(th.on_surface_variant),
        )),
    )
}
