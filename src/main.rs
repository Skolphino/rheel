use eframe::egui;
use eframe::{App, Frame, NativeOptions};
use rand::{Rng, SeedableRng};
use rodio::{OutputStream, OutputStreamHandle, Sink, source::Source};
use serde::Deserialize;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::f32::consts::PI;
use std::fs;
use std::hash::{Hash, Hasher};
use std::time::Duration;

// --- CONFIGURATION STRUCTS ---

#[derive(Clone, Deserialize, Debug)]
struct SegmentConfig {
    label: String,
    weight: u32,
    color: Option<String>,
}

#[derive(Deserialize, Debug)]
struct AppConfig {
    spin_duration_ms: f32,
    center_color: Option<String>,
    center_radius_ratio: Option<f32>,
    winner_message: Option<String>,
    winner_font_size: Option<f32>,
    label_font_size: Option<f32>,
    show_segments_borders: Option<bool>,
    segments: Vec<SegmentConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            spin_duration_ms: 5000.0,
            center_color: Some("#202020".to_string()),
            center_radius_ratio: Some(0.25),
            winner_message: Some("Winner:\n{label}".to_string()),
            winner_font_size: Some(40.0),
            label_font_size: Some(20.0),
            show_segments_borders: Some(true),
            segments: vec![
                SegmentConfig {
                    label: "1".into(),
                    weight: 1,
                    color: None,
                },
                SegmentConfig {
                    label: "2".into(),
                    weight: 1,
                    color: None,
                },
                SegmentConfig {
                    label: "3".into(),
                    weight: 1,
                    color: None,
                },
                SegmentConfig {
                    label: "4".into(),
                    weight: 1,
                    color: None,
                },
                SegmentConfig {
                    label: "5".into(),
                    weight: 1,
                    color: None,
                },
            ],
        }
    }
}

// --- RUNTIME STRUCTS ---

struct ProcessedSegment {
    label: String,
    weight: u32,
    color: egui::Color32,
}

struct OverlayApp {
    // Spin animation
    rotation: f32,
    start_rotation: f32,
    target_rotation: f32,
    current_spin_time: f32,
    spin_duration_ms: f32,
    is_spinning: bool,

    // Audio
    _audio_stream: OutputStream,
    audio_handle: OutputStreamHandle,
    last_segment_index: Option<usize>,

    // Visuals
    center_color: egui::Color32,
    center_radius_ratio: f32,
    winner_template: String,
    winner_font_size: f32,
    label_font_size: f32,
    show_segments_borders: bool,

    // Data
    segments: Vec<ProcessedSegment>,
    total_weight: u32,
    winning_label: Option<String>,
}

impl OverlayApp {
    fn new(config: AppConfig) -> Self {
        let total_weight = config.segments.iter().map(|s| s.weight).sum();

        let segments = config
            .segments
            .into_iter()
            .map(|s| {
                let color = s
                    .color
                    .as_deref()
                    .and_then(parse_hex_color)
                    .unwrap_or_else(|| generate_deterministic_color(&s.label));

                ProcessedSegment {
                    label: s.label,
                    weight: s.weight,
                    color,
                }
            })
            .collect();

        let center_color = config
            .center_color
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(egui::Color32::from_gray(32));

        let center_radius_ratio = config.center_radius_ratio.unwrap_or(0.2).clamp(0.0, 0.8);

        // Process winner configuration
        let winner_template = config
            .winner_message
            .unwrap_or_else(|| "Winner:\n{label}".to_string());

        let winner_font_size = config.winner_font_size.unwrap_or(40.0);
        let label_font_size = config.label_font_size.unwrap_or(20.0);
        let show_segments_borders = config.show_segments_borders.unwrap_or(true);

        let mut rng = rand::rng();

        // Initialize Audio System
        let (_stream, stream_handle) =
            OutputStream::try_default().expect("Failed to initialize audio");

        Self {
            rotation: rng.random_range(0.0..2.0 * PI),
            start_rotation: 0.0,
            target_rotation: 0.0,
            current_spin_time: 0.0,
            spin_duration_ms: config.spin_duration_ms,
            is_spinning: false,

            _audio_stream: _stream,
            audio_handle: stream_handle,
            last_segment_index: None,

            center_color,
            center_radius_ratio,
            winner_template,
            winner_font_size,
            label_font_size,
            show_segments_borders,
            segments,
            total_weight,
            winning_label: None,
        }
    }

    fn start_spin(&mut self) {
        let mut rng = rand::rng();

        self.is_spinning = true;
        self.current_spin_time = 0.0;
        self.start_rotation = self.rotation;
        self.winning_label = None;
        self.last_segment_index = None;

        let extra_spins = rng.random_range(10.0..14.0);
        let random_offset = rng.random_range(0.0..2.0 * PI);

        self.target_rotation = self.rotation + extra_spins * 2.0 * PI + random_offset;
    }

    fn play_tick_sound(&self) {
        if let Ok(sink) = Sink::try_new(&self.audio_handle) {
            let mut rng = rand::rng();

            let pitch_jitter = rng.random_range(550.0..650.0);
            let volume_jitter = rng.random_range(0.0005..0.0015);

            let source = rodio::source::SineWave::new(pitch_jitter)
                .take_duration(Duration::from_millis(30))
                .amplify(volume_jitter);

            sink.append(source);
            sink.detach();
        }
    }

    fn get_current_segment_info(&self) -> (usize, &str, egui::Color32) {
        let normalized_rotation = self.rotation.rem_euclid(2.0 * PI);
        let pointer_angle = 1.5 * PI;

        let mut hit_angle = pointer_angle - normalized_rotation;
        hit_angle = hit_angle.rem_euclid(2.0 * PI);

        let mut cursor = 0.0;
        for (i, seg) in self.segments.iter().enumerate() {
            let width = (seg.weight as f32 / self.total_weight as f32) * 2.0 * PI;
            if hit_angle >= cursor && hit_angle < cursor + width {
                return (i, &seg.label, seg.color);
            }
            cursor += width;
        }

        let last_idx = self.segments.len() - 1;
        let last = &self.segments[last_idx];
        (last_idx, &last.label, last.color)
    }
}

impl App for OverlayApp {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Space)) && !self.is_spinning {
            self.start_spin();
        }

        let dt = ctx.input(|i| i.stable_dt).min(0.1);

        if self.is_spinning {
            self.current_spin_time += dt;
            let duration = self.spin_duration_ms / 1000.0;
            let t = (self.current_spin_time / duration).clamp(0.0, 1.0);

            let eased = 1.0 - (1.0 - t).powi(5);

            self.rotation =
                self.start_rotation + eased * (self.target_rotation - self.start_rotation);

            // --- AUDIO TRIGGER LOGIC ---

            let (current_index, label_text) = {
                let (idx, lbl, _) = self.get_current_segment_info();
                (idx, lbl.to_string())
            };

            if self.last_segment_index.is_none() {
                self.last_segment_index = Some(current_index);
            } else if let Some(last_index) = self.last_segment_index {
                if last_index != current_index {
                    self.play_tick_sound();
                    self.last_segment_index = Some(current_index);
                }
            }

            if t >= 1.0 {
                self.is_spinning = false;
                self.winning_label = Some(label_text);
            }

            ctx.request_repaint();
        }

        // --- DRAWING ---
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let center = rect.center();
                let outer_radius = 250.0;
                let inner_radius = outer_radius * self.center_radius_ratio;

                let (_, _, pointer_color) = self.get_current_segment_info();

                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if pos.distance(center) <= outer_radius
                        && ctx.input(|i| i.pointer.primary_clicked())
                        && !self.is_spinning
                    {
                        self.start_spin();
                    }
                }

                ui.painter().circle_filled(
                    center,
                    outer_radius + 5.0,
                    egui::Color32::from_black_alpha(220),
                );

                let mut angle = self.rotation;

                for seg in &self.segments {
                    let width = (seg.weight as f32 / self.total_weight as f32) * 2.0 * PI;
                    let end = angle + width;
                    let steps = (width * 15.0).max(3.0) as usize;
                    let mut points = vec![center];

                    for i in 0..=steps {
                        let a = angle + (i as f32 / steps as f32) * width;
                        points.push(egui::pos2(
                            center.x + outer_radius * a.cos(),
                            center.y + outer_radius * a.sin(),
                        ));
                    }

                    let stroke = if self.show_segments_borders {
                        egui::Stroke::new(1.0, egui::Color32::BLACK)
                    } else {
                        egui::Stroke::new(1.0, seg.color)
                    };

                    ui.painter()
                        .add(egui::Shape::convex_polygon(points, seg.color, stroke));

                    // Text drawing logic - skips if size is 0
                    if self.label_font_size > 0.0 {
                        let text_r = inner_radius + (outer_radius - inner_radius) * 0.5;
                        let text_a = angle + width * 0.5;
                        let text_pos = egui::pos2(
                            center.x + text_r * text_a.cos(),
                            center.y + text_r * text_a.sin(),
                        );

                        ui.painter().text(
                            text_pos,
                            egui::Align2::CENTER_CENTER,
                            &seg.label,
                            egui::FontId::proportional(self.label_font_size),
                            if is_bright(seg.color) {
                                egui::Color32::BLACK
                            } else {
                                egui::Color32::WHITE
                            },
                        );
                    }

                    angle = end;
                }

                ui.painter().circle(
                    center,
                    inner_radius,
                    self.center_color,
                    egui::Stroke::new(2.0, egui::Color32::BLACK),
                );

                ui.painter().add(egui::Shape::convex_polygon(
                    vec![
                        egui::pos2(center.x - 15.0, center.y - outer_radius - 20.0),
                        egui::pos2(center.x + 15.0, center.y - outer_radius - 20.0),
                        egui::pos2(center.x, center.y - outer_radius + 10.0),
                    ],
                    pointer_color,
                    egui::Stroke::new(2.0, egui::Color32::BLACK),
                ));

                if let Some(winner) = &self.winning_label {
                    ui.centered_and_justified(|ui| {
                        let message = self.winner_template.replace("{label}", winner);

                        ui.label(
                            egui::RichText::new(message)
                                .size(self.winner_font_size)
                                .strong()
                                .background_color(egui::Color32::from_black_alpha(200))
                                .color(egui::Color32::WHITE),
                        );
                    });
                }
            });
    }
}

// --- HELPERS ---

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> egui::Color32 {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    egui::Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn generate_deterministic_color(seed: &str) -> egui::Color32 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let mut rng = rand::rngs::StdRng::seed_from_u64(hasher.finish());
    hsv_to_rgb(
        rng.random_range(0.0..360.0),
        rng.random_range(0.7..0.9),
        rng.random_range(0.8..0.95),
    )
}

fn parse_hex_color(hex: &str) -> Option<egui::Color32> {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        Some(egui::Color32::from_rgb(
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        ))
    } else {
        None
    }
}

fn is_bright(c: egui::Color32) -> bool {
    (0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32) > 128.0
}

fn load_config() -> AppConfig {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return AppConfig::default();
    }
    fs::read_to_string(&args[1])
        .ok()
        .and_then(|c| toml::from_str(&c).ok())
        .unwrap_or_default()
}

fn main() -> eframe::Result<()> {
    let config = load_config();
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_inner_size([600.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rheel",
        options,
        Box::new(|_| Ok(Box::new(OverlayApp::new(config)))),
    )
}
