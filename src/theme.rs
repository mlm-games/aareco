use repose_core::Dp;
use repose_core::color::Color;
use repose_core::locals::{ColorScheme, Theme, Typography};

pub const BG: &str = "#0B0C0F";

pub fn app_theme(dark: bool) -> Theme {
    let colors = if dark { dark_scheme() } else { light_scheme() };
    let mut typography = Typography::default();
    typography.display_small = repose_core::Sp(38.0);
    typography.headline_medium = repose_core::Sp(26.0);
    typography.headline_small = repose_core::Sp(22.0);
    typography.title_large = repose_core::Sp(20.0);
    typography.title_medium = repose_core::Sp(16.0);
    typography.body_medium = repose_core::Sp(14.0);
    typography.label_large = repose_core::Sp(14.0);

    let mut theme = Theme::default().with_colors(colors);
    theme.typography = typography;
    theme.focus = colors.primary;
    theme.scrollbar_track = Color::TRANSPARENT;
    theme.scrollbar_thumb = colors.outline.with_alpha(150);
    theme.shapes.extra_large = Dp(28.0);
    theme.shapes.large = Dp(20.0);
    theme.shapes.medium = Dp(14.0);
    theme
}

fn dark_scheme() -> ColorScheme {
    let mut c = ColorScheme::dark();
    c.background = Color::from_hex(BG);
    c.on_background = Color::from_hex("#F3EFEA");
    c.surface = Color::from_hex("#101116");
    c.on_surface = Color::from_hex("#F3EFEA");
    c.surface_variant = Color::from_hex("#26262B");
    c.on_surface_variant = Color::from_hex("#B7B0A8");
    c.surface_container_lowest = Color::from_hex("#0A0B0E");
    c.surface_container_low = Color::from_hex("#14161C");
    c.surface_container = Color::from_hex("#181B22");
    c.surface_container_high = Color::from_hex("#1F232C");
    c.surface_container_highest = Color::from_hex("#282D38");
    c.surface_bright = Color::from_hex("#2B303C");
    c.surface_dim = Color::from_hex("#07080A");
    c.surface_tint = Color::from_hex("#FF8A7A");

    c.primary = Color::from_hex("#FF8A7A");
    c.on_primary = Color::from_hex("#2B0905");
    c.primary_container = Color::from_hex("#5C1B13");
    c.on_primary_container = Color::from_hex("#FFDAD4");
    c.secondary = Color::from_hex("#9BE3C0");
    c.on_secondary = Color::from_hex("#003824");
    c.secondary_container = Color::from_hex("#0F5138");
    c.on_secondary_container = Color::from_hex("#B8F5D6");
    c.tertiary = Color::from_hex("#FFD37A");
    c.on_tertiary = Color::from_hex("#3B2A00");
    c.tertiary_container = Color::from_hex("#574100");
    c.on_tertiary_container = Color::from_hex("#FFE8B0");

    c.outline = Color::from_hex("#8B847C");
    c.outline_variant = Color::from_hex("#3B3936");
    c.inverse_surface = Color::from_hex("#F3EFEA");
    c.inverse_on_surface = Color::from_hex("#1A1C20");
    c.inverse_primary = Color::from_hex("#A63B2E");
    c.error = Color::from_hex("#FFB4AB");
    c.on_error = Color::from_hex("#690005");
    c.error_container = Color::from_hex("#93000A");
    c.on_error_container = Color::from_hex("#FFDAD6");
    c.scrim = Color::from_hex("#000000");
    c.focus = c.primary;
    c
}

fn light_scheme() -> ColorScheme {
    let mut c = ColorScheme::light();
    c.background = Color::from_hex("#FAF6F0");
    c.on_background = Color::from_hex("#211D19");
    c.surface = Color::from_hex("#FFFBF6");
    c.on_surface = Color::from_hex("#211D19");
    c.surface_variant = Color::from_hex("#EDE4D9");
    c.on_surface_variant = Color::from_hex("#5C554D");
    c.surface_container_lowest = Color::from_hex("#FFFFFF");
    c.surface_container_low = Color::from_hex("#F6F0E8");
    c.surface_container = Color::from_hex("#F0E8DD");
    c.surface_container_high = Color::from_hex("#EAE0D4");
    c.surface_container_highest = Color::from_hex("#E4D8CA");
    c.primary = Color::from_hex("#B3362A");
    c.on_primary = Color::from_hex("#FFFFFF");
    c.primary_container = Color::from_hex("#FFDAD4");
    c.on_primary_container = Color::from_hex("#410001");
    c.secondary = Color::from_hex("#2F6B4F");
    c.on_secondary = Color::from_hex("#FFFFFF");
    c.secondary_container = Color::from_hex("#B4F1CE");
    c.on_secondary_container = Color::from_hex("#002113");
    c.tertiary = Color::from_hex("#765B00");
    c.on_tertiary = Color::from_hex("#FFFFFF");
    c.tertiary_container = Color::from_hex("#FFE08B");
    c.on_tertiary_container = Color::from_hex("#241A00");
    c.outline = Color::from_hex("#877269");
    c.outline_variant = Color::from_hex("#D9C2B8");
    c.focus = c.primary;
    c
}
