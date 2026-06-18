pub mod font;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use clap::Parser;
use image::{GrayImage, ImageFormat, Rgba32FImage};
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

impl FromStr for BlendMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "average" | "avg" => Ok(Self::Average),
            "geometric" | "geo" => Ok(Self::Geometric),
            "brightest" | "bright" | "max" => Ok(Self::Brightest),
            "darkest" | "dark" | "min" => Ok(Self::Darkest),
            _ => Err(anyhow::Error::msg(format!("Unknown blend type {}", s)))
        }
    }
}

#[derive(Parser)]
struct ProgramArgs {

    #[arg(short='I', long, required = true)]
    image: PathBuf,

    #[arg(short='B', long = "bcutoff", default_value = "0.0")]
    brightness_cutoff: f64,

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
}

#[allow(unused)]
struct GenContext {
    image: Rgba32FImage,
    group_size: u8,
    oheight: usize,
    owidth: usize,
    char_buf: Vec<u8>,
    denoise_buf: Vec<u8>,
    cbuf: Vec<f64>,
    blend_mode: BlendMode,
    bright_multiplier: f64,
    noise_thresh: u8,
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

                self.cbuf[i] = (b * self.bright_multiplier).clamp(0.0, 1.0);

                i += 1;
            }
        }

        let brightness = self.blend_mode.blend(&self.cbuf[0..i]);
        self.char_buf[y * self.owidth + x] = (brightness * (BRIGHT_MAP.len()-1) as f64) as u8;
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
    let denoise_buf = vec![
        0; owidth * oheight
    ];
    let cbuf = vec![0.0; (args.group * args.group) as usize];

    let mut context = GenContext {
        image,
        oheight,
        owidth,
        char_buf: b_map,
        cbuf,
        group_size: args.group,
        blend_mode: args.blend,
        denoise_buf,
        bright_multiplier: args.multiplier,
        noise_thresh: args.noise_thresh
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


    if let Some(output) = args.output {
        let w = 8 * owidth + owidth * args.character_spacing as usize;
        let h = 8 * oheight + oheight * args.character_spacing as usize;
        let mut res_buf = vec![0u8; w * h ];


        for x in 0..owidth {
            for y in 0..oheight {

                let px = 8 * x + x * args.character_spacing as usize;
                let py = 8 * y + y * args.character_spacing as usize;
                let char = context.char_buf[y * owidth + x] as usize;

                for i in 0..8 {
                    for j in 0..8 {

                        let ci = i * 8 + j;

                        res_buf[(py + i) * w + (px + j)] = CHARACTERS[char][ci] * 255;

                    }
                }

            }
        }

        let res = GrayImage::from_raw(w as u32, h as u32, res_buf).unwrap();

        res.save(output).expect("Failed to save image file");
    }

    //output the text to the terminal
    for y in 0..oheight {
        for x in 0..owidth {
            print!("{}", BRIGHT_MAP[context.char_buf[y * owidth + x] as usize..].chars().next().unwrap())
        }
        println!()
    }

}
