
use std::io::Result as IOResult;
use comdrive::*;
use Application;
use winapi::shared::dxgiformat::*;
use metrics::*;
use widestring::WideCStr;
use svgparser::path::Token as Segment;
use std::mem::zeroed;
use winapi::shared::ntdef::HANDLE;
use winapi::um::winbase::INFINITE;
use winapi::um::handleapi::CloseHandle;
use winapi::um::synchapi::{CreateEventA, WaitForSingleObject};
use std::cell::RefCell;

pub struct PathImage { inner: d2::PathGeometry }
impl super::VectorImage for PathImage {}

const BACKBUFFER_COUNT: usize = 2;

pub struct DescriptorHandles
{
    #[allow(dead_code)] rtv_heap: d3d12::DescriptorHeap,

    rth_scbuffer_base: d3d12::HostDescriptorHandle
}
pub struct RenderControl
{
    counter: u64, fence: d3d12::Fence, event: HANDLE
}
impl Drop for RenderControl { fn drop(&mut self) { unsafe { CloseHandle(self.event); } } }
pub struct RenderDevice
{
    adapter: dxgi::Adapter, dev12: d3d12::Device, dev11: d3d11::Device, imm: d3d11::ImmediateContext, wdev: d3d11on12::Device, dev2: d2::Device, dc2: d2::DeviceContext,
    queue: d3d12::CommandQueue, swapchain: dxgi::SwapChain,

    agent_str: Option<String>, dh: DescriptorHandles, scbuffers: [(d3d12::Resource, d3d11on12::WrappedResource<d3d11::Texture2D>); BACKBUFFER_COUNT],
    render_control: RefCell<RenderControl>
}
impl RenderDevice
{
    pub fn init() -> IOResult<Self>
    {
        let xf = dxgi::Factory::new(cfg!(feature = "debug"))?;
        let adapter = xf.adapter(0)?;
        #[cfg(feature = "debug")] d3d12::Device::enable_debug_layer()?;
        let dev12 = d3d12::Device::new(&adapter, d3d::FeatureLevel::v11)?;
        let queue = dev12.new_command_queue(d3d12::CommandType::Direct, 0).expect("Failed to create a command queue");
        let (wdev, imm) = d3d11on12::Device::new(&dev12, &[&queue], true, cfg!(feature = "debug")).expect("Failed to create a Direct3D11on12 interop device");
        let dev11 = wdev.query_interface().expect("Failed to get underlying device");
        let dev2 = d2::Device::new(&wdev).expect("Failed to create a Direct2D Device");
        let dc2 = dev2.new_context().expect("Failed to create a Direct2D Device Context");

        let ref target = Application::get().main_window;
        let cdev = dcomp::Device::new(None).expect("Failed to create a DirectComposition Device");
        let ctarget = cdev.new_target_for(&(target.native() as _)).expect("Failed to create a composition target");
        let cv_root = cdev.new_visual().expect("Failed to create a composition visual");
        ctarget.set_root(&cv_root).expect("Failed to update the composition tree");
        let (cw, ch) = target.client_size();
        let swapchain = xf.new_swapchain(&queue, Size2U(cw as _, ch as _),
            DXGI_FORMAT_R8G8B8A8_UNORM, dxgi::AlphaMode::Ignored, BACKBUFFER_COUNT, true).expect("Failed to create a swapchain");
        cv_root.set_content(Some(&swapchain)).expect("Failed to update the composition tree");
        cdev.commit().expect("Failed to update the composition tree");

        let dh = DescriptorHandles::init(&dev12);
        let mut scbuffers: [(_, _); BACKBUFFER_COUNT] = unsafe { zeroed() };
        for (i, r) in scbuffers.iter_mut().enumerate()
        {
            r.0 = swapchain.back_buffer(i).expect("Failed to retrieve a back buffer from the swap chain");
            dev12.create_render_target_view(&r.0, None, *dh.rth_scbuffer_base.offset(i).as_ref());
            r.1 = wdev.new_wrapped_resource(&r.0, d3d11::BindFlags::new().render_target(), d3d12::ResourceState::Present, d3d12::ResourceState::Present)
                .expect("Failed to wrap a d3d12 resource as d3d11 resource");
        }

        Ok(RenderDevice
        {
            render_control: RefCell::new(RenderControl
            {
                fence: dev12.new_fence(0).expect("Failed to create a fence"),
                counter: 0, event: unsafe { CreateEventA(0 as _, false as _, false as _, "Fence Event\x00".as_ptr() as _) }
            }),
            adapter, dev12, dev11, imm, wdev, dev2, dc2, queue, swapchain, agent_str: None, dh, scbuffers
        })
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

    pub fn begin_render(&self) -> IOResult<u32>
    {
        self.render_control.borrow().wait()?;
        let findex = self.swapchain.current_back_buffer_index();
        self.wdev.acquire_wrapped_resources(&[self.scbuffers[findex as usize].1.as_ptr()]);
        // let xs: dxgi::Surface = self.scbuffers[findex as usize].1.query_interface()?;
        let xbmp = self.dc2.new_bitmap_for_render_target(d2::RenderableBitmapSource::FromDxgiSurface(&self.scbuffers[findex as usize].1),
            ::winapi::shared::dxgiformat::DXGI_FORMAT_R8G8B8A8_UNORM, dxgi::AlphaMode::Ignored)?;
        self.dc2.set_target(&xbmp)
            .begin_draw().clear(&d2::ColorF { r: 1.0, g: 1.0, b: 1.0, a: 0.5 }).end_draw()?;
        Ok(findex)
    }
    pub fn end_render(&self, findex: u32) -> IOResult<()>
    {
        self.wdev.release_wrapped_resources(&[self.scbuffers[findex as usize].1.as_ptr()]);
        self.imm.flush();
        self.swapchain.present()?;
        self.render_control.borrow_mut().signal_queue(&self.queue, false)
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
        sink.close()?;

        Ok(PathImage { inner: p })
    }
}

impl DescriptorHandles
{
    const RTVS: usize = BACKBUFFER_COUNT;

    fn init(dev: &d3d12::Device) -> Self
    {
        let rtv_heap = dev.new_descriptor_heap(d3d12::DescriptorHeapContents::RenderTargetViews, Self::RTVS, false)
            .expect("Failed to create a DescriptorHeap for RenderTargets");
        
        DescriptorHandles
        {
            rth_scbuffer_base: rtv_heap.host_descriptor_handle_base(),
            rtv_heap
        }
    }
}

impl RenderControl
{
    fn signal_queue(&mut self, q: &d3d12::CommandQueue, wait: bool) -> IOResult<()>
    {
        if self.counter > self.fence.completed_value()
        {
            // wait
            self.fence.set_event_notification(self.counter, self.event)?;
            if wait { unsafe { WaitForSingleObject(self.event, INFINITE); } }
        }
        self.counter += 1;
        q.signal(&self.fence, self.counter).map(drop)
    }
    fn wait(&self) -> IOResult<()>
    {
        if self.counter > self.fence.completed_value()
        {
            // wait
            self.fence.set_event_notification(self.counter, self.event)?;
            unsafe { WaitForSingleObject(self.event, INFINITE); }
        }
        Ok(())
    }
}
impl Drop for RenderDevice { fn drop(&mut self) { self.render_control.borrow().wait().unwrap(); } }
