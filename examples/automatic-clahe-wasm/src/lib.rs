#[wasm_bindgen::prelude::wasm_bindgen]
pub fn enhance_rgba_image(pixels: &mut [u8], width: u32) {
    automatic_clahe::AutomaticClahe::new().enhance_rgba_image(pixels, width as usize);
}
