#import constants

@group(0) @binding(0)
var<storage, read_write> result: array<f32, 1>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    result[0] = f32(constants::X);
}