use super::context::{ChartMapping, LinearPriceMap, RenderContext};
use crate::config::{CrosshairLineStyle, CrosshairMode, CrosshairStyle};
use crate::model::Bar;
use crate::styles::typography;
use crate::tokens::DESIGN_TOKENS;
use egui::{Color32, FontId, Pos2, Rect, Stroke};

/// Renders crosshair with full customization: style, mode, color, line width, line style
pub fn render_crosshair_full(
    context: &RenderContext,
    hover_pos: Pos2,
    visible_data: &[Bar],
    price_scale: &LinearPriceMap,
    coords: &ChartMapping,
    mode: CrosshairMode,
    style: CrosshairStyle,
    crosshair_color: Color32,
    line_width: f32,
    line_style: CrosshairLineStyle,
) {
    // Arrow style = no crosshair at all (just standard mouse cursor)
    if style == CrosshairStyle::Arrow {
        return;
    }

    let label_bg = DESIGN_TOKENS.semantic.extended.chart_crosshair_label_bg;

    // Claude Opus 4.8 AI，修复于 2026 年 07 月 07 日。逻辑：
    // 原代码同样使用手动公式计算 bar index（缺少 -1.0 + 用 floor），
    // 导致悬停在 bar 41 中心时 t_idx = 40，Magnet 模式吸附到错误的 bar 40，
    // 改用 local_idx_at_x 确保悬停在哪根 bar 就吸附到哪根 bar
    let (actual_hover_x, actual_hover_y) = if let Some(candle_idx) = coords.local_idx_at_x(hover_pos.x, visible_data.len()) {
        let candle = &visible_data[candle_idx];
        let global_idx = coords.start_idx + candle_idx;

        match mode {
            CrosshairMode::Normal => (hover_pos.x, hover_pos.y),
            CrosshairMode::Magnet => {
                // Snap to nearest candle X position using ChartCoords helper
                let candle_x = coords.idx_to_x(global_idx);

                // Snap to nearest OHLC price
                let snap_price =
                    snap_to_nearest_ohlc(hover_pos.y, context.rect, price_scale, candle);
                let snap_y = price_scale.price_to_y(snap_price, context.rect);

                (candle_x, snap_y)
            }
        }
    } else {
        (hover_pos.x, hover_pos.y)
    };

    let actual_pos = Pos2::new(actual_hover_x, actual_hover_y);

    // Render based on style
    match style {
        CrosshairStyle::Full => {
            // Full crosshair with vertical and horizontal lines
            match line_style {
                CrosshairLineStyle::Solid => {
                    // Solid vertical line
                    context.painter.line_segment(
                        [
                            Pos2::new(actual_pos.x, context.rect.min.y),
                            Pos2::new(actual_pos.x, context.rect.max.y),
                        ],
                        Stroke::new(line_width, crosshair_color),
                    );
                    // Solid horizontal line
                    context.painter.line_segment(
                        [
                            Pos2::new(context.rect.min.x, actual_pos.y),
                            Pos2::new(context.rect.max.x, actual_pos.y),
                        ],
                        Stroke::new(line_width, crosshair_color),
                    );
                }
                CrosshairLineStyle::Dashed => {
                    let dash_pattern = [
                        DESIGN_TOKENS.spacing.sm + 1.0,
                        DESIGN_TOKENS.spacing.xs + 1.0,
                    ];
                    draw_dashed_vline(
                        context.painter,
                        actual_pos.x,
                        context.rect.y_range(),
                        crosshair_color,
                        line_width,
                        &dash_pattern,
                    );
                    draw_dashed_hline(
                        context.painter,
                        context.rect.x_range(),
                        actual_pos.y,
                        crosshair_color,
                        line_width,
                        &dash_pattern,
                    );
                }
                CrosshairLineStyle::Dotted => {
                    let dotted_pattern = [DESIGN_TOKENS.spacing.xs, DESIGN_TOKENS.spacing.xs];
                    draw_dashed_vline(
                        context.painter,
                        actual_pos.x,
                        context.rect.y_range(),
                        crosshair_color,
                        line_width,
                        &dotted_pattern,
                    );
                    draw_dashed_hline(
                        context.painter,
                        context.rect.x_range(),
                        actual_pos.y,
                        crosshair_color,
                        line_width,
                        &dotted_pattern,
                    );
                }
            }
        }
        CrosshairStyle::Dot => {
            // Just a dot at the intersection point
            context.painter.circle_filled(
                actual_pos,
                DESIGN_TOKENS.sizing.crosshair.dot_radius,
                crosshair_color,
            );
        }
        CrosshairStyle::Arrow => {
            // Already handled by early return above
            unreachable!()
        }
    }

    // Calculate price at cursor
    let price_ratio = (context.rect.max.y - actual_pos.y) / context.rect.height();
    let price = price_scale.min_price + price_ratio as f64 * price_scale.price_range();

    // Price label on Y-axis with "+" alert indicator
    let price_label = format!("{price:.3}");
    let price_pos = Pos2::new(
        context.rect.max.x + DESIGN_TOKENS.sizing.crosshair.label_offset_x,
        actual_pos.y,
    );

    // Draw label background
    let text_size = context.painter.text(
        price_pos,
        egui::Align2::LEFT_CENTER,
        &price_label,
        FontId::proportional(typography::XS),
        Color32::TRANSPARENT,
    );

    // Extend background to fit "+" symbol
    let extended_bg = text_size.expand2(egui::Vec2::new(
        DESIGN_TOKENS.sizing.crosshair.plus_symbol_spacing,
        0.0,
    )); // Extra space for "+"
    context.painter.rect_filled(
        extended_bg,
        DESIGN_TOKENS.sizing.crosshair.label_rounding,
        label_bg,
    );

    // Draw price text
    context.painter.text(
        price_pos,
        egui::Align2::LEFT_CENTER,
        price_label,
        FontId::proportional(typography::XS),
        Color32::WHITE,
    );

    // Draw "+" alert indicator
    let plus_pos = Pos2::new(text_size.max.x + 4.0, actual_pos.y);
    context.painter.text(
        plus_pos,
        egui::Align2::LEFT_CENTER,
        "+",
        FontId::proportional(typography::SM),
        DESIGN_TOKENS.semantic.extended.chart_text_muted,
    );

    // Claude Opus 4.8 AI，修复于 2026 年 07 月 07 日。逻辑：
    // 原代码手动计算 bar index 时缺少 idx_to_x 公式中的 -1.0 偏移，且用 floor 而非 round，
    // 导致悬停在 bar 中心时 t_float = idx - 1/bar_spacing（如 40.98），floor 后错误地显示前一根 bar 的时间；
    // 改用 ChartMapping::local_idx_at_x，它正确实现了 x_to_idx 的逆运算（含 -1.0 + round），
    // 确保悬停在 bar 41 中心时显示 bar 41 的时间，而非 bar 40
    if let Some(candle_idx) = coords.local_idx_at_x(hover_pos.x, visible_data.len()) {
        let candle = &visible_data[candle_idx];

        // Detect timeframe based on bar duration to choose appropriate format
        let time_label = if visible_data.len() >= 2 {
            let time_diff = visible_data[1]
                .time
                .signed_duration_since(visible_data[0].time);
            let seconds = time_diff.num_seconds().abs();

            if seconds < 1 {
                // Sub-second: show milliseconds
                candle.time.format("%H:%M:%S%.3f").to_string()
            } else if seconds < 60 {
                // Seconds: show time with seconds
                candle.time.format("%H:%M:%S").to_string()
            } else if seconds < 3600 {
                // Minutes: show time
                candle.time.format("%H:%M").to_string()
            } else if seconds < 86400 {
                // Hours: show date and time
                candle.time.format("%b %d %H:%M").to_string()
            } else {
                // Daily or longer: show date only
                candle.time.format("%b %d, %Y").to_string()
            }
        } else {
            candle.time.format("%H:%M:%S").to_string()
        };

        let time_pos = Pos2::new(actual_pos.x, context.rect.max.y + 5.0);

        // Draw label background
        let text_size = context.painter.text(
            time_pos,
            egui::Align2::CENTER_TOP,
            &time_label,
            FontId::proportional(typography::XS),
            Color32::TRANSPARENT,
        );
        context.painter.rect_filled(text_size, 2.0, label_bg);

        // Draw time text
        context.painter.text(
            time_pos,
            egui::Align2::CENTER_TOP,
            time_label,
            FontId::proportional(typography::XS),
            Color32::WHITE,
        );
    }
}

/// Snap to nearest OHLC price (for Magnet mode)
fn snap_to_nearest_ohlc(
    hover_y: f32,
    price_rect: Rect,
    price_scale: &LinearPriceMap,
    candle: &Bar,
) -> f64 {
    // Get Y positions of all OHLC values using LinearPriceMap helper
    let open_y = price_scale.price_to_y(candle.open, price_rect);
    let high_y = price_scale.price_to_y(candle.high, price_rect);
    let low_y = price_scale.price_to_y(candle.low, price_rect);
    let close_y = price_scale.price_to_y(candle.close, price_rect);

    // Find the nearest one
    let candidates = [
        (open_y, candle.open),
        (high_y, candle.high),
        (low_y, candle.low),
        (close_y, candle.close),
    ];

    let mut nearest_price = candle.close;
    let mut min_distance = f32::MAX;

    for (y, price) in candidates {
        let distance = (hover_y - y).abs();
        if distance < min_distance {
            min_distance = distance;
            nearest_price = price;
        }
    }

    nearest_price
}

/// Draw dashed vertical line
fn draw_dashed_vline(
    painter: &egui::Painter,
    x: f32,
    y_range: egui::Rangef,
    color: Color32,
    width: f32,
    pattern: &[f32], // [dash_len, gap_len, ...]
) {
    let mut y = y_range.min;
    let y_end = y_range.max;
    let mut pattern_idx = 0;
    let mut is_dash = true;

    while y < y_end {
        let segment_len = pattern[pattern_idx % pattern.len()];
        let next_y = (y + segment_len).min(y_end);

        if is_dash {
            painter.line_segment(
                [Pos2::new(x, y), Pos2::new(x, next_y)],
                Stroke::new(width, color),
            );
        }

        y = next_y;
        pattern_idx += 1;
        is_dash = !is_dash;
    }
}

/// Draw dashed horizontal line
fn draw_dashed_hline(
    painter: &egui::Painter,
    x_range: egui::Rangef,
    y: f32,
    color: Color32,
    width: f32,
    pattern: &[f32],
) {
    let mut x = x_range.min;
    let x_end = x_range.max;
    let mut pattern_idx = 0;
    let mut is_dash = true;

    while x < x_end {
        let segment_len = pattern[pattern_idx % pattern.len()];
        let next_x = (x + segment_len).min(x_end);

        if is_dash {
            painter.line_segment(
                [Pos2::new(x, y), Pos2::new(next_x, y)],
                Stroke::new(width, color),
            );
        }

        x = next_x;
        pattern_idx += 1;
        is_dash = !is_dash;
    }
}
