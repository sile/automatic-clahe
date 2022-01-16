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

    pub fn enhance_rgb_image(&self, pixels: &mut [u8], width: usize) {
        //let image = Image::<3>::new(pixels, width);
    }
}

// #[derive(Debug)]
// struct Image<'a, const N: usize> {
//     pixels: &'a mut [u8],
//     width: usize,
// }

// impl<'a, const N: usize> Image<'a, N> {
//     fn new(pixels: &'a mut [u8], width: usize) -> Self {
//         Self { pixels, width }
//     }

//     fn blocks(&self, block_width: usize, block_height: usize) -> impl '_ + Iterator<Item = Block> {}
// }
