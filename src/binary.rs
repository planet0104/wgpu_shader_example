use std::borrow::Cow;
use anyhow::Ok;
use anyhow::Result;
use image::load_from_memory;
use pollster::FutureExt;

use crate::utils::padded_bytes_per_row;

/// 填充索引

pub fn main() -> Result<()>{
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptionsBase::default())
        .block_on()
        .ok_or(anyhow::anyhow!("Couldn't create the adapter"))?;

    let (device, queue) = adapter
        .request_device(&Default::default(), None)
        .block_on()?;

    // 输入图像
    let input_image = load_from_memory(include_bytes!("../images/capture.jpg"))?.to_rgba8();
    let (width, height) = input_image.dimensions();
    println!("图像大小:{}x{}", width, height);
    
    let texture_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let input_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("input texture"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // 输出图像
    let output_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("output texture"),
        size: texture_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    });

    // 计算着色器

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("compute_shader_module"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/binary.wgsl"))),
    });

    // 流水线设置
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("compute_pipeline"),
        layout: None,
        module: &shader,
        entry_point: "main",
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &compute_pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &input_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(
                    &output_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                ),
            },
        ],
        label: Some("bind_group"),
    });

    println!("shader 创建成功:{:?}", shader.global_id());

    // 命令提交

    queue.write_texture(
        input_texture.as_image_copy(),
        bytemuck::cast_slice(input_image.as_raw()),
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: None, // Doesn't need to be specified as we are writing a single image.
        },
        texture_size,
    );

    let mut encoder = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: None },
    );

    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default() );
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let workgroup_count_x = (width + 8 - 1) / 8;
        let workgroup_count_y = (height + 8 - 1) / 8;
        println!("workgroup_count_x={workgroup_count_x}");
        println!("workgroup_count_y={workgroup_count_y}");
        cpass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
    }

    let padded_bytes_per_row = padded_bytes_per_row(width);
    let unpadded_bytes_per_row = width as usize * 4;

    let output_buffer_size =
        padded_bytes_per_row as u64 * height as u64 * std::mem::size_of::<u8>() as u64;
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            aspect: wgpu::TextureAspect::All,
            texture: &output_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        },
        wgpu::ImageCopyBuffer {
            buffer: &output_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row as u32),
                rows_per_image: Some(height),
            },
        },
        texture_size,
    );

    // Submit GPU commands.
    queue.submit(Some(encoder.finish()));

    println!("command提交成功..");

    // 读取结果

    let buffer_slice = output_buffer.slice(..);
    buffer_slice.map_async(wgpu::MapMode::Read, |_| {});

    device.poll(wgpu::Maintain::Wait);

    let padded_data = buffer_slice.get_mapped_range();

    let mut pixels: Vec<u8> = vec![0; unpadded_bytes_per_row * height as usize];
    for (padded, pixels) in padded_data
        .chunks_exact(padded_bytes_per_row)
        .zip(pixels.chunks_exact_mut(unpadded_bytes_per_row))
    {
        pixels.copy_from_slice(&padded[..unpadded_bytes_per_row]);
    }

    if let Some(output_image) =
        image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, &pixels[..])
    {
        println!("二值图转换成功{}x{}", output_image.width(), output_image.height());
        output_image.save("./outputs/binary.png")?;
    }
    
    Ok(())
}
