use earcut::{Earcut, deviation};
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2,
};

#[path = "../tests/fixtures/mod.rs"]
mod fixtures;

const INDEX_LABEL_LIMIT: usize = 3000;

#[derive(Clone, Copy)]
struct VertexRef {
    ring: usize,
    point: usize,
}

struct Palette {
    background: Color32,
    border: Color32,
    fill: Color32,
    mesh: Color32,
    outer_ring: Color32,
    hole_ring: Color32,
    vertex: Color32,
    index_label: Color32,
    steiner_fill: Color32,
    steiner_outline: Color32,
    nearest_fill: Color32,
    nearest_outline: Color32,
    nearest_label: Color32,
}

impl Palette {
    fn dark() -> Self {
        Self {
            background: Color32::from_rgb(18, 20, 23),
            border: Color32::from_gray(48),
            fill: Color32::from_rgba_unmultiplied(79, 143, 220, 58),
            mesh: Color32::from_rgba_unmultiplied(126, 181, 246, 135),
            outer_ring: Color32::from_rgb(91, 211, 135),
            hole_ring: Color32::from_rgb(238, 177, 88),
            vertex: Color32::from_rgb(244, 244, 246),
            index_label: Color32::from_gray(210),
            steiner_fill: Color32::from_rgb(255, 92, 92),
            steiner_outline: Color32::from_rgb(255, 255, 255),
            nearest_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 230),
            nearest_outline: Color32::from_rgb(255, 218, 112),
            nearest_label: Color32::from_rgb(255, 238, 190),
        }
    }

    fn light() -> Self {
        Self {
            background: Color32::from_rgb(248, 248, 250),
            border: Color32::from_gray(180),
            fill: Color32::from_rgba_unmultiplied(60, 130, 200, 50),
            mesh: Color32::from_rgba_unmultiplied(40, 100, 200, 180),
            outer_ring: Color32::from_rgb(40, 150, 80),
            hole_ring: Color32::from_rgb(200, 130, 30),
            vertex: Color32::from_rgb(40, 40, 45),
            index_label: Color32::from_gray(80),
            steiner_fill: Color32::from_rgb(220, 60, 60),
            steiner_outline: Color32::from_rgb(40, 40, 45),
            nearest_fill: Color32::from_rgba_unmultiplied(40, 40, 45, 230),
            nearest_outline: Color32::from_rgb(220, 150, 30),
            nearest_label: Color32::from_rgb(110, 70, 20),
        }
    }

    fn for_visuals(v: &egui::Visuals) -> Self {
        if v.dark_mode {
            Self::dark()
        } else {
            Self::light()
        }
    }
}

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 820.0]),
        ..Default::default()
    };
    eframe::run_native(
        "earcut visualizer",
        native_options,
        Box::new(|_| Ok(Box::new(Visualizer::new()))),
    )
}

struct Visualizer {
    selected: usize,
    rings: Vec<Vec<[f64; 2]>>,
    triangulation: Triangulation,
    view_bbox: Bbox,
    active_vertex: Option<VertexRef>,
    show_fill: bool,
    show_mesh: bool,
    show_rings: bool,
    show_points: bool,
    show_indices: bool,
    show_steiner: bool,
    zoom: f32,
    pan: Vec2,
}

impl Visualizer {
    fn new() -> Self {
        let selected = fixtures::FIXTURES
            .iter()
            .position(|(name, _)| *name == "building")
            .unwrap_or(0);
        let rings = rings_of(selected);
        let triangulation = Triangulation::new(fixtures::FIXTURES[selected].0, &rings);
        let view_bbox = triangulation.bbox;
        Self {
            selected,
            rings,
            triangulation,
            view_bbox,
            active_vertex: None,
            show_fill: true,
            show_mesh: true,
            show_rings: true,
            show_points: false,
            show_indices: false,
            show_steiner: true,
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }

    fn select_fixture(&mut self, index: usize) {
        self.selected = index;
        self.rings = rings_of(index);
        self.retriangulate();
        self.view_bbox = self.triangulation.bbox;
        self.active_vertex = None;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    fn retriangulate(&mut self) {
        self.triangulation = Triangulation::new(fixtures::FIXTURES[self.selected].0, &self.rings);
    }

    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let palette = Palette::for_visuals(ui.visuals());
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
        let rect = response.rect;

        painter.rect_filled(rect, 0.0, palette.background);
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, palette.border),
            StrokeKind::Inside,
        );

        if self.triangulation.data.is_empty() {
            return;
        }

        let mut to_screen = Transform::new(rect, self.view_bbox, self.zoom, self.pan);
        let pointer = ui.input(|input| input.pointer.hover_pos());
        let nearest = pointer
            .filter(|_| response.hovered() || self.active_vertex.is_some())
            .and_then(|p| self.nearest_vertex(&to_screen, p));

        if response.drag_started() {
            self.active_vertex = nearest.map(|(v, _, _)| v);
        }
        if self.active_vertex.is_none() && response.dragged() {
            self.pan += response.drag_delta();
            to_screen = Transform::new(rect, self.view_bbox, self.zoom, self.pan);
        }
        if let (Some(vertex), Some(pointer)) = (self.active_vertex, pointer)
            && ui.input(|input| input.pointer.primary_down())
        {
            let world = to_screen.screen_to_world(pointer);
            self.move_vertex(vertex, world);
            self.retriangulate();
            to_screen = Transform::new(rect, self.view_bbox, self.zoom, self.pan);
        }
        if response.drag_stopped() {
            self.active_vertex = None;
        }

        ui.input(|input| {
            if !response.hovered() {
                return;
            }
            let scroll = input.smooth_scroll_delta.y;
            let Some(p) = input.pointer.hover_pos() else {
                return;
            };
            if scroll == 0.0 {
                return;
            }
            let world = to_screen.screen_to_world(p);
            self.zoom = (self.zoom * (scroll * 0.0015).exp()).clamp(0.02, 200.0);
            to_screen = Transform::new(rect, self.view_bbox, self.zoom, self.pan);
            self.pan += p - to_screen.point(world);
            to_screen = Transform::new(rect, self.view_bbox, self.zoom, self.pan);
        });

        self.paint_geometry(&painter, &to_screen, &palette);

        let active = self
            .active_vertex
            .and_then(|v| self.vertex_pos(v, &to_screen).map(|p| (v, p)));
        if let Some((vertex, pos)) = active.or_else(|| nearest.map(|(v, p, _)| (v, p))) {
            paint_nearest(&painter, pos, self.global_index(vertex), &palette);
        }
    }

    fn paint_geometry(&self, painter: &egui::Painter, to_screen: &Transform, palette: &Palette) {
        if self.show_fill {
            let mut mesh = egui::epaint::Mesh::default();
            for p in &self.triangulation.data {
                mesh.colored_vertex(to_screen.point(*p), palette.fill);
            }
            mesh.indices
                .extend_from_slice(&self.triangulation.triangles);
            painter.add(Shape::mesh(mesh));
        }

        if self.show_mesh {
            let stroke = Stroke::new(0.8, palette.mesh);
            for tri in self.triangulation.triangles.chunks_exact(3) {
                let p = |i: u32| to_screen.point(self.triangulation.data[i as usize]);
                let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
                painter.line_segment([a, b], stroke);
                painter.line_segment([b, c], stroke);
                painter.line_segment([c, a], stroke);
            }
        }

        if self.show_rings {
            for (i, ring) in self.rings.iter().enumerate() {
                let color = if i == 0 {
                    palette.outer_ring
                } else {
                    palette.hole_ring
                };
                draw_ring(painter, ring, to_screen, Stroke::new(2.0, color));
            }
        }

        let labels_visible =
            self.show_indices && self.triangulation.data.len() <= INDEX_LABEL_LIMIT;
        if self.show_points || labels_visible {
            for (i, p) in self.triangulation.data.iter().enumerate() {
                let pos = to_screen.point(*p);
                if self.show_points {
                    painter.circle_filled(pos, 2.5, palette.vertex);
                }
                if labels_visible {
                    painter.text(
                        pos + Vec2::new(4.0, -4.0),
                        Align2::LEFT_BOTTOM,
                        i.to_string(),
                        FontId::monospace(10.0),
                        palette.index_label,
                    );
                }
            }
        }

        if self.show_steiner {
            let outline = Stroke::new(1.0, palette.steiner_outline);
            for ring in self.rings.iter().skip(1).filter(|r| r.len() == 1) {
                let pos = to_screen.point(ring[0]);
                painter.circle_filled(pos, 3.0, palette.steiner_fill);
                painter.circle_stroke(pos, 3.5, outline);
            }
        }
    }

    /// Returns the closest vertex within click radius along with its screen
    /// position and global (flat) index.
    fn nearest_vertex(
        &self,
        to_screen: &Transform,
        pointer: Pos2,
    ) -> Option<(VertexRef, Pos2, usize)> {
        let max_distance_sq = 12.0f32.powi(2);
        let mut best: Option<(VertexRef, Pos2, usize, f32)> = None;
        let mut global = 0usize;
        for (ring, points) in self.rings.iter().enumerate() {
            for (point, p) in points.iter().enumerate() {
                let pos = to_screen.point(*p);
                let d = pos.distance_sq(pointer);
                if d <= max_distance_sq && best.as_ref().is_none_or(|b| d < b.3) {
                    best = Some((VertexRef { ring, point }, pos, global, d));
                }
                global += 1;
            }
        }
        best.map(|(v, pos, idx, _)| (v, pos, idx))
    }

    fn move_vertex(&mut self, vertex: VertexRef, point: [f64; 2]) {
        let Some(ring) = self.rings.get_mut(vertex.ring) else {
            return;
        };
        if vertex.point >= ring.len() {
            return;
        }
        let last = ring.len().saturating_sub(1);
        let closed = ring.len() > 1 && ring[0] == ring[last];
        ring[vertex.point] = point;
        if closed && (vertex.point == 0 || vertex.point == last) {
            ring[if vertex.point == 0 { last } else { 0 }] = point;
        }
    }

    fn vertex_pos(&self, vertex: VertexRef, to_screen: &Transform) -> Option<Pos2> {
        self.rings
            .get(vertex.ring)?
            .get(vertex.point)
            .map(|p| to_screen.point(*p))
    }

    fn global_index(&self, vertex: VertexRef) -> usize {
        self.rings
            .iter()
            .take(vertex.ring)
            .map(Vec::len)
            .sum::<usize>()
            + vertex.point
    }
}

impl eframe::App for Visualizer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("fixtures")
            .resizable(true)
            .default_size(210.0)
            .show_inside(ui, |ui| {
                ui.heading("Fixtures");
                ui.add_space(6.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, (name, _)) in fixtures::FIXTURES.iter().enumerate() {
                        if ui.selectable_label(i == self.selected, *name).clicked() {
                            self.select_fixture(i);
                        }
                    }
                });
            });

        egui::Panel::right("controls")
            .resizable(false)
            .default_size(190.0)
            .show_inside(ui, |ui| {
                ui.heading("View");
                ui.add_space(6.0);
                egui::widgets::global_theme_preference_buttons(ui);
                ui.add_space(4.0);
                ui.checkbox(&mut self.show_fill, "fill triangles");
                ui.checkbox(&mut self.show_mesh, "triangle mesh");
                ui.checkbox(&mut self.show_rings, "rings");
                ui.checkbox(&mut self.show_points, "vertices");
                ui.checkbox(&mut self.show_indices, "vertex indices");
                ui.checkbox(&mut self.show_steiner, "steiner points");
                ui.add(
                    egui::Slider::new(&mut self.zoom, 0.02..=50.0)
                        .logarithmic(true)
                        .text("zoom"),
                );
                if ui.button("Reset view").clicked() {
                    self.view_bbox = self.triangulation.bbox;
                    self.zoom = 1.0;
                    self.pan = Vec2::ZERO;
                }

                ui.separator();
                ui.heading("Stats");
                ui.add_space(6.0);
                let t = &self.triangulation;
                let steiner_count = self.rings.iter().skip(1).filter(|r| r.len() == 1).count();
                ui.label(format!("name: {}", t.name));
                ui.label(format!("rings: {}", self.rings.len()));
                ui.label(format!("holes: {}", t.hole_indices.len()));
                ui.label(format!("steiner: {steiner_count}"));
                ui.label(format!("vertices: {}", t.data.len()));
                ui.label(format!("triangles: {}", t.triangles.len() / 3));
                ui.label(format!("deviation: {:.6e}", t.deviation));
                if self.show_indices && t.data.len() > INDEX_LABEL_LIMIT {
                    ui.label(format!("indices hidden above {INDEX_LABEL_LIMIT} vertices"));
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| self.draw_canvas(ui));
    }
}

struct Triangulation {
    name: &'static str,
    data: Vec<[f64; 2]>,
    hole_indices: Vec<u32>,
    triangles: Vec<u32>,
    deviation: f64,
    bbox: Bbox,
}

impl Triangulation {
    fn new(name: &'static str, rings: &[Vec<[f64; 2]>]) -> Self {
        let data: Vec<[f64; 2]> = rings.iter().flat_map(|r| r.iter().copied()).collect();
        let hole_indices: Vec<u32> = rings
            .iter()
            .take(rings.len().saturating_sub(1))
            .scan(0u32, |sum, r| {
                *sum += r.len() as u32;
                Some(*sum)
            })
            .collect();

        let mut triangles = Vec::new();
        Earcut::new().earcut(data.iter().copied(), &hole_indices, &mut triangles);
        let dev = if triangles.is_empty() {
            0.0
        } else {
            deviation(data.iter().copied(), &hole_indices, &triangles)
        };
        let bbox = Bbox::from_points(&data);
        Self {
            name,
            data,
            hole_indices,
            triangles,
            deviation: dev,
            bbox,
        }
    }
}

#[derive(Clone, Copy)]
struct Bbox {
    min: [f64; 2],
    max: [f64; 2],
}

impl Bbox {
    fn from_points(points: &[[f64; 2]]) -> Self {
        let mut b = Self {
            min: [f64::INFINITY; 2],
            max: [f64::NEG_INFINITY; 2],
        };
        for &[x, y] in points {
            b.min = [b.min[0].min(x), b.min[1].min(y)];
            b.max = [b.max[0].max(x), b.max[1].max(y)];
        }
        b
    }
}

struct Transform {
    rect: Rect,
    center: [f64; 2],
    scale: f32,
    pan: Vec2,
}

impl Transform {
    fn new(rect: Rect, bbox: Bbox, zoom: f32, pan: Vec2) -> Self {
        let size = [
            (bbox.max[0] - bbox.min[0]).max(1.0) as f32,
            (bbox.max[1] - bbox.min[1]).max(1.0) as f32,
        ];
        let margin = 32.0;
        let sx = ((rect.width() - margin * 2.0).max(1.0) / size[0]).max(0.0001);
        let sy = ((rect.height() - margin * 2.0).max(1.0) / size[1]).max(0.0001);
        Self {
            rect,
            center: [
                (bbox.min[0] + bbox.max[0]) * 0.5,
                (bbox.min[1] + bbox.max[1]) * 0.5,
            ],
            scale: sx.min(sy) * zoom,
            pan,
        }
    }

    fn point(&self, [x, y]: [f64; 2]) -> Pos2 {
        self.rect.center()
            + self.pan
            + Vec2::new(
                ((x - self.center[0]) as f32) * self.scale,
                -((y - self.center[1]) as f32) * self.scale,
            )
    }

    fn screen_to_world(&self, pos: Pos2) -> [f64; 2] {
        let d = pos - self.rect.center() - self.pan;
        [
            self.center[0] + (d.x / self.scale) as f64,
            self.center[1] - (d.y / self.scale) as f64,
        ]
    }
}

fn draw_ring(painter: &egui::Painter, ring: &[[f64; 2]], to_screen: &Transform, stroke: Stroke) {
    let n = ring.len();
    if n < 2 {
        return;
    }
    for i in 0..n {
        painter.line_segment(
            [to_screen.point(ring[i]), to_screen.point(ring[(i + 1) % n])],
            stroke,
        );
    }
}

fn paint_nearest(painter: &egui::Painter, pos: Pos2, global_index: usize, palette: &Palette) {
    painter.circle_filled(pos, 4.0, palette.nearest_fill);
    painter.circle_stroke(pos, 8.0, Stroke::new(1.5, palette.nearest_outline));
    painter.text(
        pos + Vec2::new(8.0, -8.0),
        Align2::LEFT_BOTTOM,
        global_index.to_string(),
        FontId::monospace(11.0),
        palette.nearest_label,
    );
}

fn rings_of(index: usize) -> Vec<Vec<[f64; 2]>> {
    fixtures::FIXTURES[index]
        .1
        .iter()
        .map(|r| (*r).to_vec())
        .collect()
}
