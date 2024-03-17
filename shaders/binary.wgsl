@group(0) @binding(0) var input_texture : texture_2d<f32>;
@group(0) @binding(1) var output_texture : texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16,16)
fn main(@builtin(global_invocation_id) global_id : vec3u) {
    let dimensions = textureDimensions(input_texture);
    let coords = vec2<i32>(global_id.xy);

    if(coords.x >= i32(dimensions.x) || coords.y >= i32(dimensions.y)) {
        return;
    }

    /*
        算法: 基于视网膜原理的边缘检测
        JiaYe 2018年1月

        视网膜水平细胞和双极细胞的功能如下:
        双极细胞 -- 亮光兴奋，弱光抑制。
        水平细胞 -- 亮光抑制，弱光兴奋，和双极细胞正好相反。

        算法：
        1.把每个像素点当作一个双极细胞，其右边和下边的像素点看作水平细胞，将像素点的亮度作为细胞输入。
        2.给定一个阈值，双极细胞和水平细胞根据阈值判断输入自身的是亮光还是弱光。
        3.计算将三个细胞的输出之和(双极细胞取两次)，如果没有抵消那么代表检测到一个边缘，否则没有检测到边缘。
        
        举例说明:
        
        B H B H B H
        H b h B H B
        B h B H B H
        H B H B H B

        上图中，字母代表图片的像素，B代表双极细胞, H代表水平细胞。
        小写b点代表当前像素点，那么当前像素点的输出等于4个细胞输出值之和除以4:
        pixel(1,1) = Sum(outB+outH+outB+outH)/4 (左下两个h点各取一次, b点取两次)))
        
        B和H的输出，根据亮度计算,如果像素亮度超过阈值，B输出255，H输出-255，没有超过阈值，二者都输出0。
    */

    let threshold = 0.19;
    let val = vec3<f32>(0.299, 0.587, 0.114);

    // >>>> 计算双极细胞输出 >>>>>
    // (当前细胞为双极细胞 -- 亮光兴奋，弱光抑制)
    let bipolar_cell_color = textureLoad(input_texture, coords.xy, 0);
    var bipolar_cell_output = -1.;
    if dot(val, bipolar_cell_color.rgb) >= threshold{
        bipolar_cell_output = 1.;
    }

    // >>>> 计算右侧水平细胞输出(亮光抑制，弱光兴奋) >>>>
    let right_horizontal_cell_color = textureLoad(input_texture, vec2<i32>(coords.x+1, coords.y), 0);
    var right_horizontal_cell_output = 1.;
    if dot(val, right_horizontal_cell_color.rgb) >= threshold{
        right_horizontal_cell_output = -1.;
    }

    // >>>> 计算下边水平细胞输出(亮光抑制，弱光兴奋) >>>>
    let bottom_horizontal_cell_color = textureLoad(input_texture, vec2<i32>(coords.x, coords.y+1), 0);
    var bottom_horizontal_cell_output = 1.;
    if dot(val, bottom_horizontal_cell_color.rgb) >= threshold{
        bottom_horizontal_cell_output = -1.;
    }

    var output = 0.;
    if bipolar_cell_output*2.0+right_horizontal_cell_output+bottom_horizontal_cell_output != 0.0{
        output = 1.;
    }
    
    textureStore(output_texture, coords.xy, vec4<f32>(output, output, output, 1.));
}
