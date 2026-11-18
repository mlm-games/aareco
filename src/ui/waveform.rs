use std::rc::Rc;

use repose_canvas::Canvas;
use repose_core::StrokeCap;
use repose_core::prelude::*;

use crate::settings::WaveformStyle;
use crate::ui::common::PAGE_MAX;

pub fn waveform(
    peaks: Vec<f32>,
    progress: f32,
    marks: Vec<f32>,
    height: Dp,
    on_seek: Option<Rc<dyn Fn(f32)>>,
    style: WaveformStyle,
) -> View {
    let th = theme();
    let played = th.primary;
    let unplayed = th.outline.with_alpha_f32(0.45);
    let mark_color = th.tertiary;
    let cursor = th.on_surface;
    let width = remember_mutable(|| 0.0f32);
    let dragging = remember_mutable(|| false);

    let mut modifier = Modifier::new()
        .fill_max_width()
        .max_width(PAGE_MAX)
        .height(height)
        .clip_rounded(Dp(12.0));

    if let Some(on_seek) = on_seek {
        let width_down = width.clone();
        let drag_down = dragging.clone();
        let seek_down = on_seek.clone();
        modifier = modifier.on_pointer_down(move |event| {
            let w = *width_down.get();
            if w > 0.0 {
                drag_down.set(true);
                seek_down((px_to_dp(Px(event.position.x)).0 / w).clamp(0.0, 1.0));
                event.consume();
            }
        });

        let width_move = width.clone();
        let drag_move = dragging.clone();
        let seek_move = on_seek.clone();
        modifier = modifier.on_pointer_move(move |event| {
            if *drag_move.get() {
                let w = *width_move.get();
                if w > 0.0 {
                    seek_move((px_to_dp(Px(event.position.x)).0 / w).clamp(0.0, 1.0));
                    event.consume();
                }
            }
        });

        let drag_up = dragging.clone();
        let drag_cancel = drag_up.clone();
        modifier = modifier
            .on_pointer_up(move |event| {
                drag_up.set(false);
                event.consume();
            })
            .on_pointer_cancel(move |_| drag_cancel.set(false));
    }

    let width_size = width.clone();
    modifier = modifier.on_size_changed(move |size| width_size.set(size.x));

    Canvas(modifier, move |scope| {
        let w = scope.size.width;
        let h = scope.size.height;
        if w <= 0.0 || h <= 1.0 {
            return;
        }
        let bar_count = ((w / 4.0) as usize).clamp(16, 220);
        let gap = w / bar_count as f32;
        let bar_width = (gap * 0.52).max(1.0);
        let radius = (bar_width * 0.5).min(2.5);
        let max_height = (h - 8.0).max(2.0);

        if peaks.is_empty() {
            scope.draw_line(
                Vec2 { x: 0.0, y: h * 0.5 },
                Vec2 { x: w, y: h * 0.5 },
                unplayed,
                Px(1.5),
                StrokeCap::Round,
            );
        } else if style == WaveformStyle::Bars {
            for index in 0..bar_count {
                let source = index * peaks.len() / bar_count;
                let amplitude = peaks
                    .get(source)
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0)
                    .sqrt();
                let bar_height = (amplitude * max_height).max(2.0);
                let x = index as f32 * gap + (gap - bar_width) * 0.5;
                let y = (h - bar_height) * 0.5;
                let ratio = index as f32 / bar_count as f32;
                let color = if ratio <= progress { played } else { unplayed };
                scope.draw_rect(
                    Rect {
                        x,
                        y,
                        w: bar_width,
                        h: bar_height,
                    },
                    color,
                    Px(radius),
                );
            }
        } else {
            let mut previous: Option<(f32, f32)> = None;
            for index in 0..bar_count {
                let source = index * peaks.len() / bar_count;
                let amplitude = peaks
                    .get(source)
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0)
                    .sqrt();
                let half = (amplitude * max_height * 0.5).max(1.0);
                let x = index as f32 * gap + gap * 0.5;
                if let Some((previous_x, previous_half)) = previous {
                    let ratio = index as f32 / bar_count as f32;
                    let color = if ratio <= progress { played } else { unplayed };
                    scope.draw_line(
                        Vec2 {
                            x: previous_x,
                            y: h * 0.5 - previous_half,
                        },
                        Vec2 {
                            x,
                            y: h * 0.5 - half,
                        },
                        color,
                        Px(1.5),
                        StrokeCap::Round,
                    );
                    scope.draw_line(
                        Vec2 {
                            x: previous_x,
                            y: h * 0.5 + previous_half,
                        },
                        Vec2 {
                            x,
                            y: h * 0.5 + half,
                        },
                        color,
                        Px(1.5),
                        StrokeCap::Round,
                    );
                }
                previous = Some((x, half));
            }
        }

        for &mark in &marks {
            let x = (mark.clamp(0.0, 1.0) * w).clamp(0.0, w - 1.0);
            scope.draw_rect(
                Rect {
                    x,
                    y: 3.0,
                    w: 1.5,
                    h: (h - 6.0).max(1.0),
                },
                mark_color.with_alpha_f32(0.8),
                Px(1.0),
            );
        }

        if progress > 0.0 {
            let x = (progress.clamp(0.0, 1.0) * w).clamp(0.0, w - 1.0);
            scope.draw_rect(
                Rect {
                    x,
                    y: 0.0,
                    w: 2.0,
                    h,
                },
                cursor,
                Px(1.0),
            );
        }
    })
}
