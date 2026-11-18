use std::rc::Rc;

use repose_core::prelude::*;
use repose_core::modifier::PaddingValues;
use repose_core::text::FontWeight;
use repose_material::Icon;
use repose_material::material3::{Slider, Switch, SwitchConfig};
use repose_ui::*;

use crate::app::App;
use crate::icons::Symbols;
use crate::settings::{SortOrder, StartTab, WaveformStyle};
use crate::ui::common::{
    card, chip, format_bytes, format_date, format_duration, icon_button, scroll_page,
    section_title, slider_config, top_bar,
};

pub fn screen(app: &Rc<App>) -> View {
    let th = theme();
    let settings = app.settings.get();
    let devices = app.devices.get();
    let library = app.library.get();
    let stats = app.store.borrow().stats();

    let default_topic = settings
        .new_recording_topic
        .as_deref()
        .filter(|id| library.topic(id).is_some())
        .map(|id| library.breadcrumb(id))
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| "Inbox".to_string());

    let (selected_device_name, selected_device_detail) = match settings.device.as_deref() {
        Some(name) => devices
            .iter()
            .find(|device| device.name == name)
            .map(|device| {
                (
                    device.name.clone(),
                    format!("{} Hz · {} channel(s)", device.sample_rate, device.channels),
                )
            })
            .unwrap_or_else(|| (name.to_string(), "Not detected right now".to_string())),
        None => (
            "System default".to_string(),
            "The microphone chosen by the system".to_string(),
        ),
    };

    let rates = [
        (None, "Default"),
        (Some(44_100), "44.1 kHz"),
        (Some(48_000), "48 kHz"),
        (Some(96_000), "96 kHz"),
    ];
    let rate_chips: Vec<View> = rates
        .iter()
        .map(|(rate, label)| {
            let app = app.clone();
            let rate = *rate;
            chip(label, settings.sample_rate == rate, move || {
                app.update_settings(|settings| settings.sample_rate = rate)
            })
        })
        .collect();

    let waveforms = [(WaveformStyle::Bars, "Bars"), (WaveformStyle::Line, "Line")];
    let waveform_chips: Vec<View> = waveforms
        .iter()
        .map(|(style, label)| {
            let app = app.clone();
            let style = *style;
            chip(label, settings.waveform_style == style, move || {
                app.update_settings(|settings| settings.waveform_style = style)
            })
        })
        .collect();

    let speeds = [0.75f32, 1.0, 1.25, 1.5, 2.0];
    let speed_chips: Vec<View> = speeds
        .iter()
        .map(|speed| {
            let app = app.clone();
            let value = *speed;
            chip(
                &format_speed(value),
                (settings.playback_speed - value).abs() < 0.01,
                move || app.update_settings(|settings| settings.playback_speed = value),
            )
        })
        .collect();

    let sort_options = [
        (SortOrder::Newest, "Newest"),
        (SortOrder::Oldest, "Oldest"),
        (SortOrder::Title, "A–Z"),
    ];
    let sort_chips: Vec<View> = sort_options
        .iter()
        .map(|(order, label)| {
            let app = app.clone();
            let order = *order;
            chip(label, settings.sort == order, move || {
                app.update_settings(|settings| settings.sort = order)
            })
        })
        .collect();

    let start_options = [
        (StartTab::Record, "Record"),
        (StartTab::Library, "Library"),
    ];
    let start_chips: Vec<View> = start_options
        .iter()
        .map(|(tab, label)| {
            let app = app.clone();
            let tab = *tab;
            chip(label, settings.start_tab == tab, move || {
                app.update_settings(|settings| settings.start_tab = tab)
            })
        })
        .collect();

    let theme_chips = vec![
        chip("Light", !settings.dark, {
            let app = app.clone();
            move || app.update_settings(|settings| settings.dark = false)
        }),
        chip("Dark", settings.dark, {
            let app = app.clone();
            move || app.update_settings(|settings| settings.dark = true)
        }),
    ];

    let last_backup = settings
        .last_backup_at
        .map(format_date)
        .unwrap_or_else(|| "Never".to_string());

    let content: Vec<View> = vec![
        top_bar(
            "Settings",
            Some("Local-first, no account required".to_string()),
            None,
            vec![],
        ),
        section_title("Microphone", None),
        card(Column(Modifier::new().fill_max_width().padding(Dp(10.0)).gap(Dp(2.0))).child(
            Box(Modifier::new()
                .fill_max_width()
                .clip_rounded(Dp(12.0))
                .clickable()
                .on_click({
                    let app = app.clone();
                    move || app.open_microphone_picker()
                })
                .padding(Dp(12.0)))
            .child(
                Row(Modifier::new()
                    .fill_max_width()
                    .align_items(AlignItems::CENTER)
                    .gap(Dp(10.0)))
                .child((
                    Icon(Symbols::mic).size(Sp(18.0)).color(th.primary),
                    Column(Modifier::new().flex_grow(1.0).gap(Dp(2.0))).child((
                        Text(selected_device_name).size(Sp(13.0)).font_weight(FontWeight(500)).color(th.on_surface).single_line().overflow_ellipsize(),
                        Text(selected_device_detail).size(Sp(11.0)).color(th.on_surface_variant).single_line(),
                    )),
                    Icon(Symbols::chevron_right).size(Sp(18.0)).color(th.on_surface_variant),
                )),
            ),
        )),
        section_title("Recording", None),
        card(Column(Modifier::new().fill_max_width().padding(Dp(16.0)).gap(Dp(12.0))).child(vec![
            setting_label(
                "Sample rate",
                "Unavailable rates fall back to the device's own.",
            ),
            Row(Modifier::new().fill_max_width().flex_wrap(FlexWrap::Wrap).gap(Dp(6.0)))
                .child(rate_chips),
            divider(),
            switch_row(
                "Ask before saving",
                "Show the name and topic dialog after every recording.",
                settings.confirm_save,
                {
                    let app = app.clone();
                    move |value| app.update_settings(|settings| settings.confirm_save = value)
                },
            ),
            divider(),
            link_row("New recordings go to", &default_topic, {
                let app = app.clone();
                move || app.open_default_topic()
            }),
            divider(),
            setting_label("Waveform", "How recordings are drawn while recording and playing."),
            Row(Modifier::new().fill_max_width().flex_wrap(FlexWrap::Wrap).gap(Dp(6.0)))
                .child(waveform_chips),
        ])),
        section_title("Playback", None),
        card(Column(Modifier::new().fill_max_width().padding(Dp(16.0)).gap(Dp(12.0))).child((
            setting_label("Default speed", "Applied when a recording starts playing."),
            Row(Modifier::new().fill_max_width().flex_wrap(FlexWrap::Wrap).gap(Dp(6.0)))
                .child(speed_chips),
            Row(Modifier::new().fill_max_width().align_items(AlignItems::CENTER).gap(Dp(12.0))).child((
                Icon(Symbols::volume_up).size(Sp(18.0)).color(th.on_surface_variant),
                Box(Modifier::new().flex_grow(1.0)).child(Slider(
                    settings.volume,
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
                Text(format!("{:.0}%", settings.volume * 100.0))
                    .size(Sp(11.0))
                    .color(th.on_surface_variant),
            )),
        ))),
        section_title("Controls", None),
        card(Column(Modifier::new().fill_max_width().padding(Dp(16.0)).gap(Dp(12.0))).child((
            stepper_row(
                "Scrub back",
                "Seconds to rewind from the player.",
                settings.scrub_back_secs,
                {
                    let app = app.clone();
                    Rc::new(move |delta| {
                        app.update_settings(|settings| {
                            settings.scrub_back_secs =
                                step_scrub(settings.scrub_back_secs, delta)
                        })
                    })
                },
            ),
            divider(),
            stepper_row(
                "Scrub forward",
                "Seconds to skip ahead from the player.",
                settings.scrub_forward_secs,
                {
                    let app = app.clone();
                    Rc::new(move |delta| {
                        app.update_settings(|settings| {
                            settings.scrub_forward_secs =
                                step_scrub(settings.scrub_forward_secs, delta)
                        })
                    })
                },
            ),
            divider(),
            stepper_row(
                "Mark rewind threshold",
                "Marks this close behind the playhead are skipped when jumping to the previous mark.",
                settings.mark_rewind_secs,
                {
                    let app = app.clone();
                    Rc::new(move |delta| {
                        app.update_settings(|settings| {
                            settings.mark_rewind_secs =
                                step_threshold(settings.mark_rewind_secs, delta)
                        })
                    })
                },
            ),
            divider(),
            stepper_row(
                "Mark nudge",
                "Seconds a mark moves with the nudge buttons.",
                settings.mark_nudge_secs,
                {
                    let app = app.clone();
                    Rc::new(move |delta| {
                        app.update_settings(|settings| {
                            settings.mark_nudge_secs =
                                step_nudge(settings.mark_nudge_secs, delta)
                        })
                    })
                },
            ),
        ))),
        section_title("Library", None),
        card(Column(Modifier::new().fill_max_width().padding(Dp(16.0)).gap(Dp(12.0))).child((
            setting_label("Sort recordings", "Applies to the library, topics and inbox."),
            Row(Modifier::new().fill_max_width().flex_wrap(FlexWrap::Wrap).gap(Dp(6.0)))
                .child(sort_chips),
            divider(),
            setting_label("Open on", "The tab shown when Aareco starts."),
            Row(Modifier::new().fill_max_width().flex_wrap(FlexWrap::Wrap).gap(Dp(6.0)))
                .child(start_chips),
        ))),
        section_title("Appearance", None),
        card(Column(Modifier::new().fill_max_width().padding(Dp(16.0)).gap(Dp(12.0))).child((
            setting_label("Theme", "Warm charcoal for late nights, paper for daylight."),
            Row(Modifier::new().fill_max_width().flex_wrap(FlexWrap::Wrap).gap(Dp(6.0)))
                .child(theme_chips),
        ))),
        section_title("Storage", None),
        card(Column(Modifier::new().fill_max_width().padding(Dp(16.0)).gap(Dp(12.0))).child((
            Row(Modifier::new().fill_max_width().align_items(AlignItems::CENTER).gap(Dp(10.0))).child((
                Icon(Symbols::storage).size(Sp(20.0)).color(th.secondary),
                Column(Modifier::new().flex_grow(1.0).gap(Dp(2.0))).child((
                    Text(format!(
                        "{} recordings · {} · {}",
                        stats.recordings,
                        format_duration(stats.duration_ms),
                        format_bytes(stats.bytes)
                    ))
                    .size(Sp(13.0))
                    .font_weight(FontWeight(500))
                    .color(th.on_surface),
                    Text(format!("Last backup · {last_backup}"))
                        .size(Sp(11.0))
                        .color(th.on_surface_variant),
                )),
            )),
            Text(app.root.display().to_string())
                .size(Sp(11.0))
                .color(th.on_surface_variant)
                .max_lines(2),
            Row(Modifier::new().fill_max_width().flex_wrap(FlexWrap::Wrap).gap(Dp(8.0))).child((
                crate::ui::common::primary_button("Back up now", {
                    let app = app.clone();
                    move || app.backup_now()
                }),
                crate::ui::common::secondary_button("Restore backup", {
                    let app = app.clone();
                    move || app.restore_backup()
                }),
                crate::ui::common::secondary_button("Reveal folder", {
                    let app = app.clone();
                    move || app.reveal_data_folder()
                }),
            )),
        ))),
        section_title("About", None),
        card(Column(Modifier::new().fill_max_width().padding(Dp(16.0)).gap(Dp(8.0))).child((
            Row(Modifier::new().align_items(AlignItems::CENTER).gap(Dp(8.0))).child((
                Text("aareco".to_string())
                    .size(Sp(20.0))
                    .font_weight(FontWeight(700))
                    .color(th.on_surface),
                Box(Modifier::new()
                    .size(Dp(6.0), Dp(6.0))
                    .background(th.primary)
                    .clip_rounded(Dp(999.0))),
                Text(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .size(Sp(11.0))
                    .color(th.on_surface_variant),
            )),
            Text("A voice recorder with a topic tree. Audio never leaves this device unless you back it up.".to_string())
                .size(Sp(12.0))
                .color(th.on_surface_variant)
                .max_lines(3),
            Text("Recorder core from Yadaw. Interface built with Repose. GNU AGPL-3.0-or-later.".to_string())
                .size(Sp(11.0))
                .color(th.on_surface_variant)
                .max_lines(3),
        ))),
    ];

    scroll_page("aareco:settings", Dp(12.0), content)
}

fn setting_label(title: &str, subtitle: &str) -> View {
    let th = theme();
    Column(Modifier::new().fill_max_width().gap(Dp(2.0))).child((
        Text(title.to_string())
            .size(Sp(13.0))
            .font_weight(FontWeight(500))
            .color(th.on_surface),
        Text(subtitle.to_string())
            .size(Sp(11.0))
            .color(th.on_surface_variant)
            .max_lines(2),
    ))
}

fn switch_row(
    title: &str,
    subtitle: &str,
    value: bool,
    on_change: impl Fn(bool) + 'static,
) -> View {
    Row(Modifier::new()
        .fill_max_width()
        .align_items(AlignItems::CENTER)
        .gap(Dp(12.0)))
    .child((
        Box(Modifier::new().flex_grow(1.0)).child(setting_label(title, subtitle)),
        Switch(
            value,
            on_change,
            SwitchConfig {
                modifier: Modifier::new().flex_shrink(0.0),
                ..Default::default()
            },
        ),
    ))
}

fn link_row(title: &str, value: &str, on_click: impl Fn() + 'static) -> View {
    let th = theme();
    Box(Modifier::new()
        .fill_max_width()
        .clip_rounded(Dp(10.0))
        .clickable()
        .on_click(on_click)
        .padding_values(PaddingValues {
            left: Dp(2.0),
            right: Dp(2.0),
            top: Dp(4.0),
            bottom: Dp(4.0),
        }))
    .child(
        Row(Modifier::new().fill_max_width().align_items(AlignItems::CENTER).gap(Dp(12.0))).child((
            Box(Modifier::new().flex_grow(1.0)).child(
                Column(Modifier::new().gap(Dp(2.0))).child((
                    Text(title.to_string())
                        .size(Sp(13.0))
                        .font_weight(FontWeight(500))
                        .color(th.on_surface),
                    Text(value.to_string())
                        .size(Sp(11.0))
                        .color(th.on_surface_variant)
                        .single_line(),
                )),
            ),
            Icon(Symbols::chevron_right)
                .size(Sp(18.0))
                .color(th.on_surface_variant),
        )),
    )
}

fn divider() -> View {
    let th = theme();
    Box(Modifier::new()
        .fill_max_width()
        .height(Dp(1.0))
        .background(th.outline_variant.with_alpha_f32(0.45)))
}

fn stepper_row(
    title: &str,
    subtitle: &str,
    value: u32,
    on_step: Rc<dyn Fn(i32)>,
) -> View {
    let th = theme();
    let back = on_step.clone();
    Row(Modifier::new()
        .fill_max_width()
        .align_items(AlignItems::CENTER)
        .gap(Dp(10.0)))
    .child((
        Box(Modifier::new().flex_grow(1.0)).child(setting_label(title, subtitle)),
        icon_button(Symbols::remove, move || back(-1), Some(th.on_surface)),
        Box(Modifier::new()
            .width(Dp(44.0))
            .align_items(AlignItems::CENTER)
            .justify_content(JustifyContent::CENTER))
        .child(
            Text(format!("{value}s"))
                .size(Sp(14.0))
                .font_weight(FontWeight(600))
                .color(th.on_surface)
                .single_line(),
        ),
        icon_button(Symbols::add, move || on_step(1), Some(th.on_surface)),
    ))
}

fn step_scrub(value: u32, delta: i32) -> u32 {
    (value as i32 + delta * 5).clamp(5, 120) as u32
}

fn step_threshold(value: u32, delta: i32) -> u32 {
    (value as i32 + delta).clamp(0, 10) as u32
}

fn step_nudge(value: u32, delta: i32) -> u32 {
    (value as i32 + delta).clamp(1, 30) as u32
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
