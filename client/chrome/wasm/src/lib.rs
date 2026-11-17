use wasm_bindgen::prelude::*;
use html_to_markdown_rs::convert;

#[wasm_bindgen]
pub fn html_to_markdown(html: &str) -> Result<String, JsValue> {
    match convert(html, None) {
        Ok(markdown) => Ok(markdown),
        Err(e) => Err(JsValue::from_str(&format!("Conversion error: {:?}", e))),
    }
}
