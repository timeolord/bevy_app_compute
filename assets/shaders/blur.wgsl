@group(0) @binding(0)
var<storage, read> image: array<f32, 256>;
@group(0) @binding(1)
var<storage, read_write> result: array<f32, 256>;
@group(0) @binding(2)
var<storage, read> image_size: vec2<u32>;
@group(0) @binding(3)
var<storage, read> blur_size: vec2<u32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let pos = vec2<i32>(invocation_id.xy);
    let radius = vec2<i32>(blur_size / 2);
    let image_size = vec2<i32>(image_size);
    var sum = 0.0;
    var length = 0;
    for (var x: i32 = -radius.x; x < radius.x; x++) {
        for (var y: i32 = -radius.y; y < radius.y; y++) {
            let offset = vec2<i32>(x, y);
            let sample_pos = pos + offset;
            if (sample_pos.x < 0 || sample_pos.x >= image_size.x || sample_pos.y < 0 || sample_pos.y >= image_size.y) {
                continue;
            }
            let sample_index = sample_pos.y * image_size.x + sample_pos.x;
            sum += image[sample_index];
            length += 1;
        }
    }
    workgroupBarrier();
    let pos_index = pos.y * image_size.x + pos.x;
    result[pos_index] = sum / f32(length);
}