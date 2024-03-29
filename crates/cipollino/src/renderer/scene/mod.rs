
mod meshgen;

use std::sync::Arc;

use glam::{vec4, Vec4};
use glow::{Context, HasContext};

use crate::project::{graphic::Graphic, layer::LayerKind, obj::ObjPtr, stroke::Stroke, Project};

use super::{shader::Shader, fb::Framebuffer, mesh::Mesh};

pub struct SceneRenderer {
    pub flat_color_shader: Shader,
    pub circle_shader: Shader,
    pub quad: Mesh,

    pub clip_shadow_mesh: Mesh, 
    pub clip_shadow_shader: Shader,

    pub screen_quad: Mesh,
    pub screen_shader: Shader,
}

impl SceneRenderer {

    pub fn new(gl: &Arc<Context>) -> Self {
        let mut quad = Mesh::new(vec![2], gl);
        quad.upload(&vec![
            -0.5, -0.5,
             0.5, -0.5,
            -0.5,  0.5,
             0.5,  0.5
        ], &vec![
            0, 1, 2,
            1, 2, 3
        ], gl);

        let mut clip_shadow_mesh = Mesh::new(vec![2], gl);
        clip_shadow_mesh.upload(&vec![
            -1.0,  1.0,
             1.0,  1.0,
             1.0, -1.0,
            -1.0, -1.0,
            -0.5,  0.5,
             0.5,  0.5,
             0.5, -0.5,
            -0.5, -0.5
        ], &vec![
            0, 1, 4,
            1, 5, 4,
            1, 2, 5,
            2, 6, 5,
            2, 6, 3,
            3, 6, 7,
            3, 7, 0,
            0, 4, 7
        ], gl);

        let mut screen_quad = Mesh::new(vec![2, 2], gl);
        screen_quad.upload(&vec![
            -1.0, -1.0, 0.0, 0.0,
             1.0, -1.0, 1.0, 0.0,
            -1.0,  1.0, 0.0, 1.0,
             1.0,  1.0, 1.0, 1.0
        ], &vec![
            0, 1, 2,
            1, 2, 3
        ], gl);

        Self {
            flat_color_shader: Shader::new(include_str!("shaders/flat_color_vs.glsl"), include_str!("shaders/flat_color_fs.glsl"), gl),
            circle_shader: Shader::new(include_str!("shaders/circle_vs.glsl"), include_str!("shaders/circle_fs.glsl"), gl),
            quad,

            clip_shadow_mesh,
            clip_shadow_shader: Shader::new(include_str!("shaders/clip_shadow_vs.glsl"), include_str!("shaders/clip_shadow_fs.glsl"), gl),

            screen_quad,
            screen_shader: Shader::new(include_str!("shaders/screen_vs.glsl"), include_str!("shaders/screen_fs.glsl"), gl)
        }
    }

    pub fn render(
        &mut self,

        fb: &mut Framebuffer,
        fb_pick: Option<(&mut Framebuffer, &mut Vec<ObjPtr<Stroke>>)>,
        w: u32,
        h: u32,

        cam_pos: glam::Vec2,
        cam_size: f32,

        project: &mut Project,
        gfx: ObjPtr<Graphic>,
        time: i32,

        onion_before: i32,
        onion_after: i32,

        gl: &Arc<Context>
    ) -> Option<glam::Mat4> {

        fb.resize(w, h, gl);
        fb.render_to(gl);

        unsafe {
            gl.clear_color(1.0, 1.0, 1.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            gl.enable(glow::BLEND);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
        }

        let aspect = (w as f32) / (h as f32);
        let proj = glam::Mat4::orthographic_rh_gl(-aspect * cam_size, aspect * cam_size, -cam_size, cam_size, -1.0, 1.0);
        let view = glam::Mat4::from_translation(-glam::vec3(cam_pos.x, cam_pos.y, 0.0));
        let proj_view = proj * view;
        self.flat_color_shader.enable(gl);
        self.flat_color_shader.set_mat4("uTrans", &proj_view, gl);

        let mut onion_strokes = Vec::new();
        let mut stroke_keys = Vec::new();
        for layer in project.graphics.get(gfx)?.layers.iter().rev() {
            let layer = layer.get(project);
            if !layer.show || layer.kind != LayerKind::Animation {
                continue;
            }
            if let Some(frame) = layer.get_frame_at(project, time) {
                let mut curr_time = frame.get(project).time;
                let mut alpha = 0.75;
                for _i in 0..onion_before {
                    if let Some(frame) = layer.get_frame_before(project, curr_time) {
                        onion_strokes.append(&mut (frame.get(project).strokes
                            .iter()
                            .filter(|stroke| !stroke.get(project).filled) 
                            .map(|key| (glam::vec4(1.0, 0.3, 1.0, alpha), key.make_ptr())).collect()));
                        alpha *= 0.8;
                        curr_time = frame.get(project).time;
                    }
                }
                // Ugly bug fix: make sure the oldest strokes are drawn at the back
                onion_strokes.reverse();
                let mut curr_time = frame.get(project).time;
                let mut alpha = 0.75;
                for _i in 0..onion_after {
                    if let Some(frame) = layer.get_frame_after(project, curr_time) {
                        onion_strokes.append(&mut (frame.get(project,).strokes
                            .iter()
                            .filter(|stroke| !stroke.get(project).filled) 
                            .map(|key| (glam::vec4(0.3, 1.0, 1.0, alpha), key.make_ptr())).collect()));
                        alpha *= 0.8;
                        curr_time = frame.get(project).time;
                    }
                }
                stroke_keys.append(&mut frame.get(project).strokes.iter().map(|stroke| stroke.make_ptr()).collect());
            }
        }
        for (color, key) in onion_strokes {
            if let Some(mesh) = meshgen::get_mesh(project, key, gl) {
                self.flat_color_shader.set_vec4("uColor", color, gl);
                mesh.render(gl);
            }
        }

        let mut render_stroke_mesh = |mesh: &Mesh, color: Vec4, filled: bool, gl: &Arc<glow::Context>| {
            if !filled {
                unsafe {
                    gl.clear(glow::DEPTH_BUFFER_BIT);
                }
                self.flat_color_shader.set_vec4("uColor", glam::vec4(color.x, color.y, color.z, color.w), gl);
                mesh.render(gl);
            } else {
                self.flat_color_shader.set_vec4("uColor", glam::vec4(color.x, color.y, color.z, 1.0), gl);
                unsafe {
                    gl.enable(glow::STENCIL_TEST);
                    gl.stencil_mask(0xFF);
                    gl.clear(glow::STENCIL_BUFFER_BIT);
                    gl.stencil_func(glow::NEVER, 1, 0xFF);
                    gl.stencil_op(glow::INVERT, glow::INVERT, glow::INVERT);
                }
                mesh.render(gl);
                unsafe {
                    gl.stencil_func(glow::EQUAL, 0xFF, 0xFF);
                    gl.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
                    gl.stencil_mask(0);
                    gl.clear(glow::DEPTH_BUFFER_BIT);
                }
                self.flat_color_shader.set_vec4("uColor", glam::vec4(color.x, color.y, color.z, color.w), gl);
                mesh.render(gl);
                unsafe {
                    gl.disable(glow::STENCIL_TEST);
                }
            }
        };

        for stroke_ptr in &stroke_keys {
            let stroke = project.strokes.get(*stroke_ptr);
            if stroke.is_none() {
                continue;
            }
            let stroke = stroke.unwrap();
            let color = stroke.color.get_color(&project);
            let filled = stroke.filled;
            if let Some(mesh) = meshgen::get_mesh(project, *stroke_ptr, gl) {
                render_stroke_mesh(mesh, color, filled, gl); 
            }
        }
       
        if let Some((fb_pick, color_key_map)) = fb_pick {
            fb_pick.resize(w, h, gl);
            fb_pick.render_to(gl);
            color_key_map.clear();

            unsafe {
                gl.clear_color(0.0, 0.0, 0.0, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
            for stroke_ptr in &stroke_keys {
                let stroke = project.strokes.get(*stroke_ptr)?;
                let filled = stroke.filled;
                if let Some(mesh) = meshgen::get_mesh(project, *stroke_ptr, gl) {
                    let mut color = 0 as u32;
                    for i in 0..color_key_map.len() {
                        if color_key_map[i] == *stroke_ptr {
                            color = i as u32;
                            break;
                        } 
                    };
                    if color == 0 {
                        color_key_map.push(*stroke_ptr);
                        color = color_key_map.len() as u32;
                    }
                    let bytes = color.to_le_bytes();
                    let r = (bytes[0] as f32) / 255.0;
                    let g = (bytes[1] as f32) / 255.0;
                    let b = (bytes[2] as f32) / 255.0;

                    render_stroke_mesh(mesh, vec4(r, g, b, 1.0), filled, gl); 
                }
            }
            fb.render_to(gl);
        }

        Some(proj_view) 
        
    }

}
