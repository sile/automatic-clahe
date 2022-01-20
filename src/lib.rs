mod color_format;

#[derive(Debug, Clone)]
pub struct AutomaticClaheOptions {
    block_width: usize,
    block_height: usize,
    alpha: f32,
    p: f32,
    d_threshold: f32, // TODO: u8
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
struct Image<'a, const N: usize> {
    pixels: &'a mut [u8],
    width: usize,
    height: usize,
    luminances: Vec<u8>,
    l_max: f32,
    enhancement_weight_factor: f32,
}

impl<'a, const N: usize> Image<'a, N> {
    fn new(pixels: &'a mut [u8], width: usize) -> Self {
        let mut luminances = Vec::with_capacity(pixels.len() / N);
        let mut l_max = 0;
        for p in pixels.chunks(N) {
            let l = std::cmp::max(p[0], std::cmp::max(p[1], p[2]));
            luminances.push(l);
            l_max = std::cmp::max(l, l_max);
        }
        let l_max = f32::from(l_max);

        let pdf = Pdf::new(luminances.iter().copied());
        let cdf = Cdf::new(&pdf);
        let l_alpha = cdf.0.iter().take_while(|&&x| x <= 0.75).count() as f32;

        Self {
            pixels,
            width,
            height: luminances.len() / width,
            luminances,
            l_max,
            enhancement_weight_factor: l_max / l_alpha,
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
        let image = Image::<4>::new(pixels, width);
        let height = image.height;

        let mut block_cdfs: Vec<BlockCdf> = Vec::new();
        for block in Blocks::new(
            image.luminances.len(),
            width,
            self.options.block_width,
            self.options.block_height,
        ) {
            // clip point
            let avg = mean(block.pixels(&image.luminances));
            let sigma = stddev(block.pixels(&image.luminances), avg);
            let l_max = f32::from(block.pixels(&image.luminances).max().unwrap_or(0));
            let l_min = f32::from(block.pixels(&image.luminances).min().unwrap_or(0));
            let beta = block.len() as f32 / (l_max - l_min + f32::EPSILON)
                * (1.0
                    + self.options.p * l_max / f32::from(u8::MAX)
                    + (self.options.alpha / 100.0) * (sigma / (avg + f32::EPSILON)));

            // dual gamma correction
            let pdf = Pdf::new(block.pixels(&image.luminances));
            let pdf = pdf.redistribute(beta / block.len() as f32);
            let cdf = Cdf::new(&pdf);
            let cdf_w = Cdf::new(&pdf.to_weighting_distribution());
            let r = l_max - l_min;

            block_cdfs.push(BlockCdf {
                region: block,
                l_max,
                cdf,
                cdf_w,
                use_cdf_w: r > self.options.d_threshold,
            });
        }

        // bilinear interpolation
        for y in 0..image.height {
            for x in 0..image.width {
                let a = self.get_block_a(y, x, width, height, &block_cdfs);
                let b = self.get_block_b(y, x, width, height, &block_cdfs);
                let c = self.get_block_c(y, x, width, height, &block_cdfs);
                let d = self.get_block_d(y, x, width, height, &block_cdfs);

                let m = match (a.map(|a| a.center_y()), c.map(|c| c.center_y())) {
                    (Some(a), Some(c)) => (c - y) as f32 / (c - a) as f32,
                    _ => {
                        if a.is_some() || b.is_some() {
                            1.0
                        } else {
                            0.0
                        }
                    }
                };
                let n = match (a.map(|a| a.center_x()), b.map(|b| b.center_x())) {
                    (Some(a), Some(b)) => (b - x) as f32 / (b - a) as f32,
                    _ => {
                        if a.is_some() || c.is_some() {
                            1.0
                        } else {
                            0.0
                        }
                    }
                };

                let l0 = image.luminances[y * width + x];

                let la = a.map(|a| n * a.enhance(l0, &image)).unwrap_or(0.0);
                let lb = b.map(|b| (1.0 - n) * b.enhance(l0, &image)).unwrap_or(0.0);
                let lc = c.map(|c| n * c.enhance(l0, &image)).unwrap_or(0.0);
                let ld = d.map(|d| (1.0 - n) * d.enhance(l0, &image)).unwrap_or(0.0);
                let l = m * (la + lb) + (1.0 - m) * (lc + ld);

                let i = (y * width + x) * 4; // TODO
                let (h, s, _) = self::color_format::rgb_to_hsv(
                    image.pixels[i],
                    image.pixels[i + 1],
                    image.pixels[i + 2],
                );
                let (r, g, b) = self::color_format::hsv_to_rgb(h, s, l.max(0.0).min(255.0) as u8); // TODO
                image.pixels[i] = r;
                image.pixels[i + 1] = g;
                image.pixels[i + 2] = b;
            }
        }
    }

    fn get_block_a<'a>(
        &self,
        mut y: usize,
        mut x: usize,
        mut w: usize,
        mut h: usize,
        block_cdfs: &'a [BlockCdf],
    ) -> Option<&'a BlockCdf> {
        // TODO: handle edges correctly
        h = h / self.options.block_height * self.options.block_height;
        w = w / self.options.block_width * self.options.block_width;
        y = std::cmp::min(y, h - 1);
        x = std::cmp::min(x, w - 1);

        if y < self.options.block_height / 2 || x < self.options.block_width / 2 {
            return None;
        }

        let block_y = (y - self.options.block_height / 2) / self.options.block_height;
        let block_x = (x - self.options.block_width / 2) / self.options.block_width;
        let block_w = w / self.options.block_width;
        Some(&block_cdfs[block_y * block_w + block_x])
    }

    fn get_block_b<'a>(
        &self,
        mut y: usize,
        mut x: usize,
        mut w: usize,
        mut h: usize,
        block_cdfs: &'a [BlockCdf],
    ) -> Option<&'a BlockCdf> {
        // TODO: handle edges correctly
        h = h / self.options.block_height * self.options.block_height;
        w = w / self.options.block_width * self.options.block_width;
        y = std::cmp::min(y, h - 1);
        x = std::cmp::min(x, w - 1);

        if y < self.options.block_height / 2 || w <= (x + self.options.block_width / 2) {
            return None;
        }

        let block_y = (y - self.options.block_height / 2) / self.options.block_height;
        let block_x = (x + self.options.block_width / 2) / self.options.block_width;
        let block_w = w / self.options.block_width;
        Some(&block_cdfs[block_y * block_w + block_x])
    }

    fn get_block_c<'a>(
        &self,
        mut y: usize,
        mut x: usize,
        mut w: usize,
        mut h: usize,
        block_cdfs: &'a [BlockCdf],
    ) -> Option<&'a BlockCdf> {
        // TODO: handle edges correctly
        h = h / self.options.block_height * self.options.block_height;
        w = w / self.options.block_width * self.options.block_width;
        y = std::cmp::min(y, h - 1);
        x = std::cmp::min(x, w - 1);

        if h <= (y + self.options.block_height / 2) || x < self.options.block_width / 2 {
            return None;
        }

        let block_y = (y + self.options.block_height / 2) / self.options.block_height;
        let block_x = (x - self.options.block_width / 2) / self.options.block_width;
        let block_w = w / self.options.block_width;
        Some(&block_cdfs[block_y * block_w + block_x])
    }

    fn get_block_d<'a>(
        &self,
        mut y: usize,
        mut x: usize,
        mut w: usize,
        mut h: usize,
        block_cdfs: &'a [BlockCdf],
    ) -> Option<&'a BlockCdf> {
        // TODO: handle edges correctly
        h = h / self.options.block_height * self.options.block_height;
        w = w / self.options.block_width * self.options.block_width;
        y = std::cmp::min(y, h - 1);
        x = std::cmp::min(x, w - 1);

        if h <= (y + self.options.block_height / 2) || w <= (x + self.options.block_width / 2) {
            return None;
        }

        let block_y = (y + self.options.block_height / 2) / self.options.block_height;
        let block_x = (x + self.options.block_width / 2) / self.options.block_width;
        let block_w = w / self.options.block_width;
        if block_y * block_w + block_x == block_cdfs.len() {
            dbg!((y, x, h, w));
            dbg!((block_y, block_w, block_x));
            dbg!(block_y * block_w + block_x);
        }
        Some(&block_cdfs[block_y * block_w + block_x])
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

#[derive(Debug)]
pub struct BlockCdf {
    cdf: Cdf,
    cdf_w: Cdf,
    use_cdf_w: bool, // TODO: rename
    region: Region,
    l_max: f32,
}

impl BlockCdf {
    fn center_y(&self) -> usize {
        (self.region.end.y - self.region.start.y) / 2 + self.region.start.y
    }

    fn center_x(&self) -> usize {
        (self.region.end.x - self.region.start.x) / 2 + self.region.start.x
    }

    fn enhance<const N: usize>(&self, l: u8, image: &Image<N>) -> f32 {
        let l2 = image.l_max * (f32::from(l) / image.l_max).powf(self.cdf_w.gamma_2(1));
        let enhanced_l = if self.use_cdf_w {
            let w_en = image.enhancement_weight_factor.powf(self.cdf.gamma_1(l));
            let l1 = self.l_max * w_en * self.cdf.0[usize::from(l)];
            l1.max(l2)
        } else {
            l2
        };
        enhanced_l // TODO: range check
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
        let mut pdf = self.0;
        let mut exceeded = 0.0;
        for x in &mut pdf {
            if *x > beta {
                exceeded += *x - beta;
                *x = beta;
            }
        }
        if exceeded > 0.0 {
            let offset = exceeded / 256.0;
            for x in &mut pdf {
                *x += offset;
            }
        }
        Self(pdf)
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
