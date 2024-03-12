use anyhow::Ok;
use anyhow::Result;

mod utils;
mod triangle;
mod grayscale;
mod yuv2rgb;

fn main() -> Result<()> {
    // triangle::triangle()?;
    grayscale::grayscale()?;
    Ok(())
}