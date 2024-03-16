struct ArrayData {
    data: array<f32>,
}

@group(0) @binding(0) var<storage, read> input_array : ArrayData;
@group(0) @binding(1) var<storage, read_write> output_array : ArrayData;

@compute @workgroup_size(8)
fn main(@builtin(global_invocation_id) global_id : vec3u) {
    output_array.data[global_id.x] = input_array.data[global_id.x] +1.;
}
