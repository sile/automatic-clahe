#[derive(Debug, Clone)]
pub struct AutomaticClaheOptions {
    block_width: usize,
    block_height: usize,
}

impl Default for AutomaticClaheOptions {
    fn default() -> Self {
        Self {
            block_width: 32,
            block_height: 32,
        }
    }
}

#[derive(Debug)]
pub struct AutomaticClahe {
    options: AutomaticClaheOptions,
}

impl AutomaticClahe {
    pub fn new() -> Self {
        Self {
            options: Default::default(),
        }
    }

    pub fn enhance_rgba_image(&self, _pixels: &mut [u8], _width: usize) {
        todo!()
    }

    pub fn enhance_rgb_image(&self, pixels: &mut [u8], width: usize) {
        let luminances = pixels
            .chunks(3)
            .map(|p| std::cmp::max(p[0], std::cmp::max(p[1], p[2])))
            .collect::<Vec<_>>();
        for block in blocks(
            pixels.len(),
            width,
            self.options.block_width,
            self.options.block_height,
        ) {
            //
        }
    }
}

fn blocks(
    pixels: usize,
    width: usize,
    block_width: usize,
    block_height: usize,
) -> impl Iterator<Item = Region> {
    let height = pixels / width;
    let yn = height / block_height + if height % block_height == 0 { 0 } else { 1 };
    let xn = width / block_width + if width % block_width == 0 { 0 } else { 1 };
    (0..yn).flat_map(move |yi| {
        let y0 = yi * block_height;
        let y1 = std::cmp::min(y0 + block_height, height);
        (0..xn).map(move |xi| {
            let x0 = xi * block_width;
            let x1 = std::cmp::min(x0 + block_width, width);
            Region {
                start: Point::new(x0, y0),
                end: Point::new(x1, y1),
            }
        })
    })
}

#[derive(Debug, Clone, Copy)]
struct Region {
    start: Point,
    end: Point,
}

#[derive(Debug, Clone, Copy)]
struct Point {
    x: usize,
    y: usize,
}

impl Point {
    fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

// #[derive(Debug)]
// struct Image<'a, const N: usize> {
//     image_width: usize,
//     pixels: &'a mut [u8],
//     luminances: Vec<u8>,
// }

// impl<'a, const N: usize> Image<'a, N> {
//     fn new(
//         pixels: &'a mut [u8],
//         image_width: usize,
//         block_width: usize,
//         block_height: usize,
//     ) -> Self {
//         Self {
//             image_width,
//             pixels,
//         }
//     }

//     //fn blocks(&self, block_width: usize, block_height: usize) -> impl '_ + Iterator<Item = Block> {}
// }

// #[derive(Debug)]
// struct Block {
//     x: usize,
//     y: usize,
// }
