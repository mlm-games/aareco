pub mod app;
pub mod icons;
pub mod paths;
pub mod playback;
pub mod settings;
pub mod theme;
pub mod ui;

#[cfg(target_os = "android")]
pub mod android;

use std::rc::Rc;

use repose_core::locals::set_theme_default;
use repose_core::prelude::*;
use repose_material::material3::MaterialTheme;
use repose_navigation::{Navigator, remember_back_stack};
use repose_platform::RenderContext;
use repose_ui::*;

use crate::app::{App, Route};

fn app_root(_s: &mut Scheduler, _rc: &RenderContext) -> View {
    let stack = remember_back_stack(Route::Home);
    let nav = Navigator {
        stack: (*stack).clone(),
    };
    let app = remember(|| App::new(nav.clone()).map(Rc::new));

    let app = match &*app {
        Ok(app) => app.clone(),
        Err(err) => return startup_error(err),
    };

    let start_applied = remember_mutable(|| false);
    if !*start_applied.get() {
        start_applied.set(true);
        let requested = std::env::var("AARECO_START_ROUTE").ok();
        let target = match requested.as_deref() {
            Some("library") => Some(Route::Library),
            Some("topics") => Some(Route::Topics),
            Some("settings") => Some(Route::Settings),
            Some("record") => Some(Route::Record),
            Some("inbox") => Some(Route::Inbox),
            Some("player") => app
                .library
                .get()
                .recordings
                .first()
                .map(|recording| Route::Player {
                    id: recording.id.clone(),
                }),
            Some(_) => None,
            None => match app.settings.get().start_tab {
                crate::settings::StartTab::Library => Some(Route::Library),
                crate::settings::StartTab::Record => None,
            },
        };
        if let Some(target) = target {
            app.nav.push(target);
        }
    }

    let app_theme = crate::theme::app_theme(app.settings.get().dark);
    set_theme_default(app_theme.clone());

    let back_app = app.clone();
    let can_app = app.clone();
    repose_navigation::back::set_checked(
        Some(Rc::new(move || back_app.go_back())),
        Some(Rc::new(move || can_app.can_go_back())),
    );

    if let Some(warning) = app.startup_warning() {
        app.toast(warning);
    }
    if app.session.borrow().is_some() || app.is_playing() {
        request_frame();
    }

    let route = app.current_route();
    let content = ui::shell::shell(&app, route);

    MaterialTheme(app_theme, || {
        Box(Modifier::new()
            .fill_max_size()
            .system_bars_padding()
            .ime_padding()
            .background(theme().background))
        .child((
            content,
            ui::dialogs::dialog(&app),
        ))
    })
}

fn startup_error(message: &str) -> View {
    let th = theme();
    Box(Modifier::new()
        .fill_max_size()
        .background(th.background)
        .padding(Dp(32.0)))
    .child(
        Column(Modifier::new()
            .fill_max_size()
            .align_items(AlignItems::CENTER)
            .justify_content(JustifyContent::CENTER)
            .gap(Dp(12.0)))
        .child((
            Text("Aareco could not open its library")
                .size(Sp(20.0))
                .color(th.on_surface),
            Text(message.to_string())
                .size(Sp(13.0))
                .color(th.on_surface_variant)
                .max_lines(6),
        )),
    )
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn run_desktop() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    rlobkit_dialogs::init();
    let window_size = std::env::var("AARECO_WINDOW")
        .ok()
        .and_then(|value| {
            let (width, height) = value.split_once('x')?;
            Some((width.parse().ok()?, height.parse().ok()?))
        })
        .unwrap_or((1120, 780));
    repose_platform::run_desktop_app_with_config(
        app_root,
        repose_platform::AppConfig {
            window_title: "Aareco".to_string(),
            window_size,
            enable_inspector: true,
            ..Default::default()
        },
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn android_main(
    android_app: winit::platform::android::activity::AndroidApp,
) {
    rlobkit_app_events::android_log::init(
        env!("CARGO_PKG_NAME"),
        concat!(env!("CARGO_PKG_NAME"), "=info"),
    );
    crate::android::install(&android_app);
    crate::android::request_microphone_permission(&android_app);
    if let Err(err) = repose_platform::android::run_android_app_with_options(
        android_app,
        app_root,
        repose_platform::android::AndroidOptions::default(),
    ) {
        log::error!("Aareco stopped: {err:?}");
    }
}
