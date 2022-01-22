mod color_format;

#[derive(Debug, Clone)]
pub struct AutomaticClaheOptions {
    pub block_width: usize,
    pub block_height: usize,
    pub alpha: f32,
    pub p: f32,
    pub d_threshold: u8,
}

impl Default for AutomaticClaheOptions {
    fn default() -> Self {
        Self {
            block_width: 32,
            block_height: 32,
            alpha: 100.0,
            p: 1.5,
            d_threshold: 50,
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

        let height = luminances.len() / width;
        Self {
            pixels,
            width,
            height,
            luminances,
            l_max,
            enhancement_weight_factor: l_max / l_alpha,
        }
    }

    fn update_luminances(&mut self) {
        for (i, p) in self.pixels.chunks_mut(N).enumerate() {
            let (h, s, _) = self::color_format::rgb_to_hsv(p[0], p[1], p[2]);
            let (r, g, b) = self::color_format::hsv_to_rgb(h, s, self.luminances[i]);
            p[0] = r;
            p[1] = g;
            p[2] = b;
        }
    }
}

#[derive(Debug)]
struct Block {
    enable_dual_gamma_correction: bool,
    l_max: f32,
    region: Region,
    cdf: Cdf,
    cdf_w: Cdf,
    table: [f32; 256],
}

impl Block {
    fn new<const N: usize>(
        image: &Image<N>,
        options: &AutomaticClaheOptions,
        region: Region,
    ) -> Self {
        let mut l_sum = 0;
        let mut l_max = 0;
        let mut l_min = u8::MAX;
        for l in region.items(&image.luminances) {
            l_sum += usize::from(l);
            l_max = std::cmp::max(l_max, l);
            l_min = std::cmp::min(l_min, l);
        }
        let m = region.len() as f32;
        let avg = l_sum as f32 / m;
        let sigma = (region
            .items(&image.luminances)
            .map(|l| (f32::from(l) - avg).powi(2))
            .sum::<f32>()
            / m)
            .sqrt();
        let n = f32::from(l_max - l_min) + f32::EPSILON;

        let clip_point = (1.0
            + options.p * f32::from(l_max) / f32::from(u8::MAX)
            + (options.alpha / 100.0) * (sigma / (avg + f32::EPSILON)))
            / n;

        let pdf = Pdf::new(region.items(&image.luminances)).redistribute(clip_point);
        let cdf = Cdf::new(&pdf);
        let cdf_w = Cdf::new(&pdf.to_weighting_distribution());

        let mut this = Self {
            enable_dual_gamma_correction: (l_max - l_min) > options.d_threshold,
            l_max: f32::from(l_max),
            region,
            cdf,
            cdf_w,
            table: [0.0; 256],
        };
        for l in 0..256 {
            this.table[l] = this.enhance0(l as u8, image);
        }
        this
    }

    fn center_y(&self) -> usize {
        (self.region.end.y - self.region.start.y) / 2 + self.region.start.y
    }

    fn center_x(&self) -> usize {
        (self.region.end.x - self.region.start.x) / 2 + self.region.start.x
    }

    fn enhance(&self, l: u8) -> f32 {
        self.table[usize::from(l)]
    }

    fn enhance0<const N: usize>(&self, l: u8, image: &Image<N>) -> f32 {
        let l2 = image.l_max * (f32::from(l) / image.l_max).powf(self.cdf_w.gamma_2(l));
        let enhanced_l = if self.enable_dual_gamma_correction {
            let w_en = image
                .enhancement_weight_factor
                .powf(1.0 - self.cdf.gamma_1(l));
            let l1 = self.l_max * w_en * self.cdf.0[usize::from(l)];
            l1.max(l2)
        } else {
            l2
        };
        enhanced_l
    }
}

#[derive(Debug)]
struct SurroundingBlocks<'a>(&'a ());

impl<'a> Iterator for SurroundingBlocks<'a> {
    type Item = (
        Point,
        Option<&'a Block>,
        Option<&'a Block>,
        Option<&'a Block>,
        Option<&'a Block>,
    );

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

#[derive(Debug)]
pub struct AutomaticClahe {
    options: AutomaticClaheOptions,
}

impl AutomaticClahe {
    pub fn with_options(options: AutomaticClaheOptions) -> Self {
        Self { options }
    }

    pub fn new() -> Self {
        Self {
            options: Default::default(),
        }
    }

    pub fn enhance_rgba_image(&self, pixels: &mut [u8], width: usize) {
        let mut image = Image::<4>::new(pixels, width);
        let blocks = BlockRegions::new(&image, &self.options)
            .map(|region| Block::new(&image, &self.options, region))
            .collect::<Vec<_>>();

        let aligned_height = image.height / self.options.block_height * self.options.block_height;
        let aligned_width = image.width / self.options.block_width * self.options.block_width;
        let line_blocks = image.width / self.options.block_width;
        for y in 0..image.height {
            let y0 = std::cmp::min(y, aligned_height - 1);
            for x in 0..image.width {
                let x0 = std::cmp::min(x, aligned_width - 1);

                let a = self.get_block_a(y0, x0, line_blocks, &blocks);
                let b = self.get_block_b(y0, x0, aligned_width, line_blocks, &blocks);
                let c = self.get_block_c(y0, x0, aligned_height, line_blocks, &blocks);
                let d =
                    self.get_block_d(y0, x0, aligned_height, aligned_width, line_blocks, &blocks);

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

                let i = y * width + x;
                let l0 = image.luminances[i];

                let la = a.map(|a| n * a.enhance(l0)).unwrap_or(0.0);
                let lb = b.map(|b| (1.0 - n) * b.enhance(l0)).unwrap_or(0.0);
                let lc = c.map(|c| n * c.enhance(l0)).unwrap_or(0.0);
                let ld = d.map(|d| (1.0 - n) * d.enhance(l0)).unwrap_or(0.0);
                let l = m * (la + lb) + (1.0 - m) * (lc + ld);
                image.luminances[i] = l.max(0.0).min(255.0) as u8;
            }
        }
        image.update_luminances();
    }

    fn get_block_a<'a>(
        &self,
        y: usize,
        x: usize,
        line_blocks: usize,
        blocks: &'a [Block],
    ) -> Option<&'a Block> {
        if y < self.options.block_height / 2 || x < self.options.block_width / 2 {
            None
        } else {
            let block_y = (y - self.options.block_height / 2) / self.options.block_height;
            let block_x = (x - self.options.block_width / 2) / self.options.block_width;
            Some(&blocks[block_y * line_blocks + block_x])
        }
    }

    fn get_block_b<'a>(
        &self,
        y: usize,
        x: usize,
        w: usize,
        line_blocks: usize,
        blocks: &'a [Block],
    ) -> Option<&'a Block> {
        if y < self.options.block_height / 2 || w <= (x + self.options.block_width / 2) {
            None
        } else {
            let block_y = (y - self.options.block_height / 2) / self.options.block_height;
            let block_x = (x + self.options.block_width / 2) / self.options.block_width;
            Some(&blocks[block_y * line_blocks + block_x])
        }
    }

    fn get_block_c<'a>(
        &self,
        y: usize,
        x: usize,
        h: usize,
        line_blocks: usize,
        blocks: &'a [Block],
    ) -> Option<&'a Block> {
        if h <= (y + self.options.block_height / 2) || x < self.options.block_width / 2 {
            None
        } else {
            let block_y = (y + self.options.block_height / 2) / self.options.block_height;
            let block_x = (x - self.options.block_width / 2) / self.options.block_width;
            Some(&blocks[block_y * line_blocks + block_x])
        }
    }

    fn get_block_d<'a>(
        &self,
        y: usize,
        x: usize,
        h: usize,
        w: usize,
        line_blocks: usize,
        blocks: &'a [Block],
    ) -> Option<&'a Block> {
        if h <= (y + self.options.block_height / 2) || w <= (x + self.options.block_width / 2) {
            None
        } else {
            let block_y = (y + self.options.block_height / 2) / self.options.block_height;
            let block_x = (x + self.options.block_width / 2) / self.options.block_width;
            Some(&blocks[block_y * line_blocks + block_x])
        }
    }

    pub fn enhance_rgb_image(&self, _pixels: &mut [u8], _width: usize) {
        todo!()
    }
}

#[derive(Debug)]
struct BlockRegions {
    start: Point,
    image_width: usize,
    image_height: usize,
    block_width: usize,
    block_height: usize,
}

impl BlockRegions {
    fn new<const N: usize>(image: &Image<N>, options: &AutomaticClaheOptions) -> Self {
        Self {
            start: Point::new(0, 0),
            image_width: image.width,
            image_height: image.height,
            block_width: options.block_width,
            block_height: options.block_height,
        }
    }
}

impl Iterator for BlockRegions {
    type Item = Region;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start.y == self.image_height {
            return None;
        }

        let start = self.start;
        let mut end = Point::new(start.x + self.block_width, start.y + self.block_height);
        if self.image_width < end.x + self.block_width {
            end.x = self.image_width;
        }
        if self.image_height < end.y + self.block_height {
            end.y = self.image_height;
        }

        self.start.x = end.x;
        if self.start.x == self.image_width {
            self.start.x = 0;
            self.start.y = end.y;
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

    fn items<T: Copy>(self, all_items: &[T]) -> impl '_ + Iterator<Item = T> {
        let width = self.end.x - self.start.x;
        (self.start.y..self.end.y).flat_map(move |y| {
            let offset = y * width;
            (all_items[offset..][self.start.x..self.end.x])
                .iter()
                .copied()
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
        let mut max = self.0[0];
        let mut min = self.0[0];
        for &x in &self.0[1..] {
            max = max.max(x);
            min = min.min(x);
        }

        let mut pdf_w = self.0;
        let range = max - min + f32::EPSILON;
        for x in &mut pdf_w {
            *x = max * ((*x - min) / range);
        }
        Self(pdf_w)
    }

    fn redistribute(mut self, clip_point: f32) -> Self {
        let mut exceeded = 0.0;
        for x in &mut self.0 {
            if *x > clip_point {
                exceeded += *x - clip_point;
                *x = clip_point;
            }
        }
        if exceeded > 0.0 {
            let offset = exceeded / 256.0;
            for x in &mut self.0 {
                *x += offset;
            }
        }
        self
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
