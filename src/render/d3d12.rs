
use std::io::Result as IOResult;
use comdrive::*;
use Application;
use winapi::shared::dxgiformat::*;
use metrics::*;
use widestring::WideCStr;
// use svgparse::{ArcSweepingDirection, Segment};

pub struct PathImage { inner: d2::PathGeometry }
impl super::VectorImage for PathImage {}

pub struct RenderDevice
{
    adapter: dxgi::Adapter, dev12: d3d12::Device, dev11: d3d11::Device, imm: d3d11::ImmediateContext, dev2: d2::Device,
    queue: d3d12::CommandQueue, swapchain: dxgi::SwapChain,

    agent_str: Option<String>
}
impl RenderDevice
{
    pub fn init() -> IOResult<Self>
    {
        let xf = dxgi::Factory::new(cfg!(feature = "debug"))?;
        let adapter = xf.adapter(0)?;
        #[cfg(feature = "debug")]
        d3d12::Device::enable_debug_layer()?;
        let dev12 = d3d12::Device::new(&adapter, d3d::FeatureLevel::v11)?;
        let queue = dev12.new_command_queue(d3d12::CommandType::Direct, 0).expect("Failed to create a command queue");
        let (dev11, imm) = d3d11::Device::new(Some(&adapter), true).expect("Failed to create a Direct3D11 Device");
        let dev2 = d2::Device::new(&dev11).expect("Failed to create a Direct2D Device");

        let ref target = Application::get().main_window;
        let cdev = dcomp::Device::new(None).expect("Failed to create a DirectComposition Device");
        let ctarget = cdev.new_target_for(&(target.native() as _)).expect("Failed to create a composition target");
        let cv_root = cdev.new_visual().expect("Failed to create a composition visual");
        ctarget.set_root(&cv_root).expect("Failed to update the composition tree");
        let (cw, ch) = target.client_size();
        let swapchain = xf.new_swapchain(&queue, Size2U(cw as _, ch as _),
            DXGI_FORMAT_R8G8B8A8_UNORM, dxgi::AlphaMode::Ignored, 2, true).expect("Failed to create a swapchain");
        cv_root.set_content(Some(&swapchain)).expect("Failed to update the composition tree");
        cdev.commit().expect("Failed to update the composition tree");

        Ok(RenderDevice { adapter, dev12, dev11, imm, dev2, queue, swapchain, agent_str: None })
    }

    pub fn agent(&self) -> &str
    {
        if self.agent_str.is_none()
        {
            let p = &self.agent_str as *const _ as *mut _;
            let adapter_desc = self.adapter.desc().expect("Failed to retrieve an adapter description");
            let desc_str = unsafe { WideCStr::from_ptr_str(adapter_desc.Description.as_ptr()).to_string_lossy() };
            unsafe { *p = Some(format!("Direct3D12 {:?}", desc_str)); }
        }
        self.agent_str.as_ref().unwrap()
    }

    /*pub fn realize_svg_segments<'a, Iter: Iterator>(&self, provider: Iter) -> IOResult<PathImage> where
        Iter::Item: Iterator<Item = &'a Segment>
    {
        let p = self.dev2.factory().new_path_geometry()?;
        let sink = p.open()?;
        for figure in provider
        {
            let mut prev = Point2F(0.0, 0.0);
            let mut last_curve_pvec = Point2F(0.0, 0.0);
            let mut closed = false;
            for segment in figure
            {
                fn reforming(p: &Point2LF) -> Point2F { Point2F(p.0 as _, p.1 as _) }
                fn reforming2(p: &Point2LF) -> d2::Point2F { d2::Point2F { x: p.0 as _, y: p.1 as _ } }
                match *segment
                {
                    Segment::MoveTo(ref p) => { prev = prev + p; sink.begin_figure(prev, true); },
                    Segment::MoveToAbs(ref p) => { prev = reforming(p); sink.begin_figure(prev, true); },
                    Segment::LineTo(ref p) => { prev = prev + p; sink.add(&prev); },
                    Segment::LineToAbs(ref p) => { prev = reforming(p); sink.add(&prev); },
                    Segment::HorizontalLineTo(x) => { prev.0 += x as f32; sink.add(&prev); },
                    Segment::HorizontalLineToAbs(x) => { prev.0 = x as f32; sink.add(&prev); },
                    Segment::VerticalLineTo(y) => { prev.1 += y as f32; sink.add(&prev); },
                    Segment::VerticalLineToAbs(y) => { prev.1 = y as f32; sink.add(&prev); },
                    Segment::CurveTo(ref p1, ref p2, ref p3) =>
                    {
                        let p2 = prev + &reforming(p2); prev = prev + &reforming(p3);
                        last_curve_pvec = prev - &p2;
                        sink.add(&d2::BezierSegment
                        {
                            point1: *transmute_safe(&(prev + &reforming(p1))), point2: *transmute_safe(&p2), point3: *transmute_safe(&prev)
                        });
                    },
                    Segment::CurveToAbs(ref p1, ref p2, ref p3) =>
                    {
                        prev = reforming(p3); last_curve_pvec = prev - &reforming(p2);
                        sink.add(&d2::BezierSegment { point1: reforming2(p1), point2: reforming2(p2), point3: *transmute_safe(&prev) });
                    },
                    Segment::QuadCurveTo(ref p1, ref p2) =>
                    {
                        let p1 = prev + &reforming(p1); prev = prev + &reforming(p2);
                        last_curve_pvec = prev - &p1;
                        sink.add(&d2::QuadraticBezierSegment { point1: *transmute_safe(&p1), point2: *transmute_safe(&prev) });
                    },
                    Segment::QuadCurveToAbs(ref p1, ref p2) =>
                    {
                        prev = reforming(p2); last_curve_pvec = prev - &reforming(p1);
                        sink.add(&d2::QuadraticBezierSegment { point1: reforming2(p1), point2: *transmute_safe(&prev) });
                    },
                    Segment::SmoothCurveTo(ref p2, ref p3) =>
                    {
                        let p1 = prev + &last_curve_pvec;
                        let p2 = prev + &reforming(p2); prev = prev + &reforming(p3);
                        last_curve_pvec = prev - &p2;
                        sink.add(&d2::BezierSegment
                        {
                            point1: *transmute_safe(&p1), point2: *transmute_safe(&p2), point3: *transmute_safe(&prev)
                        });
                    },
                    Segment::SmoothCurveToAbs(ref p2, ref p3) =>
                    {
                        let p1 = prev + &last_curve_pvec;
                        prev = reforming(p3); last_curve_pvec = prev - &reforming(p2);
                        sink.add(&d2::BezierSegment { point1: *transmute_safe(&p1), point2: reforming2(p2), point3: *transmute_safe(&prev) });
                    },
                    Segment::SmoothQuadCurveTo(ref p2) =>
                    {
                        let p1 = prev + &last_curve_pvec; prev = prev + &reforming(p2);
                        last_curve_pvec = prev - &p1;
                        sink.add(&d2::QuadraticBezierSegment { point1: *transmute_safe(&p1), point2: *transmute_safe(&prev) });
                    },
                    Segment::SmoothQuadCurveToAbs(ref p2) =>
                    {
                        let p1 = prev + &last_curve_pvec;
                        prev = reforming(p2); last_curve_pvec = prev - &p1;
                        sink.add(&d2::QuadraticBezierSegment { point1: *transmute_safe(&p1), point2: *transmute_safe(&prev) });
                    },
                    Segment::ArcTo { ref size, xrot, large_arc, sweep, ref to } =>
                    {
                        prev = prev + &reforming(to);
                        sink.add(&d2::ArcSegment
                        {
                            point: *transmute_safe(&prev), size: d2::SizeF { width: size.0 as _, height: size.1 as _ },
                            rotationAngle: xrot as _, arcSize: if large_arc { d2::ArcSize::Large } else { d2::ArcSize::Small } as _,
                            sweepDirection: if sweep == ArcSweepingDirection::CW { d2::SweepDirection::CW } else { d2::SweepDirection::CCW } as _
                        });
                    },
                    Segment::ArcToAbs { ref size, xrot, large_arc, sweep, ref to } =>
                    {
                        prev = reforming(to);
                        sink.add(&d2::ArcSegment
                        {
                            point: *transmute_safe(&prev), size: d2::SizeF { width: size.0 as _, height: size.1 as _ },
                            rotationAngle: xrot as _, arcSize: if large_arc { d2::ArcSize::Large } else { d2::ArcSize::Small } as _,
                            sweepDirection: if sweep == ArcSweepingDirection::CW { d2::SweepDirection::CW } else { d2::SweepDirection::CCW } as _
                        });
                    },
                    Segment::Close =>
                    {
                        assert!(!closed); closed = true;
                        sink.end_figure(true);
                    }
                }
            }
            if !closed { sink.end_figure(false); }
        }
        sink.close();

        Ok(PathImage { inner: p })
    }*/
}
