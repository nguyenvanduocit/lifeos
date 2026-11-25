//! Reading the words in a picture with macOS's own text recognition (Vision), for what shows no accessibility tree.

use std::path::Path;

use anyhow::{Result, anyhow};
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

/// One line of text found, with where it is in the picture (points from its top left).
pub struct Line {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The lines of text in the picture at `file`, which is `width` × `height` points; top to bottom, left to right.
pub fn read(file: &Path, width: f64, height: f64) -> Result<Vec<Line>> {
    let path = NSString::from_str(&file.to_string_lossy());
    let url = NSURL::fileURLWithPath(&path);
    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    request.setUsesLanguageCorrection(true);
    request.setAutomaticallyDetectsLanguage(true);
    let langs = NSArray::from_retained_slice(&[NSString::from_str("vi-VT"), NSString::from_str("en-US")]);
    request.setRecognitionLanguages(&langs);
    let options = NSDictionary::new();
    // SAFETY: an empty options dictionary is of the right type.
    let handler = unsafe { VNImageRequestHandler::initWithURL_options(VNImageRequestHandler::alloc(), &url, &options) };
    let as_request: Retained<VNRequest> = Retained::into_super(Retained::into_super(request.clone()));
    let requests = NSArray::from_retained_slice(&[as_request]);
    handler.performRequests_error(&requests).map_err(|e| anyhow!("text recognition failed: {}", e.localizedDescription()))?;
    let mut lines = Vec::new();
    for obs in request.results().iter().flat_map(|r| r.iter()) {
        let Some(best) = obs.topCandidates(1).firstObject() else { continue };
        // SAFETY: a plain struct getter.
        let b = unsafe { obs.boundingBox() };
        lines.push(Line {
            text: best.string().to_string(),
            x: b.origin.x * width,
            // Vision's origin is the bottom left.
            y: (1.0 - b.origin.y - b.size.height) * height,
            w: b.size.width * width,
            h: b.size.height * height,
        });
    }
    lines.sort_by(|a, b| (a.y / 8.0).round().total_cmp(&(b.y / 8.0).round()).then(a.x.total_cmp(&b.x)));
    Ok(lines)
}
