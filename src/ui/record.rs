use std::rc::Rc;

use aareco_recorder::RecorderState;
use repose_core::modifier::PaddingValues;
use repose_core::prelude::*;
use repose_core::text::FontWeight;
use repose_material::Icon;
use repose_ui::*;

use crate::app::App;
use crate::icons::Symbols;
use crate::ui::common::{format_timer, icon_button};
use crate::ui::waveform::waveform;

pub fn screen(app: &Rc<App>) -> View {
    let th = theme();
    let Some(snapshot) = app.session_snapshot() else {
        return Box(Modifier::new().fill_max_size());
    };
    let paused = snapshot.state == RecorderState::Paused;
    let error = snapshot.error.clone();
    let duration = snapshot.duration_ms;
    let marks: Vec<f32> = snapshot
        .marks
        .iter()
        .map(|mark| {
            if duration == 0 {
                0.0
            } else {
                (mark.offset_ms as f32 / duration as f32).clamp(0.0, 1.0)
            }
        })
        .collect();
    let peaks = snapshot.peaks.clone();
    let level = snapshot.level.clamp(0.0, 1.0);

    let top = Row(Modifier::new()
        .fill_max_width()
        .height(Dp(64.0))
        .align_items(AlignItems::CENTER)
        .gap(Dp(8.0)))
    .child((
        icon_button(
            Symbols::close,
            {
                let app = app.clone();
                move || app.open_discard_dialog()
            },
            None,
        ),
        Column(Modifier::new().flex_grow(1.0).gap(Dp(1.0))).child((
            Text("Recording".to_string())
                .size(Sp(20.0))
                .font_weight(FontWeight(600))
                .color(th.on_surface),
            Text(format!(
                "{} · {}",
                app.session
                    .borrow()
                    .as_ref()
                    .map(|session| session.recorder.device_name().to_string())
                    .unwrap_or_default(),
                app.session
                    .borrow()
                    .as_ref()
                    .map(|session| format!("{} Hz", session.recorder.sample_rate()))
                    .unwrap_or_default()
            ))
            .size(Sp(11.0))
            .color(th.on_surface_variant)
            .single_line(),
        )),
        state_badge(paused),
    ));

    let timer = Column(Modifier::new().align_items(AlignItems::CENTER).gap(Dp(4.0))).child((
        Text(format_timer(duration))
            .size(Sp(56.0))
            .font_weight(FontWeight(300))
            .letter_spacing(Sp(1.0))
            .color(th.on_surface),
        Text(if paused {
            "Paused".to_string()
        } else {
            "Recording".to_string()
        })
        .size(Sp(12.0))
        .color(if paused { th.tertiary } else { th.primary }),
    ));

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
                1.0,
                marks,
                Dp(150.0),
                None,
                app.settings.get().waveform_style,
            ),
            Box(Modifier::new()
                .fill_max_width()
                .height(Dp(4.0))
                .background(th.surface_container_highest)
                .clip_rounded(Dp(999.0)))
            .child(
                Row(Modifier::new().fill_max_size()).child((
                    Box(Modifier::new()
                        .height(Dp(4.0))
                        .flex_grow(level.max(0.02))
                        .background(if level > 0.92 { th.error } else { th.secondary })
                        .clip_rounded(Dp(999.0))),
                    Box(Modifier::new()
                        .height(Dp(4.0))
                        .flex_grow((1.0 - level).max(0.02))),
                )),
            ),
            Row(Modifier::new()
                .fill_max_width()
                .align_items(AlignItems::CENTER))
            .child((
                Text(
                    snapshot
                        .marks
                        .last()
                        .map(|mark| format!("Last mark at {}", format_timer(mark.offset_ms)))
                        .unwrap_or_else(|| {
                            "Drop marks on the moments you want to find again".to_string()
                        }),
                )
                .size(Sp(11.0))
                .color(th.on_surface_variant)
                .single_line(),
                Spacer(),
                Text(format!("{} mark(s)", snapshot.marks.len()))
                    .size(Sp(11.0))
                    .color(th.tertiary),
            )),
        )),
    );

    let controls = Row(Modifier::new()
        .fill_max_width()
        .align_items(AlignItems::CENTER)
        .justify_content(JustifyContent::SPACE_EVENLY)
        .padding_values(PaddingValues {
            left: Dp(12.0),
            right: Dp(12.0),
            top: Dp(8.0),
            bottom: Dp(8.0),
        }))
    .child((
        control_button(
            Symbols::bookmark_add,
            "Mark",
            th.tertiary_container,
            th.on_tertiary_container,
            {
                let app = app.clone();
                move || app.add_mark()
            },
            Dp(62.0),
        ),
        control_button(
            if paused {
                Symbols::play_arrow
            } else {
                Symbols::pause
            },
            if paused { "Resume" } else { "Pause" },
            th.primary,
            th.on_primary,
            {
                let app = app.clone();
                move || app.toggle_pause_recording()
            },
            Dp(94.0),
        ),
        control_button(
            Symbols::stop,
            "Stop",
            th.error_container,
            th.on_error_container,
            {
                let app = app.clone();
                move || app.stop_recording()
            },
            Dp(62.0),
        ),
    ));

    let mark_chips: Vec<View> = snapshot
        .marks
        .iter()
        .rev()
        .take(8)
        .map(|mark| {
            Box(Modifier::new()
                .background(th.tertiary_container.with_alpha_f32(0.5))
                .clip_rounded(Dp(999.0))
                .padding_values(PaddingValues {
                    left: Dp(10.0),
                    right: Dp(10.0),
                    top: Dp(6.0),
                    bottom: Dp(6.0),
                }))
            .child(
                Text(format_timer(mark.offset_ms))
                    .size(Sp(11.0))
                    .color(th.on_tertiary_container),
            )
        })
        .collect();

    let mut children: Vec<View> = vec![
        top,
        Box(Modifier::new().height(Dp(10.0))),
        timer,
        Box(Modifier::new().height(Dp(6.0))),
        waveform_card,
        controls,
    ];
    if !mark_chips.is_empty() {
        children.push(
            Row(Modifier::new()
                .fill_max_width()
                .flex_wrap(FlexWrap::Wrap)
                .gap(Dp(6.0)))
            .child(mark_chips),
        );
    }
    if let Some(error) = error {
        children.push(
            Box(Modifier::new()
                .fill_max_width()
                .background(th.error_container.with_alpha_f32(0.5))
                .border(Dp(1.0), th.error, Dp(14.0))
                .clip_rounded(Dp(14.0))
                .padding(Dp(12.0)))
            .child(
                Text(error)
                    .size(Sp(12.0))
                    .color(th.on_error_container)
                    .max_lines(3),
            ),
        );
    }

    Box(Modifier::new()
        .fill_max_size()
        .align_items(AlignItems::CENTER)
        .padding_values(PaddingValues {
            left: Dp(20.0),
            right: Dp(20.0),
            top: Dp(8.0),
            bottom: Dp(24.0),
        }))
    .child(
        Column(
            Modifier::new()
                .fill_max_width()
                .max_width(Dp(760.0))
                .gap(Dp(14.0)),
        )
        .child(children),
    )
}

fn state_badge(paused: bool) -> View {
    let th = theme();
    Box(Modifier::new()
        .background(if paused {
            th.tertiary_container.with_alpha_f32(0.6)
        } else {
            th.primary_container.with_alpha_f32(0.7)
        })
        .clip_rounded(Dp(999.0))
        .padding_values(PaddingValues {
            left: Dp(10.0),
            right: Dp(10.0),
            top: Dp(6.0),
            bottom: Dp(6.0),
        }))
    .child(
        Row(Modifier::new().align_items(AlignItems::CENTER).gap(Dp(6.0))).child((
            Box(Modifier::new()
                .size(Dp(6.0), Dp(6.0))
                .background(if paused { th.tertiary } else { th.primary })
                .clip_rounded(Dp(999.0))),
            Text(if paused { "PAUSED" } else { "REC" })
                .size(Sp(10.0))
                .font_weight(FontWeight(700))
                .letter_spacing(Sp(1.0))
                .color(if paused {
                    th.on_tertiary_container
                } else {
                    th.on_primary_container
                }),
        )),
    )
}

fn control_button(
    symbol: repose_material::Symbol,
    label: &str,
    background: Color,
    foreground: Color,
    on_click: impl Fn() + 'static,
    size: Dp,
) -> View {
    let th = theme();
    Column(Modifier::new().align_items(AlignItems::CENTER).gap(Dp(7.0))).child((
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
            .child(Icon(symbol).size(Sp(size.0 * 0.36)).color(foreground)),
        ),
        Text(label.to_string())
            .size(Sp(11.0))
            .font_weight(FontWeight(500))
            .color(th.on_surface_variant),
    ))
}
