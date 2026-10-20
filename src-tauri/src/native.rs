use anyhow::{Result, bail};
use image::RgbImage;
use interview_core::model::OcrRegion;
use std::path::Path;

#[cfg(windows)]
pub struct Ocr {
    engine: paddle_ocr_rs::ocr_lite::OcrLite,
}
#[cfg(not(windows))]
pub struct Ocr;
impl Ocr {
    pub fn load(resources: &Path) -> Result<Self> {
        #[cfg(windows)]
        {
            let dll = resources.join("resources/runtime/onnxruntime.dll");
            ort::init_from(
                dll.to_str()
                    .ok_or_else(|| anyhow::anyhow!("运行库路径无效"))?,
            )
            .commit()?;
            let models = resources.join("resources/models");
            let path = |name: &str| models.join(name).to_string_lossy().into_owned();
            let mut engine = paddle_ocr_rs::ocr_lite::OcrLite::new();
            engine.init_models_with_dict(
                &path("det.onnx"),
                &path("cls.onnx"),
                &path("rec.onnx"),
                &path("keys.txt"),
                2,
            )?;
            Ok(Self { engine })
        }
        #[cfg(not(windows))]
        {
            let _ = resources;
            bail!("屏幕 OCR 仅在 Windows 版本中启用；当前平台可开发和测试界面、资料与模型功能")
        }
    }
    pub fn recognize(&mut self, image: &RgbImage) -> Result<String> {
        #[cfg(windows)]
        {
            let result = self
                .engine
                .detect(image, 20, 1600, 0.5, 0.3, 1.6, false, false)?;
            let text = result
                .text_blocks
                .iter()
                .filter(|b| b.text_score > 0.4)
                .map(|b| b.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::ensure!(!text.trim().is_empty(), "没有识别到文字，请重试或重新框选");
            Ok(text)
        }
        #[cfg(not(windows))]
        {
            let _ = image;
            bail!("当前系统不支持 Windows OCR")
        }
    }
}

pub fn capture(region: &OcrRegion) -> Result<RgbImage> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::{Graphics::Gdi::*, UI::HiDpi::*};
        let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let dc = GetDC(None);
        if dc.is_invalid() {
            SetThreadDpiAwarenessContext(previous);
            bail!("无法读取屏幕");
        }
        let mem = CreateCompatibleDC(Some(dc));
        let bitmap = CreateCompatibleBitmap(dc, region.width as i32, region.height as i32);
        if mem.is_invalid() || bitmap.is_invalid() {
            let _ = DeleteDC(mem);
            let _ = DeleteObject(bitmap.into());
            ReleaseDC(None, dc);
            SetThreadDpiAwarenessContext(previous);
            bail!("无法创建屏幕缓冲区");
        }
        let old = SelectObject(mem, bitmap.into());
        let copied = BitBlt(
            mem,
            0,
            0,
            region.width as i32,
            region.height as i32,
            Some(dc),
            region.monitor.x + region.x as i32,
            region.monitor.y + region.y as i32,
            SRCCOPY | CAPTUREBLT,
        );
        SelectObject(mem, old);
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = region.width as i32;
        info.bmiHeader.biHeight = -(region.height as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;
        let mut bgra = vec![0u8; region.width as usize * region.height as usize * 4];
        let rows = if copied.is_ok() {
            GetDIBits(
                mem,
                bitmap,
                0,
                region.height,
                Some(bgra.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            )
        } else {
            0
        };
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, dc);
        SetThreadDpiAwarenessContext(previous);
        anyhow::ensure!(rows == region.height as i32, "截图失败，请重试");
        let mut rgb = Vec::with_capacity(region.width as usize * region.height as usize * 3);
        for p in bgra.as_chunks::<4>().0 {
            rgb.extend_from_slice(&[p[2], p[1], p[0]]);
        }
        RgbImage::from_raw(region.width, region.height, rgb)
            .ok_or_else(|| anyhow::anyhow!("截图数据无效"))
    }
    #[cfg(not(windows))]
    {
        let _ = region;
        bail!("屏幕截图仅支持 Windows")
    }
}

pub fn compositor_flush() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::Graphics::Dwm::DwmFlush();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn bundled_models_recognize_english_question_on_cpu() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut ocr = Ocr::load(root).expect("bundled ONNX runtime and models should load");
        let image = image::load_from_memory(include_bytes!("../../tests/fixtures/question.png"))
            .unwrap()
            .into_rgb8();
        let text = ocr.recognize(&image).expect("fixture should contain text");
        let text = text.to_lowercase();
        assert!(text.contains("tell me about yourself"), "{text}");
        assert!(text.contains("project"), "{text}");
    }
}
