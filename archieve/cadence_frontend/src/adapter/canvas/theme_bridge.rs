#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasThemeSnapshot {
    pub canvas_background: &'static str,
    pub grid_minor: &'static str,
    pub grid_major: &'static str,
    pub tile_fill: &'static str,
    pub tile_border: &'static str,
    pub tile_selected_border: &'static str,
    pub connection: &'static str,
    pub connection_preview: &'static str,
    pub port_input: &'static str,
    pub port_output: &'static str,
    pub selection: &'static str,
    pub drag_preview: &'static str,
    pub marquee_fill: &'static str,
    pub marquee_border: &'static str,
    pub diagnostics: &'static str,
    pub text: &'static str,
}

pub fn default_canvas_theme() -> CanvasThemeSnapshot {
    CanvasThemeSnapshot {
        canvas_background: "var(--color-shell, #11161b)",
        grid_minor: "var(--color-grid-minor, rgba(255,255,255,0.08))",
        grid_major: "var(--color-grid-major, rgba(255,255,255,0.14))",
        tile_fill: "var(--color-tile-fill, #22303a)",
        tile_border: "var(--color-tile-border, #6b8ba4)",
        tile_selected_border: "var(--color-tile-selected-border, #f4d35e)",
        connection: "var(--color-connection, #7bdff2)",
        connection_preview: "var(--color-connection-preview, #9cf6f6)",
        port_input: "var(--color-port-input, #f7a072)",
        port_output: "var(--color-port-output, #7bdff2)",
        selection: "var(--color-selection, rgba(244,211,94,0.16))",
        drag_preview: "var(--color-drag-preview, rgba(123,223,242,0.18))",
        marquee_fill: "var(--color-marquee-fill, rgba(244,211,94,0.12))",
        marquee_border: "var(--color-marquee-border, #f4d35e)",
        diagnostics: "var(--color-diagnostics, #ff6b6b)",
        text: "var(--color-canvas-text, #f5f7fa)",
    }
}
