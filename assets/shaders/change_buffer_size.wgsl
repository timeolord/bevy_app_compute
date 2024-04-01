@group(0) @binding(0)
var<uniform> uni: f32;

@group(0) @binding(1)
var<storage, read_write> my_storage: array<f32>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    for (var i: i32 = 0; i < i32(arrayLength(&my_storage)); i++) {
        my_storage[i] = my_storage[i] + uni;
    }
}