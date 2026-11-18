use std::rc::Rc;

use aareco_store::{Library, Recording};
use chrono::{DateTime, Local, TimeZone};
use repose_core::prelude::*;
use repose_core::modifier::PaddingValues;
use repose_core::text::FontWeight;
use repose_material::Icon;
use repose_material::material3::{IconButton, IconButtonConfig, SliderConfig};
use repose_ui::scroll::{ScrollArea, remember_scroll_state};
use repose_ui::*;

use crate::app::App;
use crate::icons::Symbols;
use crate::settings::SortOrder;

pub const PAGE_MAX: Dp = Dp(920.0);

pub fn scroll_page(key: impl Into<String>, gap: Dp, content: impl IntoChildren) -> View {
    ScrollArea(
        Modifier::new()
            .fill_max_size()
            .padding_values(PaddingValues {
                left: Dp(20.0),
                right: Dp(20.0),
                top: Dp(0.0),
                bottom: Dp(0.0),
            }),
        remember_scroll_state(key),
        Box(Modifier::new()
            .fill_max_width()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::FLEX_START))
        .child(
            Column(Modifier::new()
                .fill_max_width()
                .max_width(PAGE_MAX)
                .gap(gap))
            .child(content)
            .child(Box(Modifier::new().height(Dp(24.0)))),
        ),
    )
}

pub fn format_duration(ms: u64) -> String {
    let total = ms / 1000;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

pub fn format_timer(ms: u64) -> String {
    let total = ms / 1000;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    let tenths = (ms % 1000) / 100;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}.{tenths}")
    }
}

pub fn format_date(ms: i64) -> String {
    let Some(dt) = Local.timestamp_millis_opt(ms).single() else {
        return String::new();
    };
    let today = Local::now().date_naive();
    if dt.date_naive() == today {
        format!("Today · {}", dt.format("%H:%M"))
    } else if dt.date_naive() == today.pred_opt().unwrap_or(today) {
        format!("Yesterday · {}", dt.format("%H:%M"))
    } else {
        dt.format("%b %-d · %H:%M").to_string()
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let value = bytes as f64;
    if value >= KB * KB * KB {
        format!("{:.1} GB", value / (KB * KB * KB))
    } else if value >= KB * KB {
        format!("{:.0} MB", value / (KB * KB))
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

pub fn file_date(path: &std::path::Path, fallback: i64) -> i64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(fallback)
}

pub fn timestamp_date(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|dt: DateTime<Local>| dt.format("%b %-d, %H:%M").to_string())
        .unwrap_or_default()
}

pub fn page(modifier: Modifier, children: impl IntoChildren) -> View {
    Column(
        Modifier::new()
            .fill_max_width()
            .max_width(PAGE_MAX)
            .gap(Dp(20.0))
            .then(modifier),
    )
    .child(children)
}

pub fn top_bar(
    title: impl Into<String>,
    subtitle: Option<String>,
    on_back: Option<Rc<dyn Fn()>>,
    actions: Vec<View>,
) -> View {
    let th = theme();
    let title = title.into();
    let mut row = Row(Modifier::new()
        .fill_max_width()
        .height(Dp(64.0))
        .padding_values(PaddingValues {
            left: Dp(12.0),
            right: Dp(12.0),
            top: Dp(8.0),
            bottom: Dp(8.0),
        })
        .align_items(AlignItems::CENTER)
        .gap(Dp(8.0)));

    if let Some(on_back) = on_back {
        row = row.child(icon_button(Symbols::arrow_back, move || on_back(), None));
    }

    row = row.child(
        Column(Modifier::new()
            .gap(Dp(1.0))
            .flex_grow(1.0)
            .flex_shrink(1.0))
        .child((
            Text(title)
                .size(Sp(22.0))
                .font_weight(FontWeight(600))
                .color(th.on_surface)
                .single_line()
                .overflow_ellipsize(),
            match subtitle {
                Some(subtitle) => Text(subtitle)
                    .size(Sp(12.0))
                    .color(th.on_surface_variant)
                    .single_line()
                    .overflow_ellipsize(),
                None => Spacer(),
            },
        )),
    );
    for action in actions {
        row = row.child(action);
    }
    row
}

pub fn icon_button(
    symbol: repose_material::Symbol,
    on_click: impl Fn() + 'static,
    tint: Option<Color>,
) -> View {
    let th = theme();
    IconButton(
        Icon(symbol)
            .size(Sp(22.0))
            .color(tint.unwrap_or(th.on_surface_variant)),
        on_click,
        IconButtonConfig::default(),
    )
}

pub fn round_icon_button(
    symbol: repose_material::Symbol,
    on_click: impl Fn() + 'static,
    background: Color,
    foreground: Color,
    size: Dp,
) -> View {
    Box(Modifier::new()
        .size(size, size)
        .background(background)
        .clip_rounded(Dp(999.0))
        .clickable()
        .on_click(on_click))
    .child(
        Box(Modifier::new()
            .fill_max_size()
            .align_items(AlignItems::CENTER)
            .justify_content(JustifyContent::CENTER))
        .child(Icon(symbol).size(Sp(size.0 * 0.42)).color(foreground)),
    )
}

pub fn primary_button(label: &str, on_click: impl Fn() + 'static) -> View {
    let th = theme();
    Box(Modifier::new()
        .background(th.primary)
        .clip_rounded(Dp(999.0))
        .padding_values(PaddingValues {
            left: Dp(20.0),
            right: Dp(20.0),
            top: Dp(12.0),
            bottom: Dp(12.0),
        })
        .clickable()
        .on_click(on_click))
    .child(
        Text(label)
            .size(Sp(14.0))
            .font_weight(FontWeight(600))
            .color(th.on_primary)
            .single_line(),
    )
}

pub fn secondary_button(label: &str, on_click: impl Fn() + 'static) -> View {
    let th = theme();
    Box(Modifier::new()
        .background(th.surface_container_high)
        .border(Dp(1.0), th.outline_variant, Dp(999.0))
        .clip_rounded(Dp(999.0))
        .padding_values(PaddingValues {
            left: Dp(18.0),
            right: Dp(18.0),
            top: Dp(11.0),
            bottom: Dp(11.0),
        })
        .clickable()
        .on_click(on_click))
    .child(
        Text(label)
            .size(Sp(14.0))
            .font_weight(FontWeight(500))
            .color(th.on_surface)
            .single_line(),
    )
}

pub fn chip(label: &str, selected: bool, on_click: impl Fn() + 'static) -> View {
    let th = theme();
    let (bg, fg) = if selected {
        (th.secondary_container, th.on_secondary_container)
    } else {
        (th.surface_container_high, th.on_surface_variant)
    };
    Box(Modifier::new()
        .background(bg)
        .clip_rounded(Dp(999.0))
        .padding_values(PaddingValues {
            left: Dp(14.0),
            right: Dp(14.0),
            top: Dp(8.0),
            bottom: Dp(8.0),
        })
        .clickable()
        .on_click(on_click))
    .child(
        Text(label)
            .size(Sp(12.0))
            .font_weight(FontWeight(500))
            .color(fg)
            .single_line(),
    )
}

pub fn section_title(title: &str, trailing: Option<View>) -> View {
    let th = theme();
    let mut row = Row(Modifier::new()
        .fill_max_width()
        .align_items(AlignItems::CENTER)
        .gap(Dp(8.0)))
    .child(
        Text(title)
            .size(Sp(13.0))
            .font_weight(FontWeight(600))
            .color(th.on_surface_variant),
    )
    .child(Spacer());
    if let Some(trailing) = trailing {
        row = row.child(trailing);
    }
    row
}

pub fn card(children: impl IntoChildren) -> View {
    let th = theme();
    Box(Modifier::new()
        .fill_max_width()
        .background(th.surface_container)
        .border(Dp(1.0), th.outline_variant.with_alpha_f32(0.5), Dp(20.0))
        .clip_rounded(Dp(20.0)))
    .child(children)
}

pub fn empty_state(icon: repose_material::Symbol, title: &str, body: &str) -> View {
    let th = theme();
    Column(Modifier::new()
        .fill_max_width()
        .padding(Dp(32.0))
        .align_items(AlignItems::CENTER)
        .gap(Dp(10.0)))
    .child((
        Box(Modifier::new()
            .size(Dp(64.0), Dp(64.0))
            .background(th.surface_container_high)
            .clip_rounded(Dp(999.0)))
        .child(
            Box(Modifier::new()
                .fill_max_size()
                .align_items(AlignItems::CENTER)
                .justify_content(JustifyContent::CENTER))
            .child(Icon(icon).size(Sp(28.0)).color(th.on_surface_variant)),
        ),
        Text(title)
            .size(Sp(16.0))
            .font_weight(FontWeight(600))
            .color(th.on_surface),
        Text(body)
            .size(Sp(13.0))
            .color(th.on_surface_variant)
            .text_align(repose_core::text::TextAlign::Center)
            .max_lines(3),
    ))
}

pub fn sort_recordings(recordings: &mut [Recording], order: SortOrder) {
    match order {
        SortOrder::Newest => recordings.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        SortOrder::Oldest => recordings.sort_by(|a, b| a.created_at.cmp(&b.created_at)),
        SortOrder::Title => {
            recordings.sort_by_cached_key(|recording| recording.title.to_lowercase())
        }
    }
}

pub fn recording_topic_label(library: &Library, recording: &Recording) -> String {
    recording
        .topic_id
        .as_deref()
        .and_then(|id| library.topic(id))
        .map(|_| {
            library.breadcrumb(recording.topic_id.as_deref().unwrap_or_default())
        })
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| "Inbox".to_string())
}

pub fn recording_row(app: &Rc<App>, recording: &Recording) -> View {
    let th = theme();
    let library = app.library.get();
    let snapshot = app.playback_snapshot();
    let is_current = snapshot.recording_id.as_deref() == Some(recording.id.as_str());
    let playing = is_current
        && snapshot.state == crate::playback::PlayState::Playing;
    let topic = recording_topic_label(&library, recording);
    let subtitle = format!("{}  ·  {}", format_date(recording.created_at), topic);
    let missing = !app.store.borrow().recording_file_exists(recording);
    let id = recording.id.clone();
    let open = {
        let app = app.clone();
        move || app.open_player(&id)
    };

    Box(Modifier::new()
        .fill_max_width()
        .background(th.surface_container)
        .border(Dp(1.0), th.outline_variant.with_alpha_f32(0.4), Dp(16.0))
        .clip_rounded(Dp(16.0))
        .clickable()
        .on_click(open))
    .child(
        Row(Modifier::new()
            .fill_max_width()
            .padding(Dp(12.0))
            .align_items(AlignItems::CENTER)
            .gap(Dp(12.0)))
        .child(round_icon_button(
            if playing { Symbols::pause } else { Symbols::play_arrow },
            {
                let app = app.clone();
                let id = recording.id.clone();
                move || app.toggle_recording(&id)
            },
            if is_current { th.primary } else { th.surface_container_highest },
            if is_current { th.on_primary } else { th.on_surface },
            Dp(42.0),
        ))
        .child(
            Column(Modifier::new().flex_grow(1.0).gap(Dp(3.0))).child((
                Text(recording.title.clone())
                    .size(Sp(15.0))
                    .font_weight(FontWeight(500))
                    .color(if missing { th.error } else { th.on_surface })
                    .single_line(),
                Text(if missing {
                    format!("Missing file · {topic}")
                } else {
                    subtitle
                })
                .size(Sp(12.0))
                .color(if missing { th.error } else { th.on_surface_variant })
                .single_line(),
            )),
        )
        .child(
            Column(Modifier::new().align_items(AlignItems::END).gap(Dp(6.0))).child((
                Text(format_duration(recording.duration_ms))
                    .size(Sp(13.0))
                    .font_weight(FontWeight(500))
                    .color(th.on_surface)
                    .single_line(),
                if recording.marks.is_empty() {
                    Spacer()
                } else {
                    Row(Modifier::new().align_items(AlignItems::CENTER).gap(Dp(4.0))).child((
                        Icon(Symbols::bookmark_add)
                            .size(Sp(12.0))
                            .color(th.tertiary),
                        Text(recording.marks.len().to_string())
                            .size(Sp(11.0))
                            .color(th.on_surface_variant),
                    ))
                },
            )),
        ),
    )
}

pub fn topic_picker(app: &Rc<App>, selected: &Signal<Option<String>>) -> View {
    let library = app.library.get();
    let current = selected.get();
    let mut rows: Vec<View> = Vec::new();
    rows.push(topic_pick_row(
        "Inbox",
        0,
        current.is_none(),
        {
            let selected = selected.clone();
            move || selected.set(None)
        },
    ));
    append_topic_rows(&library, None, 0, &current, selected, &mut rows);
    ScrollArea(
        Modifier::new()
            .fill_max_width()
            .max_height(Dp(240.0))
            .border(Dp(1.0), theme().outline_variant, Dp(14.0))
            .clip_rounded(Dp(14.0)),
        remember_scroll_state("aareco:topic-picker"),
        Column(Modifier::new().fill_max_width().padding(Dp(6.0)).gap(Dp(2.0)))
            .child(rows),
    )
}

fn append_topic_rows(
    library: &Library,
    parent: Option<&str>,
    depth: usize,
    selected: &Option<String>,
    signal: &Signal<Option<String>>,
    rows: &mut Vec<View>,
) {
    for topic in library.children_of(parent) {
        let is_selected = selected.as_deref() == Some(topic.id.as_str());
        let id = topic.id.clone();
        rows.push(topic_pick_row(&topic.name, depth + 1, is_selected, {
            let signal = signal.clone();
            move || signal.set(Some(id.clone()))
        }));
        append_topic_rows(library, Some(&topic.id), depth + 1, selected, signal, rows);
    }
}

fn topic_pick_row(
    name: &str,
    depth: usize,
    selected: bool,
    on_click: impl Fn() + 'static,
) -> View {
    let th = theme();
    Box(Modifier::new()
        .fill_max_width()
        .background(if selected {
            th.primary_container.with_alpha_f32(0.55)
        } else {
            Color::TRANSPARENT
        })
        .clip_rounded(Dp(10.0))
        .clickable()
        .on_click(on_click)
        .padding_values(PaddingValues {
            left: Dp(10.0 + depth as f32 * 16.0),
            right: Dp(10.0),
            top: Dp(9.0),
            bottom: Dp(9.0),
        }))
    .child(
        Row(Modifier::new()
            .fill_max_width()
            .align_items(AlignItems::CENTER)
            .gap(Dp(8.0)))
        .child((
            Icon(if depth == 0 { Symbols::folder_special } else { Symbols::folder })
                .size(Sp(16.0))
                .color(if selected { th.primary } else { th.on_surface_variant }),
            Text(name.to_string())
                .size(Sp(13.0))
                .color(if selected { th.on_surface } else { th.on_surface_variant })
                .single_line(),
            Spacer(),
            if selected {
                Icon(Symbols::check).size(Sp(16.0)).color(th.primary)
            } else {
                Spacer()
            },
        )),
    )
}

pub fn slider_config(on_finish: Option<Rc<dyn Fn()>>) -> SliderConfig {
    let th = theme();
    SliderConfig {
        active_track_color: th.primary,
        inactive_track_color: th.outline.with_alpha_f32(0.35),
        thumb_color: th.primary,
        active_tick_color: th.primary,
        inactive_tick_color: th.outline_variant,
        track_height: Dp(4.0),
        thumb_width: Dp(16.0),
        thumb_height: Dp(16.0),
        on_value_change_finished: on_finish,
        ..Default::default()
    }
}
