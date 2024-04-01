@group(0) @binding(0)
var<storage, read_write> my_storage: array<f32>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    my_storage[invocation_id.x] = f32(invocation_id.x) + 1;
}