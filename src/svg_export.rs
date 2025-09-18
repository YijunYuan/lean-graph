use egui::Color32;
use petgraph::graph::NodeIndex;

use crate::{ConstCategory, G, NodePayload};

fn hex_rgb(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn color_from_payload(payload: &NodePayload, dark_mode: bool) -> Color32 {
    let base = payload.comp_color();
    let mapped = if dark_mode {
        [1.0 - base[0], 1.0 - base[1], 1.0 - base[2]]
    } else {
        [base[0].sqrt(), base[1].sqrt(), base[2].sqrt()]
    };
    Color32::from_rgb(
        (mapped[0] * 256.0).clamp(0.0, 255.0) as u8,
        (mapped[1] * 256.0).clamp(0.0, 255.0) as u8,
        (mapped[2] * 256.0).clamp(0.0, 255.0) as u8,
    )
}

fn polygon_points(n: usize, cx: f32, cy: f32, r: f32) -> String {
    use std::f32::consts::TAU;
    let step = TAU / n as f32;
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let ang = i as f32 * step;
        let x = cx + r * ang.cos();
        let y = cy + r * ang.sin();
        pts.push(format!("{},{}", x, y));
    }
    pts.join(" ")
}

struct SvgNode {
    idx: NodeIndex<u32>,
    x: f32,
    y: f32,
    radius: f32,
    label: String,
    category: ConstCategory,
    color: Color32,
    selected: bool,
    size: f32,
}

pub fn export_svg(g: &G, dark_mode: bool, margin: f32) -> String {
    // Collect nodes with geometry and colors
    let mut nodes: Vec<SvgNode> = Vec::new();
    for ni in g.g.node_indices() {
        let props = g.g[ni].props();
        let payload = g.g[ni].payload();
        let color = color_from_payload(payload, dark_mode);
        let radius = 10.0_f32 * payload.size;
        nodes.push(SvgNode {
            idx: ni,
            x: props.location.x,
            y: props.location.y,
            radius,
            label: payload.name.clone(),
            category: payload.const_category.clone(),
            color,
            selected: g.g[ni].selected(),
            size: payload.size,
        });
    }

    if nodes.is_empty() {
        return r#"<?xml version="1.0" encoding="UTF-8"?><svg xmlns="http://www.w3.org/2000/svg"/>"#.to_string();
    }

    // Compute bounds
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for n in &nodes {
        min_x = min_x.min(n.x - n.radius);
        min_y = min_y.min(n.y - n.radius * 2.5_f32); // include label above
        max_x = max_x.max(n.x + n.radius);
        max_y = max_y.max(n.y + n.radius);
    }
    min_x -= margin;
    min_y -= margin;
    max_x += margin;
    max_y += margin;
    let width = (max_x - min_x).max(1.0_f32);
    let height = (max_y - min_y).max(1.0_f32);

    // Fast lookup
    use std::collections::HashMap;
    let mut by_idx: HashMap<NodeIndex<u32>, usize> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        by_idx.insert(n.idx, i);
    }

    // Build SVG
    let mut svg = String::new();
    svg.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    svg.push('\n');
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" fill="none">"#,
        min_x, min_y, width, height
    ));
    svg.push('\n');

    // Parameters controlling arrow geometry
    let head_rel_width: f32 = 3.0_f32;     // head base is ~1.7x shaft width
    let desired_head_len: f32 = 24.0_f32;  // target head length in px
    let min_head_len_factor: f32 = 1.0_f32; // >= 1.0 * half_w
    let max_head_frac: f32 = 0.45_f32;     // head <= 45% of available length
    let neck_len_default: f32 = 0.6_f32;   // neck length in units of half_w

    // Edges as single filled 7-point polygons (shaft + neck + wide head)
    for ei in g.g.edge_indices() {
        if let Some((u, v)) = g.g.edge_endpoints(ei) {
            if u == v {
                // Skip loops for this straight-edge exporter
                continue;
            }
            let (su, sv) = (by_idx[&u], by_idx[&v]);
            let (a, b) = (&nodes[su], &nodes[sv]);

            // Center-line vector
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let d = (dx * dx + dy * dy).sqrt();
            if d < 1.0e-3_f32 {
                continue;
            }
            let ux = dx / d;
            let uy = dy / d;

            // Entry on start boundary and apex on end boundary
            let sx = a.x + ux * a.radius;
            let sy = a.y + uy * a.radius;
            let apex_x = b.x - ux * b.radius;
            let apex_y = b.y - uy * b.radius;

            // Available center-line length
            let avail = ((apex_x - sx).powi(2) + (apex_y - sy).powi(2)).sqrt();
            if avail < 0.5_f32 {
                continue;
            }

            // Style
            let fill_col = hex_rgb(a.color);
            let fill_opacity: f32 = if dark_mode { 0.2_f32 } else { 0.7_f32 };

            // Shaft thickness similar to in-app stroke
            let sw: f32 = (2.0_f32 * a.size.min(b.size)).max(1.0_f32);
            let half_w = sw * 0.5_f32;

            // Head length
            let min_head = (min_head_len_factor * half_w).max(4.0_f32);
            let max_head = (max_head_frac * avail).max(min_head);
            let mut tip_len = desired_head_len.clamp(min_head, max_head);

            // Neck length (small spacer between shaft end and head base)
            let mut neck_len = (neck_len_default * half_w).min((avail - tip_len) * 0.5_f32);
            if neck_len < 0.0_f32 {
                neck_len = 0.0_f32;
            }

            // Ensure visible shaft: keep at least half_w of shaft before the neck
            let min_shaft_len = half_w;
            if avail - (tip_len + neck_len) < min_shaft_len {
                let deficit = min_shaft_len - (avail - (tip_len + neck_len));
                // Reduce head first, then neck if needed
                let reduce_head = deficit.min(tip_len - min_head);
                tip_len -= reduce_head;
                let still = deficit - reduce_head;
                if still > 0.0_f32 {
                    neck_len = (neck_len - still).max(0.0_f32);
                }
            }

            // Derived centers
            let base_x = apex_x - ux * tip_len;          // center of head base
            let base_y = apex_y - uy * tip_len;
            let join_x = base_x - ux * neck_len;         // end of shaft (start of neck)
            let join_y = base_y - uy * neck_len;

            // Head base width (wider than shaft)
            let head_half = (half_w * head_rel_width).min(tip_len.max(half_w * 1.1_f32));

            // Perpendicular
            let nx = -uy;
            let ny = ux;

            // 7 vertices, clockwise:
            // 1) start_right
            let p1x = sx + nx * half_w;
            let p1y = sy + ny * half_w;
            // 2) neck_right (end of shaft)
            let p2x = join_x + nx * half_w;
            let p2y = join_y + ny * half_w;
            // 3) head_base_right (wider than shaft)
            let p3x = base_x + nx * head_half;
            let p3y = base_y + ny * head_half;
            // 4) apex
            let p4x = apex_x;
            let p4y = apex_y;
            // 5) head_base_left
            let p5x = base_x - nx * head_half;
            let p5y = base_y - ny * head_half;
            // 6) neck_left
            let p6x = join_x - nx * half_w;
            let p6y = join_y - ny * half_w;
            // 7) start_left
            let p7x = sx - nx * half_w;
            let p7y = sy - ny * half_w;

            // Build polygon
            let points = format!(
                "{:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3}",
                p1x, p1y, p2x, p2y, p3x, p3y, p4x, p4y, p5x, p5y, p6x, p6y, p7x, p7y
            );

            svg.push_str(&format!(
                r#"<polygon points="{}" fill="{}" fill-opacity="{}"/>"#,
                points, fill_col, fill_opacity
            ));
            svg.push('\n');
        }
    }

    // Nodes and labels
    let text_fill = if dark_mode { "#ffffff" } else { "#000000" };

    for n in &nodes {
        let fill = hex_rgb(n.color);
        match n.category {
            ConstCategory::Axiom => {
                svg.push_str(&format!(
                    r#"<circle cx="{}" cy="{}" r="{}" fill="{}"/>"#,
                    n.x, n.y, n.radius, fill
                ));
                svg.push('\n');
            }
            ConstCategory::Definition => {
                let pts = polygon_points(3, n.x, n.y, n.radius);
                svg.push_str(&format!(r#"<polygon points="{}" fill="{}"/>"#, pts, fill));
                svg.push('\n');
            }
            ConstCategory::Theorem => {
                let pts = polygon_points(5, n.x, n.y, n.radius);
                svg.push_str(&format!(r#"<polygon points="{}" fill="{}"/>"#, pts, fill));
                svg.push('\n');
            }
            ConstCategory::Other => {
                let pts = polygon_points(4, n.x, n.y, n.radius);
                svg.push_str(&format!(r#"<polygon points="{}" fill="{}"/>"#, pts, fill));
                svg.push('\n');
            }
        }

        // Label above the node
        let ty = n.y - 2.0_f32 * n.radius;
        let fs = n.radius; // approximate label size
        svg.push_str(&format!(
            r#"<text x="{}" y="{}" font-family="monospace" font-size="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
            n.x, ty, fs, text_fill, xml_escape(&n.label)
        ));
        svg.push('\n');
    }

    svg.push_str("</svg>\n");
    svg
}

// Basic XML escape for text content
fn xml_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            _ => c.to_string(),
        })
        .collect()
}