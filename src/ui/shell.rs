use std::rc::Rc;

use repose_core::modifier::PaddingValues;
use repose_core::prelude::*;
use repose_core::text::FontWeight;
use repose_material::Icon;
use repose_material::material3::{
    NavItem, NavigationBar, NavigationBarConfig, NavigationBarDefaults,
};
use repose_ui::*;

use crate::app::{App, Route};
use crate::icons::Symbols;
use crate::ui::{home, library, player, record, settings, topics};

pub fn shell(app: &Rc<App>, route: Route) -> View {
    let content = match &route {
        Route::Home => home::screen(app),
        Route::Library => library::screen(app),
        Route::Topics => topics::screen(app),
        Route::Settings => settings::screen(app),
        Route::Record => record::screen(app),
        Route::Player { id } => player::screen(app, id),
        Route::Topic { id } => library::topic_detail(app, id),
        Route::Inbox => library::inbox(app),
    };

    if !route.is_root() {
        return content;
    }

    if compact() {
        Column(Modifier::new().fill_max_size()).child((
            Box(Modifier::new().fill_max_size()).child(content),
            Box(Modifier::new()
                .fill_max_width()
                .height(Dp(1.0))
                .background(theme().outline_variant.with_alpha_f32(0.4))),
            bottom_bar(app, &route),
        ))
    } else {
        Row(Modifier::new().fill_max_size()).child((
            sidebar(app, &route),
            Box(Modifier::new().flex_grow(1.0).fill_max_height()).child(content),
        ))
    }
}

fn compact() -> bool {
    !window_size_class().is_at_least_medium_width()
}

fn sidebar(app: &Rc<App>, route: &Route) -> View {
    let th = theme();
    let stats = app.store.borrow().stats();
    let selected = route.tab_index();

    Box(Modifier::new()
        .width(Dp(236.0))
        .flex_shrink(0.0)
        .fill_max_height()
        .background(th.surface_container_low)
        .border(Dp(1.0), th.outline_variant.with_alpha_f32(0.4), Dp(0.0)))
    .child(
        Column(
            Modifier::new()
                .fill_max_size()
                .padding_values(PaddingValues {
                    left: Dp(14.0),
                    right: Dp(14.0),
                    top: Dp(22.0),
                    bottom: Dp(18.0),
                })
                .gap(Dp(6.0)),
        )
        .child((
            nav_row(app, Symbols::mic, "Record", selected == 0, Route::Home),
            nav_row(
                app,
                Symbols::library_music,
                "Library",
                selected == 1,
                Route::Library,
            ),
            nav_row(app, Symbols::folder, "Topics", selected == 2, Route::Topics),
            nav_row(
                app,
                Symbols::settings,
                "Settings",
                selected == 3,
                Route::Settings,
            ),
            Spacer(),
            Box(Modifier::new()
                .fill_max_width()
                .background(th.surface_container)
                .border(Dp(1.0), th.outline_variant.with_alpha_f32(0.4), Dp(14.0))
                .clip_rounded(Dp(14.0))
                .padding(Dp(12.0)))
            .child(
                Column(Modifier::new().fill_max_width().gap(Dp(4.0))).child((
                    Row(Modifier::new().align_items(AlignItems::CENTER).gap(Dp(6.0))).child((
                        Icon(Symbols::storage).size(Sp(14.0)).color(th.secondary),
                        Text("Storage".to_string())
                            .size(Sp(11.0))
                            .font_weight(FontWeight(600))
                            .color(th.on_surface),
                    )),
                    Text(format!(
                        "{} recordings · {}",
                        stats.recordings,
                        crate::ui::common::format_bytes(stats.bytes)
                    ))
                    .size(Sp(10.0))
                    .color(th.on_surface_variant)
                    .max_lines(2),
                )),
            ),
        )),
    )
}

fn nav_row(
    app: &Rc<App>,
    symbol: repose_material::Symbol,
    label: &str,
    selected: bool,
    route: Route,
) -> View {
    let th = theme();
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
            move || app.nav.clear_and_push(route.clone())
        })
        .padding_values(PaddingValues {
            left: Dp(12.0),
            right: Dp(12.0),
            top: Dp(11.0),
            bottom: Dp(11.0),
        }))
    .child(
        Row(Modifier::new()
            .align_items(AlignItems::CENTER)
            .gap(Dp(12.0)))
        .child((
            Icon(symbol).size(Sp(19.0)).color(if selected {
                th.primary
            } else {
                th.on_surface_variant
            }),
            Text(label.to_string())
                .size(Sp(13.0))
                .font_weight(if selected {
                    FontWeight(600)
                } else {
                    FontWeight(400)
                })
                .color(if selected {
                    th.on_surface
                } else {
                    th.on_surface_variant
                }),
        )),
    )
}

fn bottom_bar(app: &Rc<App>, route: &Route) -> View {
    let th = theme();
    let selected = route.tab_index();
    let items = vec![
        NavItem {
            icon: Icon(Symbols::mic).size(Sp(22.0)),
            label: "Record".to_string(),
            on_click: {
                let app = app.clone();
                Rc::new(move || app.nav.clear_and_push(Route::Home))
            },
            enabled: true,
            interaction_source: None,
        },
        NavItem {
            icon: Icon(Symbols::library_music).size(Sp(22.0)),
            label: "Library".to_string(),
            on_click: {
                let app = app.clone();
                Rc::new(move || app.nav.clear_and_push(Route::Library))
            },
            enabled: true,
            interaction_source: None,
        },
        NavItem {
            icon: Icon(Symbols::folder).size(Sp(22.0)),
            label: "Topics".to_string(),
            on_click: {
                let app = app.clone();
                Rc::new(move || app.nav.clear_and_push(Route::Topics))
            },
            enabled: true,
            interaction_source: None,
        },
        NavItem {
            icon: Icon(Symbols::settings).size(Sp(22.0)),
            label: "Settings".to_string(),
            on_click: {
                let app = app.clone();
                Rc::new(move || app.nav.clear_and_push(Route::Settings))
            },
            enabled: true,
            interaction_source: None,
        },
    ];
    NavigationBar(
        selected,
        items,
        NavigationBarConfig {
            modifier: Modifier::new(),
            container_color: th.surface_container_low,
            selected_icon_color: th.primary,
            selected_text_color: th.on_surface,
            unselected_icon_color: th.on_surface_variant,
            unselected_text_color: th.on_surface_variant,
            indicator_color: th.primary_container,
            tonal_elevation: NavigationBarDefaults::TONAL_ELEVATION,
            ..Default::default()
        },
    )
}
