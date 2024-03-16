use std::borrow::Cow;
use anyhow::Ok;
use anyhow::Result;
use pollster::FutureExt;
use wgpu::util::BufferInitDescriptor;
use wgpu::util::DeviceExt;
use wgpu::PipelineLayoutDescriptor;

/// 矩阵计算
/// 参考 https://developer.chrome.com/docs/capabilities/web-apis/gpu-compute?hl=zh-cn

#[allow(dead_code)]
pub fn main() -> Result<()>{
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptionsBase::default())
        .block_on()
        .ok_or(anyhow::anyhow!("Couldn't create the adapter"))?;

    let (device, queue) = adapter
        .request_device(&Default::default(), None)
        .block_on()?;

    // 第一个矩阵
    let first_matrix = &[
        2f32 /* rows */, 4. /* columns */,
        1., 2., 3., 4.,
        5., 6., 7., 8.
      ];
    
    let first_matrix_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        usage: wgpu::BufferUsages::STORAGE,
        contents: bytemuck::cast_slice(first_matrix),
    });

    // 第二个矩阵
    let second_matrix = &[
        4f32 /* rows */, 2. /* columns */,
        1., 2.,
        3., 4.,
        5., 6.,
        7., 8.
      ];
    
    let second_matrix_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        usage: wgpu::BufferUsages::STORAGE,
        contents: bytemuck::cast_slice(second_matrix),
    });

    // 结果矩阵
    let result_matrix_buffer_size = std::mem::size_of::<f32>() * (2 + first_matrix[0] as usize * second_matrix[1] as usize);

    let result_matrix_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: result_matrix_buffer_size as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // 绑定组布局和绑定组
    let bind_group_layout =
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
        label: Some("bind_group_layout"),
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: first_matrix_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: second_matrix_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: result_matrix_buffer.as_entire_binding(),
            },
        ],
        label: Some("bind_group"),
    });

    // 计算着色器

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("compute_shader_module"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/matrix.wgsl"))),
    });

    println!("shader 创建成功:{:?}", shader.global_id());

    // 流水线设置
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("compute_pipeline"),
        layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor{
            label: Some("compute_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        })),
        module: &shader,
        entry_point: "main",
    });

    // 命令提交

    let mut encoder = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: None },
    );

    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default() );
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let workgroup_count_x = (first_matrix[0] / 8.).ceil() as u32;
        let workgroup_count_y = (second_matrix[1] / 8.).ceil() as u32;
        cpass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
    }

    // 获取用于在未映射状态下读取的 GPU 缓冲区
    // 创建一个 GPU 缓冲区作为目的地，以使用 copyBufferToBuffer 复制结果矩阵缓冲区。
    // 最后，使用 copyEncoder.finish() 完成编码命令，然后使用 GPU 命令调用 device.queue.submit()，将这些命令提交到 GPU 设备队列。
    let gpu_read_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: result_matrix_buffer_size as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // 编码用于将缓冲区复制到缓冲区的命令。
    encoder.copy_buffer_to_buffer(&result_matrix_buffer, 0, &gpu_read_buffer, 0, result_matrix_buffer_size as u64);

    // Submit GPU commands.
    queue.submit(Some(encoder.finish()));

    println!("command提交成功..");

    // 读取结果矩阵

    let buffer_slice = gpu_read_buffer.slice(..);
    buffer_slice.map_async(wgpu::MapMode::Read, |_| {});

    device.poll(wgpu::Maintain::Wait);

    let padded = buffer_slice.get_mapped_range();
    let mut data: Vec<u8> = vec![0; result_matrix_buffer_size];
    data.copy_from_slice(&padded);

    println!("读取成功{:?}", data);

    let data:&[f32] = bytemuck::cast_slice(&padded[..]);

    println!("转换成功{:?}", data);
    
    Ok(())
}
