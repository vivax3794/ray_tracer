#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Line {
    a: vec2<f32>,
    b: vec2<f32>,
    color: vec4<f32>,
    mirror: u32, // bool
}
struct Inputs {
    player: vec2<f32>
}
@group(0) @binding(2) var<uniform> inputs: Inputs;
@group(0) @binding(3) var<storage, read> lines: array<Line>;

const player_size: f32 = 10;

struct RayResult {
    visible: bool,
    sample: vec2<f32>,
    shift: vec4<f32>,
}
struct HitInfo {
    hit: bool,
    pos: vec2<f32>,
    dst: f32
}


struct Ray {
    start: vec2<f32>,
    angle: f32,
    target_dis: f32,
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> { 
    var screen = in.position.xy;
    var size = screen * 1 / in.uv;
    var world = screen - size / 2 + inputs.player; 

    return do_world(world);
}


fn do_world(world: vec2<f32>) -> vec4<f32> {
    var ang = angle(world - inputs.player);
    var ray = Ray(
        inputs.player,
        ang,
        distance(world, inputs.player),
    );

    let res = ray_trace(ray);
    if (res.visible) {
        return background(res.sample) * res.shift;
    }
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}

fn background(world: vec2<f32>) -> vec4<f32> {
    /// Render player
    if distance(world, inputs.player) <= player_size {
        return vec4<f32>(
            1.0,
            1.0,
            1.0,
            1.0
        );
    }
    if distance(world, inputs.player) <= player_size + 5 {
        return vec4<f32>(
            0.0,
            0.5,
            1.0,
            1.0
        );
    }

    var x = abs(world.x) % 50 > 25;
    if world.x < 0 {
        x = !x;
    }
    var y = abs(world.y) % 50 > 25;
    if world.y < 0 {
        y = !y;
    }

    let col = u32(x) ^ u32(y);
    let res = vec3<f32>(0.5, 0.5, 0.5) + vec3<f32>(0.2, 0.2, 0.2) * f32(col);
    //let res = vec3<f32>((world.x + 500.0) / 1000.0, world.y / 1000.0, 0.0);
    return vec4<f32>(res, 1.0);
}

fn ray_trace(ray_inp: Ray) -> RayResult {
    var ray = ray_inp;

    var ignore: u32 = 1000;
    var shift = vec4<f32>(1.0, 1.0, 1.0, 1.0);

    for (var limit = 0; limit < 10; limit++) {
        var hit_line = Line();
        var hit_info = HitInfo(true, vec2<f32>(), 1000000.0);
        var hit_index: u32 = 1000;
        var was_hit = false;

        for (var i: u32 = 0; i < arrayLength(&lines); i++) {
            if i != ignore {
                var line = lines[i];
                let hit = line_hit(ray, line);

                if hit.hit {
                   if hit.dst < hit_info.dst {
                        hit_info = hit;
                        hit_line = line;
                        hit_index = i;
                        was_hit = true;
                   }
                }
            }
        }

        if was_hit {
           if hit_line.mirror == 1 {
               let line_angle = angle(hit_line.b - hit_line.a);
               var angle = 2*line_angle - ray.angle;

               if angle > 3.14 {
                    angle = angle - 6.28;
               }
               if angle < -3.14 {
                    angle = angle + 6.28;
               }

               ray = Ray(
                   hit_info.pos,
                   angle,
                   ray.target_dis - hit_info.dst,
               ); 
               ignore = hit_index;
               shift *= hit_line.color;
               continue;
           }
           return RayResult(false, vec2<f32>(), shift);
        }

        let pos = ray.start + vec2<f32>(cos(ray.angle), sin(ray.angle)) * ray.target_dis;
        return RayResult(true, pos, shift);
    }
    // error 
    return RayResult(false, vec2<f32>(), shift);
}

fn angle(v: vec2<f32>) -> f32 {
    var angle = acos(dot(normalize(v), vec2<f32>(1.0, 0.0)));
    if v.y < 0 {
        return -angle;
    }
    return angle;
}


fn line_hit(ray: Ray, line: Line) -> HitInfo {
    var line_pa = line.a - ray.start;
    var line_pb = line.b - ray.start;

    var ray_a = sin(ray.angle) / cos(ray.angle); 

    var line_a = (line_pb.y - line_pa.y) / (line_pb.x - line_pa.x);
    var line_b = line_pb.y - line_pb.x * line_a;

    // line_a * x + line_b = ray_a * x;
    // lina_a + line_b/x = ray_a;
    // line_b/x = ray_a - line_a;
    // x = line_b / (ray_a - line_a)

    var x = line_b / (ray_a - line_a);

    if line_pa.x == line_pb.x {
        x = line_pa.x;
    }

    var y = ray_a * x;
    var p = vec2<f32>(x, y);

    if abs(angle(p) - ray.angle) > 0.1 {
        return HitInfo();
    }

    var min_x = min(line_pa.x, line_pb.x);
    var max_x = max(line_pa.x, line_pb.x);
    if x < min_x || x > max_x {
        return HitInfo();
    }

    if line_pa.x == line_pb.x {
        var min_y = min(line_pa.y, line_pb.y);
        var may_y = max(line_pa.y, line_pb.y);
        if y < min_y || y > may_y {
            return HitInfo();
        }
    }

    if length(p) > ray.target_dis {
        return HitInfo();
    }
    //if ray.target_dis - length(p) > 100.0 {
    //    return HitInfo();
    //}

    return HitInfo (
        true,
        p + ray.start,
        length(p)
    );
}
