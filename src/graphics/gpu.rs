//! Meshes kept on the graphics card, and the materials that draw them,
//! straight through Miniquad.
//!
//! Macroquad copies every mesh it draws into buffers of its own each frame
//! and sends them all to the graphics card again, into the same buffers as
//! the frame before, which the card may still be reading: the driver can
//! then hold the program until the card catches up. Most of the scene never
//! changes, and what does is a small part of it. So a mesh here is sent once
//! and kept on the card. One that is never edited lives in a fixed buffer;
//! one that is edited is written, after each edit, to the next of a few
//! buffers in turn, never to one that a frame still being drawn may read.
//!
//! The materials compile the same GLSL that Macroquad's would and set the
//! same uniforms, `Projection` and `Model` first, so the shaders are shared
//! with it. Drawing goes into the pass the camera now set draws into, after
//! whatever Macroquad has batched, so the order of drawing is kept.

use std::cell::{
    Cell,
    OnceCell,
    RefCell, //
};

use macroquad::miniquad::{
    Bindings,
    BufferId,
    BufferLayout,
    BufferSource,
    BufferType,
    BufferUsage,
    PassAction,
    Pipeline,
    PipelineParams,
    RenderingBackend,
    ShaderMeta,
    ShaderSource,
    TextureId,
    UniformBlockLayout,
    UniformDesc,
    UniformType,
    VertexAttribute,
    VertexFormat, //
};
use macroquad::models::{
    Mesh,
    Vertex, //
};
use macroquad::prelude::Mat4;
use macroquad::window::get_internal_gl;

use crate::ui::diagnostics::{
    self,
    Section, //
};

/// Buffers an edited mesh takes turns in. The graphics card runs at most a
/// frame or two behind, so a buffer written this frame was last read three
/// frames ago.
const TURNS: usize = 3;
/// How much room an edited mesh's buffers leave to grow, as a multiple of
/// what it holds when they are made, so a mesh that grows a little is not
/// sent to new buffers every frame.
const ROOM_TO_GROW: f32 = 1.5;
/// Uniforms every material starts with, as Macroquad's do.
const STANDARD_UNIFORMS: [(&str, UniformType); 2] = [
    ("Projection", UniformType::Mat4),
    ("Model", UniformType::Mat4),
];
/// The texture every shader reads, as Macroquad's do.
const TEXTURE_NAME: &str = "Texture";
/// Bytes in a 32-bit float.
const FLOAT_BYTES: usize = 4;

thread_local! {
    /// The plain white texture bound for `TEXTURE_NAME`, made on the first
    /// draw (it needs the window).
    static WHITE: OnceCell<TextureId> = const { OnceCell::new() };
    /// Buffers of meshes dropped since the last draw, deleted then (a mesh
    /// may be dropped where the window is gone).
    static RETIRED: RefCell<Vec<BufferId>> = const { RefCell::new(Vec::new()) };
}

/// Where Macroquad's `Vertex` fields sit, in the order its shaders name
/// them, for a vertex buffer of `Vertex`.
pub fn vertex_attributes() -> [VertexAttribute; 4] {
    [
        VertexAttribute::new("position", VertexFormat::Float3),
        VertexAttribute::new("texcoord", VertexFormat::Float2),
        VertexAttribute::new("color0", VertexFormat::Byte4),
        VertexAttribute::new("normal", VertexFormat::Float4),
    ]
}

/// One set of buffers holding a mesh, ready to bind.
struct Slot {
    /// The vertex and index buffers, and the white texture.
    bindings: Bindings,
    /// Indices to draw.
    indices: i32,
}

/// Where a mesh's buffers are on the graphics card.
enum Buffers {
    /// Not sent yet.
    Unsent,
    /// Sent once, never edited.
    Fixed(Slot),
    /// Edited: written to each slot in turn.
    Turns {
        /// The buffers, `TURNS` of them.
        slots: Vec<Slot>,
        /// The slot last written.
        current: usize,
        /// Vertices each slot has room for.
        vertex_room: usize,
        /// Indices each slot has room for.
        index_room: usize,
    },
}

/// A mesh kept on the graphics card (see the module documentation).
///
/// It keeps its own copy, which is edited through `edit`; the next draw
/// sends it.
pub struct GpuMesh {
    /// The mesh as last built or edited.
    mesh: Mesh,
    /// Whether it was edited since it was last sent.
    edited: Cell<bool>,
    /// Its buffers.
    buffers: RefCell<Buffers>,
}

impl GpuMesh {
    /// Keeps `mesh`, sending it on its first draw.
    pub fn new(mesh: Mesh) -> Self {
        Self {
            mesh,
            edited: Cell::new(false),
            buffers: RefCell::new(Buffers::Unsent),
        }
    }

    /// An empty mesh, to be filled with `edit`.
    pub fn empty() -> Self {
        Self::new(Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        })
    }

    /// The mesh as last built or edited.
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    /// The mesh, to change: the next draw sends it again.
    pub fn edit(&mut self) -> &mut Mesh {
        self.edited.set(true);
        &mut self.mesh
    }

    /// Runs `draw` with the context, the mesh's bindings and its index
    /// count, sending the mesh first where it was not sent or was edited
    /// since; nothing for an empty mesh.
    fn with_bindings(
        &self,
        context: &mut dyn RenderingBackend,
        draw: impl FnOnce(&mut dyn RenderingBackend, &Bindings, i32),
    ) {
        if self.mesh.indices.is_empty() {
            return;
        }
        let mut buffers = self.buffers.borrow_mut();
        let edited = self.edited.replace(false);
        let fits = |vertex_room: usize, index_room: usize| {
            self.mesh.vertices.len() <= vertex_room && self.mesh.indices.len() <= index_room
        };
        match (&mut *buffers, edited) {
            // Edits before it was first sent do not count.
            (Buffers::Unsent, _) => *buffers = Buffers::Fixed(fixed_slot(context, &self.mesh)),
            (Buffers::Fixed(_) | Buffers::Turns { .. }, false) => {}
            (
                Buffers::Turns {
                    slots,
                    current,
                    vertex_room,
                    index_room,
                },
                true,
            ) if fits(*vertex_room, *index_room) => {
                *current = (*current + 1) % slots.len();
                fill(context, &mut slots[*current], &self.mesh);
            }
            (_, true) => {
                retire(std::mem::replace(&mut *buffers, Buffers::Unsent));
                *buffers = turns(context, &self.mesh);
            }
        }
        let slot = match &*buffers {
            Buffers::Unsent => return,
            Buffers::Fixed(slot) => slot,
            Buffers::Turns { slots, current, .. } => &slots[*current],
        };
        draw(context, &slot.bindings, slot.indices);
    }
}

/// Fixed buffers holding `mesh` as it is.
fn fixed_slot(context: &mut dyn RenderingBackend, mesh: &Mesh) -> Slot {
    let vertex = context.new_buffer(
        BufferType::VertexBuffer,
        BufferUsage::Immutable,
        BufferSource::slice(&mesh.vertices),
    );
    let index = context.new_buffer(
        BufferType::IndexBuffer,
        BufferUsage::Immutable,
        BufferSource::slice(&mesh.indices),
    );
    Slot {
        bindings: bindings(context, vec![vertex], index),
        indices: mesh.indices.len() as i32,
    }
}

/// `TURNS` slots with room for `mesh` to grow, the first holding it as it
/// is.
fn turns(context: &mut dyn RenderingBackend, mesh: &Mesh) -> Buffers {
    let vertex_room = room(mesh.vertices.len());
    let index_room = room(mesh.indices.len());
    let mut slots: Vec<Slot> = (0..TURNS)
        .map(|_| {
            let vertex = context.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Stream,
                BufferSource::empty::<Vertex>(vertex_room),
            );
            let index = context.new_buffer(
                BufferType::IndexBuffer,
                BufferUsage::Stream,
                BufferSource::empty::<u16>(index_room),
            );
            Slot {
                bindings: bindings(context, vec![vertex], index),
                indices: 0,
            }
        })
        .collect();
    fill(context, &mut slots[0], mesh);
    Buffers::Turns {
        slots,
        current: 0,
        vertex_room,
        index_room,
    }
}

/// Writes `mesh` to `slot`, which has room for it.
fn fill(context: &mut dyn RenderingBackend, slot: &mut Slot, mesh: &Mesh) {
    context.buffer_update(
        slot.bindings.vertex_buffers[0],
        BufferSource::slice(&mesh.vertices),
    );
    context.buffer_update(
        slot.bindings.index_buffer,
        BufferSource::slice(&mesh.indices),
    );
    slot.indices = mesh.indices.len() as i32;
}

/// Hands `buffers` to be deleted on the next draw.
fn retire(buffers: Buffers) {
    let slots = match buffers {
        Buffers::Unsent => Vec::new(),
        Buffers::Fixed(slot) => vec![slot],
        Buffers::Turns { slots, .. } => slots,
    };
    RETIRED.with(|retired| {
        let mut retired = retired.borrow_mut();
        for slot in slots {
            retired.extend(slot.bindings.vertex_buffers);
            retired.push(slot.bindings.index_buffer);
        }
    });
}

impl Drop for GpuMesh {
    fn drop(&mut self) {
        retire(std::mem::replace(self.buffers.get_mut(), Buffers::Unsent));
    }
}

/// Room for `count` items and some to grow.
fn room(count: usize) -> usize {
    ((count as f32 * ROOM_TO_GROW).ceil() as usize).max(1)
}

/// Bindings of `vertex_buffers` and `index_buffer` with the white texture.
fn bindings(
    context: &mut dyn RenderingBackend,
    vertex_buffers: Vec<BufferId>,
    index_buffer: BufferId,
) -> Bindings {
    Bindings {
        vertex_buffers,
        index_buffer,
        images: vec![white(context)],
    }
}

/// The plain white texture, made the first time it is asked for.
fn white(context: &mut dyn RenderingBackend) -> TextureId {
    WHITE.with(|white| *white.get_or_init(|| context.new_texture_from_rgba8(1, 1, &[u8::MAX; 4])))
}

/// Deletes the buffers of meshes dropped since the last draw.
fn delete_retired(context: &mut dyn RenderingBackend) {
    let retired = RETIRED.with(|retired| std::mem::take(&mut *retired.borrow_mut()));
    for buffer in retired {
        context.delete_buffer(buffer);
    }
}

/// Per-instance data drawn with one mesh many times over, written each
/// frame to the next of `TURNS` buffers.
pub struct InstanceBuffer {
    /// The buffers, made on the first write.
    slots: Vec<BufferId>,
    /// The buffer last written.
    current: usize,
    /// Bytes each buffer has room for.
    room: usize,
    /// Instances last written.
    count: usize,
}

impl InstanceBuffer {
    /// No instances yet.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            current: 0,
            room: 0,
            count: 0,
        }
    }

    /// Writes `instances` to the next buffer.
    ///
    /// `T` must be plain data laid out as the material's per-instance
    /// attributes say (`#[repr(C)]`).
    pub fn write<T: Copy>(&mut self, instances: &[T]) {
        let gl = unsafe { get_internal_gl() };
        let context = gl.quad_context;
        let bytes = std::mem::size_of_val(instances);
        if bytes > self.room {
            RETIRED.with(|retired| retired.borrow_mut().append(&mut self.slots));
            self.room = room(bytes);
            self.slots = (0..TURNS)
                .map(|_| {
                    context.new_buffer(
                        BufferType::VertexBuffer,
                        BufferUsage::Stream,
                        BufferSource::empty::<u8>(self.room),
                    )
                })
                .collect();
        }
        self.count = instances.len();
        if self.slots.is_empty() || instances.is_empty() {
            return;
        }
        self.current = (self.current + 1) % self.slots.len();
        context.buffer_update(self.slots[self.current], BufferSource::slice(instances));
    }
}

impl Drop for InstanceBuffer {
    fn drop(&mut self) {
        RETIRED.with(|retired| retired.borrow_mut().append(&mut self.slots));
    }
}

/// One uniform's place among a material's values, in floats.
struct UniformPlace {
    /// Its name in the shader.
    name: String,
    /// Where it starts.
    start: usize,
    /// How many floats it takes.
    length: usize,
}

/// A shader and its pipeline for drawing `GpuMesh`es, with its uniforms'
/// values (see the module documentation).
pub struct GpuMaterial {
    /// The pipeline.
    pipeline: Pipeline,
    /// Where each uniform's value goes in `values`.
    places: Vec<UniformPlace>,
    /// The uniforms' values, packed one after another as Miniquad reads
    /// them.
    values: RefCell<Vec<f32>>,
}

impl GpuMaterial {
    /// A material of the `vertex` and `fragment` shaders, whose uniforms
    /// are `Projection` and `Model` and then `uniforms`, drawn with
    /// `params`, for meshes of Macroquad's `Vertex`; `None` if the shaders
    /// cannot be built.
    pub fn new(
        vertex: &str,
        fragment: &str,
        uniforms: &[(&str, UniformType)],
        params: PipelineParams,
    ) -> Option<Self> {
        let layout = [BufferLayout::default()];
        Self::with_layout(
            vertex,
            fragment,
            uniforms,
            params,
            &layout,
            &vertex_attributes(),
        )
    }

    /// A material as `new` makes, for vertex buffers of `layout` holding
    /// `attributes`, such as a per-instance buffer beside the mesh's.
    pub fn with_layout(
        vertex: &str,
        fragment: &str,
        uniforms: &[(&str, UniformType)],
        params: PipelineParams,
        layout: &[BufferLayout],
        attributes: &[VertexAttribute],
    ) -> Option<Self> {
        let gl = unsafe { get_internal_gl() };
        let context = gl.quad_context;
        let all: Vec<(&str, UniformType)> =
            STANDARD_UNIFORMS.iter().chain(uniforms).copied().collect();
        let meta = ShaderMeta {
            uniforms: UniformBlockLayout {
                uniforms: all
                    .iter()
                    .map(|&(name, kind)| UniformDesc::new(name, kind))
                    .collect(),
            },
            images: vec![TEXTURE_NAME.to_owned()],
        };
        let shader = context
            .new_shader(ShaderSource::Glsl { vertex, fragment }, meta)
            .ok()?;
        let pipeline = context.new_pipeline(layout, attributes, shader, params);
        let mut places = Vec::with_capacity(all.len());
        let mut start = 0;
        for (name, kind) in all {
            let length = kind.size() / FLOAT_BYTES;
            places.push(UniformPlace {
                name: name.to_owned(),
                start,
                length,
            });
            start += length;
        }
        let mut values = vec![0.0; start];
        // An identity `Model` until set, as Macroquad's.
        values[places[1].start..places[1].start + places[1].length]
            .copy_from_slice(&Mat4::IDENTITY.to_cols_array());
        Some(Self {
            pipeline,
            places,
            values: RefCell::new(values),
        })
    }

    /// Sets the uniform `name` to `value`, which must take as many bytes as
    /// the uniform: `f32`, `Vec3`, `Vec4` or `Mat4`, for example.
    ///
    /// # Panics
    ///
    /// Panics if the material has no such uniform, or `value` is the wrong
    /// size for it.
    pub fn set_uniform<T: Copy>(&self, name: &str, value: T) {
        let place = self
            .places
            .iter()
            .find(|place| place.name == name)
            .unwrap_or_else(|| panic!("no uniform {name}"));
        let bytes = std::mem::size_of::<T>();
        assert_eq!(
            bytes,
            place.length * FLOAT_BYTES,
            "uniform {name} is the wrong size"
        );
        let mut values = self.values.borrow_mut();
        let target = &mut values[place.start..place.start + place.length];
        // SAFETY: `T` is plain data of exactly the bytes the floats take.
        unsafe {
            std::ptr::copy_nonoverlapping(
                std::ptr::from_ref(&value).cast::<u8>(),
                target.as_mut_ptr().cast::<u8>(),
                bytes,
            );
        }
    }

    /// Draws `meshes` placed by `model` under the camera now set.
    pub fn draw<'a>(&self, meshes: impl IntoIterator<Item = &'a GpuMesh>, model: Mat4) {
        let meshes: Vec<&GpuMesh> = meshes
            .into_iter()
            .filter(|mesh| !mesh.mesh.indices.is_empty())
            .collect();
        // Nothing to draw: Macroquad's batch is left to go on growing.
        if meshes.is_empty() {
            return;
        }
        self.in_pass(model, |context| {
            for mesh in meshes {
                mesh.with_bindings(context, |context, bindings, indices| {
                    context.apply_bindings(bindings);
                    context.draw(0, indices, 1);
                });
            }
        });
    }

    /// Draws `mesh` once for each instance last written to `instances`,
    /// which feeds the second vertex buffer of the material's layout.
    pub fn draw_instanced(&self, mesh: &GpuMesh, instances: &InstanceBuffer) {
        if instances.count == 0 || instances.slots.is_empty() {
            return;
        }
        self.in_pass(Mat4::IDENTITY, |context| {
            mesh.with_bindings(context, |context, bindings, indices| {
                let both = Bindings {
                    vertex_buffers: vec![
                        bindings.vertex_buffers[0],
                        instances.slots[instances.current],
                    ],
                    index_buffer: bindings.index_buffer,
                    images: bindings.images.clone(),
                };
                context.apply_bindings(&both);
                context.draw(0, indices, instances.count as i32);
            });
        });
    }

    /// Runs `draw` in the pass the camera now set draws into, after what
    /// Macroquad has batched, with the material's pipeline and uniforms
    /// applied, `model` placing what it draws.
    fn in_pass(&self, model: Mat4, draw: impl FnOnce(&mut dyn RenderingBackend)) {
        diagnostics::time(Section::GraphicsCalls, || self.in_pass_untimed(model, draw));
    }

    /// `in_pass`, untimed.
    fn in_pass_untimed(&self, model: Mat4, draw: impl FnOnce(&mut dyn RenderingBackend)) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let projection = gl.quad_gl.get_projection_matrix();
        let pass = gl.quad_gl.get_active_render_pass();
        let context = gl.quad_context;
        delete_retired(context);
        self.set_uniform("Projection", projection);
        self.set_uniform("Model", model);
        context.begin_pass(pass, PassAction::Nothing);
        context.apply_pipeline(&self.pipeline);
        {
            let values = self.values.borrow();
            context.apply_uniforms_from_bytes(
                values.as_ptr().cast::<u8>(),
                values.len() * FLOAT_BYTES,
            );
        }
        draw(context);
        context.end_render_pass();
    }
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/gpu.rs"]
mod tests;
