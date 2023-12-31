
use std::sync::{Arc, Mutex};
use glam::Vec2;
use glow::HasContext;

use crate::{
    editor::{EditorState, EditorRenderer},
    renderer::{fb::Framebuffer, mesh::Mesh, shader::Shader, scene::SceneRenderer}, util::curve,
};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ScenePanel {
    #[serde(skip)]
    fb: Arc<Mutex<Option<Framebuffer>>>,
    #[serde(skip)]
    fb_pick: Arc<Mutex<Option<Framebuffer>>>,
    #[serde(skip)]
    color_key_map: Vec<u64>, 

    #[serde(skip)]
    cam_pos: glam::Vec2,
    #[serde(default)]
    pub cam_size: f32,
}

impl Default for ScenePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ScenePanel {
    pub fn new() -> Self {
        Self {
            fb: Arc::new(Mutex::new(None)),
            fb_pick: Arc::new(Mutex::new(None)),
            color_key_map: Vec::new(),
            cam_pos: glam::vec2(0.0, 0.0),
            cam_size: 5.0,
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, state: &mut EditorState, renderer: &mut EditorRenderer) {
        let gfx = state.open_graphic();
        let no_margin = egui::Frame {
            inner_margin: egui::Margin::same(0.0),
            ..Default::default()
        };
        if let Some(_gfx) = gfx {
            egui::SidePanel::new(egui::panel::Side::Left, "toolbar")
                .resizable(false)
                .exact_width(34.0)
                .frame(no_margin)
                .show_inside(ui, |ui| {
                    if ui
                        .button(
                            egui::RichText::new(format!("{}", egui_phosphor::regular::CURSOR))
                                .size(20.0),
                        )
                        .clicked()
                    {
                        state.curr_tool.clone().borrow_mut().reset(state);
                        state.curr_tool = state.select.clone();
                    }
                    if ui
                        .button(
                            egui::RichText::new(format!("{}", egui_phosphor::regular::PENCIL))
                                .size(20.0),
                        )
                        .clicked()
                    {
                        state.curr_tool.clone().borrow_mut().reset(state);
                        state.curr_tool = state.pencil.clone();
                    }
                });
            egui::CentralPanel::default()
                .frame(no_margin)
                .show_inside(ui, |ui| {
                    egui::Frame::canvas(ui.style()).show(ui, |ui| {
                        let (rect, response) = ui.allocate_exact_size(
                            ui.available_size(),
                            egui::Sense::click_and_drag(),
                        );

                        let gl_ctx_copy = renderer.gl_ctx.clone();
                        let fb_copy = self.fb.clone();

                        let cb = egui_glow::CallbackFn::new(move |_info, painter| {
                            let gl = painter.gl();
                            *gl_ctx_copy.lock().unwrap() = Some(gl.clone());
                            if let Some(fb) = fb_copy.lock().unwrap().as_ref() {
                                // TODO: this is scuffed and inneficient
                                let mut shader = Shader::new(
                                    "
                                    #version 100\n 

                                    attribute vec2 aPos;

                                    varying vec2 uv;
                            
                                    void main() {
                                        gl_Position = vec4(aPos, 0.0, 1.0);                
                                        uv = aPos;
                                    } 
                                ",
                                    "
                                    #version 100\n 

                                    uniform sampler2D tex;
                                    varying mediump vec2 uv;

                                    void main() {
                                        gl_FragColor = texture2D(tex, uv / 2.0 + 0.5); 
                                    }
                                ",
                                    gl,
                                );

                                let mut quad = Mesh::new(vec![2], gl);
                                quad.upload(
                                    &vec![-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0],
                                    &vec![0, 1, 2, 1, 2, 3],
                                    gl,
                                );

                                shader.enable(gl);
                                shader.set_int("tex", 1, gl);
                                unsafe {
                                    gl.active_texture(glow::TEXTURE1);
                                    gl.bind_texture(glow::TEXTURE_2D, Some(fb.color));
                                }
                                quad.render(gl);

                                shader.delete(gl);
                                quad.delete(gl);
                            }
                        });

                        self.render_scene(state, &rect, &response, ui, state.open_graphic, renderer);

                        let callback = egui::PaintCallback {
                            rect,
                            callback: Arc::new(cb),
                        };
                        ui.painter().add(callback);
                    });
                });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("No Graphic Open");
            });
        }
    }

    fn render_scene(&mut self, state: &mut EditorState, rect: &egui::Rect, response: &egui::Response, ui: &mut egui::Ui, gfx_key: u64, renderer: &mut EditorRenderer) {
        if let Some(mouse_pos) = response.hover_pos() {
            let mouse_pos = self.cam_size * (mouse_pos - rect.center()) / (rect.height() * 0.5);
            let mouse_pos = glam::vec2(mouse_pos.x, -mouse_pos.y) + self.cam_pos;

            let zoom_fac =
                (1.05 as f32).powf(-ui.input(|i| i.scroll_delta.y.clamp(-6.0, 6.0) * 0.7));
            let next_cam_size = self.cam_size * zoom_fac;
            if next_cam_size > 0.02 && next_cam_size < 1000.0 {
                self.cam_pos -= (mouse_pos - self.cam_pos) * (zoom_fac - 1.0);
                self.cam_size = next_cam_size;
            }

            let tool = state.curr_tool.clone();
            renderer.use_renderer(|gl, _rendererer| {
                if response.drag_started() {
                    tool.borrow_mut().mouse_click(mouse_pos, state, ui, self, gl);
                }
                if response.dragged() {
                    tool.borrow_mut().mouse_down(mouse_pos, state);
                }
                if response.drag_released() {
                    tool.borrow_mut().mouse_release(mouse_pos, state, ui);
                }
                if response.hovered() {
                    ui.ctx().output_mut(|o| {
                        let tool = state.curr_tool.clone();
                        o.cursor_icon = tool.borrow_mut().mouse_cursor(mouse_pos, state, self, gl);
                    });
                }
            });
        }

        let frame = state.frame();
        renderer.use_renderer(|gl, renderer| {
            let mut fb = self.fb.lock().unwrap();
            let mut fb_pick = self.fb_pick.lock().unwrap();
            if let None = fb.as_ref() {
                *fb = Some(Framebuffer::new(100, 100, gl));
                *fb_pick = Some(Framebuffer::new(100, 100, gl));
            }
            let fb = fb.as_mut().unwrap();
            let fb_pick = fb_pick.as_mut().unwrap();

            if let Some(proj_view) = renderer.render(
                fb,
                Some((fb_pick, &mut self.color_key_map)),
                (rect.width() as u32) * 4,
                (rect.height() as u32) * 4,
                self.cam_pos,
                self.cam_size,
                &mut state.project,
                gfx_key,
                frame,
                if state.playing { 0 } else { state.onion_before },
                if state.playing { 0 } else { state.onion_after },
                gl,
            ) {
                self.render_overlays(gfx_key, renderer, gl, proj_view, state);
            }

            Framebuffer::render_to_win(
                ui.ctx().screen_rect().width() as u32,
                ui.ctx().screen_rect().height() as u32,
                gl,
            );
        });
        
    }

    fn render_overlays(&self, gfx_key: u64, renderer: &mut SceneRenderer, gl: &Arc<glow::Context>, proj_view: glam::Mat4, state: &mut EditorState) {
        renderer.flat_color_shader.enable(gl);
        renderer
            .flat_color_shader
            .set_vec4("uColor", glam::vec4(0.0, 0.0, 0.0, 0.3), gl);
        let mut draw_quad = |left: f32, right: f32, top: f32, bottom: f32| {
            let center = glam::vec3((left + right) / 2.0, (top + bottom) / 2.0, 0.0);
            let scale = glam::vec3(right - left, top - bottom, 1.0);
            let model =
                glam::Mat4::from_translation(center) * glam::Mat4::from_scale(scale);
            let trans = proj_view * model;
            renderer.flat_color_shader.set_mat4("uTrans", &trans, gl);
            renderer.quad.render(gl);
        };

        let gfx = state.project.graphics.get(&gfx_key).unwrap();
        if gfx.data.clip {
            let cam_top = 5.0;
            let cam_btm = -cam_top;
            let cam_right = cam_top * ((gfx.data.w as f32) / (gfx.data.h as f32));
            let cam_left = -cam_right;
            let huge = 100000.0;
            draw_quad(cam_left, cam_right, huge, cam_top);
            draw_quad(cam_left, cam_right, cam_btm, -huge);
            draw_quad(-huge, cam_left, cam_top, cam_btm);
            draw_quad(cam_right, huge, cam_top, cam_btm);
            draw_quad(-huge, cam_left, huge, cam_top);
            draw_quad(cam_right, huge, huge, cam_top);
            draw_quad(cam_right, huge, cam_btm, -huge);
            draw_quad(-huge, cam_left, cam_btm, -huge);
        }

        let mut overlay = OverlayRenderer::new(renderer, gl, proj_view, self.cam_size);
        state.curr_tool.clone().borrow_mut().draw_overlay(&mut overlay, state); 
        for stroke in &state.selected_strokes {
            if let Some(stroke) = state.project.strokes.get(stroke) {
                for (p0, p1) in stroke.iter_point_pairs() {
                    let p0 = state.project.points.get(&p0).unwrap(); 
                    let p1 = state.project.points.get(&p1).unwrap(); 
                    let n = 20;
                    for i in 0..n {
                        let t = (i as f32) / (n as f32);
                        overlay.line(
                            curve::bezier_sample(t, p0.data.pt, p0.data.b, p1.data.a, p1.data.pt),
                            curve::bezier_sample(t + 1.0 / (n as f32), p0.data.pt, p0.data.b, p1.data.a, p1.data.pt),
                            glam::vec4(0.0, 1.0, 1.0, 1.0) 
                        );
                    }
                }
            }
        }

    }

    pub fn sample_pick(&mut self, pos: Vec2, gl: &Arc<glow::Context>) -> Option<u64> {
        if let Some(fb_pick) = self.fb_pick.lock().unwrap().as_ref() {

            let h_cam_size = self.cam_size * (fb_pick.w as f32) / (fb_pick.h as f32);
            let left = self.cam_pos.x - h_cam_size;
            let right = self.cam_pos.x + h_cam_size;
            let top = self.cam_pos.y + self.cam_size;
            let bottom = self.cam_pos.y - self.cam_size;

            let x = (pos.x - left) / (right - left);
            let y = (pos.y - bottom) / (top - bottom);

            let px = (x * (fb_pick.w as f32)) as i32;
            let py = (y * (fb_pick.h as f32)) as i32;

            if px < 0 || py < 0 || px >= fb_pick.w as i32 || py >= fb_pick.h as i32 {
                return None;
            }

            let mut pixel_data = [0; 4];
            unsafe {
                gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fb_pick.fbo));
                gl.read_pixels(px, py, 1, 1, glow::RGB, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(&mut pixel_data[0..3]));
            }
            let color = u32::from_le_bytes(pixel_data);
            if color > 0 && color <= self.color_key_map.len() as u32 {
                return Some(self.color_key_map[color as usize - 1]);
            }
        }
        None
    }

}

pub struct OverlayRenderer<'a> {
    renderer: &'a mut SceneRenderer,
    gl: &'a Arc<glow::Context>,
    proj_view: glam::Mat4,
    pub cam_size: f32
}

impl<'a> OverlayRenderer<'a> {

    pub fn new(renderer: &'a mut SceneRenderer, gl: &'a Arc<glow::Context>, proj_view: glam::Mat4, cam_size: f32) -> Self {
        Self {
            renderer,
            gl,
            proj_view,
            cam_size
        }
    }

    pub fn line(&mut self, p0: glam::Vec2, p1: glam::Vec2, color: glam::Vec4) {
        let center = (p0 + p1) / 2.0;
        let len = (p1 - p0).length();
        let scale = glam::vec3(len, 0.004 * self.cam_size, 1.0);
        let angle = glam::vec2(1.0, 0.0).angle_between(p1 - p0);
        let model = glam::Mat4::from_translation(glam::vec3(center.x, center.y, 0.0)) * glam::Mat4::from_axis_angle(glam::vec3(0.0, 0.0, 1.0), angle) * glam::Mat4::from_scale(scale);
        let trans = self.proj_view * model; 
        self.renderer.flat_color_shader.enable(self.gl);
        self.renderer.flat_color_shader.set_mat4("uTrans", &trans, self.gl);
        self.renderer.flat_color_shader.set_vec4("uColor", color, self.gl);
        self.renderer.quad.render(self.gl);
    }

    pub fn circle(&mut self, pt: Vec2, color: glam::Vec4, r: f32) {
        let model = glam::Mat4::from_translation(glam::vec3(pt.x, pt.y, 0.0)) * glam::Mat4::from_scale(glam::Vec3::splat(r * 2.0));
        let trans = self.proj_view * model;
        self.renderer.circle_shader.enable(self.gl);
        self.renderer.flat_color_shader.set_mat4("uTrans", &trans, self.gl);
        self.renderer.flat_color_shader.set_vec4("uColor", color, self.gl);
        self.renderer.quad.render(self.gl);
    }

}
