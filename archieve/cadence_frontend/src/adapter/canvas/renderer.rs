use crate::adapter::canvas::{CanvasDrawCommand, CanvasThemeSnapshot};

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasRenderFrame {
    pub commands: Vec<CanvasDrawCommand>,
    pub theme: CanvasThemeSnapshot,
}

pub fn render_frame(commands: Vec<CanvasDrawCommand>, theme: CanvasThemeSnapshot) -> CanvasRenderFrame {
    CanvasRenderFrame { commands, theme }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

    use super::{CanvasDrawCommand, CanvasRenderFrame, CanvasThemeSnapshot};

    pub fn render_canvas_by_id(
        canvas_id: &str,
        logical_width_px: i32,
        logical_height_px: i32,
        frame: &CanvasRenderFrame,
    ) -> Result<(), String> {
        let window = web_sys::window().ok_or_else(|| "window unavailable".to_string())?;
        let document = window
            .document()
            .ok_or_else(|| "document unavailable".to_string())?;
        let element = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| format!("canvas #{canvas_id} not found"))?;
        let canvas = element
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| format!("element #{canvas_id} is not a canvas"))?;
        let context = canvas
            .get_context("2d")
            .map_err(js_error)?
            .ok_or_else(|| "2d context unavailable".to_string())?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "failed to cast 2d context".to_string())?;
        let device_pixel_ratio = window.device_pixel_ratio();
        let pixel_width = (f64::from(logical_width_px) * device_pixel_ratio).round() as u32;
        let pixel_height = (f64::from(logical_height_px) * device_pixel_ratio).round() as u32;
        if canvas.width() != pixel_width {
            canvas.set_width(pixel_width);
        }
        if canvas.height() != pixel_height {
            canvas.set_height(pixel_height);
        }
        let style = canvas.style();
        style
            .set_property("width", &format!("{logical_width_px}px"))
            .map_err(js_error)?;
        style
            .set_property("height", &format!("{logical_height_px}px"))
            .map_err(js_error)?;

        context.save();
        context.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).map_err(js_error)?;
        context.clear_rect(0.0, 0.0, f64::from(pixel_width), f64::from(pixel_height));
        context.scale(device_pixel_ratio, device_pixel_ratio).map_err(js_error)?;
        draw_commands(&context, &frame.commands, &frame.theme)?;
        context.restore();
        Ok(())
    }

    fn draw_commands(
        context: &CanvasRenderingContext2d,
        commands: &[CanvasDrawCommand],
        theme: &CanvasThemeSnapshot,
    ) -> Result<(), String> {
        context.set_font("12px ui-monospace, SFMono-Regular, Menlo, monospace");
        context.set_text_baseline("middle");
        for command in commands {
            match command {
                CanvasDrawCommand::DrawBackground { width_px, height_px } => {
                    context.set_fill_style(&JsValue::from_str(theme.canvas_background));
                    context.fill_rect(0.0, 0.0, f64::from(*width_px), f64::from(*height_px));
                }
                CanvasDrawCommand::DrawGrid {
                    cols,
                    rows,
                    cell_width_px,
                    cell_height_px,
                    origin_x_px,
                    origin_y_px,
                } => {
                    context.set_stroke_style(&JsValue::from_str(theme.grid_minor));
                    context.set_line_width(1.0);
                    for col in 0..=*cols {
                        let x = f64::from(*origin_x_px + col as i32 * *cell_width_px);
                        let top = f64::from(*origin_y_px);
                        let bottom = f64::from(*origin_y_px + *rows as i32 * *cell_height_px);
                        stroke_line(context, x, top, x, bottom);
                    }
                    for row in 0..=*rows {
                        let y = f64::from(*origin_y_px + row as i32 * *cell_height_px);
                        let left = f64::from(*origin_x_px);
                        let right = f64::from(*origin_x_px + *cols as i32 * *cell_width_px);
                        stroke_line(context, left, y, right, y);
                    }
                }
                CanvasDrawCommand::DrawConnection {
                    from_x_px,
                    from_y_px,
                    to_x_px,
                    to_y_px,
                } => {
                    context.set_stroke_style(&JsValue::from_str(theme.connection));
                    context.set_line_width(2.0);
                    stroke_line(
                        context,
                        f64::from(*from_x_px),
                        f64::from(*from_y_px),
                        f64::from(*to_x_px),
                        f64::from(*to_y_px),
                    );
                }
                CanvasDrawCommand::DrawTileFrame {
                    x_px,
                    y_px,
                    width_px,
                    height_px,
                    selected,
                    ..
                } => {
                    context.set_fill_style(&JsValue::from_str(theme.tile_fill));
                    context.fill_rect(
                        f64::from(*x_px),
                        f64::from(*y_px),
                        f64::from(*width_px),
                        f64::from(*height_px),
                    );
                    context.set_stroke_style(&JsValue::from_str(if *selected {
                        theme.tile_selected_border
                    } else {
                        theme.tile_border
                    }));
                    context.set_line_width(2.0);
                    context.stroke_rect(
                        f64::from(*x_px),
                        f64::from(*y_px),
                        f64::from(*width_px),
                        f64::from(*height_px),
                    );
                }
                CanvasDrawCommand::DrawTileLabel { x_px, y_px, text } => {
                    context.set_fill_style(&JsValue::from_str(theme.text));
                    context
                        .fill_text(text, f64::from(*x_px), f64::from(*y_px))
                        .map_err(js_error)?;
                }
                CanvasDrawCommand::DrawPort {
                    x_px,
                    y_px,
                    radius_px,
                    is_output,
                } => {
                    context.begin_path();
                    context
                        .arc(
                            f64::from(*x_px),
                            f64::from(*y_px),
                            f64::from(*radius_px),
                            0.0,
                            std::f64::consts::TAU,
                        )
                        .map_err(js_error)?;
                    context.set_fill_style(&JsValue::from_str(if *is_output {
                        theme.port_output
                    } else {
                        theme.port_input
                    }));
                    context.fill();
                }
                CanvasDrawCommand::DrawSelectionCell {
                    x_px,
                    y_px,
                    width_px,
                    height_px,
                } => {
                    context.set_fill_style(&JsValue::from_str(theme.selection));
                    context.fill_rect(
                        f64::from(*x_px),
                        f64::from(*y_px),
                        f64::from(*width_px),
                        f64::from(*height_px),
                    );
                }
                CanvasDrawCommand::DrawDragPreview {
                    x_px,
                    y_px,
                    width_px,
                    height_px,
                    ..
                } => {
                    context.set_fill_style(&JsValue::from_str(theme.drag_preview));
                    context.fill_rect(
                        f64::from(*x_px),
                        f64::from(*y_px),
                        f64::from(*width_px),
                        f64::from(*height_px),
                    );
                }
                CanvasDrawCommand::DrawConnectionPreview {
                    from_x_px,
                    from_y_px,
                    to_x_px,
                    to_y_px,
                } => {
                    context.set_stroke_style(&JsValue::from_str(theme.connection_preview));
                    context.set_line_width(2.0);
                    stroke_line(
                        context,
                        f64::from(*from_x_px),
                        f64::from(*from_y_px),
                        f64::from(*to_x_px),
                        f64::from(*to_y_px),
                    );
                }
                CanvasDrawCommand::DrawMarquee {
                    left_px,
                    top_px,
                    width_px,
                    height_px,
                } => {
                    context.set_fill_style(&JsValue::from_str(theme.marquee_fill));
                    context.fill_rect(
                        f64::from(*left_px),
                        f64::from(*top_px),
                        f64::from(*width_px),
                        f64::from(*height_px),
                    );
                    context.set_stroke_style(&JsValue::from_str(theme.marquee_border));
                    context.stroke_rect(
                        f64::from(*left_px),
                        f64::from(*top_px),
                        f64::from(*width_px),
                        f64::from(*height_px),
                    );
                }
                CanvasDrawCommand::DrawDiagnosticsBanner { text } => {
                    context.set_fill_style(&JsValue::from_str(theme.diagnostics));
                    context.fill_rect(12.0, 12.0, 180.0, 24.0);
                    context.set_fill_style(&JsValue::from_str(theme.text));
                    context.fill_text(text, 20.0, 24.0).map_err(js_error)?;
                }
            }
        }
        Ok(())
    }

    fn stroke_line(
        context: &CanvasRenderingContext2d,
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
    ) {
        context.begin_path();
        context.move_to(from_x, from_y);
        context.line_to(to_x, to_y);
        context.stroke();
    }

    fn js_error(value: JsValue) -> String {
        value
            .as_string()
            .unwrap_or_else(|| "web canvas api failed".to_string())
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::render_canvas_by_id;

#[cfg(not(target_arch = "wasm32"))]
pub fn render_canvas_by_id(
    _canvas_id: &str,
    _logical_width_px: i32,
    _logical_height_px: i32,
    _frame: &CanvasRenderFrame,
) -> Result<(), String> {
    Ok(())
}
