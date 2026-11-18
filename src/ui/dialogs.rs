use std::rc::Rc;

use repose_core::prelude::*;
use repose_core::modifier::PaddingValues;
use repose_core::text::FontWeight;
use repose_material::Icon;
use repose_material::material3::dialog::{Dialog, DialogProperties};
use repose_material::material3::{TextField, TextFieldConfig};
use repose_ui::scroll::{ScrollArea, remember_scroll_state};
use repose_ui::*;

use crate::app::{App, DialogKind};
use crate::icons::Symbols;
use crate::ui::common::{primary_button, secondary_button, topic_picker};

pub fn dialog(app: &Rc<App>) -> View {
    let kind = app.dialog.get();
    if kind == DialogKind::None {
        return Box(Modifier::new());
    }
    let th = theme();
    let dismiss_app = app.clone();
    let properties = DialogProperties {
        on_dismiss_request: Some(Rc::new(move || dismiss_app.on_dialog_dismiss())),
        dismiss_on_click_outside: true,
        dismiss_on_back_press: true,
        ..Default::default()
    };

    let body = match &kind {
        DialogKind::None => Box(Modifier::new()),
        DialogKind::SaveRecording => save_body(app),
        DialogKind::DiscardRecording => discard_body(app),
        DialogKind::RenameRecording(_) => rename_body(app, "Rename recording"),
        DialogKind::RenameTopic(_) => rename_body(app, "Rename topic"),
        DialogKind::NewTopic { parent } => new_topic_body(app, parent.as_deref()),
        DialogKind::DeleteRecording(_) => delete_recording_body(app),
        DialogKind::DeleteTopic(_) => delete_topic_body(app),
        DialogKind::MoveRecording(_) => move_body(app),
        DialogKind::DefaultTopic => default_topic_body(app),
        DialogKind::Microphone => microphone_body(app),
        DialogKind::Error(message) => error_body(app, message),
    };

    let content = Box(Modifier::new()
        .min_width(Dp(300.0))
        .max_width(Dp(480.0))
        .background(th.surface_container_high)
        .border(Dp(1.0), th.outline_variant, Dp(28.0))
        .clip_rounded(Dp(28.0))
        .padding(Dp(22.0)))
    .child(body);

    Dialog(
        app.dialog_state.clone(),
        Modifier::new().fill_max_width().max_width(Dp(520.0)),
        properties,
        content,
    )
}

fn dialog_title(text: &str) -> View {
    let th = theme();
    Text(text.to_string())
        .size(Sp(20.0))
        .font_weight(FontWeight(600))
        .color(th.on_surface)
        .max_lines(2)
}

fn dialog_caption(text: &str) -> View {
    let th = theme();
    Text(text.to_string())
        .size(Sp(12.0))
        .color(th.on_surface_variant)
        .max_lines(4)
}

fn actions(buttons: Vec<View>) -> View {
    Row(Modifier::new()
        .fill_max_width()
        .align_items(AlignItems::CENTER)
        .justify_content(JustifyContent::END)
        .gap(Dp(8.0)))
    .child(buttons)
}

fn save_body(app: &Rc<App>) -> View {
    let th = theme();
    let title = app.dialog_title.get();
    let pending_error = app
        .pending
        .borrow()
        .as_ref()
        .and_then(|pending| pending.stop.error.clone());
    let error = match pending_error {
        Some(error) => Box(Modifier::new()
            .fill_max_width()
            .background(th.error_container.with_alpha_f32(0.4))
            .clip_rounded(Dp(10.0))
            .padding(Dp(10.0)))
        .child(
            Text(error)
                .size(Sp(11.0))
                .color(th.on_error_container)
                .max_lines(3),
        ),
        None => Spacer(),
    };

    Column(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        dialog_title("Save recording"),
        TextField(
            Modifier::new().fill_max_width(),
            title,
            {
                let app = app.clone();
                move |value| app.dialog_title.set(value)
            },
            TextFieldConfig {
                label: Some("Name".to_string()),
                single_line: true,
                ..Default::default()
            },
        ),
        Text("Topic".to_string())
            .size(Sp(12.0))
            .font_weight(FontWeight(500))
            .color(th.on_surface_variant),
        topic_picker(app, &app.dialog_topic),
        error,
        actions(vec![
            secondary_button("Discard", {
                let app = app.clone();
                move || app.discard_pending()
            }),
            primary_button("Save", {
                let app = app.clone();
                move || app.confirm_dialog()
            }),
        ]),
    ))
}

fn discard_body(app: &Rc<App>) -> View {
    Column(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        dialog_title("Discard this recording?"),
        dialog_caption("The audio captured so far will be deleted. This cannot be undone."),
        actions(vec![
            secondary_button("Keep recording", {
                let app = app.clone();
                move || app.close_dialog()
            }),
            danger_button("Discard", {
                let app = app.clone();
                move || app.confirm_dialog()
            }),
        ]),
    ))
}

fn default_topic_body(app: &Rc<App>) -> View {
    Column(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        dialog_title("Where should new recordings go?"),
        dialog_caption(
            "The save dialog still lets you change the topic per recording.",
        ),
        topic_picker(app, &app.dialog_topic),
        actions(vec![
            secondary_button("Cancel", {
                let app = app.clone();
                move || app.close_dialog()
            }),
            primary_button("Save", {
                let app = app.clone();
                move || app.confirm_dialog()
            }),
        ]),
    ))
}

fn microphone_body(app: &Rc<App>) -> View {
    let th = theme();
    let settings = app.settings.get();
    let devices = app.devices.get();
    let mut rows: Vec<View> = Vec::new();

    for (value, name, detail) in std::iter::once((None, "System default".to_string(), String::new()))
        .chain(devices.iter().map(|device| {
            (
                Some(device.name.clone()),
                device.name.clone(),
                format!(
                    "{} Hz · {} channel(s){}",
                    device.sample_rate,
                    device.channels,
                    if device.is_default { " · default" } else { "" }
                ),
            )
        }))
    {
        let selected = settings.device == value;
        rows.push(
            Box(Modifier::new()
                .fill_max_width()
                .background(if selected {
                    th.primary_container.with_alpha_f32(0.45)
                } else {
                    Color::TRANSPARENT
                })
                .clip_rounded(Dp(12.0))
                .clickable()
                .on_click({
                    let app = app.clone();
                    let value = value.clone();
                    move || {
                        app.update_settings(|settings| settings.device = value.clone());
                        app.close_dialog();
                    }
                })
                .padding(Dp(12.0)))
            .child(
                Row(Modifier::new()
                    .fill_max_width()
                    .align_items(AlignItems::CENTER)
                    .gap(Dp(10.0)))
                .child((
                    Icon(if selected { Symbols::mic } else { Symbols::headphones })
                        .size(Sp(18.0))
                        .color(if selected { th.primary } else { th.on_surface_variant }),
                    Column(Modifier::new().flex_grow(1.0).gap(Dp(2.0))).child((
                        Text(name)
                            .size(Sp(13.0))
                            .font_weight(FontWeight(500))
                            .color(th.on_surface)
                            .single_line()
                            .overflow_ellipsize(),
                        Text(if detail.is_empty() {
                            "The microphone chosen by the system".to_string()
                        } else {
                            detail
                        })
                        .size(Sp(11.0))
                        .color(th.on_surface_variant)
                        .single_line(),
                    )),
                    if selected {
                        Icon(Symbols::check).size(Sp(18.0)).color(th.primary)
                    } else {
                        Spacer()
                    },
                )),
            ),
        );
    }

    Column(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        dialog_title("Microphone"),
        dialog_caption("Devices detected on this system. The system default follows your OS settings."),
        ScrollArea(
            Modifier::new().fill_max_width().max_height(Dp(320.0)),
            remember_scroll_state("aareco:dialog:microphones"),
            Column(Modifier::new().fill_max_width().gap(Dp(2.0))).child(rows),
        ),
        actions(vec![
            secondary_button("Refresh", {
                let app = app.clone();
                move || app.refresh_devices()
            }),
            primary_button("Done", {
                let app = app.clone();
                move || app.close_dialog()
            }),
        ]),
    ))
}

fn rename_body(app: &Rc<App>, heading: &str) -> View {
    let title = app.dialog_title.get();
    Column(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        dialog_title(heading),
        TextField(
            Modifier::new().fill_max_width(),
            title,
            {
                let app = app.clone();
                move |value| app.dialog_title.set(value)
            },
            TextFieldConfig {
                label: Some("Name".to_string()),
                single_line: true,
                on_submit: Some(Rc::new({
                    let app = app.clone();
                    move |_| app.confirm_dialog()
                })),
                ..Default::default()
            },
        ),
        actions(vec![
            secondary_button("Cancel", {
                let app = app.clone();
                move || app.close_dialog()
            }),
            primary_button("Save", {
                let app = app.clone();
                move || app.confirm_dialog()
            }),
        ]),
    ))
}

fn new_topic_body(app: &Rc<App>, parent: Option<&str>) -> View {
    let th = theme();
    let library = app.library.get();
    let parent_label = parent
        .map(|id| library.breadcrumb(id))
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| "Inbox".to_string());
    let title = app.dialog_title.get();
    Column(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        dialog_title("New topic"),
        dialog_caption(&format!("It will live inside {parent_label}.")),
        TextField(
            Modifier::new().fill_max_width(),
            title,
            {
                let app = app.clone();
                move |value| app.dialog_title.set(value)
            },
            TextFieldConfig {
                label: Some("Topic name".to_string()),
                placeholder: Some("Work, Ideas, Interviews…".to_string()),
                single_line: true,
                on_submit: Some(Rc::new({
                    let app = app.clone();
                    move |_| app.confirm_dialog()
                })),
                ..Default::default()
            },
        ),
        Row(Modifier::new().align_items(AlignItems::CENTER).gap(Dp(6.0))).child((
            Icon(Symbols::folder).size(Sp(14.0)).color(th.tertiary),
            Text(parent_label)
                .size(Sp(11.0))
                .color(th.on_surface_variant)
                .single_line(),
        )),
        actions(vec![
            secondary_button("Cancel", {
                let app = app.clone();
                move || app.close_dialog()
            }),
            primary_button("Create", {
                let app = app.clone();
                move || app.confirm_dialog()
            }),
        ]),
    ))
}

fn delete_recording_body(app: &Rc<App>) -> View {
    Column(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        dialog_title("Delete recording?"),
        dialog_caption(
            "The WAV file, its marks, and its place in the topic tree will be removed.",
        ),
        actions(vec![
            secondary_button("Cancel", {
                let app = app.clone();
                move || app.close_dialog()
            }),
            danger_button("Delete", {
                let app = app.clone();
                move || app.confirm_dialog()
            }),
        ]),
    ))
}

fn delete_topic_body(app: &Rc<App>) -> View {
    Column(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        dialog_title("Delete this topic?"),
        dialog_caption(
            "Nested topics will be deleted too. Recordings inside move to the parent topic, so no audio is lost.",
        ),
        actions(vec![
            secondary_button("Cancel", {
                let app = app.clone();
                move || app.close_dialog()
            }),
            danger_button("Delete topic", {
                let app = app.clone();
                move || app.confirm_dialog()
            }),
        ]),
    ))
}

fn move_body(app: &Rc<App>) -> View {
    let th = theme();
    Column(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        dialog_title("Move to topic"),
        dialog_caption("Pick a folder, or leave it in the Inbox."),
        topic_picker(app, &app.dialog_topic),
        Row(Modifier::new().align_items(AlignItems::CENTER).gap(Dp(6.0))).child((
            Icon(Symbols::info).size(Sp(13.0)).color(th.on_surface_variant),
            Text("The audio file stays where it is".to_string())
                .size(Sp(11.0))
                .color(th.on_surface_variant),
        )),
        actions(vec![
            secondary_button("Cancel", {
                let app = app.clone();
                move || app.close_dialog()
            }),
            primary_button("Move", {
                let app = app.clone();
                move || app.confirm_dialog()
            }),
        ]),
    ))
}

fn error_body(app: &Rc<App>, message: &str) -> View {
    Column(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        dialog_title("Something went wrong"),
        dialog_caption(message),
        actions(vec![primary_button("OK", {
            let app = app.clone();
            move || app.confirm_dialog()
        })]),
    ))
}

fn danger_button(label: &str, on_click: impl Fn() + 'static) -> View {
    let th = theme();
    Box(Modifier::new()
        .background(th.error_container)
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
        Text(label.to_string())
            .size(Sp(14.0))
            .font_weight(FontWeight(600))
            .color(th.on_error_container)
            .single_line(),
    )
}
