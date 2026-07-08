use super::context::{ChartMapping, LinearPriceMap, RenderContext};
use crate::config::{TooltipMode, TooltipOptions};
use crate::model::Bar;
use crate::styles::typography;
/// Tooltip Renderers
///
/// Provides three tooltip variants:
/// - Floating: Classic tooltip that follows cursor position
/// - Tracking: Fixed horizontal bar at top of chart showing OHLC data
/// - Magnifier: Circular zoom lens for detailed data inspection
use crate::tokens::DESIGN_TOKENS;
use egui::{Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2, epaint::StrokeKind};

/// Renders tooltip based on configured mode
///
/// `prev_close`：悬停K线前一根K线的收盘价，用于计算真实涨跌幅（相对前收），
/// 传 None 时降级为以当根开盘价作基准
pub fn render_tooltip_with_options(
    context: &RenderContext,
    hover_pos: Pos2,
    candle: &Bar,
    options: &TooltipOptions,
    price_scale: &LinearPriceMap,
    coords: &ChartMapping,
    visible_data: &[Bar],
    // Claude Opus 4.8 AI，新增于 2026 年 07 月 08 日。逻辑：
    // 传入前一根K线收盘价，使悬停K线的涨跌幅以前收为基准，与第一行（最新K线）计算口径一致
    prev_close: Option<f64>,
) {
    match options.mode {
        TooltipMode::Floating => {
            render_floating_tooltip(context.painter, hover_pos, context.rect, candle, options);
        }
        TooltipMode::Tracking => {
            render_tracking_tooltip(context.painter, context.rect, candle, options, prev_close);
        }
        TooltipMode::Magnifier => {
            render_magnifier_tooltip(
                context,
                hover_pos,
                candle,
                options,
                price_scale,
                coords,
                visible_data,
            );
        }
    }
}

/// Renders floating tooltip near cursor
/// Classic tooltip that follows the mouse
pub fn render_floating_tooltip(
    painter: &Painter,
    hover_pos: Pos2,
    price_rect: Rect,
    candle: &Bar,
    options: &TooltipOptions,
) {
    let mut tooltip_lines = Vec::new();

    if options.show_time {
        tooltip_lines.push(format!("Time: {}", candle.time.format("%Y-%m-%d %H:%M:%S")));
    }

    if options.show_ohlc {
        let precision = options.price_precision;
        tooltip_lines.push(format!("Open:   {:.precision$}", candle.open));
        tooltip_lines.push(format!("High:   {:.precision$}", candle.high));
        tooltip_lines.push(format!("Low:    {:.precision$}", candle.low));
        tooltip_lines.push(format!("Close:  {:.precision$}", candle.close));
    }

    if options.show_volume {
        tooltip_lines.push(format!("Volume: {:.2}", candle.volume));
    }

    if options.show_change {
        let change_pct = (candle.close - candle.open) / candle.open * 100.0;
        let sign = if change_pct >= 0.0 { "+" } else { "" };
        tooltip_lines.push(format!("Change: {sign}{change_pct:.2}%"));
    }

    if tooltip_lines.is_empty() {
        return;
    }

    let font_id = FontId::monospace(options.font_size);
    let line_height = options.font_size + DESIGN_TOKENS.spacing.sm + DESIGN_TOKENS.spacing.hairline;
    let padding = DESIGN_TOKENS.spacing.lg;

    let border_color = if candle.is_bullish() {
        options.border_color_bullish
    } else {
        options.border_color_bearish
    };

    // Calculate tooltip size
    let mut max_width = 0.0f32;
    for line in &tooltip_lines {
        let text_size = painter.text(
            Pos2::ZERO,
            egui::Align2::LEFT_TOP,
            line,
            font_id.clone(),
            Color32::TRANSPARENT,
        );
        max_width = max_width.max(text_size.width());
    }

    let tooltip_width = max_width + padding * 2.0;
    let tooltip_height = tooltip_lines.len() as f32 * line_height + padding * 2.0;

    // Pos tooltip (prefer right of cursor, fallback to left)
    let cursor_offset = DESIGN_TOKENS.sizing.tooltip.cursor_offset_x;
    let mut tooltip_x = hover_pos.x + cursor_offset;
    let mut tooltip_y = hover_pos.y - tooltip_height / 2.0;

    // Keep within bounds. Prefer the right of the cursor; flip to the left when
    // the right edge would clip, then clamp both axes so a flipped or oversized
    // tooltip never spills past the chart on any side.
    if tooltip_x + tooltip_width > price_rect.max.x {
        tooltip_x = hover_pos.x - tooltip_width - cursor_offset;
    }
    if tooltip_x < price_rect.min.x {
        tooltip_x = price_rect.min.x;
    }
    if tooltip_y < price_rect.min.y {
        tooltip_y = price_rect.min.y;
    }
    if tooltip_y + tooltip_height > price_rect.max.y {
        tooltip_y = price_rect.max.y - tooltip_height;
    }

    let tooltip_rect = Rect::from_min_size(
        Pos2::new(tooltip_x, tooltip_y),
        Vec2::new(tooltip_width, tooltip_height),
    );

    // Draw tooltip background with border
    painter.rect_filled(
        tooltip_rect,
        DESIGN_TOKENS.rounding.sm,
        options.background_color,
    );
    painter.rect_stroke(
        tooltip_rect,
        DESIGN_TOKENS.rounding.sm,
        Stroke::new(DESIGN_TOKENS.spacing.xs, border_color),
        StrokeKind::Outside,
    );

    // Draw tooltip text
    for (i, line) in tooltip_lines.iter().enumerate() {
        let text_pos = Pos2::new(
            tooltip_rect.min.x + padding,
            tooltip_rect.min.y + padding + i as f32 * line_height,
        );
        painter.text(
            text_pos,
            egui::Align2::LEFT_TOP,
            line,
            font_id.clone(),
            options.text_color,
        );
    }
}

/// Renders tracking tooltip as horizontal bar at top of chart
/// Shows: yy-MM-dd HH:mm  O xxx  H xxx  L xxx  C xxx  Vol xxx  +xx.xx(+x.xx%)
/// 标题字母（O/H/L/C/Vol）保持原色；数值和涨跌幅根据涨跌着色（涨红跌绿）
///
/// `prev_close`：前一根K线收盘价，用于计算基于前收的涨跌额和涨跌幅，
/// 传 None 时降级为以当根开盘价作基准
// Claude Opus 4.8 AI，修改于 2026 年 07 月 08 日。逻辑：
// 新增 prev_close 参数，使涨跌幅口径与第一行（render_legend）一致（相对前收），
// 同时补充涨跌额绝对值显示（格式：+x.xx(+x.xx%)）
pub fn render_tracking_tooltip(
    painter: &Painter,
    chart_rect: Rect,
    candle: &Bar,
    options: &TooltipOptions,
    prev_close: Option<f64>,
) {
    let bar_height = options.tracking_bar_height;
    let bar_rect = Rect::from_min_size(chart_rect.min, Vec2::new(chart_rect.width(), bar_height));

    // 绘制横条背景
    painter.rect_filled(bar_rect, 0.0, options.tracking_bar_background);

    let font_id = FontId::proportional(options.font_size);
    let text_y = bar_rect.center().y;
    // 当前 x 光标，每次渲染后右移
    let mut x = bar_rect.min.x + 10.0;

    // 涨跌颜色：收盘≥开盘为涨（红），否则为跌（绿）
    let is_bullish = candle.close >= candle.open;
    let val_color = if is_bullish {
        options.border_color_bullish
    } else {
        options.border_color_bearish
    };

    // 渲染一段文字，返回文字矩形的右边界 x
    macro_rules! draw_seg {
        ($text:expr, $color:expr) => {{
            painter
                .text(
                    Pos2::new(x, text_y),
                    egui::Align2::LEFT_CENTER,
                    $text,
                    font_id.clone(),
                    $color,
                )
                .max
                .x
        }};
    }

    // 时间日期：统一格式 yy-MM-dd HH:mm（始终包含日期）
    if options.show_time {
        let time_str = candle.time.format("%y-%m-%d %H:%M").to_string();
        x = draw_seg!(&time_str, options.text_color) + 12.0;
    }

    // OHLC：标题用原色，数值用涨跌色
    if options.show_ohlc {
        let precision = options.price_precision.min(4);

        x = draw_seg!("O ", options.text_color);
        x = draw_seg!(&format!("{:.precision$}", candle.open), val_color) + 10.0;

        x = draw_seg!("H ", options.text_color);
        x = draw_seg!(&format!("{:.precision$}", candle.high), val_color) + 10.0;

        x = draw_seg!("L ", options.text_color);
        x = draw_seg!(&format!("{:.precision$}", candle.low), val_color) + 10.0;

        x = draw_seg!("C ", options.text_color);
        x = draw_seg!(&format!("{:.precision$}", candle.close), val_color) + 10.0;
    }

    // Vol：标题用原色，数值用涨跌色
    if options.show_volume {
        x = draw_seg!("Vol ", options.text_color);
        x = draw_seg!(&format!("{:.0}", candle.volume), val_color) + 10.0;
    }

    // 涨跌额 + 涨跌幅：以前一根K线收盘价（prev_close）为基准，与第一行口径一致；
    // prev_close 未传入时降级为当根开盘价
    // Claude Opus 4.8 AI，修改于 2026 年 07 月 08 日。逻辑：
    // 原先用 (close - open) / open，与第一行（relative to prev_close）口径不同导致数值差异，
    // 且缺少绝对涨跌额，现统一改为相对前收计算，并补充涨跌额
    if options.show_change {
        let reference = prev_close.unwrap_or(candle.open);
        let change = candle.close - reference;
        let change_pct = if reference != 0.0 {
            (change / reference) * 100.0
        } else {
            0.0
        };
        let sign = if change >= 0.0 { "+" } else { "" };
        let change_color = if change >= 0.0 {
            options.border_color_bullish
        } else {
            options.border_color_bearish
        };
        // 格式：+x.xx(+x.xx%) 与第一行 render_legend 保持一致
        let precision = options.price_precision.min(4);
        let change_text = format!("{sign}{change:.precision$}({sign}{change_pct:.2}%)");
        draw_seg!(&change_text, change_color);
    }
}

/// Renders magnifier tooltip as circular zoom lens
/// Shows enlarged view of candles around cursor position
pub fn render_magnifier_tooltip(
    context: &RenderContext,
    hover_pos: Pos2,
    _candle: &Bar,
    options: &TooltipOptions,
    price_scale: &LinearPriceMap,
    coords: &ChartMapping,
    visible_data: &[Bar],
) {
    let zoom = options.magnifier_zoom;
    let size = options.magnifier_size;
    let radius = size / 2.0;

    // Pos magnifier (centered on cursor, but kept in bounds)
    let mut center = hover_pos;
    if center.x - radius < context.rect.min.x {
        center.x = context.rect.min.x + radius;
    }
    if center.x + radius > context.rect.max.x {
        center.x = context.rect.max.x - radius;
    }
    if center.y - radius < context.rect.min.y {
        center.y = context.rect.min.y + radius;
    }
    if center.y + radius > context.rect.max.y {
        center.y = context.rect.max.y - radius;
    }

    // Draw magnifier background (dark circle)
    let lens_bg = DESIGN_TOKENS.semantic.extended.chart_tooltip_bg;
    context.painter.circle_filled(center, radius, lens_bg);

    // Draw magnifier border
    let border_color = DESIGN_TOKENS.semantic.extended.chart_text_muted;
    context.painter.circle_stroke(
        center,
        radius,
        Stroke::new(DESIGN_TOKENS.stroke.thick, border_color),
    );

    // Calculate visible range in the magnifier
    // The magnifier shows a zoomed view centered on the hover position
    let src_width = size / zoom;
    let src_height = size / zoom;

    // Calculate the price range visible in magnifier. A zero-height rect yields
    // no price-per-pixel gradient rather than an infinite/NaN one.
    let rect_height = context.rect.height() as f64;
    let price_per_pixel = if rect_height.abs() < f64::EPSILON {
        0.0
    } else {
        price_scale.price_range() / rect_height
    };
    let center_price =
        price_scale.min_price + (context.rect.max.y - hover_pos.y) as f64 * price_per_pixel;

    let mag_price_range = src_height as f64 * price_per_pixel;
    let mag_min_price = center_price - mag_price_range / 2.0;
    let mag_max_price = center_price + mag_price_range / 2.0;

    // Calculate bar indices visible in magnifier. Degenerate bar spacing maps
    // every pixel onto the same bar rather than dividing by zero.
    let bars_per_pixel = if coords.bar_spacing.abs() < f32::EPSILON {
        0.0
    } else {
        1.0 / coords.bar_spacing
    };
    let center_bars_from_right =
        (context.rect.max.x - hover_pos.x) * bars_per_pixel - 0.5 - coords.right_offset;
    let src_bars = (src_width * bars_per_pixel) as isize;
    let half_bars = src_bars / 2;

    // Render candles within the magnifier clip region
    // Use clip rect to constrain drawing to the circle
    let clip_rect = Rect::from_center_size(center, Vec2::splat(size));

    // Draw zoomed candles
    let zoomed_bar_spacing = coords.bar_spacing * zoom;
    let candle_width = zoomed_bar_spacing * 0.7;

    for i in -half_bars..=half_bars {
        let bar_offset = center_bars_from_right + i as f32;
        let global_idx = (coords.base_idx as f32 - bar_offset).floor() as isize;

        if global_idx < coords.start_idx as isize {
            continue;
        }
        let local_idx = (global_idx as usize).saturating_sub(coords.start_idx);
        if local_idx >= visible_data.len() {
            continue;
        }

        let bar = &visible_data[local_idx];

        // Calculate position within magnifier
        let bar_x = center.x - (i as f32 * zoomed_bar_spacing);

        // Skip if outside magnifier bounds
        if bar_x < clip_rect.min.x - candle_width || bar_x > clip_rect.max.x + candle_width {
            continue;
        }

        // Map prices to magnifier coords
        let price_to_mag_y = |price: f64| -> f32 {
            let ratio = ((price - mag_min_price) / (mag_max_price - mag_min_price)) as f32;
            center.y + radius - ratio * size
        };

        let open_y = price_to_mag_y(bar.open);
        let close_y = price_to_mag_y(bar.close);
        let high_y = price_to_mag_y(bar.high);
        let low_y = price_to_mag_y(bar.low);

        let is_bullish = bar.close >= bar.open;
        let candle_color = if is_bullish {
            options.border_color_bullish
        } else {
            options.border_color_bearish
        };

        // Draw wick
        context.painter.line_segment(
            [Pos2::new(bar_x, high_y), Pos2::new(bar_x, low_y)],
            Stroke::new(DESIGN_TOKENS.stroke.hairline, candle_color),
        );

        // Draw body
        let body_top = open_y.min(close_y);
        let body_bottom = open_y.max(close_y);
        let body_height = (body_bottom - body_top).max(1.0);

        let body_rect = Rect::from_min_size(
            Pos2::new(bar_x - candle_width / 2.0, body_top),
            Vec2::new(candle_width, body_height),
        );

        if is_bullish {
            context.painter.rect_stroke(
                body_rect,
                0.0,
                Stroke::new(DESIGN_TOKENS.stroke.hairline, candle_color),
                StrokeKind::Inside,
            );
        } else {
            context.painter.rect_filled(body_rect, 0.0, candle_color);
        }
    }

    // Draw crosshair in center of magnifier
    let crosshair_color = DESIGN_TOKENS.semantic.chart.crosshair_line;
    context.painter.line_segment(
        [
            Pos2::new(center.x, center.y - radius * 0.3),
            Pos2::new(center.x, center.y + radius * 0.3),
        ],
        Stroke::new(DESIGN_TOKENS.stroke.hairline, crosshair_color),
    );
    context.painter.line_segment(
        [
            Pos2::new(center.x - radius * 0.3, center.y),
            Pos2::new(center.x + radius * 0.3, center.y),
        ],
        Stroke::new(DESIGN_TOKENS.stroke.hairline, crosshair_color),
    );

    // Draw zoom indicator
    let zoom_text = format!("{zoom:.1}x");
    context.painter.text(
        Pos2::new(center.x, center.y + radius - 12.0),
        egui::Align2::CENTER_BOTTOM,
        &zoom_text,
        FontId::proportional(typography::TINY),
        DESIGN_TOKENS.semantic.extended.disabled,
    );
}
