pub mod font;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use clap::Parser;
use image::{GrayImage, ImageFormat, Rgba32FImage, RgbImage};
use rand::RngExt;
use crate::font::CHARACTERS;

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
enum BlendMode {
    Average,
    Geometric,
    Brightest,
    Darkest,
}

impl BlendMode {

    //blends the values using the specified method
    pub fn blend(&self, colors: &[f64]) -> f64 {
        let mut res = 0.0;


        if *self == BlendMode::Geometric {
            res = 1.0;
        }

        for c in colors {
            match self {
                BlendMode::Average => res += c,
                BlendMode::Geometric => res *= c,
                BlendMode::Brightest => res = res.max(*c),
                BlendMode::Darkest => res = res.min(*c),
            }
        }

        match self {
            BlendMode::Average => res /= colors.len() as f64,
            BlendMode::Geometric => res = res.sqrt(),
            BlendMode::Brightest |
            BlendMode::Darkest => {}
        }


        res
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
enum SampleMode {
    Average,
    Geometric,
    Random,
    Center,
}

impl SampleMode {
    pub fn sample(&self, colors: &[[f64; 3]]) -> [f64; 3] {
        if let SampleMode::Average | SampleMode::Geometric = self {

            let mut res = [0.0; 3];

            if let SampleMode::Geometric = self {
                res.fill(1.0);
            }

            for c in colors {
                if let SampleMode::Average = self {
                    res[0] += c[0];
                    res[1] += c[1];
                    res[2] += c[2];
                } else {
                    res[0] *= c[0];
                    res[1] *= c[1];
                    res[2] *= c[2];
                }
            }

            if let SampleMode::Average = self {
                res[0] /= colors.len() as f64;
                res[1] /= colors.len() as f64;
                res[2] /= colors.len() as f64;
            } else {

                res[0] = res[0].sqrt();
                res[1] = res[1].sqrt();
                res[2] = res[2].sqrt();
            }

            res

        } else {
            if let SampleMode::Random = self {
                let mut rng = rand::rng();

                let n = rng.random_range(0..colors.len());
                colors[n]
            } else {
                colors[colors.len() / 2]
            }
        }
    }
}

impl FromStr for SampleMode {

    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "average" | "avg" => Ok(Self::Average),
            "geometric" | "geo" => Ok(Self::Geometric),
            "random" | "rand" => Ok(Self::Random),
            "center" | "cent" => Ok(Self::Center),
            s => Err(anyhow::Error::msg(format!("Unknown sampling type: \"{}\"", s)))
        }
    }
}

impl FromStr for BlendMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "average" | "avg" => Ok(Self::Average),
            "geometric" | "geo" => Ok(Self::Geometric),
            "brightest" | "bright" | "max" => Ok(Self::Brightest),
            "darkest" | "dark" | "min" => Ok(Self::Darkest),
            _ => Err(anyhow::Error::msg(format!("Unknown blend type \"{}\"", s)))
        }
    }
}

#[derive(Parser)]
struct ProgramArgs {

    #[arg(short='I', long, required = true)]
    image: PathBuf,

    //default is pixels are grouped into 2*2 areas, all 4 colors are blended
    //and the result is turned into a ascii pixel
    #[arg(short, long, default_value = "2")]
    group: u8,

    #[arg(short, long, default_value = "average")]
    blend: BlendMode,

    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(long, default_value = "1.0")]
    multiplier: f64,

    #[arg(long, short = 'D', default_value = "1")]
    denoise_iterations: u8,

    #[arg(long, default_value = "2")]
    noise_thresh: u8,


    ///only used for outputting to a image
    #[arg(long, default_value = "1")]
    character_spacing: u16,

    #[arg(short = 'S', long, default_value = "1.0")]
    scale: f64,

    #[arg(long)]
    scale_blend: Option<BlendMode>,


    //whether to output in color or not
    #[arg(long)]
    color: bool,

    #[arg(short='T', long, default_value = "avg")]
    sample_method: SampleMode
}

#[allow(unused)]
struct GenContext {
    image: Rgba32FImage,
    group_size: u8,
    oheight: usize,
    owidth: usize,
    //supports color now aswell
    char_buf: Vec<u8>,
    color_buf: Vec<[f64; 3]>,
    denoise_buf: Vec<u8>,
    bright_buf: Vec<f64>,
    cbuf: Vec<[f64; 3]>,
    blend_mode: BlendMode,
    bright_multiplier: f64,
    noise_thresh: u8,
    sample_mode: SampleMode,
}

const BRIGHT_MAP: &str = " .,:ilwW08NXQM@#";
const BRIGHT_MAP_LEN: usize = BRIGHT_MAP.len();

impl GenContext {
    pub fn update(&mut self, x: usize, y: usize) {
        let mut i = 0;
        let img_x = x * self.group_size as usize;
        let img_y = y * self.group_size as usize;
        for dx in 0..self.group_size as usize {
            for dy in 0..self.group_size as usize {
                let px = img_x + dx;
                let py = img_y + dy;

                if px >= self.image.width() as usize || py >= self.image.height() as usize {
                    continue;
                }

                let pixel = self.image.get_pixel(px as u32, py as u32).0;

                let b =
                    0.299 * pixel[0] as f64 +
                    0.587 * pixel[1] as f64 +
                    0.114 * pixel[2] as f64;

                self.bright_buf[i] = (b * self.bright_multiplier).clamp(0.0, 1.0);
                self.cbuf[i][0] = pixel[0] as f64;
                self.cbuf[i][1] = pixel[1] as f64;
                self.cbuf[i][2] = pixel[2] as f64;

                i += 1;
            }
        }

        let pixel_count = i;
        let brightness = self.blend_mode.blend(&self.bright_buf[0..pixel_count]);
        let flat = y * self.owidth + x;

        self.char_buf[flat] = (brightness * (BRIGHT_MAP.len()-1) as f64) as u8;
        self.color_buf[flat] = self.sample_mode.sample(&self.cbuf[0..pixel_count]);
    }

    pub fn denoise(&mut self, x: usize, y: usize) {

        //get the mode
        let mut count = [0; BRIGHT_MAP_LEN];

        for i in -1..1 {
            for j in -1..1 {
                let x = x as isize + i;
                let y = y as isize + j;

                if x >= self.owidth as isize || y >= self.oheight as isize || x < 0 || y < 0 {
                    continue;
                }

                count[self.char_buf[y as usize * self.owidth + x as usize] as usize] += 1;
            }
        }

        let max = count.iter().enumerate().max_by(
            |(i, c1), (j, c2)| {
                (**c1).cmp(*c2)
            }
        ).unwrap().0 as u8;

        let diff = max.abs_diff(self.char_buf[y * self.owidth + x]);

        if diff > self.noise_thresh {
            self.denoise_buf[y * self.owidth + x] = max;

        } else {
            self.denoise_buf[y * self.owidth + x] = self.char_buf[y * self.owidth + x];
        }

    }

    pub fn swap_denoise(&mut self) {
        std::mem::swap(&mut self.denoise_buf, &mut self.char_buf);
    }
}

fn downsample_bufs(
    char_buf: &[u8],
    color_buf: &[[f64; 3]],
    width: usize,
    height: usize,
    target_scale: f64,
    blend_mode: BlendMode,
) -> (Vec<u8>, Vec<[f64; 3]>, usize, usize) {
    let mut cur_chars = char_buf.to_vec();
    let mut cur_colors = color_buf.to_vec();
    let mut w = width;
    let mut h = height;
    let mut cur_scale = 1.0f64;

    while w >= 2 && h >= 2 && cur_scale / 2.0 >= target_scale {
        let new_w = w / 2;
        let new_h = h / 2;
        let mut next_chars = vec![0u8; new_w * new_h];
        let mut next_colors = vec![[0.0f64; 3]; new_w * new_h];

        for y in 0..new_h {
            for x in 0..new_w {
                let idxs = [
                    (y * 2    ) * w + (x * 2    ),
                    (y * 2    ) * w + (x * 2 + 1),
                    (y * 2 + 1) * w + (x * 2    ),
                    (y * 2 + 1) * w + (x * 2 + 1),
                ];

                let brightness_samples = idxs.map(|i| cur_chars[i] as f64 / (BRIGHT_MAP_LEN - 1) as f64);
                let blended = blend_mode.blend(&brightness_samples);
                next_chars[y * new_w + x] = (blended * (BRIGHT_MAP_LEN - 1) as f64).round() as u8;

                let mut avg = [0.0f64; 3];
                for &i in &idxs {
                    avg[0] += cur_colors[i][0];
                    avg[1] += cur_colors[i][1];
                    avg[2] += cur_colors[i][2];
                }
                next_colors[y * new_w + x] = avg.map(|c| c / 4.0);
            }
        }

        cur_chars = next_chars;
        cur_colors = next_colors;
        w = new_w;
        h = new_h;
        cur_scale /= 2.0;
    }

    (cur_chars, cur_colors, w, h)
}

pub fn get_image_format(path: &Path, default: Option<ImageFormat>) -> ImageFormat {

    match path.extension()
        .expect("Cannot infer image type because it has no extension!")
        .to_string_lossy()
        .to_string()
        .to_lowercase()
        .as_str() {

        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "avif" => ImageFormat::Avif,
        e => if let Some(fmt) = default { fmt } else { panic!("Unsupported image type: {}", e) }
    }
}

fn main() {
    let args = ProgramArgs::parse();

    let format = get_image_format(&args.image, None);


    let image = image::load(
        BufReader::new(
            File::open(args.image).expect("Failed to find image")
        ), format
    ).expect("Failed to load image from file").into_rgba32f();

    let oheight = (image.height() / args.group as u32) as usize;
    let owidth =  (image.width() / args.group as u32) as usize;

    let b_map = vec![
        0; owidth * oheight
    ];
    let color_buf = vec![
        [0.0; 3]; owidth * oheight
    ];

    let denoise_buf = vec![
        0; owidth * oheight
    ];
    let bright_buf = vec![0.0; (args.group * args.group) as usize];
    let cbuf = vec![[0.0; 3]; (args.group * args.group) as usize];

    let mut context = GenContext {
        image,
        oheight,
        owidth,
        char_buf: b_map,
        color_buf,
        bright_buf,
        cbuf,
        group_size: args.group,
        blend_mode: args.blend,
        denoise_buf,
        bright_multiplier: args.multiplier,
        noise_thresh: args.noise_thresh,
        sample_mode: args.sample_method,
    };

    for y in 0..oheight {
        for x in 0..owidth {

            context.update(x, y)

        }
    }

    for i in 0..args.denoise_iterations {
        //denoise it: basically, take the mode of the pixels in a 9x9 area, if a pixel is
        //within 2 of the mode, leave it be, otherwise set it to the mode
        for y in 0..oheight {
            for x in 0..owidth {
                context.denoise(x, y);
            }
        }

        context.swap_denoise();
    }


    let scale = args.scale;
    let (output_buf, output_color_buf, output_w, output_h) = if scale < 1.0 {
        let blend = args.scale_blend.unwrap_or(context.blend_mode);
        downsample_bufs(&context.char_buf, &context.color_buf, owidth, oheight, scale, blend)
    } else {
        let sw = ((owidth as f64 * scale).round() as usize).max(1);
        let sh = ((oheight as f64 * scale).round() as usize).max(1);
        let mut scaled_chars = vec![0u8; sw * sh];
        let mut scaled_colors = vec![[0.0f64; 3]; sw * sh];
        for sy in 0..sh {
            for sx in 0..sw {
                let src_x = ((sx as f64 / scale) as usize).min(owidth - 1);
                let src_y = ((sy as f64 / scale) as usize).min(oheight - 1);
                let src = src_y * owidth + src_x;
                scaled_chars[sy * sw + sx] = context.char_buf[src];
                scaled_colors[sy * sw + sx] = context.color_buf[src];
            }
        }
        (scaled_chars, scaled_colors, sw, sh)
    };

    if let Some(output) = args.output {
        let w = 8 * output_w + output_w * args.character_spacing as usize;
        let h = 8 * output_h + output_h * args.character_spacing as usize;

        if !args.color {
            let mut res_buf = vec![0u8; w * h];

            for y in 0..output_h {
                for x in 0..output_w {
                    let px = 8 * x + x * args.character_spacing as usize;
                    let py = 8 * y + y * args.character_spacing as usize;
                    let char = output_buf[y * output_w + x] as usize;

                    for i in 0..8 {
                        for j in 0..8 {
                            res_buf[(py + i) * w + (px + j)] = CHARACTERS[char][i * 8 + j] * 255;
                        }
                    }
                }
            }

            let res = GrayImage::from_raw(w as u32, h as u32, res_buf).unwrap();
            res.save(output).expect("Failed to save image file");
        } else {
            let mut res_buf = vec![0u8; w * h * 3];

            for y in 0..output_h {
                for x in 0..output_w {
                    let px = 8 * x + x * args.character_spacing as usize;
                    let py = 8 * y + y * args.character_spacing as usize;
                    let char = output_buf[y * output_w + x] as usize;
                    let color = output_color_buf[y * output_w + x];
                    let r = (color[0] * 255.0).clamp(0.0, 255.0) as u8;
                    let g = (color[1] * 255.0).clamp(0.0, 255.0) as u8;
                    let b = (color[2] * 255.0).clamp(0.0, 255.0) as u8;

                    for i in 0..8 {
                        for j in 0..8 {
                            if CHARACTERS[char][i * 8 + j] == 1 {
                                let off = ((py + i) * w + (px + j)) * 3;
                                res_buf[off    ] = r;
                                res_buf[off + 1] = g;
                                res_buf[off + 2] = b;
                            }
                        }
                    }
                }
            }

            let res = RgbImage::from_raw(w as u32, h as u32, res_buf).unwrap();
            res.save(output).expect("Failed to save image file");
        }
    }

    for y in 0..output_h {
        for x in 0..output_w {
            print!("{}", BRIGHT_MAP[output_buf[y * output_w + x] as usize..].chars().next().unwrap())
        }
        println!()
    }

}
