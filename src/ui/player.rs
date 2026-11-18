use std::rc::Rc;

use repose_core::modifier::PaddingValues;
use repose_core::prelude::*;
use repose_core::text::FontWeight;
use repose_material::Icon;
use repose_material::material3::{Slider, Switch, SwitchConfig};
use repose_ui::*;

use crate::app::App;
use crate::icons::Symbols;
use crate::playback::PlayState;
use crate::ui::common::{
    card, chip, empty_state, format_bytes, format_date, format_duration, icon_button,
    primary_button, recording_topic_label, scroll_page, secondary_button, section_title,
    slider_config, top_bar,
};
use crate::ui::waveform::waveform;

pub fn screen(app: &Rc<App>, id: &str) -> View {
    let th = theme();
    let library = app.library.get();
    let Some(recording) = library.recording(id).cloned() else {
        return empty_state(
            Symbols::music_note,
            "Recording not found",
            "It may have been deleted.",
        );
    };
    let snapshot = app.playback_snapshot();
    let active = snapshot.recording_id.as_deref() == Some(id);
    let position = if active { snapshot.position_ms } else { 0 };
    let duration = recording.duration_ms.max(1);
    let progress = (position as f32 / duration as f32).clamp(0.0, 1.0);
    let speed = if active {
        snapshot.speed
    } else {
        app.settings.get().playback_speed
    };
    let mark_nudge = app.settings.get().mark_nudge_secs;
    let volume = app.settings.get().volume;
    let topic = recording_topic_label(&library, &recording);
    let error = if active { snapshot.error.clone() } else { None };
    let playing = active && snapshot.state == PlayState::Playing;
    let missing = !app.store.borrow().recording_file_exists(&recording);

    let back = {
        let app = app.clone();
        move || {
            app.nav.pop();
        }
    };
    let actions = vec![
        icon_button(
            Symbols::edit,
            {
                let app = app.clone();
                let id = id.to_string();
                move || app.open_rename_recording(&id)
            },
            None,
        ),
        icon_button(
            Symbols::folder_special,
            {
                let app = app.clone();
                let id = id.to_string();
                move || app.open_move_recording(&id)
            },
            None,
        ),
        icon_button(
            Symbols::delete,
            {
                let app = app.clone();
                let id = id.to_string();
                move || app.open_delete_recording(&id)
            },
            Some(th.error),
        ),
    ];

    let peaks: Vec<f32> = recording
        .waveform
        .iter()
        .map(|peak| *peak as f32 / 255.0)
        .collect();
    let mark_ratios: Vec<f32> = recording
        .marks
        .iter()
        .map(|mark| (mark.offset_ms as f32 / duration as f32).clamp(0.0, 1.0))
        .collect();
    let seek = {
        let app = app.clone();
        Rc::new(move |ratio: f32| {
            app.player_seek((ratio.clamp(0.0, 1.0) as f64 * duration as f64) as u64);
        })
    };
    let scrub_seek = seek.clone();
    let scrub_back = app.settings.get().scrub_back_secs;
    let scrub_forward = app.settings.get().scrub_forward_secs;

    let waveform_card = Box(Modifier::new()
        .fill_max_width()
        .background(th.surface_container)
        .border(Dp(1.0), th.outline_variant.with_alpha_f32(0.5), Dp(22.0))
        .clip_rounded(Dp(22.0)))
    .child(
        Column(
            Modifier::new()
                .fill_max_width()
                .padding(Dp(18.0))
                .gap(Dp(10.0)),
        )
        .child((
            waveform(
                peaks,
                progress,
                mark_ratios,
                Dp(160.0),
                Some(seek),
                app.settings.get().waveform_style,
            ),
            Slider(
                progress,
                (0.0, 1.0),
                None,
                move |value| scrub_seek(value),
                slider_config(None),
            ),
            Row(Modifier::new()
                .fill_max_width()
                .align_items(AlignItems::CENTER)
                .gap(Dp(8.0)))
            .child((
                Text(format_duration(position))
                    .size(Sp(12.0))
                    .color(th.primary)
                    .single_line(),
                Spacer(),
                Text(format_duration(recording.duration_ms))
                    .size(Sp(12.0))
                    .color(th.on_surface_variant)
                    .single_line(),
            )),
        )),
    );

    let transport = Row(Modifier::new()
        .fill_max_width()
        .align_items(AlignItems::CENTER)
        .justify_content(JustifyContent::CENTER)
        .gap(Dp(22.0)))
    .child((
        transport_button(Symbols::fast_rewind, -(scrub_back as i64) * 1000, app),
        Box(Modifier::new()
            .size(Dp(84.0), Dp(84.0))
            .background(if playing {
                th.tertiary_container
            } else {
                th.primary
            })
            .clip_rounded(Dp(999.0))
            .clickable()
            .on_click({
                let app = app.clone();
                move || app.player_toggle()
            }))
        .child(
            Box(Modifier::new()
                .fill_max_size()
                .align_items(AlignItems::CENTER)
                .justify_content(JustifyContent::CENTER))
            .child(
                Icon(if playing {
                    Symbols::pause
                } else {
                    Symbols::play_arrow
                })
                .size(Sp(38.0))
                .color(if playing {
                    th.on_tertiary_container
                } else {
                    th.on_primary
                }),
            ),
        ),
        transport_button(Symbols::fast_forward, scrub_forward as i64 * 1000, app),
    ));

    let speeds = [0.75f32, 1.0, 1.25, 1.5, 2.0];
    let speed_chips: Vec<View> = speeds
        .iter()
        .map(|candidate| {
            let app = app.clone();
            let value = *candidate;
            chip(
                &format_speed(value),
                (speed - value).abs() < 0.01,
                move || app.player_set_speed(value),
            )
        })
        .collect();

    let mut mark_views: Vec<View> = Vec::new();
    mark_views.push(
        Row(Modifier::new()
            .fill_max_width()
            .align_items(AlignItems::CENTER)
            .gap(Dp(4.0)))
        .child((
            Text("Marks".to_string())
                .size(Sp(13.0))
                .font_weight(FontWeight(600))
                .color(th.on_surface_variant),
            Spacer(),
            icon_button(
                Symbols::skip_previous,
                {
                    let app = app.clone();
                    let id = id.to_string();
                    move || app.jump_to_mark(&id, false)
                },
                None,
            ),
            icon_button(
                Symbols::skip_next,
                {
                    let app = app.clone();
                    let id = id.to_string();
                    move || app.jump_to_mark(&id, true)
                },
                None,
            ),
            crate::ui::common::secondary_button("Add mark here", {
                let app = app.clone();
                let id = id.to_string();
                let position = position;
                move || app.add_mark_to_recording(&id, position)
            }),
        )),
    );
    if recording.marks.is_empty() {
        mark_views.push(
            Text("Tap the waveform to seek. Marks let you jump back to a moment.".to_string())
                .size(Sp(12.0))
                .color(th.on_surface_variant)
                .max_lines(2),
        );
    } else {
        for mark in &recording.marks {
            let offset = mark.offset_ms;
            let mark_id = mark.id.clone();
            mark_views.push(
                Box(Modifier::new()
                    .fill_max_width()
                    .background(th.surface_container_low)
                    .clip_rounded(Dp(12.0))
                    .padding_values(PaddingValues {
                        left: Dp(12.0),
                        right: Dp(6.0),
                        top: Dp(8.0),
                        bottom: Dp(8.0),
                    })
                    .clickable()
                    .on_click({
                        let app = app.clone();
                        move || app.player_seek(offset)
                    }))
                .child(
                    Row(Modifier::new()
                        .fill_max_width()
                        .align_items(AlignItems::CENTER)
                        .gap(Dp(8.0)))
                    .child((
                        Icon(Symbols::bookmark_add)
                            .size(Sp(14.0))
                            .color(th.tertiary),
                        Text(format_duration(offset))
                            .size(Sp(12.0))
                            .font_weight(FontWeight(500))
                            .color(th.on_surface),
                        Text(mark.label.clone().unwrap_or_else(|| "Mark".to_string()))
                            .size(Sp(12.0))
                            .color(th.on_surface_variant)
                            .single_line(),
                        Spacer(),
                        icon_button(
                            Symbols::chevron_left,
                            {
                                let app = app.clone();
                                let id = id.to_string();
                                let mark_id = mark_id.clone();
                                move || app.nudge_mark(&id, &mark_id, -(mark_nudge as i32))
                            },
                            Some(th.on_surface_variant),
                        ),
                        icon_button(
                            Symbols::chevron_right,
                            {
                                let app = app.clone();
                                let id = id.to_string();
                                let mark_id = mark_id.clone();
                                move || app.nudge_mark(&id, &mark_id, mark_nudge as i32)
                            },
                            Some(th.on_surface_variant),
                        ),
                        icon_button(
                            Symbols::close,
                            {
                                let app = app.clone();
                                let id = id.to_string();
                                move || app.delete_mark(&id, &mark_id)
                            },
                            None,
                        ),
                    )),
                ),
            );
        }
    }

    let mut content: Vec<View> = vec![
        top_bar(
            recording.title.clone(),
            Some(format!("{} · {}", format_date(recording.created_at), topic)),
            Some(Rc::new(back)),
            actions,
        ),
        waveform_card,
        transport,
        Row(Modifier::new()
            .fill_max_width()
            .flex_wrap(FlexWrap::Wrap)
            .gap(Dp(6.0)))
        .child(speed_chips),
        card(
            Row(Modifier::new()
                .fill_max_width()
                .padding(Dp(14.0))
                .align_items(AlignItems::CENTER)
                .gap(Dp(12.0)))
            .child((
                Icon(if volume == 0.0 {
                    Symbols::volume_down
                } else {
                    Symbols::volume_up
                })
                .size(Sp(18.0))
                .color(th.on_surface_variant),
                Box(Modifier::new().flex_grow(1.0)).child(Slider(
                    volume,
                    (0.0, 1.0),
                    None,
                    {
                        let app = app.clone();
                        move |value| app.player_set_volume(value)
                    },
                    slider_config(Some(Rc::new({
                        let app = app.clone();
                        move || app.persist_settings()
                    }))),
                )),
            )),
        ),
        effects_card(app, &recording),
        card(
            Column(
                Modifier::new()
                    .fill_max_width()
                    .padding(Dp(14.0))
                    .gap(Dp(8.0)),
            )
            .child(mark_views),
        ),
    ];

    content.push(section_title("Details", None));
    content.push(card(
        Column(
            Modifier::new()
                .fill_max_width()
                .padding(Dp(14.0))
                .gap(Dp(8.0)),
        )
        .child((
            detail_row("Recorded", format_date(recording.created_at)),
            detail_row("Topic", topic.clone()),
            detail_row(
                "Format",
                format!("WAV · {} Hz · mono", recording.sample_rate),
            ),
            detail_row("Size", format_bytes(recording.size_bytes)),
            detail_row("File", recording.file_name.clone()),
        )),
    ));

    if missing {
        content.push(
            Box(Modifier::new()
                .fill_max_width()
                .background(th.error_container.with_alpha_f32(0.45))
                .border(Dp(1.0), th.error, Dp(14.0))
                .clip_rounded(Dp(14.0))
                .padding(Dp(12.0)))
            .child(
                Text("The audio file for this recording is missing from disk.".to_string())
                    .size(Sp(12.0))
                    .color(th.on_error_container),
            ),
        );
    }
    if let Some(error) = error {
        content.push(
            Box(Modifier::new()
                .fill_max_width()
                .background(th.error_container.with_alpha_f32(0.45))
                .border(Dp(1.0), th.error, Dp(14.0))
                .clip_rounded(Dp(14.0))
                .padding(Dp(12.0)))
            .child(Text(error).size(Sp(12.0)).color(th.on_error_container)),
        );
    }

    scroll_page(format!("aareco:player:{id}"), Dp(16.0), content)
}

fn transport_button(symbol: repose_material::Symbol, delta: i64, app: &Rc<App>) -> View {
    let th = theme();
    Box(Modifier::new()
        .size(Dp(48.0), Dp(48.0))
        .background(th.surface_container_high)
        .clip_rounded(Dp(999.0))
        .clickable()
        .on_click({
            let app = app.clone();
            move || app.player_seek_relative(delta)
        }))
    .child(
        Box(Modifier::new()
            .fill_max_size()
            .align_items(AlignItems::CENTER)
            .justify_content(JustifyContent::CENTER))
        .child(Icon(symbol).size(Sp(22.0)).color(th.on_surface)),
    )
}

fn format_speed(speed: f32) -> String {
    if (speed - 1.0).abs() < 0.01 {
        "1×".to_string()
    } else if (speed.fract() - 0.0).abs() < 0.01 {
        format!("{speed:.0}×")
    } else {
        format!("{speed:.2}×")
    }
}

fn detail_row(label: &str, value: String) -> View {
    let th = theme();
    Row(Modifier::new().fill_max_width().gap(Dp(12.0))).child((
        Box(Modifier::new().width(Dp(78.0))).child(
            Text(label.to_string())
                .size(Sp(12.0))
                .color(th.on_surface_variant),
        ),
        Text(value)
            .size(Sp(12.0))
            .color(th.on_surface)
            .single_line()
            .overflow_ellipsize(),
    ))
}

fn effects_card(app: &Rc<App>, recording: &aareco_store::Recording) -> View {
    let th = theme();
    let id = recording.id.clone();
    let rendering = app
        .fx_render
        .borrow()
        .as_ref()
        .is_some_and(|job| job.recording_id == id);
    let mut rows: Vec<View> = vec![
        Row(Modifier::new()
            .fill_max_width()
            .align_items(AlignItems::CENTER)
            .gap(Dp(8.0)))
        .child((
            Icon(Symbols::equalizer).size(Sp(17.0)).color(th.secondary),
            Text("Effects".to_string())
                .size(Sp(13.0))
                .font_weight(FontWeight(600))
                .color(th.on_surface_variant),
            Spacer(),
            if recording.source_file.is_some() && !rendering {
                secondary_button("Revert", {
                    let app = app.clone();
                    let id = id.clone();
                    move || app.revert_effects(&id)
                })
            } else {
                Spacer()
            },
        )),
    ];

    if recording.fx.is_empty() {
        rows.push(
            Text("Add an equalizer or compressor to smooth the recording.".to_string())
                .size(Sp(12.0))
                .color(th.on_surface_variant)
                .max_lines(2),
        );
    }

    for (index, slot) in recording.fx.iter().enumerate() {
        let backend = match slot.backend {
            aareco_store::FxBackend::Vst3 => "VST3",
            aareco_store::FxBackend::Lv2 => "LV2",
            aareco_store::FxBackend::Clap => "CLAP",
        };
        rows.push(divider());
        rows.push(
            Row(Modifier::new()
                .fill_max_width()
                .align_items(AlignItems::CENTER)
                .gap(Dp(8.0)))
            .child((
                Column(Modifier::new().flex_grow(1.0).gap(Dp(1.0))).child((
                    Text(slot.name.clone())
                        .size(Sp(13.0))
                        .font_weight(FontWeight(500))
                        .color(th.on_surface)
                        .single_line()
                        .overflow_ellipsize(),
                    Text(format!(
                        "{backend} · {}",
                        if slot.bypass { "bypassed" } else { "active" }
                    ))
                    .size(Sp(10.0))
                    .color(th.on_surface_variant),
                )),
                Switch(
                    !slot.bypass,
                    {
                        let app = app.clone();
                        let id = id.clone();
                        move |active| app.set_effect_bypass(&id, index, !active)
                    },
                    SwitchConfig {
                        modifier: Modifier::new().flex_shrink(0.0),
                        ..Default::default()
                    },
                ),
            )),
        );
        rows.push(
            Row(Modifier::new()
                .fill_max_width()
                .align_items(AlignItems::CENTER)
                .gap(Dp(4.0)))
            .child((
                secondary_button("Parameters", {
                    let app = app.clone();
                    let id = id.clone();
                    move || app.open_effect_params(&id, index)
                }),
                Spacer(),
                icon_button(
                    Symbols::arrow_upward,
                    {
                        let app = app.clone();
                        let id = id.clone();
                        move || app.move_effect(&id, index, -1)
                    },
                    Some(th.on_surface_variant),
                ),
                icon_button(
                    Symbols::arrow_downward,
                    {
                        let app = app.clone();
                        let id = id.clone();
                        move || app.move_effect(&id, index, 1)
                    },
                    Some(th.on_surface_variant),
                ),
                icon_button(
                    Symbols::close,
                    {
                        let app = app.clone();
                        let id = id.clone();
                        move || app.remove_effect(&id, index)
                    },
                    Some(th.error),
                ),
            )),
        );
    }

    rows.push(divider());
    let active = recording.fx.iter().any(|slot| !slot.bypass);
    rows.push(
        Row(Modifier::new()
            .fill_max_width()
            .align_items(AlignItems::CENTER)
            .gap(Dp(8.0)))
        .child((
            secondary_button("Add effect", {
                let app = app.clone();
                let id = id.clone();
                move || app.open_add_effect(&id)
            }),
            Spacer(),
            if rendering {
                Text("Applying effects…".to_string())
                    .size(Sp(12.0))
                    .color(th.on_surface_variant)
            } else if active {
                primary_button("Apply", {
                    let app = app.clone();
                    let id = id.clone();
                    move || app.apply_effects(&id)
                })
            } else {
                Spacer()
            },
        )),
    );

    card(
        Column(
            Modifier::new()
                .fill_max_width()
                .padding(Dp(14.0))
                .gap(Dp(8.0)),
        )
        .child(rows),
    )
}

fn divider() -> View {
    let th = theme();
    Box(Modifier::new()
        .fill_max_width()
        .height(Dp(1.0))
        .background(th.outline_variant.with_alpha_f32(0.4)))
}
