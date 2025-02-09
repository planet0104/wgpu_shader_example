#![allow(dead_code, unused_imports)]

use std::borrow::Cow;
use std::time::Instant;

use anyhow::Ok;
use anyhow::Result;
use pollster::FutureExt;
use wgpu::MemoryHints;
use wgpu::PipelineCompilationOptions;

use crate::utils::padded_bytes_per_row;

//参考： https://github.com/firdawolf/gameview/blob/71bf4a109dc37c390a34e45ba2870b7063cd7e18/src/wgpugst/qtpreceive/wgpusurface.rs#L468

pub fn yuv2rgb() -> Result<()> {
    let src_image = image::load_from_memory(include_bytes!("../images/capture.jpg"))?.to_rgba8();
    let (width, height) = (src_image.width(), src_image.height());
    let src_yuv = include_bytes!("../images/capture.yuv");
    
    // let test_num = 2000;
    // let t = Instant::now();
    // let mut total_len = 0;
    // for _ in 0..test_num{
    //     let rgb_data = yuv_to_rgba_cpu(src_yuv, width as i32, height as i32);
    //     total_len += rgb_data.len();
    // }
    // println!("CPU YUV to RGBA 转换耗时:{}ms total_len={total_len} 次数:{test_num}",t.elapsed().as_millis());

    //获取Y数据和UV数据
    let y_data = &src_yuv[..(width*height) as usize];
    let uv_data = &src_yuv[(width*height) as usize..];

    // println!("src_yuv:{}", src_yuv.len());
    // println!("y_data:{}", y_data.len());
    // println!("uv_data:{}", uv_data.len());

    //------------------------------------------------------
    // 初始化硬件设备
    //------------------------------------------------------

    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptionsBase {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .block_on()
        .ok_or(anyhow::anyhow!("Couldn't create the adapter"))?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER,
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: MemoryHints::default(),
            },
            None,
        )
        .block_on()?;

    //------------------------------------------------------
    // 创建 pipeline layout、compute pipeline、bind group layout 和 shader module
    //------------------------------------------------------

    let compute_texture_yuv_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(
                        // SamplerBindingType::Comparison is only for TextureSampleType::Depth
                        // SamplerBindingType::Filtering if the sample_type of the texture is:
                        //     TextureSampleType::Float { filterable: true }
                        // Otherwise you'll get an error.
                        wgpu::SamplerBindingType::Filtering,
                    ),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

    let compute_yuv_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&compute_texture_yuv_bind_group_layout],
            push_constant_ranges: &[],
        });

    let compute_pipeline_yuv = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("compute_pipeline"),
        layout: Some(&compute_yuv_pipeline_layout),
        module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compute_shader_module"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/yuv2rgb.wgsl"))),
        }),
        entry_point: Some("main"),
        compilation_options: PipelineCompilationOptions::default(),
        cache: None
    });

    //------------------------------------------------------
    // 创建纹理、纹理视图、采样器和缓冲区，并设置它们的相关描述符
    //------------------------------------------------------
    
    let texture_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let u_size = wgpu::Extent3d {
        width: width / 2,
        height: height / 2,
        depth_or_array_layers: 1,
    };

    let y_texture = device.create_texture(&wgpu::TextureDescriptor {
        // All textures are stored as 3D, we represent our 2D texture
        // by setting depth to 1.
        size: texture_size,
        mip_level_count: 1, // We'll talk about this a little later
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Most images are stored using sRGB so we need to reflect that here.
        format: wgpu::TextureFormat::R8Unorm,
        // TEXTURE_BINDING tells wgpu that we want to use this texture in shaders
        // COPY_DST means that we want to copy data to this texture
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: Some("y_texture"),
        view_formats: &[],
    });
    let u_texture = device.create_texture(&wgpu::TextureDescriptor {
        // All textures are stored as 3D, we represent our 2D texture
        // by setting depth to 1.
        size: u_size,
        mip_level_count: 1, // We'll talk about this a little later
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Most images are stored using sRGB so we need to reflect that here.
        format: wgpu::TextureFormat::Rg8Unorm,
        // TEXTURE_BINDING tells wgpu that we want to use this texture in shaders
        // COPY_DST means that we want to copy data to this texture
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: Some("uv_texture"),
        view_formats: &[],
    });

    let easu_texture = device.create_texture(&wgpu::TextureDescriptor {
        // All textures are stored as 3D, we represent our 2D texture
        // by setting depth to 1.
        size: texture_size,
        mip_level_count: 1, // We'll talk about this a little later
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Most images are stored using sRGB so we need to reflect that here.
        // format: wgpu::TextureFormat::Rgba8Unorm,
        format: wgpu::TextureFormat::Rgba8Unorm,
        // TEXTURE_BINDING tells wgpu that we want to use this texture in shaders
        // COPY_DST means that we want to copy data to this texture
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::STORAGE_BINDING,
        label: Some("diffuse_texture"),
        view_formats: &[],
    });

    let y_texture_view = y_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let u_texture_view = u_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let easu_texture_view = easu_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let uv_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToBorder,
        address_mode_v: wgpu::AddressMode::ClampToBorder,
        address_mode_w: wgpu::AddressMode::ClampToBorder,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let compute_yuv_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &compute_texture_yuv_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&y_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&u_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&uv_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&easu_texture_view),
            },
        ],
        label: Some("yuv_bind_group2"),
    });

    // println!("write y_texture texture_size={:?}", texture_size);

    //------------------------------------------------------
    // 创建encoder(命令编码器)
    //------------------------------------------------------

    // let t = Instant::now();
    // let mut total_len = 0;
    // for _ in 0..test_num{
        //------------------------------------------------------
        // YUV数据写入纹理中
        //------------------------------------------------------

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &y_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &y_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width as u32),
                rows_per_image: Some(height as u32),
            },
            texture_size,
        );

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &u_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &uv_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width as u32),
                rows_per_image: Some(height as u32),
            },
            u_size,
        );
        

        //------------------------------------------------------
        // 开始新的计算 pass
        //------------------------------------------------------

        let mut encoder = device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: None },
        );

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default() );
            cpass.set_pipeline(&compute_pipeline_yuv);
            cpass.set_bind_group(0, &compute_yuv_bind_group, &[]);
            cpass.dispatch_workgroups(width / 8, height / 8, 1);
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
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &easu_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row as u32),
                    rows_per_image: Some(height),
                },
            },
            texture_size,
        );

        queue.submit(Some(encoder.finish()));

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

        // total_len += pixels.len();
    // }
    
    // println!("GPU YUV to RGBA 转换耗时:{}ms total_len={total_len} 次数:{test_num}", t.elapsed().as_millis());

    if let Some(output_image) =
        image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, &pixels[..])
    {
        output_image.save("./outputs/capture.png")?;
    }
    Ok(())
}

pub fn yuv_to_rgba_cpu(data:&[u8], width:i32, height:i32) -> Vec<u8>{
    let frame_size = width * height;
    let mut yp = 0;
    let mut rgba_data = Vec::with_capacity(frame_size as usize*4);
    for j in 0..height{
        let (mut uvp, mut u, mut v) = ((frame_size + (j >> 1) * width) as usize, 0, 0);
        for i in 0..width{
            let mut y = (0xff & data[yp] as i32) - 16;  
            if y < 0 { y = 0; }
            if i & 1 == 0{
                v = (0xff & data[uvp] as i32) - 128;
                uvp += 1;
                u = (0xff & data[uvp] as i32) - 128;  
                uvp += 1;
            }

            let y1192 = 1192 * y;  
            let mut r = y1192 + 1634 * v;
            let mut g = y1192 - 833 * v - 400 * u;
            let mut b = y1192 + 2066 * u;

            if r < 0 { r = 0; } else if r > 262143 { r = 262143; };
            if g < 0 { g = 0; } else if g > 262143 { g = 262143; }
            if b < 0 { b = 0;} else if b > 262143 { b = 262143; }

            let r = (r>>10) & 0xff;
            let g = (g>>10) & 0xff;
            let b = (b>>10) & 0xff;
            rgba_data.extend_from_slice(&[r as u8, g as u8, b as u8, 255]);
            yp += 1;
        }
    }

    rgba_data
}