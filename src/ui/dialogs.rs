use std::rc::Rc;

use repose_core::modifier::PaddingValues;
use repose_core::prelude::*;
use repose_core::text::FontWeight;
use repose_material::Icon;
use repose_material::material3::dialog::{Dialog, DialogProperties};
use repose_material::material3::{Slider, TextField, TextFieldConfig};
use repose_ui::BoxWithConstraints;
use repose_ui::scroll::{ScrollArea, remember_scroll_state};
use repose_ui::*;

use crate::app::{App, DialogKind, TemplateChoice};
use crate::icons::Symbols;
use crate::ui::common::{chip, primary_button, secondary_button, slider_config, topic_picker};

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
        DialogKind::NewTemplate => rename_body(app, "New template"),
        DialogKind::RenameTemplate(_) => rename_body(app, "Rename template"),
        DialogKind::SaveAsTemplate(_) => rename_body(app, "Save as template"),
        DialogKind::SelectTemplate(id) => select_template_body(app, id),
        DialogKind::AddEffect => add_effect_body(app),
        DialogKind::EffectParams(index) => effect_params_body(app, *index),
        DialogKind::Error(message) => error_body(app, message),
    };

    let content = Box(Modifier::new().fill_max_width().padding(Dp(22.0))).child(body);

    Dialog(
        app.dialog_state.clone(),
        Modifier::new()
            .fill_max_width()
            .max_width(Dp(520.0))
            .border(Dp(1.0), th.outline_variant, Dp(28.0)),
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
        Text("Effects".to_string())
            .size(Sp(12.0))
            .font_weight(FontWeight(500))
            .color(th.on_surface_variant),
        Row(Modifier::new()
            .fill_max_width()
            .flex_wrap(FlexWrap::Wrap)
            .gap(Dp(6.0)))
        .child(save_template_chips(app)),
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

fn save_template_chips(app: &Rc<App>) -> Vec<View> {
    let choice = app.dialog_template.get();
    let default_name = app
        .settings
        .get()
        .default_template
        .and_then(|id| {
            app.library
                .get()
                .templates
                .into_iter()
                .find(|template| template.id == id)
                .map(|template| template.name)
        })
        .unwrap_or_else(|| "None".to_string());
    let mut chips = vec![
        chip(
            &format!("Default ({default_name})"),
            choice == TemplateChoice::Default,
            {
                let app = app.clone();
                move || app.choose_template(TemplateChoice::Default)
            },
        ),
        chip("None", choice == TemplateChoice::None, {
            let app = app.clone();
            move || app.choose_template(TemplateChoice::None)
        }),
    ];
    for template in app.library.get().templates {
        let id = template.id.clone();
        chips.push(chip(
            &template.name,
            choice == TemplateChoice::Template(id.clone()),
            {
                let app = app.clone();
                move || app.choose_template(TemplateChoice::Template(id.clone()))
            },
        ));
    }
    chips
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
        dialog_caption("The save dialog still lets you change the topic per recording."),
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

    for (value, name, detail) in
        std::iter::once((None, "System default".to_string(), String::new())).chain(
            devices.iter().map(|device| {
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
            }),
        )
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
                    Icon(if selected {
                        Symbols::mic
                    } else {
                        Symbols::headphones
                    })
                    .size(Sp(18.0))
                    .color(if selected {
                        th.primary
                    } else {
                        th.on_surface_variant
                    }),
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

    let list = Column(Modifier::new().fill_max_width().gap(Dp(2.0))).child(rows);
    let action_row = actions(vec![
        secondary_button("Refresh", {
            let app = app.clone();
            move || app.refresh_devices()
        }),
        primary_button("Done", {
            let app = app.clone();
            move || app.close_dialog()
        }),
    ]);
    BoxWithConstraints(Modifier::new().fill_max_width(), move |scope| {
        Column(
            Modifier::new()
                .fill_max_width()
                .height(scope.max_height)
                .gap(Dp(12.0)),
        )
        .child((
            dialog_title("Microphone"),
            dialog_caption(
                "Devices detected on this system. The system default follows your OS settings.",
            ),
            ScrollArea(
                Modifier::new()
                    .fill_max_width()
                    .flex_grow(1.0)
                    .min_height(Dp(0.0)),
                remember_scroll_state("aareco:dialog:microphones"),
                list.clone(),
            ),
            action_row.clone(),
        ))
    })
}

fn add_effect_body(app: &Rc<App>) -> View {
    let th = theme();
    let scan = app.fx_scan.borrow();
    let status = scan.status.clone();
    let effects = scan.effects.clone();
    drop(scan);

    let mut top: Vec<View> = vec![
        dialog_title("Add effect"),
        dialog_caption("Equalizers, compressors and other plugins, applied to this recording."),
    ];

    let mut list: Option<View> = None;
    match status {
        crate::app::FxScanStatus::Scanning | crate::app::FxScanStatus::Idle => {
            top.push(
                Box(Modifier::new()
                    .fill_max_width()
                    .padding(Dp(18.0))
                    .align_items(AlignItems::CENTER))
                .child(
                    Text("Looking for installed effects…".to_string())
                        .size(Sp(12.0))
                        .color(th.on_surface_variant),
                ),
            );
        }
        crate::app::FxScanStatus::Failed(err) => {
            top.push(dialog_caption(&format!("Scan failed: {err}")));
        }
        crate::app::FxScanStatus::Ready => {
            let search = app.fx_search.get().to_lowercase();
            let rows: Vec<View> = effects
                .iter()
                .filter(|effect| search.is_empty() || effect.name.to_lowercase().contains(&search))
                .map(|effect| {
                    let app = app.clone();
                    let effect_for_click = effect.clone();
                    let name = effect.name.clone();
                    let backend = effect.backend;
                    Box(Modifier::new()
                        .fill_max_width()
                        .clip_rounded(Dp(10.0))
                        .clickable()
                        .on_click(move || app.add_effect(&effect_for_click))
                        .padding(Dp(10.0)))
                    .child(
                        Row(Modifier::new()
                            .fill_max_width()
                            .align_items(AlignItems::CENTER)
                            .gap(Dp(10.0)))
                        .child((
                            Icon(Symbols::equalizer)
                                .size(Sp(16.0))
                                .color(th.on_surface_variant),
                            Column(Modifier::new().flex_grow(1.0).gap(Dp(2.0))).child((
                                Text(name)
                                    .size(Sp(13.0))
                                    .color(th.on_surface)
                                    .single_line()
                                    .overflow_ellipsize(),
                                Text(match backend {
                                    aareco_store::FxBackend::Builtin => "Built-in",
                                    aareco_store::FxBackend::Vst3 => "VST3",
                                    aareco_store::FxBackend::Lv2 => "LV2",
                                    aareco_store::FxBackend::Clap => "CLAP",
                                })
                                .size(Sp(10.0))
                                .color(th.on_surface_variant),
                            )),
                        )),
                    )
                })
                .collect();
            if effects.is_empty() {
                top.push(dialog_caption("No effect plugins were found."));
            } else {
                top.push(TextField(
                    Modifier::new().fill_max_width(),
                    app.fx_search.get(),
                    {
                        let app = app.clone();
                        move |value| app.fx_search.set(value)
                    },
                    TextFieldConfig {
                        label: Some("Search".to_string()),
                        single_line: true,
                        ..Default::default()
                    },
                ));
                list = Some(Column(Modifier::new().fill_max_width().gap(Dp(2.0))).child(rows));
            }
        }
    }

    let mut buttons: Vec<View> = Vec::new();
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    {
        buttons.push(secondary_button("Import", {
            let app = app.clone();
            move || app.import_plugin_bundle()
        }));
        buttons.push(Spacer());
    }
    buttons.push(secondary_button("Rescan", {
        let app = app.clone();
        move || app.start_fx_scan()
    }));
    buttons.push(secondary_button("Close", {
        let app = app.clone();
        move || app.close_dialog()
    }));
    let actions_view = actions(buttons);

    BoxWithConstraints(Modifier::new().fill_max_width(), move |scope| {
        let mut children = top.clone();
        if let Some(list) = &list {
            // Reserve room for the title, caption, search field and the action
            // row, then give the remainder to the scrollable list so it has a
            // definite height and the buttons stay pinned.
            children.push(ScrollArea(
                Modifier::new()
                    .fill_max_width()
                    .flex_grow(1.0)
                    .min_height(Dp(0.0)),
                remember_scroll_state("aareco:dialog:effects"),
                list.clone(),
            ));
        }
        children.push(actions_view.clone());
        Column(
            Modifier::new()
                .fill_max_width()
                .height(scope.max_height)
                .gap(Dp(12.0)),
        )
        .child(children)
    })
}

fn effect_params_body(app: &Rc<App>, _index: usize) -> View {
    let th = theme();
    let name = app.dialog_title.get();
    let params = app.fx_params.borrow();
    let values = app.fx_values.borrow();
    let mut rows: Vec<View> = Vec::new();

    for param in params.iter() {
        let key = param.key.clone();
        let Some(value) = values
            .iter()
            .find(|value| value.key == key)
            .map(|value| value.value)
        else {
            continue;
        };
        let range = (param.max - param.min).abs().max(1e-6);
        let plain = param.min + value * range;
        let shown = match param.kind {
            aareco_fx::FxParamKind::Bool => {
                if plain >= 0.5 {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            }
            aareco_fx::FxParamKind::Int | aareco_fx::FxParamKind::Enum => {
                format!("{plain:.0}")
            }
            aareco_fx::FxParamKind::Float => format!("{plain:.2}"),
        };
        let shown = if matches!(param.kind, aareco_fx::FxParamKind::Float)
            && let Some(unit) = &param.unit
            && !unit.is_empty()
        {
            format!("{shown} {unit}")
        } else {
            shown
        };
        rows.push(
            Column(Modifier::new().fill_max_width().gap(Dp(2.0))).child((
                Row(Modifier::new()
                    .fill_max_width()
                    .align_items(AlignItems::CENTER)
                    .gap(Dp(8.0)))
                .child((
                    Text(param.name.clone())
                        .size(Sp(12.0))
                        .color(th.on_surface)
                        .single_line()
                        .overflow_ellipsize()
                        .modifier(Modifier::new().flex_grow(1.0)),
                    Text(shown.clone())
                        .size(Sp(11.0))
                        .color(th.on_surface_variant)
                        .single_line(),
                )),
                Slider(
                    value,
                    (0.0, 1.0),
                    None,
                    {
                        let app = app.clone();
                        move |value| app.set_effect_param(&key, value)
                    },
                    slider_config(None),
                ),
            )),
        );
    }
    drop(values);
    drop(params);

    let body: View = if rows.is_empty() {
        Box(Modifier::new()
            .fill_max_width()
            .padding(Dp(16.0))
            .align_items(AlignItems::CENTER))
        .child(
            Text("This effect exposes no editable parameters.".to_string())
                .size(Sp(12.0))
                .color(th.on_surface_variant),
        )
    } else {
        Column(Modifier::new().fill_max_width().gap(Dp(10.0))).child(rows)
    };

    let action_row = actions(vec![
        secondary_button("Cancel", {
            let app = app.clone();
            move || app.close_dialog()
        }),
        primary_button("Save", {
            let app = app.clone();
            move || app.confirm_dialog()
        }),
    ]);
    let index = _index;
    BoxWithConstraints(Modifier::new().fill_max_width(), move |scope| {
        Column(
            Modifier::new()
                .fill_max_width()
                .height(scope.max_height)
                .gap(Dp(12.0)),
        )
        .child((
            dialog_title(&name),
            ScrollArea(
                Modifier::new()
                    .fill_max_width()
                    .flex_grow(1.0)
                    .min_height(Dp(0.0)),
                remember_scroll_state(format!("aareco:dialog:fx-params:{index}")),
                body.clone(),
            ),
            action_row.clone(),
        ))
    })
}

fn select_template_body(app: &Rc<App>, recording_id: &str) -> View {
    let library = app.library.get();
    let mut rows: Vec<View> = vec![
        dialog_title("Apply a template"),
        dialog_caption("Replaces the effect chain on this recording."),
    ];

    let app_clear = app.clone();
    let id = recording_id.to_string();
    rows.push(row_button("No effects", Symbols::remove, move || {
        app_clear.clear_effects(&id)
    }));
    for template in &library.templates {
        let app = app.clone();
        let id = recording_id.to_string();
        let template_id = template.id.clone();
        let name = template.name.clone();
        let count = template.fx.len();
        rows.push(row_button(
            &format!("{name} · {count} effect(s)"),
            Symbols::tune,
            move || app.apply_template(&id, &template_id),
        ));
    }

    if library.templates.is_empty() {
        rows.push(dialog_caption("Create a template in Settings first."));
    }
    rows.push(actions(vec![secondary_button("Close", {
        let app = app.clone();
        move || app.close_dialog()
    })]));

    Column(Modifier::new().fill_max_width().gap(Dp(10.0))).child(rows)
}

fn row_button(label: &str, symbol: repose_material::Symbol, on_click: impl Fn() + 'static) -> View {
    let th = theme();
    Box(Modifier::new()
        .fill_max_width()
        .background(th.surface_container_high)
        .clip_rounded(Dp(10.0))
        .clickable()
        .on_click(on_click)
        .padding(Dp(10.0)))
    .child(
        Row(Modifier::new().fill_max_width().gap(Dp(10.0))).child((
            Icon(symbol).size(Sp(16.0)).color(th.on_surface_variant),
            Text(label.to_string())
                .size(Sp(13.0))
                .color(th.on_surface)
                .single_line()
                .overflow_ellipsize(),
        )),
    )
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
        dialog_caption("The WAV file, its marks, and its place in the topic tree will be removed."),
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
            Icon(Symbols::info)
                .size(Sp(13.0))
                .color(th.on_surface_variant),
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
