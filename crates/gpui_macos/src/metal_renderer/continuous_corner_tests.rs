//! Continuous corners: gpui's rounded quads against Core Animation's `cornerCurve = .continuous`.
//! Shapes here are symmetric, so the bitmap's row order (Core Graphics is y-up) doesn't matter.
use super::*;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use core_graphics::{
    base::kCGImageAlphaNone, color::CGColor, color_space::CGColorSpace, context::CGContext,
};
use gpui::{BorderStyle, Corners, Edges, Hsla, solid_background, transparent_black};

pub(super) fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(ScaledPixels(x), ScaledPixels(y)),
        size: size(ScaledPixels(w), ScaledPixels(h)),
    }
}

pub(super) fn radii(r: f32) -> Corners<ScaledPixels> {
    Corners::all(ScaledPixels(r))
}

fn gray(l: f32) -> Hsla {
    Hsla { h: 0., s: 0., l, a: 1. }
}

pub(super) fn white_quad(b: Bounds<ScaledPixels>, corner_radii: Corners<ScaledPixels>) -> Quad {
    Quad {
        order: 0,
        border_style: BorderStyle::default(),
        bounds: b,
        content_mask: ContentMask { bounds: b },
        background: solid_background(gray(1.)),
        border_color: transparent_black(),
        corner_radii,
        border_widths: Edges::default(),
        fade: gpui::EdgeFadeParams::default(),
    }
}

/// What `draw` paints in white over a black backdrop, as coverage: the red channel over 255.
pub(super) fn gpui_mask(w: i32, h: i32, draw: impl FnOnce(&mut Scene)) -> Vec<f32> {
    let mut scene = Scene::default();
    let mut backdrop = white_quad(bounds(0., 0., w as f32, h as f32), Corners::default());
    backdrop.background = solid_background(gray(0.));
    scene.insert_primitive(backdrop);
    draw(&mut scene);
    scene.finish();
    let mut renderer =
        MetalRenderer::new_headless(Arc::new(Mutex::new(InstanceBufferPool::default())));
    let image = renderer
        .render_scene_to_image(&scene, size(DevicePixels(w), DevicePixels(h)))
        .expect("render_scene_to_image failed");
    image.pixels().map(|p| p[0] as f32 / 255.).collect()
}

/// Core Animation's coverage of a white `w × h` rect with `cornerRadius = r` and the given
/// `cornerCurve` (`"continuous"` or `"circular"`), at 1×.
pub(super) fn ca_mask(w: usize, h: usize, r: f64, curve: &str) -> Vec<f32> {
    let mut ctx = CGContext::create_bitmap_context(
        None,
        w,
        h,
        8,
        w,
        &CGColorSpace::create_device_gray(),
        kCGImageAlphaNone,
    );
    unsafe {
        let layer: id = msg_send![class!(CALayer), layer];
        let frame = NSRect::new(NSPoint::new(0., 0.), NSSize::new(w as f64, h as f64));
        let _: () = msg_send![layer, setFrame: frame];
        let white = CGColor::rgb(1., 1., 1., 1.);
        let _: () = msg_send![layer, setBackgroundColor: white.as_concrete_TypeRef()];
        let _: () = msg_send![layer, setCornerRadius: r];
        let _: () = msg_send![layer, setCornerCurve: NSString::alloc(nil).init_str(curve)];
        let _: () = msg_send![layer, renderInContext: ctx.as_ptr()];
    }
    ctx.data().iter().map(|&v| v as f32 / 255.).collect()
}

/// Summed absolute coverage difference, in px².
pub(super) fn diff(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum()
}

#[test]
fn continuous_corner_matches_core_animation() {
    for r in [8.0f32, 20.0, 40.0] {
        let ours = gpui_mask(200, 200, |s| {
            s.insert_primitive(white_quad(bounds(0., 0., 200., 200.), radii(r)));
        });
        let apple = ca_mask(200, 200, r as f64, "continuous");
        let circle = ca_mask(200, 200, r as f64, "circular");
        let (e, base) = (diff(&ours, &apple), diff(&circle, &apple));
        assert!(e < 0.5 * base, "r = {r}: {e:.2} px² off Apple's curve, the circle is {base:.2}");
    }
}

#[test]
fn pill_corner_is_a_circle() {
    let ours = gpui_mask(200, 40, |s| {
        s.insert_primitive(white_quad(bounds(0., 0., 200., 40.), radii(20.)));
    });
    let circle = ca_mask(200, 40, 20., "circular");
    let apple = ca_mask(200, 40, 20., "continuous");
    let (e, base) = (diff(&ours, &circle), diff(&circle, &apple));
    assert!(e < 0.25 * base, "pill: {e:.2} px² off a circle, Apple's continuous pill is {base:.2} off");
}
