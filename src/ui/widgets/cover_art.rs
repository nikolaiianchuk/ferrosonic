//! Half-block cover art widget

use image::DynamicImage;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    widgets::{Block, Widget},
};

pub struct CoverArtWidget<'a> {
    image: Option<&'a DynamicImage>,
    block: Option<Block<'a>>,
}

impl<'a> CoverArtWidget<'a> {
    pub fn new(image: Option<&'a DynamicImage>) -> Self {
        Self { image, block: None }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl Widget for CoverArtWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = if let Some(block) = self.block {
            let inner = block.inner(area);
            block.render(area, buf);
            inner
        } else {
            area
        };

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let Some(img) = self.image else {
            return;
        };

        // Each terminal row encodes 2 pixel rows via the ▀ half-block character:
        // fg = top pixel colour, bg = bottom pixel colour.
        let target_w = inner.width as u32;
        let target_h = inner.height as u32 * 2;

        let resized = img.resize(target_w, target_h, image::imageops::FilterType::Nearest);
        let rgba = resized.to_rgba8();
        let (img_w, img_h) = rgba.dimensions();

        // Centre the image inside the widget area
        let x_off = inner.x + (inner.width.saturating_sub(img_w as u16)) / 2;
        let y_off = inner.y + (inner.height.saturating_sub((img_h as u16 + 1) / 2)) / 2;

        for row in 0..(img_h / 2) {
            for col in 0..img_w {
                let top = rgba.get_pixel(col, row * 2);
                let bot = rgba.get_pixel(col, (row * 2 + 1).min(img_h - 1));

                let x = x_off + col as u16;
                let y = y_off + row as u16;

                if x < inner.x + inner.width && y < inner.y + inner.height {
                    buf[(x, y)]
                        .set_char('▀')
                        .set_fg(Color::Rgb(top[0], top[1], top[2]))
                        .set_bg(Color::Rgb(bot[0], bot[1], bot[2]));
                }
            }
        }
    }
}
