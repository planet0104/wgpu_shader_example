use anyhow::Ok;
use anyhow::Result;

mod utils;
mod triangle;
mod grayscale;
mod yuv2rgb;
mod matrix1;
mod matrix2;
mod index;
mod binary;
mod rotate;

fn main() -> Result<()> {
    // 绘制三角形
    triangle::triangle()?;
    // 灰度图片
    // grayscale::grayscale()?;
    // yuv转rgb
    // yuv2rgb::yuv2rgb()?;
    // 矩阵计算
    // matrix1::main()?;
    // matrix2::main()?;
    // 填充索引
    // index::main()?;
    // 图像二值化
    // binary::main()?;
    // 图像旋转
    // rotate::main()?;
    Ok(())
}
