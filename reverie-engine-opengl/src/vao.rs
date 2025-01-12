//! Vertex Array Object
pub mod buffer;
pub mod color_vao;
pub mod config;
pub mod renderer;
pub mod texture_vao;
pub mod vertex;

use std::mem;
use std::os::raw::c_void;

use crate::gl;
use crate::gl::types::{GLenum, GLfloat, GLint, GLsizei, GLsizeiptr};
use crate::gl::Gl;
use crate::shader::UniformVariables;

pub use {
    buffer::VaoBuffer,
    color_vao::VaoBuilder3DGeometryOutline,
    config::{VaoConfig, VaoConfigBuilder},
    renderer::{
        Color3DRenderer, Color3DRenderingInfo, Phong3DRenderer, Phong3DRenderingInfo,
        PhongRenderingInfo, Renderer,
    },
    texture_vao::builder::{CuboidTextures, VaoBuilder3DGeometry},
    vertex::{VertexType, VertexWithColor, VertexWithNormUv},
};

/// OpenGLのVertex Array ObjectとVertex Buffer Objectに対応する構造体
#[derive(Debug)]
pub struct Vao<'a> {
    gl: Gl,
    vao: u32,
    vbo: u32,
    vertex_num: i32,
    config: &'a VaoConfig,
}

impl<'a> Vao<'a> {
    #[allow(clippy::too_many_arguments)]
    /// ## Safety
    ///
    /// `data` が有効なポインタであること
    unsafe fn new(
        gl: Gl,
        size: GLsizeiptr,
        data: *const c_void,
        usage: GLenum,
        num_attributes: usize,
        attribute_types: &'static [GLenum],
        attribute_sizes: &'static [GLint],
        stride: GLsizei,
        vertex_num: i32,
        config: &'a VaoConfig,
    ) -> Self {
        debug_assert_eq!(num_attributes, attribute_types.len());
        debug_assert_eq!(num_attributes, attribute_sizes.len());

        let mut vao = 0;
        let mut vbo = 0;

        unsafe {
            // create vertex array object and vertex buffer object
            gl.GenVertexArrays(1, &mut vao);
            gl.GenBuffers(1, &mut vbo);

            // bind buffer
            gl.BindVertexArray(vao);
            gl.BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl.BufferData(gl::ARRAY_BUFFER, size, data, usage);

            let mut offset = 0;
            for i in 0..num_attributes {
                gl.EnableVertexAttribArray(i as u32);
                gl.VertexAttribPointer(
                    i as u32,
                    attribute_sizes[i],
                    attribute_types[i],
                    gl::FALSE,
                    stride,
                    (offset * mem::size_of::<GLfloat>()) as *const c_void,
                );
                offset += attribute_sizes[i] as usize;
            }

            // unbind
            gl.BindBuffer(gl::ARRAY_BUFFER, 0);
            gl.BindVertexArray(0);
        }

        Vao {
            gl,
            vao,
            vbo,
            vertex_num,
            config,
        }
    }

    fn draw(&self, _uniforms: &UniformVariables, draw_mode: GLenum) {
        unsafe {
            if self.config.depth_test {
                self.gl.Enable(gl::DEPTH_TEST);
            } else {
                self.gl.Disable(gl::DEPTH_TEST);
            }

            if self.config.blend {
                self.gl.Enable(gl::BLEND);
                self.gl.BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            } else {
                self.gl.Disable(gl::BLEND);
            }

            if self.config.wireframe {
                self.gl.PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
            } else {
                self.gl.PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
            }

            if self.config.culling {
                self.gl.Enable(gl::CULL_FACE);
            } else {
                self.gl.Disable(gl::CULL_FACE);
            }

            self.gl.BindVertexArray(self.vao);
            self.gl.DrawArrays(draw_mode, 0, self.vertex_num);
            self.gl.BindVertexArray(0);
            self.gl.BindTexture(gl::TEXTURE_2D, 0);
        }
    }

    /// ポリゴンを描画する
    fn draw_triangles(&self, uniforms: &UniformVariables) {
        self.draw(uniforms, gl::TRIANGLES);
    }
}

impl Drop for Vao<'_> {
    fn drop(&mut self) {
        unsafe {
            if self.vbo > 0 {
                self.gl.DeleteBuffers(1, &self.vbo as _);
            }
            if self.vao > 0 {
                self.gl.DeleteVertexArrays(1, &self.vao as _);
            }
        }
    }
}
