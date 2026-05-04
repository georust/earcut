use earcut::{Earcut, deviation};
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2,
};
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation as SpadeTriangulation};

#[path = "../tests/fixtures/mod.rs"]
mod fixtures;

const INDEX_LABEL_LIMIT: usize = 3000;

#[derive(Clone, Copy)]
struct VertexRef {
    ring: usize,
    point: usize,
}

#[derive(Clone, Copy)]
struct NearestVertex {
    vertex: VertexRef,
    global_index: usize,
    pos: Pos2,
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
    show_delaunay: bool,
    show_rings: bool,
    show_points: bool,
    show_indices: bool,
    show_steiner: bool,
    zoom: f32,
    pan: Vec2,
}

impl Visualizer {
    fn new() -> Self {
        let selected = fixture_index("building");
        let rings = fixture_rings(selected);
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
            show_delaunay: false,
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
        self.rings = fixture_rings(index);
        self.retriangulate();
        self.view_bbox = self.triangulation.bbox;
        self.active_vertex = None;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
        let rect = response.rect;

        painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 23));
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, Color32::from_gray(48)),
            StrokeKind::Inside,
        );

        if self.triangulation.data.is_empty() {
            return;
        }

        let mut to_screen = Transform::new(rect, self.view_bbox, self.zoom, self.pan);
        let pointer = ui.input(|input| input.pointer.hover_pos());
        let nearest_vertex = pointer
            .filter(|_| response.hovered() || self.active_vertex.is_some())
            .and_then(|pointer| self.nearest_vertex(&to_screen, pointer));

        if response.drag_started() {
            self.active_vertex = nearest_vertex.map(|nearest| nearest.vertex);
        }

        if self.active_vertex.is_none() && response.dragged() {
            self.pan += response.drag_delta();
            to_screen = Transform::new(rect, self.view_bbox, self.zoom, self.pan);
        }

        if let (Some(vertex), Some(pointer)) = (self.active_vertex, pointer) {
            if ui.input(|input| input.pointer.primary_down()) {
                let point = to_screen.screen_to_world(pointer);
                self.move_vertex(vertex, point);
                self.retriangulate();
                to_screen = Transform::new(rect, self.view_bbox, self.zoom, self.pan);
            }
        }

        if response.drag_stopped() {
            self.active_vertex = None;
        }

        ui.input(|input| {
            if response.hovered() {
                let scroll = input.smooth_scroll_delta.y;
                let Some(pointer) = input.pointer.hover_pos() else {
                    return;
                };
                if scroll != 0.0 {
                    let world = to_screen.screen_to_world(pointer);
                    let factor = (scroll * 0.0015).exp();
                    self.zoom = (self.zoom * factor).clamp(0.02, 200.0);
                    to_screen = Transform::new(rect, self.view_bbox, self.zoom, self.pan);
                    self.pan += pointer - to_screen.point(world);
                    to_screen = Transform::new(rect, self.view_bbox, self.zoom, self.pan);
                }
            }
        });

        if self.show_delaunay {
            self.triangulation.ensure_delaunay();
        }
        self.paint_geometry(&painter, &to_screen);
        let active_nearest = self.active_vertex.and_then(|vertex| {
            self.vertex_screen_pos(vertex, &to_screen)
                .map(|pos| NearestVertex {
                    vertex,
                    global_index: self.global_vertex_index(vertex),
                    pos,
                })
        });
        if let Some(nearest) = active_nearest.or_else(|| {
            pointer
                .filter(|_| response.hovered())
                .and_then(|pointer| self.nearest_vertex(&to_screen, pointer))
        }) {
            paint_nearest_vertex(&painter, nearest);
        }
    }

    fn paint_geometry(&self, painter: &egui::Painter, to_screen: &Transform) {
        if self.show_fill {
            painter.add(fill_mesh(&self.triangulation, to_screen));
        }

        if self.show_mesh {
            let stroke = Stroke::new(0.8, Color32::from_rgba_unmultiplied(126, 181, 246, 135));
            for tri in self.triangulation.triangles.chunks_exact(3) {
                let a = self.triangulation.data[tri[0] as usize];
                let b = self.triangulation.data[tri[1] as usize];
                let c = self.triangulation.data[tri[2] as usize];
                draw_mesh_edge(painter, to_screen, a, b, stroke);
                draw_mesh_edge(painter, to_screen, b, c, stroke);
                draw_mesh_edge(painter, to_screen, c, a, stroke);
            }
        }

        if self.show_delaunay
            && let Some(delaunay) = self.triangulation.delaunay_triangles.as_deref()
        {
            let stroke = Stroke::new(1.1, Color32::from_rgba_unmultiplied(248, 111, 176, 190));
            for [a, b, c] in delaunay {
                draw_mesh_edge(painter, to_screen, *a, *b, stroke);
                draw_mesh_edge(painter, to_screen, *b, *c, stroke);
                draw_mesh_edge(painter, to_screen, *c, *a, stroke);
            }
        }

        if self.show_rings {
            for (ring_index, ring) in self.rings.iter().enumerate() {
                let stroke = if ring_index == 0 {
                    Stroke::new(2.0, Color32::from_rgb(91, 211, 135))
                } else {
                    Stroke::new(2.0, Color32::from_rgb(238, 177, 88))
                };
                draw_ring(painter, ring, to_screen, stroke);
            }
        }

        if self.show_points || self.show_indices {
            for (index, point) in self.triangulation.data.iter().enumerate() {
                let pos = to_screen.point(*point);
                if self.show_points {
                    painter.circle_filled(pos, 2.5, Color32::from_rgb(244, 244, 246));
                }
                if self.show_indices && self.triangulation.data.len() <= INDEX_LABEL_LIMIT {
                    painter.text(
                        pos + Vec2::new(4.0, -4.0),
                        Align2::LEFT_BOTTOM,
                        index.to_string(),
                        FontId::monospace(10.0),
                        Color32::from_gray(210),
                    );
                }
            }
        }

        // Highlight Steiner points: single-vertex hole rings, which earcut
        // flags via `set_steiner()` so they survive `filter_points` and get
        // bridged into the outer polygon.
        if self.show_steiner {
            let fill = Color32::from_rgb(255, 92, 92);
            let outline = Stroke::new(1.0, Color32::from_rgb(255, 255, 255));
            for ring in self.rings.iter().skip(1) {
                if ring.len() == 1 {
                    let pos = to_screen.point(ring[0]);
                    painter.circle_filled(pos, 3.0, fill);
                    painter.circle_stroke(pos, 3.5, outline);
                }
            }
        }
    }

    fn retriangulate(&mut self) {
        self.triangulation = Triangulation::new(fixtures::FIXTURES[self.selected].0, &self.rings);
    }

    fn nearest_vertex(&self, to_screen: &Transform, pointer: Pos2) -> Option<NearestVertex> {
        let mut nearest = None;
        let mut global_index = 0usize;
        let max_distance_sq = 12.0f32.powi(2);

        for (ring_index, ring) in self.rings.iter().enumerate() {
            for (point_index, point) in ring.iter().enumerate() {
                let pos = to_screen.point(*point);
                let distance_sq = pos.distance_sq(pointer);
                if distance_sq <= max_distance_sq
                    && nearest
                        .as_ref()
                        .is_none_or(|(_, nearest_distance_sq)| distance_sq < *nearest_distance_sq)
                {
                    nearest = Some((
                        NearestVertex {
                            vertex: VertexRef {
                                ring: ring_index,
                                point: point_index,
                            },
                            global_index,
                            pos,
                        },
                        distance_sq,
                    ));
                }
                global_index += 1;
            }
        }

        nearest.map(|(nearest, _)| nearest)
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
        if closed {
            if vertex.point == 0 {
                ring[last] = point;
            } else if vertex.point == last {
                ring[0] = point;
            }
        }
    }

    fn vertex_screen_pos(&self, vertex: VertexRef, to_screen: &Transform) -> Option<Pos2> {
        self.rings
            .get(vertex.ring)
            .and_then(|ring| ring.get(vertex.point))
            .map(|point| to_screen.point(*point))
    }

    fn global_vertex_index(&self, vertex: VertexRef) -> usize {
        let preceding = self
            .rings
            .iter()
            .take(vertex.ring)
            .map(Vec::len)
            .sum::<usize>();
        preceding + vertex.point
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
                    for (index, (name, _)) in fixtures::FIXTURES.iter().enumerate() {
                        if ui.selectable_label(index == self.selected, *name).clicked() {
                            self.select_fixture(index);
                        }
                    }
                });
            });

        egui::Panel::right("controls")
            .resizable(false)
            .default_size(230.0)
            .show_inside(ui, |ui| {
                ui.heading("View");
                ui.add_space(6.0);
                ui.checkbox(&mut self.show_fill, "fill triangles");
                ui.checkbox(&mut self.show_mesh, "triangle mesh");
                ui.checkbox(&mut self.show_delaunay, "delaunay mesh");
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
                ui.label(format!("name: {}", self.triangulation.name));
                ui.label(format!("rings: {}", self.rings.len()));
                ui.label(format!("holes: {}", self.triangulation.hole_indices.len()));
                let steiner_count = self.rings.iter().skip(1).filter(|r| r.len() == 1).count();
                ui.label(format!("steiner: {steiner_count}"));
                ui.label(format!("vertices: {}", self.triangulation.data.len()));
                ui.label(format!(
                    "triangles: {}",
                    self.triangulation.triangles.len() / 3
                ));
                ui.label(format!(
                    "delaunay: {}",
                    self.triangulation
                        .delaunay_triangles
                        .as_ref()
                        .map(|d| d.len().to_string())
                        .unwrap_or_else(|| "—".into())
                ));
                ui.label(format!("deviation: {:.6e}", self.triangulation.deviation));
                if self.show_indices && self.triangulation.data.len() > INDEX_LABEL_LIMIT {
                    ui.label(format!("indices hidden above {INDEX_LABEL_LIMIT} vertices"));
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.draw_canvas(ui);
        });
    }
}

struct Triangulation {
    name: &'static str,
    data: Vec<[f64; 2]>,
    hole_indices: Vec<u32>,
    triangles: Vec<u32>,
    /// Computed lazily on first request; `None` until `ensure_delaunay` is called.
    delaunay_triangles: Option<Vec<[[f64; 2]; 3]>>,
    deviation: f64,
    bbox: Bbox,
}

impl Triangulation {
    fn new(name: &'static str, rings: &[Vec<[f64; 2]>]) -> Self {
        let data: Vec<[f64; 2]> = rings.iter().flat_map(|r| r.iter().copied()).collect();
        let hole_indices: Vec<u32> = rings
            .iter()
            .take(rings.len().saturating_sub(1))
            .scan(0u32, |sum, ring| {
                *sum += ring.len() as u32;
                Some(*sum)
            })
            .collect();

        let mut triangles = Vec::new();
        Earcut::new().earcut(data.iter().copied(), &hole_indices, &mut triangles);
        let deviation = if triangles.is_empty() {
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
            delaunay_triangles: None,
            deviation,
            bbox,
        }
    }

    fn ensure_delaunay(&mut self) -> &[[[f64; 2]; 3]] {
        self.delaunay_triangles
            .get_or_insert_with(|| delaunay_triangles(&self.data, &self.hole_indices))
    }
}

#[derive(Clone, Copy)]
struct Bbox {
    min: [f64; 2],
    max: [f64; 2],
}

impl Bbox {
    fn from_points(points: &[[f64; 2]]) -> Self {
        let mut bbox = Self {
            min: [f64::INFINITY, f64::INFINITY],
            max: [f64::NEG_INFINITY, f64::NEG_INFINITY],
        };
        for [x, y] in points {
            bbox.min[0] = bbox.min[0].min(*x);
            bbox.min[1] = bbox.min[1].min(*y);
            bbox.max[0] = bbox.max[0].max(*x);
            bbox.max[1] = bbox.max[1].max(*y);
        }
        bbox
    }

    fn center(self) -> [f64; 2] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
        ]
    }

    fn size(self) -> [f64; 2] {
        [
            (self.max[0] - self.min[0]).max(1.0),
            (self.max[1] - self.min[1]).max(1.0),
        ]
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
        let size = bbox.size();
        let margin = 32.0;
        let scale_x = ((rect.width() - margin * 2.0).max(1.0) / size[0] as f32).max(0.0001);
        let scale_y = ((rect.height() - margin * 2.0).max(1.0) / size[1] as f32).max(0.0001);
        Self {
            rect,
            center: bbox.center(),
            scale: scale_x.min(scale_y) * zoom,
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
        let delta = pos - self.rect.center() - self.pan;
        [
            self.center[0] + (delta.x / self.scale) as f64,
            self.center[1] - (delta.y / self.scale) as f64,
        ]
    }
}

fn draw_ring(painter: &egui::Painter, ring: &[[f64; 2]], to_screen: &Transform, stroke: Stroke) {
    if ring.len() < 2 {
        return;
    }
    for i in 0..ring.len() {
        let a = to_screen.point(ring[i]);
        let b = to_screen.point(ring[(i + 1) % ring.len()]);
        painter.line_segment([a, b], stroke);
    }
}

fn paint_nearest_vertex(painter: &egui::Painter, nearest: NearestVertex) {
    painter.circle_filled(
        nearest.pos,
        4.0,
        Color32::from_rgba_unmultiplied(255, 255, 255, 230),
    );
    painter.circle_stroke(
        nearest.pos,
        8.0,
        Stroke::new(1.5, Color32::from_rgb(255, 218, 112)),
    );
    painter.text(
        nearest.pos + Vec2::new(8.0, -8.0),
        Align2::LEFT_BOTTOM,
        nearest.global_index.to_string(),
        FontId::monospace(11.0),
        Color32::from_rgb(255, 238, 190),
    );
}

fn fill_mesh(triangulation: &Triangulation, to_screen: &Transform) -> Shape {
    let mut mesh = egui::epaint::Mesh::default();
    mesh.reserve_vertices(triangulation.data.len());
    mesh.reserve_triangles(triangulation.triangles.len() / 3);

    let color = Color32::from_rgba_unmultiplied(79, 143, 220, 58);
    for point in &triangulation.data {
        mesh.colored_vertex(to_screen.point(*point), color);
    }
    mesh.indices.extend_from_slice(&triangulation.triangles);

    Shape::mesh(mesh)
}

fn delaunay_triangles(data: &[[f64; 2]], hole_indices: &[u32]) -> Vec<[[f64; 2]; 3]> {
    // Deduplicate points and remember the mapping from input index to CDT vertex
    // index so we can add polygon edges as constraints.
    let mut points = Vec::<Point2<f64>>::new();
    let mut idx_map: Vec<Option<usize>> = Vec::with_capacity(data.len());
    for &[x, y] in data {
        if !x.is_finite() || !y.is_finite() {
            idx_map.push(None);
            continue;
        }
        if let Some(existing) = points.iter().position(|p| p.x == x && p.y == y) {
            idx_map.push(Some(existing));
        } else {
            idx_map.push(Some(points.len()));
            points.push(Point2::new(x, y));
        }
    }

    let ring_starts: Vec<usize> = std::iter::once(0)
        .chain(hole_indices.iter().map(|&i| i as usize))
        .chain(std::iter::once(data.len()))
        .collect();

    let mut edges = Vec::<[usize; 2]>::new();
    for w in ring_starts.windows(2) {
        let (start, end) = (w[0], w[1]);
        if end <= start {
            continue;
        }
        let n = end - start;
        for i in 0..n {
            let a = idx_map[start + i];
            let b = idx_map[start + (i + 1) % n];
            if let (Some(a), Some(b)) = (a, b)
                && a != b
            {
                edges.push([a, b]);
            }
        }
    }

    // Self-touching or self-intersecting fixtures can produce constraint
    // edges that conflict with the partial triangulation; silently skip them
    // instead of panicking.
    let Ok(triangulation) = ConstrainedDelaunayTriangulation::<Point2<f64>>::try_bulk_load_cdt(
        points,
        edges,
        |_conflict| {},
    ) else {
        return Vec::new();
    };

    let rings: Vec<&[[f64; 2]]> = ring_starts.windows(2).map(|w| &data[w[0]..w[1]]).collect();

    triangulation
        .inner_faces()
        .filter_map(|face| {
            let [a, b, c] = face.positions();
            let centroid = [(a.x + b.x + c.x) / 3.0, (a.y + b.y + c.y) / 3.0];
            point_in_polygon(&rings, centroid).then_some([[a.x, a.y], [b.x, b.y], [c.x, c.y]])
        })
        .collect()
}

/// Even-odd rule across all rings combined: inside iff total edge crossings
/// from a horizontal ray are odd.
fn point_in_polygon(rings: &[&[[f64; 2]]], p: [f64; 2]) -> bool {
    let mut inside = false;
    for ring in rings {
        let n = ring.len();
        if n < 2 {
            continue;
        }
        let mut j = n - 1;
        for i in 0..n {
            let pi = ring[i];
            let pj = ring[j];
            if (pi[1] > p[1]) != (pj[1] > p[1])
                && p[0] < (pj[0] - pi[0]) * (p[1] - pi[1]) / (pj[1] - pi[1]) + pi[0]
            {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}

fn draw_mesh_edge(
    painter: &egui::Painter,
    to_screen: &Transform,
    a: [f64; 2],
    b: [f64; 2],
    stroke: Stroke,
) {
    painter.line_segment([to_screen.point(a), to_screen.point(b)], stroke);
}

fn fixture_index(name: &str) -> usize {
    fixtures::FIXTURES
        .iter()
        .position(|(fixture_name, _)| *fixture_name == name)
        .unwrap_or(0)
}

fn fixture_rings(index: usize) -> Vec<Vec<[f64; 2]>> {
    fixtures::FIXTURES[index]
        .1
        .iter()
        .map(|ring| (*ring).to_vec())
        .collect()
}
