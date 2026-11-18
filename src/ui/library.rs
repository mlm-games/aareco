use std::rc::Rc;

use aareco_store::Recording;
use repose_core::prelude::*;
use repose_material::Icon;
use repose_material::material3::{TextField, TextFieldConfig};
use repose_ui::*;

use crate::app::{App, Route};
use crate::icons::Symbols;
use crate::ui::common::{
    empty_state, recording_row, scroll_page, section_title, sort_recordings, top_bar,
};

pub fn screen(app: &Rc<App>) -> View {
    let th = theme();
    let library = app.library.get();
    let query = app.search.get();
    let filter = app.library_topic.get();
    let mut recordings: Vec<Recording> = app
        .store
        .borrow()
        .search(&query, filter.as_deref())
        .into_iter()
        .cloned()
        .collect();
    sort_recordings(&mut recordings, app.settings.get().sort);
    let total = library.recordings.len();

    let mut chips: Vec<View> = vec![crate::ui::common::chip("All", filter.is_none(), {
        let app = app.clone();
        move || app.library_topic.set(None)
    })];
    for topic in library.children_of(None) {
        let id = topic.id.clone();
        let selected = filter.as_deref() == Some(topic.id.as_str());
        chips.push(crate::ui::common::chip(&topic.name, selected, {
            let app = app.clone();
            move || {
                if selected {
                    app.library_topic.set(None);
                } else {
                    app.library_topic.set(Some(id.clone()));
                }
            }
        }));
    }

    let subtitle = if total == 0 {
        "No recordings yet".to_string()
    } else if query.is_empty() && filter.is_none() {
        format!("{total} recording{}", if total == 1 { "" } else { "s" })
    } else {
        format!("{} of {total}", recordings.len())
    };

    let mut content: Vec<View> = vec![
        top_bar(
            "Library",
            Some(subtitle),
            None,
            vec![crate::ui::common::icon_button(Symbols::folder, {
                let app = app.clone();
                move || app.nav.push(Route::Topics)
            }, None)],
        ),
        TextField(
            Modifier::new().fill_max_width(),
            query.clone(),
            {
                let app = app.clone();
                move |value| app.search.set(value)
            },
            TextFieldConfig {
                placeholder: Some("Search titles, topics, marks".to_string()),
                leading_icon: Some(Icon(Symbols::search).size(Sp(18.0)).color(th.on_surface_variant)),
                trailing_icon: if query.is_empty() {
                    None
                } else {
                    Some(crate::ui::common::icon_button(Symbols::close, {
                        let app = app.clone();
                        move || app.search.set(String::new())
                    }, None))
                },
                single_line: true,
                ..Default::default()
            },
        ),
        Row(Modifier::new().fill_max_width().flex_wrap(FlexWrap::Wrap).gap(Dp(8.0))).child(chips),
        section_title("Recordings", None),
    ];

    if recordings.is_empty() {
        content.push(empty_state(
            if query.is_empty() { Symbols::library_music } else { Symbols::search },
            if query.is_empty() { "No recordings here" } else { "No matches" },
            if query.is_empty() {
                "Record something new, or browse a different topic."
            } else {
                "Try a different word or clear the search."
            },
        ));
    } else {
        let rows: Vec<View> = recordings
            .iter()
            .map(|recording| recording_row(app, recording))
            .collect();
        content.push(Column(Modifier::new().fill_max_width().gap(Dp(8.0))).child(rows));
    }

    scroll_page("aareco:library", Dp(16.0), content)
}

pub fn topic_detail(app: &Rc<App>, topic_id: &str) -> View {
    let th = theme();
    let library = app.library.get();
    let topic = library.topic(topic_id).cloned();
    let Some(topic) = topic else {
        return empty_state(Symbols::folder, "Topic not found", "It may have been deleted.");
    };
    let breadcrumb = library.breadcrumb(topic_id);
    let mut recordings: Vec<Recording> = app
        .store
        .borrow()
        .recordings_in(Some(topic_id), true)
        .into_iter()
        .cloned()
        .collect();
    sort_recordings(&mut recordings, app.settings.get().sort);
    let children = library.children_of(Some(topic_id));

    let back = {
        let app = app.clone();
        move || {
            app.nav.pop();
        }
    };
    let actions = vec![
        crate::ui::common::icon_button(Symbols::create_new_folder, {
            let app = app.clone();
            let id = topic_id.to_string();
            move || app.open_new_topic(Some(id.clone()))
        }, None),
        crate::ui::common::icon_button(Symbols::edit, {
            let app = app.clone();
            let id = topic_id.to_string();
            move || app.open_rename_topic(&id)
        }, None),
        crate::ui::common::icon_button(Symbols::delete, {
            let app = app.clone();
            let id = topic_id.to_string();
            move || app.open_delete_topic(&id)
        }, Some(th.error)),
    ];

    let mut content: Vec<View> = vec![
        top_bar(
            topic.name.clone(),
            Some(if breadcrumb.is_empty() {
                format!("{} recording(s)", recordings.len())
            } else {
                format!("{breadcrumb} · {} recording(s)", recordings.len())
            }),
            Some(Rc::new(back)),
            actions,
        ),
    ];

    if !children.is_empty() {
        let mut child_views: Vec<View> = Vec::new();
        for child in children {
            let id = child.id.clone();
            let count = library.recording_count(&child.id, true);
            child_views.push(
                Box(Modifier::new()
                    .fill_max_width()
                    .background(th.surface_container)
                    .clip_rounded(Dp(14.0))
                    .padding(Dp(14.0))
                    .clickable()
                    .on_click({
                        let app = app.clone();
                        move || app.nav.push(Route::Topic { id: id.clone() })
                    }))
                .child(
                    Row(Modifier::new()
                        .fill_max_width()
                        .align_items(AlignItems::CENTER)
                        .gap(Dp(10.0)))
                    .child((
                        Icon(Symbols::folder).size(Sp(18.0)).color(th.tertiary),
                        Text(child.name.clone())
                            .size(Sp(14.0))
                            .color(th.on_surface)
                            .single_line(),
                        Spacer(),
                        Text(format!("{count}"))
                            .size(Sp(12.0))
                            .color(th.on_surface_variant),
                        Icon(Symbols::chevron_right)
                            .size(Sp(16.0))
                            .color(th.on_surface_variant),
                    )),
                ),
            );
        }
        content.push(Column(Modifier::new().fill_max_width().gap(Dp(6.0))).child((
            section_title("Topics", None),
            Column(Modifier::new().fill_max_width().gap(Dp(6.0))).child(child_views),
        )));
    }

    content.push(section_title("Recordings", None));
    if recordings.is_empty() {
        content.push(empty_state(
            Symbols::mic,
            "This topic is quiet",
            "Record something and file it here, or move an existing recording in.",
        ));
    } else {
        let rows: Vec<View> = recordings
            .iter()
            .map(|recording| recording_row(app, recording))
            .collect();
        content.push(Column(Modifier::new().fill_max_width().gap(Dp(8.0))).child(rows));
    }

    scroll_page(format!("aareco:topic:{topic_id}"), Dp(16.0), content)
}

pub fn inbox(app: &Rc<App>) -> View {
    let mut recordings: Vec<Recording> = app
        .store
        .borrow()
        .recordings_in(None, false)
        .into_iter()
        .cloned()
        .collect();
    sort_recordings(&mut recordings, app.settings.get().sort);
    let back = {
        let app = app.clone();
        move || {
            app.nav.pop();
        }
    };
    let mut content: Vec<View> = vec![top_bar(
        "Inbox",
        Some(format!("{} unsorted recording(s)", recordings.len())),
        Some(Rc::new(back)),
        vec![crate::ui::common::icon_button(Symbols::create_new_folder, {
            let app = app.clone();
            move || app.open_new_topic(None)
        }, None)],
    )];
    if recordings.is_empty() {
        content.push(empty_state(
            Symbols::check_circle,
            "Inbox zero",
            "Every recording is filed under a topic.",
        ));
    } else {
        content.push(section_title("Unsorted", None));
        let rows: Vec<View> = recordings
            .iter()
            .map(|recording| recording_row(app, recording))
            .collect();
        content.push(Column(Modifier::new().fill_max_width().gap(Dp(8.0))).child(rows));
    }

    scroll_page("aareco:inbox", Dp(16.0), content)
}
