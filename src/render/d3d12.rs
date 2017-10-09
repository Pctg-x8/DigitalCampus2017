
use std::io::Result as IOResult;
use comdrive::*;
use Application;
use winapi::shared::dxgiformat::*;
use metrics::*;
use widestring::WideCStr;
use std::iter::Iterator;
use svgdom::types::path::Segment;

struct VectorImage { inner: d2::PathGeometry }

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

    pub fn realize_svg_segments<'a, Iter: Iterator<Item = &'a [Segment]>>(&self, provider: Iter) -> IOResult<VectorImage>
    {
        let p = self.dev2.factory().new_path_geometry()?;
        let sink = p.open()?;
        for figure in provider
        {
            for segment in figure
            {
                let mut prev = Point2F(0, 0);
                let mut last_curve_pvec = Point2F(0, 0);
                match segment.data
                {
                    Segment::MoveTo { x, y } =>
                    {
                        prev = if segment.absolute { Point2F(x, y) } else { prev + Point2F(x, y) };
                        sink.begin_figure(prev, true);
                    },
                    Segment::LineTo { x, y } =>
                    {
                        prev = if segment.absolute { Point2F(x, y) } else { prev + Point2F(x, y) };
                        sink.add(&prev);
                    },
                    Segment::HorizontalLineTo { x } =>
                    {
                        prev = if segment.absolute { Point2F(x, prev.y()) } else { prev + Point2F(x, 0) };
                        sink.add(&prev);
                    },
                    Segment::VerticalLineTo { y } =>
                    {
                        prev = if segment.absolute { Point2F(prev.x(), y) } else { prev + Point2F(0, y) };
                        sink.add(&prev)
                    },
                    Segment::CurveTo { x1, y1, x2, y2, x, y } =>
                    {
                        let p0 = if segment.absolute { d2::Point2F { x: x1, y: y1 } } else { d2::Point2F { x: x1 + prev.x(), y: y1 + prev.y() } };
                        let p1 = if segment.absolute { d2::Point2F { x: x2, y: y2 } } else { d2::Point2F { x: x2 + prev.x(), y: y2 + prev.y() } };
                        prev = if segment.absolute { Point2F(x, y) } else { prev + Point2F(x, y) };
                        last_curve_pvec = prev - p1;
                        sink.add(&d2::BezierSegment { point1: p0, point2: p1, point3: *transmute_safe(&prev) });
                    },
                    Segment::SmoothCurveTo { x2, y2, x, y } =>
                    {
                        let p0 = prev + last_curve_pvec;
                        let p1 = if segment.absolute { d2::Point2F { x: x2, y: y2 } } else { d2::Point2F { x: x2 + prev.x(), y: y2 + prev.y() } };
                        prev = if segment.absolute { Point2F(x, y) } else { prev + Point2F(x, y) };
                        last_curve_pvec = prev - p1;
                        sink.add(&d2::BezierSegment { point1: p0, point2: p1, point3: *transmute_safe(&prev) });
                    },
                    Segment::Quadratic { x1, y1, x, y } =>
                    {
                        let p0 = if segment.absolute { d2::Point2F { x: x1, y: y1 } } else { d2::Point2F { x: x1 + prev.x(), y: y1 + prev.y() } };
                        prev = if segment.absolute { Point2F(x, y) } else { prev + Point2F(x, y) };
                        last_curve_pvec = prev - p0;
                        sink.add(&d2::QuadraticBezierSegment { point1: p0, point2: *transmute_safe(&prev) });
                    },
                    Segment::SmoothQuadratic { x, y } =>
                    {
                        let p0 = prev + last_curve_pvec;
                        prev = if segment.absolute { Point2F(x, y) } else { prev + Point2F(x, y) };
                        last_curve_pvec = prev - p0;
                        sink.add(&d2::QuadraticBezierSegment { point1: p0, point2: *transmute_safe(&prev) });
                    }
                }
            }
        }
    }
}
