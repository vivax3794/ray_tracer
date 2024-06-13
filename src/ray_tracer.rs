use bevy::{
    core_pipeline::{
        core_2d::graph::{Core2d, Node2d},
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    },
    ecs::query::QueryItem,
    prelude::*,
    render::{
        self,
        extract_component::{
            ComponentUniforms, ExtractComponent,
            ExtractComponentPlugin, UniformComponentPlugin,
        },
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext,
            RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{
                sampler, storage_buffer, storage_buffer_read_only,
                texture_2d, uniform_buffer,
            },
            BindGroupEntries, BindGroupLayout,
            BindGroupLayoutEntries, CachedRenderPipelineId,
            ColorTargetState, ColorWrites, FragmentState,
            GpuArrayBuffer, MultisampleState, Operations,
            PipelineCache, PrimitiveState, RenderPassColorAttachment,
            RenderPassDescriptor, RenderPipelineDescriptor, Sampler,
            SamplerBindingType, SamplerDescriptor, ShaderSize,
            ShaderStages, ShaderType, StorageBuffer, TextureFormat,
            TextureSampleType,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::BevyDefault,
        view::ViewTarget,
        Extract, RenderApp,
    },
};

pub struct RayTracerPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct RayTracerLabel;

impl Plugin for RayTracerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Line>();
        app.add_plugins((
            ExtractComponentPlugin::<ShaderInputs>::default(),
            UniformComponentPlugin::<ShaderInputs>::default(),
        ));

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_graph_node::<ViewNodeRunner<RayTracerNode>>(
                Core2d,
                RayTracerLabel,
            )
            .add_render_graph_edges(
                Core2d,
                (
                    Node2d::Tonemapping,
                    RayTracerLabel,
                    Node2d::EndMainPassPostProcessing,
                ),
            )
            .add_systems(ExtractSchedule, write_lines_buffer);
    }

    fn finish(&self, app: &mut App) {
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.init_resource::<RayTracerPipeline>();
        render_app.init_resource::<LineBuffer>();
    }
}

#[derive(
    Default, Clone, Component, ExtractComponent, ShaderType, Reflect,
)]
pub struct ShaderInputs {
    pub player: Vec2,
}

#[derive(Resource)]
struct LineBuffer(StorageBuffer<Vec<ShaderLine>>);

impl FromWorld for LineBuffer {
    fn from_world(world: &mut World) -> Self {
        let mut buffer: StorageBuffer<Vec<ShaderLine>> =
            Vec::new().into();

        let device = world.resource::<RenderDevice>();
        let queue = world.resource::<RenderQueue>();

        buffer.write_buffer(device, queue);

        Self(buffer)
    }
}

#[derive(Clone, ShaderType)]
struct ShaderLine {
    a: Vec2,
    b: Vec2,
    color: Vec4,
    mirror: u32,
}

#[derive(Component, Clone, ExtractComponent, Reflect)]
pub struct Line {
    pub a: Vec2,
    pub b: Vec2,
    pub kind: LineKind,
}

#[derive(Clone, Default, Reflect)]
#[reflect(Default)]
pub enum LineKind {
    #[default]
    Solid,
    Mirror(Color),
}

impl Line {
    fn to_gpu(&self) -> ShaderLine {
        match self.kind {
            LineKind::Solid => ShaderLine {
                a: self.a,
                b: self.b,
                color: Vec4::ZERO,
                mirror: 0,
            },
            LineKind::Mirror(col) => ShaderLine {
                a: self.a,
                b: self.b,
                color: col.rgba_to_vec4(),
                mirror: 1,
            },
        }
    }
}

#[derive(Default)]
struct RayTracerNode;

impl ViewNode for RayTracerNode {
    type ViewQuery = &'static ViewTarget;

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        view_target: QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let post_process_pipeline =
            world.resource::<RayTracerPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) = pipeline_cache
            .get_render_pipeline(post_process_pipeline.pipeline_id)
        else {
            return Ok(());
        };

        let post_process = view_target.post_process_write();

        let input_uniforms =
            world.resource::<ComponentUniforms<ShaderInputs>>();
        let Some(input_binding) = input_uniforms.uniforms().binding()
        else {
            return Ok(());
        };

        let Some(lines) = world.get_resource::<LineBuffer>() else {
            return Ok(());
        };
        let Some(lines) = lines.0.binding() else {
            println!("Lines binding not found");
            return Ok(());
        };

        let bind_group =
            render_context.render_device().create_bind_group(
                "post_process_bind_group",
                &post_process_pipeline.layout,
                &BindGroupEntries::sequential((
                    post_process.source,
                    &post_process_pipeline.sampler,
                    input_binding.clone(),
                    lines.clone(),
                )),
            );

        let mut render_pass = render_context
            .begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("post_process_pass"),
                color_attachments: &[Some(
                    RenderPassColorAttachment {
                        view: post_process.destination,
                        resolve_target: None,
                        ops: Operations::default(),
                    },
                )],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[derive(Resource)]
struct RayTracerPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for RayTracerPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "post_process_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float {
                        filterable: true,
                    }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<ShaderInputs>(false),
                    storage_buffer_read_only::<Vec<ShaderLine>>(
                        false,
                    ),
                ),
            ),
        );

        let sampler = render_device
            .create_sampler(&SamplerDescriptor::default());

        // Get the shader handle
        let shader = world
            .resource::<AssetServer>()
            .load("shaders/tracer.wgsl");

        let pipeline_id = world
            .resource_mut::<PipelineCache>()
            .queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("post_process_pipeline".into()),
                layout: vec![layout.clone()],
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader,
                    shader_defs: vec![],
                    entry_point: "fragment".into(),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::bevy_default(),
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                push_constant_ranges: vec![],
            });

        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}

fn write_lines_buffer(
    query: Extract<Query<&Line>>,
    mut buffer: ResMut<LineBuffer>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let vec = buffer.0.get_mut();
    vec.clear();

    for line in query.iter() {
        vec.push(line.to_gpu())
    }

    buffer.0.write_buffer(&device, &queue);
}
