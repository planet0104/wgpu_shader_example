#![allow(dead_code)]

use std::borrow::Cow;
use anyhow::Ok;
use anyhow::Result;
use pollster::FutureExt;
use wgpu::util::BufferInitDescriptor;
use wgpu::util::DeviceExt;

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

    // 数组
    let input_array = &mut [0f32; 100];
    for (idx, x) in input_array.iter_mut().enumerate(){
        *x = idx as f32;
    }
    println!("input_array:{:?}", input_array);
    
    let input_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        usage: wgpu::BufferUsages::STORAGE,
        contents: bytemuck::cast_slice(input_array),
    });

    // 结果数组
    let output_buffer_size = std::mem::size_of::<f32>() * input_array.len();

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_buffer_size as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // 计算着色器

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("compute_shader_module"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/index.wgsl"))),
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
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            }
        ],
        label: Some("bind_group"),
    });

    println!("shader 创建成功:{:?}", shader.global_id());

    // 命令提交

    let mut encoder = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: None },
    );

    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default() );
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let workgroup_count_x = (input_array.len() as f32 / 8.).ceil() as u32;
        println!("workgroup_count_x={workgroup_count_x}");
        cpass.dispatch_workgroups(workgroup_count_x, 1, 1);
    }

    // 获取用于在未映射状态下读取的 GPU 缓冲区
    let gpu_read_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output_buffer_size as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // 编码用于将缓冲区复制到缓冲区的命令。
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &gpu_read_buffer, 0, output_buffer_size as u64);

    // Submit GPU commands.
    queue.submit(Some(encoder.finish()));

    println!("command提交成功..");

    // 读取结果矩阵

    let buffer_slice = gpu_read_buffer.slice(..);
    buffer_slice.map_async(wgpu::MapMode::Read, |_| {});

    device.poll(wgpu::Maintain::Wait);

    let padded = buffer_slice.get_mapped_range();
    let mut data: Vec<u8> = vec![0; output_buffer_size];
    data.copy_from_slice(&padded);

    let data:&[f32] = bytemuck::cast_slice(&padded[..]);

    println!("转换成功{:?}", data);
    
    Ok(())
}
