//! Explicit GPU smoke check: run with --ignored --nocapture.
use super::*;
use eframe::{egui_wgpu, wgpu};
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Wake, Waker},
};

fn block_on<T>(future: impl Future<Output = T>) -> T {
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(result) => return result,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[test]
#[ignore = "requires a GPU; verifies final rendered pixels during scrollbar dragging"]
fn scrollbar_drag_keeps_document_glyphs_in_the_gpu_frame() {
    let instance = wgpu::Instance::default();
    let adapter = block_on(instance.request_adapter(&Default::default())).expect("GPU adapter");
    let (device, queue) = block_on(adapter.request_device(&Default::default())).unwrap();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = egui_wgpu::Renderer::new(&device, format, Default::default());
    for (compare, compact, split, scale) in [
        (false, false, false, 1),
        (true, false, false, 1),
        (true, true, false, 1),
        (true, false, true, 1),
        (true, true, false, 2),
    ] {
        let width = 1280 * scale;
        let height = 800 * scale;
        let size = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scrollbar regression frame"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scrollbar regression pixels"),
            size: (width * height * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut app = ready_app();
        let text = format!(
            "[\n{}\n]",
            vec![
                r#"  {"id":123,"value":"long scrolling text"}"#;
                if split { 50_000 } else { 500_000 }
            ]
            .join(if compact { "," } else { ",\n" })
        );
        app.editor = text.clone();
        app.full_text = Arc::new(text.clone());
        app.search_source = app.full_text.clone();
        app.editable = true;
        app.view = if compare { View::Compare } else { View::Graph };
        app.compare_text = text.clone();
        app.compare_alignment = Some(Arc::new(forma::alignment::align(&text, &text)));
        app.comparison_done = true;
        let ctx = egui::Context::default();
        ui::install_fonts(&ctx);
        ui::configure(&ctx, false);

        let mut track = egui::Rect::ZERO;
        let frames = if scale == 2 { 90 } else { 18 };
        for frame in 0..frames {
            let point = egui::pos2(track.center().x, track.top() + 6.0);
            let pos = if split {
                egui::pos2(640.0 + (frame as f32 - 3.0) * 15.0, 400.0)
            } else {
                egui::pos2(
                    point.x,
                    track.top()
                        + track.height()
                            * if scale == 1 {
                                ((frame as f32 - 3.0) / 12.0).clamp(0.0, 1.0)
                            } else if frame < 40 {
                                (frame as f32 - 3.0) / 38.0
                            } else {
                                ((frame * 29 % 87) as f32) / 86.0
                            },
                )
            };
            let events = match frame {
                2 => vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: if split {
                            egui::pos2(640.0, 400.0)
                        } else {
                            point
                        },
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                frame if (3..frames - 3).contains(&frame) => vec![egui::Event::PointerMoved(pos)],
                frame if frame == frames - 3 => vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                _ => vec![],
            };
            let mut input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 800.0),
                )),
                time: Some(frame as f64 / 60.0),
                events,
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .native_pixels_per_point = Some(scale as f32);
            let output = ctx.run(input, |ctx| {
                if frame == 1 && scale == 2 {
                    let id = egui::Id::new("forma_source");
                    ctx.memory_mut(|memory| memory.request_focus(id));
                    let mut state =
                        egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(5),
                            egui::text::CCursor::new(1000),
                        )));
                    state.store(ctx, id);
                }
                app.workspace(ctx)
            });
            if frame >= 3 && !split {
                assert_eq!(
                    output.platform_output.num_completed_passes, 1,
                    "scrollbar movement must stay in one layout pass"
                );
            }
            track = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.rect.width() < 20.0
                            && rect.rect.height() > 100.0
                            && rect.rect.center().x < 1280.0 * app.split_ratio =>
                    {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .expect("scrollbar");
            for (id, delta) in &output.textures_delta.set {
                renderer.update_texture(&device, &queue, *id, delta);
            }
            let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
            let vertex_count: usize = jobs
                .iter()
                .map(|job| match &job.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.len(),
                    _ => 0,
                })
                .sum();
            assert!(
                vertex_count < 40_000,
                "visible document mesh must remain bounded on frame {frame}"
            );
            let screen = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [width as u32, height as u32],
                pixels_per_point: output.pixels_per_point,
            };
            let mut encoder = device.create_command_encoder(&Default::default());
            let callbacks = renderer.update_buffers(&device, &queue, &mut encoder, &jobs, &screen);
            {
                let mut pass = encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &texture.create_view(&Default::default()),
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    })
                    .forget_lifetime();
                renderer.render(&mut pass, &jobs, &screen);
            }
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some((width * 4) as u32),
                        rows_per_image: None,
                    },
                },
                size,
            );
            queue.submit(callbacks.into_iter().chain([encoder.finish()]));
            let (tx, rx) = mpsc::channel();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    tx.send(result).unwrap();
                });
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let bytes = buffer.slice(..).get_mapped_range();
            let mut largest_black_run = 0;
            for y in 680 * scale..800 * scale {
                let mut run = 0;
                for x in 65 * scale..610 * scale {
                    let pixel = &bytes[(y * width + x) * 4..][..4];
                    if pixel[..3].iter().all(|channel| *channel < 45) {
                        run += 1;
                        largest_black_run = largest_black_run.max(run);
                    } else {
                        run = 0;
                    }
                }
            }
            if largest_black_run > 60 * scale {
                let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
                for pixel in bytes.as_chunks::<4>().0 {
                    ppm.extend_from_slice(&pixel[..3]);
                }
                std::fs::write("/private/tmp/forma-black-block.ppm", ppm).unwrap();
            }
            assert!(
                largest_black_run < 60 * scale,
                "black block frame={frame} width={largest_black_run}"
            );
            let mut colored = 0;
            for y in 160 * scale..650 * scale {
                for x in 80 * scale..550 * scale {
                    let pixel = &bytes[(y * width + x) * 4..][..4];
                    if i32::from(pixel[1]) > i32::from(pixel[0]) + 15
                        && i32::from(pixel[1]) > i32::from(pixel[2]) + 15
                    {
                        colored += 1;
                    }
                }
            }
            if frame == 0 || frame == frames - 4 {
                let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
                for pixel in bytes.as_chunks::<4>().0 {
                    ppm.extend_from_slice(&pixel[..3]);
                }
                std::fs::write(
                    format!("/private/tmp/forma-gpu-scroll-{compare}-{scale}x-{frame}.ppm"),
                    ppm,
                )
                .unwrap();
            }

            assert!(
                colored > 500,
                "document disappeared in GPU frame {frame}, compare={compare}"
            );
            drop(bytes);
            buffer.unmap();
            for id in &output.textures_delta.free {
                renderer.free_texture(id);
            }
        }
    }
}
