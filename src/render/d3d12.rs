
use std::io::Result as IOResult;
use comdrive::*;
use Application;
use winapi::shared::dxgiformat::*;
use metrics::*;
use widestring::WideCStr;
use svgparser::path::Token as Segment;

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

    pub fn realize_svg_segments<'a, Iter: Iterator>(&self, mut provider: Iter) -> IOResult<PathImage> where
        Iter::Item: Iterator<Item = &'a Segment>
    {
        let p = self.dev2.factory().new_path_geometry()?;
        let sink = p.open()?;
        for figure in provider
        {
            let mut prev = Point2F(0.0, 0.0);
            let mut last_curve_pvec = Point2F(0.0, 0.0);
            let mut closed = true;
            for segment in figure
            {
                match *segment
                {
                    Segment::MoveTo { abs, x, y } =>
                    {
                        prev = if abs { Point2F(x as _, y as _) } else { Point2F(x as _, y as _) + &prev };
                        if !closed { sink.end_figure(false); } closed = false; sink.begin_figure(prev, true);
                    },
                    Segment::LineTo { abs, x, y } =>
                    {
                        prev = if abs { Point2F(x as _, y as _) } else { Point2F(x as _, y as _) + &prev };
                        sink.add(&prev);
                    },
                    Segment::HorizontalLineTo { abs, x } => { prev.0 = if abs { x as _ } else { x as f32 + prev.0 }; sink.add(&prev); },
                    Segment::VerticalLineTo   { abs, y } => { prev.1 = if abs { y as _ } else { y as f32 + prev.1 }; sink.add(&prev); },
                    Segment::CurveTo { abs, x1, y1, x2, y2, x, y } =>
                    {
                        let p1 = Point2F(x1 as _, y1 as _) + &if abs { Point2F::ZERO } else { prev };
                        let p2 = Point2F(x2 as _, y2 as _) + &if abs { Point2F::ZERO } else { prev };
                        prev = Point2F(x as _, y as _) + &if abs { Point2F::ZERO } else { prev };
                        last_curve_pvec = prev - &p2;
                        sink.add(&d2::BezierSegment { point1: *transmute_safe(&p1), point2: *transmute_safe(&p2), point3: *transmute_safe(&prev) });
                    },
                    Segment::Quadratic { abs, x1, y1, x, y } =>
                    {
                        let p1 = Point2F(x1 as _, y1 as _) + &if abs { Point2F::ZERO } else { prev };
                        prev = Point2F(x as _, y as _) + &if abs { Point2F::ZERO } else { prev };
                        last_curve_pvec = prev - &p1;
                        sink.add(&d2::QuadraticBezierSegment { point1: *transmute_safe(&p1), point2: *transmute_safe(&prev) });
                    },
                    Segment::SmoothCurveTo { abs, x2, y2, x, y } =>
                    {
                        let p1 = prev + &last_curve_pvec;
                        let p2 = Point2F(x2 as _, y2 as _) + &if abs { Point2F::ZERO } else { prev };
                        prev = Point2F(x as _, y as _) + &if abs { Point2F::ZERO } else { prev };
                        last_curve_pvec = prev - &p2;
                        sink.add(&d2::BezierSegment { point1: *transmute_safe(&p1), point2: *transmute_safe(&p2), point3: *transmute_safe(&prev) });
                    },
                    Segment::SmoothQuadratic { abs, x, y } =>
                    {
                        let p1 = prev + &last_curve_pvec;
                        prev = Point2F(x as _, y as _) + &if abs { Point2F::ZERO } else { prev };
                        last_curve_pvec = prev - &p1;
                        sink.add(&d2::QuadraticBezierSegment { point1: *transmute_safe(&p1), point2: *transmute_safe(&prev) });
                    },
                    Segment::EllipticalArc { abs, rx, ry, x_axis_rotation, large_arc, sweep, x, y } =>
                    {
                        prev = Point2F(x as _, y as _) + &if abs { Point2F::ZERO } else { prev };
                        sink.add(&d2::ArcSegment
                        {
                            point: *transmute_safe(&prev), size: d2::SizeF { width: rx as _, height: ry as _ },
                            rotationAngle: x_axis_rotation as _, arcSize: if large_arc { d2::ArcSize::Large } else { d2::ArcSize::Small } as _,
                            sweepDirection: if sweep { d2::SweepDirection::CW } else { d2::SweepDirection::CCW } as _
                        });
                    },
                    Segment::ClosePath { .. } =>
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
    }
}
