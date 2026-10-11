use image::{ImageBuffer, Pixel, Rgb, RgbImage};

pub fn update_avatar_size(url: &str) -> String {
    if url.contains("cdn.discordapp.com/embed/avatars/") {
        return url.to_string();
    }

    let base = url
        .split_once("?size=")
        .map(|(base, _)| base)
        .unwrap_or(url);
    format!("{}?size=4096", base)
}

pub async fn fetch_image_from_url(
    url: &str,
) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let res = client.get(url).send().await?.bytes().await?;
    let img = image::load_from_memory(&res)?.to_rgb8();
    Ok(img)
}

pub fn average_center_color(img: &RgbImage) -> image::Rgb<u8> {
    let (w, h) = img.dimensions();
    let side = (w.min(h) as f32 * 0.7).round() as u32;
    let x0 = (w - side) / 2;
    let y0 = (h - side) / 2;

    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;
    let mut count = 0u64;

    for y in y0..(y0 + side) {
        for x in x0..(x0 + side) {
            let pixel = img.get_pixel(x, y).to_rgb();
            let [r, g, b] = pixel.0;
            r_sum += r as u64;
            g_sum += g as u64;
            b_sum += b as u64;
            count += 1;
        }
    }

    image::Rgb([
        (r_sum / count) as u8,
        (g_sum / count) as u8,
        (b_sum / count) as u8,
    ])
}
