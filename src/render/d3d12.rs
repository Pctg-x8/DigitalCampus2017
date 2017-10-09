
use std::io::Result as IOResult;
use comdrive::*;
use Application;
use winapi::shared::dxgiformat::*;
use metrics::*;
use widestring::WideCStr;
use std::iter::Iterator;
use svgdom::types::path::{Segment, SegmentData};
use std::mem::transmute;

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

    pub fn realize_svg_segments<'a, Iter: Iterator>(&self, provider: Iter) -> IOResult<PathImage> where
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
                match segment.data
                {
                    SegmentData::MoveTo { x, y } =>
                    {
                        prev = if segment.absolute { Point2F(x as _, y as _) } else { prev + Point2F(x as _, y as _) };
                        sink.begin_figure(prev, true);
                    },
                    SegmentData::LineTo { x, y } =>
                    {
                        prev = if segment.absolute { Point2F(x as _, y as _) } else { prev + Point2F(x as _, y as _) };
                        sink.add(&prev);
                    },
                    SegmentData::HorizontalLineTo { x } =>
                    {
                        prev = if segment.absolute { Point2F(x as _, prev.y()) } else { prev + Point2F(x as _, 0.0) };
                        sink.add(&prev);
                    },
                    SegmentData::VerticalLineTo { y } =>
                    {
                        prev = if segment.absolute { Point2F(prev.x(), y as _) } else { prev + Point2F(0.0, y as _) };
                        sink.add(&prev);
                    },
                    SegmentData::CurveTo { x1, y1, x2, y2, x, y } =>
                    {
                        let p0 = if segment.absolute { d2::Point2F { x: x1 as _, y: y1 as _ } } else { d2::Point2F { x: x1 as f32 + prev.x(), y: y1 as f32 + prev.y() } };
                        let p1 = if segment.absolute { d2::Point2F { x: x2 as _, y: y2 as _ } } else { d2::Point2F { x: x2 as f32 + prev.x(), y: y2 as f32 + prev.y() } };
                        prev = if segment.absolute { Point2F(x as _, y as _) } else { prev + Point2F(x as _, y as _) };
                        last_curve_pvec = prev - unsafe { transmute::<_, Point2F>(p1) };
                        sink.add(&d2::BezierSegment { point1: p0, point2: p1, point3: *transmute_safe(&prev) });
                    },
                    SegmentData::SmoothCurveTo { x2, y2, x, y } =>
                    {
                        let p0 = prev + last_curve_pvec;
                        let p1 = if segment.absolute { d2::Point2F { x: x2 as _, y: y2 as _ } } else { d2::Point2F { x: x2 as f32 + prev.x(), y: y2 as f32 + prev.y() } };
                        prev = if segment.absolute { Point2F(x as _, y as _) } else { prev + Point2F(x as _, y as _) };
                        last_curve_pvec = prev - unsafe { transmute::<_, Point2F>(p1) };
                        sink.add(&d2::BezierSegment { point1: *transmute_safe(&p0), point2: p1, point3: *transmute_safe(&prev) });
                    },
                    SegmentData::Quadratic { x1, y1, x, y } =>
                    {
                        let p0 = if segment.absolute { d2::Point2F { x: x1 as _, y: y1 as _ } } else { d2::Point2F { x: x1 as f32 + prev.x(), y: y1 as f32 + prev.y() } };
                        prev = if segment.absolute { Point2F(x as _, y as _) } else { prev + Point2F(x as _, y as _) };
                        last_curve_pvec = prev - unsafe { transmute::<_, Point2F>(p0) };
                        sink.add(&d2::QuadraticBezierSegment { point1: p0, point2: *transmute_safe(&prev) });
                    },
                    SegmentData::SmoothQuadratic { x, y } =>
                    {
                        let p0 = prev + last_curve_pvec;
                        prev = if segment.absolute { Point2F(x as _, y as _) } else { prev + Point2F(x as _, y as _) };
                        last_curve_pvec = prev - unsafe { transmute::<_, Point2F>(p0) };
                        sink.add(&d2::QuadraticBezierSegment { point1: *transmute_safe(&p0), point2: *transmute_safe(&prev) });
                    },
                    SegmentData::EllipticalArc { rx, ry, x_axis_rotation, large_arc, sweep, x, y } =>
                    {
                        prev = if segment.absolute { Point2F(x as _, y as _) } else { prev + Point2F(x as _, y as _) };
                        sink.add(&d2::ArcSegment
                        {
                            point: *transmute_safe(&prev), size: d2::SizeF { width: rx as _, height: ry as _ },
                            rotationAngle: x_axis_rotation as _,
                            arcSize: if !large_arc { d2::ArcSize::Small } else { d2::ArcSize::Large } as _,
                            sweepDirection: if !sweep { d2::SweepDirection::CCW } else { d2::SweepDirection::CW } as _
                        });
                    },
                    SegmentData::ClosePath =>
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
