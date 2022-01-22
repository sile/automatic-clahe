use wasm_bindgen::prelude::*;

#[derive(Debug, serde::Deserialize)]
struct Options {
    #[serde(default)]
    block_width: Option<usize>,
    #[serde(default)]
    block_height: Option<usize>,
    #[serde(default)]
    alpha: Option<f32>,
    #[serde(default)]
    p: Option<f32>,
    #[serde(default)]
    d_threshold: Option<u8>,
}

#[wasm_bindgen]
pub fn enhance_rgba_image(pixels: &mut [u8], width: u32, options: &JsValue) -> Result<(), JsError> {
    let options = if options.is_object() {
        let options: Options = options.into_serde()?;
        automatic_clahe::AutomaticClaheOptions {
            block_width: options.block_width.unwrap_or(32),
            block_height: options.block_height.unwrap_or(32),
            alpha: options.alpha.unwrap_or(100.0),
            p: options.p.unwrap_or(1.5),
            d_threshold: options.d_threshold.unwrap_or(50),
        }
    } else {
        Default::default()
    };

    automatic_clahe::AutomaticClahe::with_options(options)
        .enhance_rgba_image(pixels, width as usize);
    Ok(())
}
