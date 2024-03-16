use anyhow::Ok;
use anyhow::Result;

mod utils;
mod triangle;
mod grayscale;
mod yuv2rgb;
mod matrix1;
mod matrix2;
mod index;

fn main() -> Result<()> {
    // triangle::triangle()?;
    // grayscale::grayscale()?;
    // yuv2rgb::yuv2rgb()?;
    // matrix1::main()?;
    // matrix2::main()?;
    index::main()?;
    Ok(())
}
