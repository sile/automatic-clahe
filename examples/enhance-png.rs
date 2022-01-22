use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    image_path: PathBuf,

    #[structopt(long, default_value = "enhanced.png")]
    output_path: PathBuf,

    #[structopt(long, default_value = "32")]
    block_width: usize,

    #[structopt(long, default_value = "32")]
    block_height: usize,

    #[structopt(long, default_value = "100")]
    alpha: f32,

    #[structopt(long, default_value = "1.5")]
    p: f32,

    #[structopt(long, default_value = "50")]
    d_threshold: u8,
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();

    let file = std::fs::File::open(&opt.image_path)?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info()?;

    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    println!("Image resolution: {}x{}", info.width, info.height);
    println!("Image bit depth: {:?}", info.bit_depth);
    println!("Image color type: {:?}", info.color_type);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);

    let options = automatic_clahe::AutomaticClaheOptions {
        block_width: opt.block_width,
        block_height: opt.block_height,
        alpha: opt.alpha,
        p: opt.p,
        d_threshold: opt.d_threshold,
    };
    let enhancer = automatic_clahe::AutomaticClahe::with_options(options);
    let start = std::time::Instant::now();
    match reader.info().color_type {
        png::ColorType::Rgb => {
            enhancer.enhance_rgb_image(&mut buf, info.width as usize);
        }
        png::ColorType::Rgba => {
            enhancer.enhance_rgba_image(&mut buf, info.width as usize);
        }
        ty => {
            panic!("Unsupported color type: {:?}", ty);
        }
    }
    println!("Elapsed: {:?}", start.elapsed());

    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(std::fs::File::create(&opt.output_path)?),
        info.width,
        info.height,
    );
    encoder.set_color(info.color_type);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&buf)?;

    println!("Output path: {:?}", opt.output_path);

    Ok(())
}
