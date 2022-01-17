#[derive(Debug, Clone)]
pub struct AutomaticClaheOptions {
    block_width: usize,
    block_height: usize,
    alpha: f32,
    p: f32,
    d_threshold: f32,
}

impl Default for AutomaticClaheOptions {
    fn default() -> Self {
        Self {
            block_width: 32,
            block_height: 32,
            alpha: 100.0,
            p: 1.5,
            d_threshold: 50.0,
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

    pub fn enhance_rgba_image(&self, pixels: &mut [u8], width: usize) {
        const N: usize = 4;
        let luminances = pixels
            .chunks(N)
            .map(|p| std::cmp::max(p[0], std::cmp::max(p[1], p[2])))
            .collect::<Vec<_>>();

        let g_pdf = Pdf::new(luminances.iter().copied());
        let g_cdf = Cdf::new(&g_pdf);
        let g_l_max = f32::from(luminances.iter().copied().max().unwrap_or(0));
        let g_l_alpha = g_cdf
            .0
            .iter()
            .position(|&x| x >= 0.75) // TODO
            .expect("unreachable") as f32;

        for block in Blocks::new(
            pixels.len() / N,
            width,
            self.options.block_width,
            self.options.block_height,
        ) {
            // clip point
            let avg = mean(block.pixels(&luminances));
            let sigma = stddev(block.pixels(&luminances), avg);
            let l_max = f32::from(block.pixels(&luminances).max().unwrap_or(0));
            let l_min = f32::from(block.pixels(&luminances).min().unwrap_or(0));
            let beta = block.len() as f32 / (l_max - l_min + f32::EPSILON)
                * (1.0
                    + self.options.p * l_max / f32::from(u8::MAX)
                    + (self.options.alpha / 100.0) * (sigma / (avg + f32::EPSILON)));

            // dual gamma correction
            let pdf = Pdf::new(block.pixels(&luminances));
            let pdf = pdf.redistribute(beta);
            let cdf = Cdf::new(&pdf);
            let cdf_w = Cdf::new(&pdf.to_weighting_distribution());
            let r = l_max - l_min;

            for l in block.pixels(&luminances) {
                let w_en = (g_l_max / g_l_alpha).powf(1.0 - cdf.gamma_1(l));
                let l1 = l_max * w_en * cdf.0[usize::from(l)];
                let l2 = g_l_max * (f32::from(l) / g_l_max).powf(cdf_w.gamma_2(1));
                let enhanced_l = if r > self.options.d_threshold {
                    l1.max(l2)
                } else {
                    l2
                };
            }
        }
    }

    pub fn enhance_rgb_image(&self, _pixels: &mut [u8], _width: usize) {
        todo!()
    }
}

fn mean(pixels: impl Iterator<Item = u8>) -> f32 {
    let mut sum = 0u32;
    let mut count = 0u32;

    for p in pixels {
        sum += u32::from(p);
        count += 1;
    }

    sum as f32 / count as f32
}

fn stddev(pixels: impl Iterator<Item = u8>, mean: f32) -> f32 {
    let mut sum = 0.0f32;
    let mut count = 0u32;

    for p in pixels {
        sum += (f32::from(p) - mean).powi(2);
        count += 1;
    }

    (sum / count as f32).sqrt()
}

#[derive(Debug)]
struct Blocks {
    x: usize,
    y: usize,
    image_width: usize,
    image_height: usize,
    block_width: usize,
    block_height: usize,
}

impl Blocks {
    fn new(pixels: usize, image_width: usize, block_width: usize, block_height: usize) -> Self {
        Self {
            x: 0,
            y: 0,
            image_width,
            image_height: pixels / image_width,
            block_width,
            block_height,
        }
    }
}

impl Iterator for Blocks {
    type Item = Region;

    fn next(&mut self) -> Option<Self::Item> {
        if self.y == self.image_height {
            return None;
        }

        let start = Point::new(self.x, self.y);
        let mut end = Point::new(self.x + self.block_width, self.y + self.block_height);
        if self.image_width < end.x + self.block_width {
            end.x = self.image_width;
        }
        if self.image_height < end.y + self.block_height {
            end.y = self.image_height;
        }

        self.x = end.x;
        if self.x == self.image_width {
            self.x = 0;
            self.y = end.y;
        }

        Some(Region { start, end })
    }
}

#[derive(Debug, Clone, Copy)]
struct Region {
    start: Point,
    end: Point,
}

impl Region {
    fn len(&self) -> usize {
        (self.end.y - self.start.y) * (self.end.x - self.start.x)
    }

    fn pixels<T: Copy>(self, pixels: &[T]) -> impl '_ + Iterator<Item = T> {
        let width = self.end.x - self.start.x;
        (self.start.y..self.end.y).flat_map(move |y| {
            let offset = y * width;
            (pixels[offset..][self.start.x..self.end.x]).iter().copied()
        })
    }
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

#[derive(Debug, Clone)]
struct Pdf([f32; 256]);

impl Pdf {
    fn new(pixels: impl Iterator<Item = u8>) -> Self {
        let mut histogram = [0; 256];
        let mut n = 0;
        for intensity in pixels {
            histogram[usize::from(intensity)] += 1;
            n += 1;
        }

        let mut pdf = [0.0; 256];
        let n = n as f32;
        for (i, c) in histogram.into_iter().enumerate() {
            pdf[i] = c as f32 / n;
        }
        Self(pdf)
    }

    fn to_weighting_distribution(&self) -> Self {
        let mut max_intensity = self.0[0];
        let mut min_intensity = self.0[0];
        for &x in &self.0[1..] {
            max_intensity = max_intensity.max(x);
            min_intensity = min_intensity.min(x);
        }

        let mut pdf_w = self.0;
        let range = max_intensity - min_intensity + f32::EPSILON;
        for x in &mut pdf_w {
            *x = max_intensity * ((*x - min_intensity) / range);
        }
        Self(pdf_w)
    }

    fn redistribute(&self, beta: f32) -> Self {
        todo!()
    }
}

#[derive(Debug)]
struct Cdf([f32; 256]);

impl Cdf {
    fn new(pdf: &Pdf) -> Self {
        let mut cdf = [0.0; 256];
        let mut sum = 0.0;
        for (i, x) in pdf.0.iter().copied().enumerate() {
            sum += x;
            cdf[i] = sum;
        }
        for x in &mut cdf {
            *x /= sum;
        }
        Self(cdf)
    }

    fn gamma_1(&self, l: u8) -> f32 {
        (self.0[usize::from(l)] + f32::EPSILON).ln() / 8.0
    }

    fn gamma_2(&self, l: u8) -> f32 {
        (self.0[usize::from(l)] + 1.0) / 2.0
    }
}
