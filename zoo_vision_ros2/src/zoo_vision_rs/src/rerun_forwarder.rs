use anyhow::{Error, Result};
use image;
use ndarray::prelude::*;
use rerun::{
    demo_util::grid,
    external::{glam, ndarray},
};
use std::{io::Cursor, path::Path};

pub struct RerunForwarder {
    recording: rerun::RecordingStream,
}

fn nanosec_from_ros(stamp: &builtin_interfaces::msg::rmw::Time) -> i64 {
    1e9 as i64 * stamp.sec as i64 + stamp.nanosec as i64
}

impl RerunForwarder {
    pub fn new(data_path: &Path) -> Result<Self, Error> {
        let recording = rerun::RecordingStreamBuilder::new("zoo_vision").serve_web(
            "0.0.0.0",
            Default::default(),
            Default::default(),
            rerun::MemoryLimit::from_bytes(1024 * 1024 * 1024),
            false,
        )?;

        // Load floor plan
        let world_image = image::ImageReader::open(data_path.join("floor_plan.png"))?.decode()?;
        let world_image_rr = rerun::Image::from_dynamic_image(world_image)?;
        recording.log_static("world/floor_plan", &world_image_rr)?;

        // Log an annotation context to assign a label and color to each class
        recording.log_static(
            "/",
            &rerun::AnnotationContext::new([(
                0,
                "Background",
                rerun::Rgba32::from_unmultiplied_rgba(0, 0, 0, 0),
            )]),
        )?;

        Ok(Self { recording })
    }

    pub fn test_me(&mut self, frame_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let points = grid(glam::Vec3::splat(-10.0), glam::Vec3::splat(10.0), 10);
        let colors = grid(glam::Vec3::ZERO, glam::Vec3::splat(255.0), 10)
            .map(|v| rerun::Color::from_rgb(v.x as u8, v.y as u8, v.z as u8));
        self.recording.log(
            "my_points",
            &rerun::Points3D::new(points)
                .with_colors(colors)
                .with_radii([0.5]),
        )?;
        println!("Test from forwarder, frame_id={}", frame_id);
        Ok(())
    }

    pub fn image_callback(
        &mut self,
        channel: &str,
        msg: &zoo_msgs::msg::rmw::Image12m,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let msg_data_slice = unsafe {
            std::slice::from_raw_parts(msg.data.as_ptr(), (msg.height * msg.width * 3) as usize)
        };
        let image = image::ImageBuffer::<image::Rgb<u8>, &[u8]>::from_raw(
            msg.width,
            msg.height,
            msg_data_slice,
        )
        .unwrap();

        // Compress image
        let mut jpg_writer = Cursor::new(Vec::new());
        image.write_to(&mut jpg_writer, image::ImageFormat::Jpeg)?;
        let image_jpg_data = jpg_writer.into_inner();
        let rr_image =
            rerun::EncodedImage::from_file_contents(image_jpg_data).with_media_type("image/jpeg");

        let time_ns = nanosec_from_ros(&msg.header.stamp);
        self.recording.set_time_nanos("ros_time", time_ns);
        self.recording
            .log(channel, &rr_image.with_draw_order(-1.0))?;

        // Clear out detections
        self.recording.set_time_nanos("ros_time", time_ns - 1);

        let camera_name = "input_camera";
        let image_detections_ent = format!("{}/detections", camera_name);
        self.recording
            .log(image_detections_ent, &rerun::Clear::recursive())?;
        let world_detections_ent = format!("world/{}/detections", camera_name);
        self.recording
            .log(world_detections_ent, &rerun::Clear::recursive())?;
        // println!("Test from forwarder, image id={}", unsafe {
        //     std::str::from_utf8_unchecked(msg.header.frame_id.data.as_slice())
        // });
        Ok(())
    }

    pub fn detection_callback(
        &mut self,
        channel: &str,
        msg: &zoo_msgs::msg::rmw::Detection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // // println!("Test from forwarder, mask id={}", unsafe {
        //     std::str::from_utf8_unchecked(msg.header.frame_id.data.as_slice())
        // });
        let detection_count = msg.detection_count as usize;

        // Set rerun time
        self.recording
            .set_time_nanos("ros_time", nanosec_from_ros(&msg.header.stamp));

        let ids: Vec<_> = (1..(detection_count + 1) as u16).collect();

        // Map message data to image array
        assert!(msg.detection_count == msg.masks.sizes[0]);
        let mask_height = msg.masks.sizes[1] as usize;
        let mask_width = msg.masks.sizes[2] as usize;
        let masks: ArrayBase<ndarray::ViewRepr<&u8>, Ix3> = unsafe {
            ArrayView::from_shape_ptr(
                (detection_count, mask_height, mask_width),
                msg.masks.data.as_ptr(),
            )
        };

        let world_positions: ArrayBase<ndarray::ViewRepr<&f32>, Ix2> = unsafe {
            ArrayView::from_shape_ptr((detection_count, 3), msg.world_positions.as_ptr())
        };

        // Log bounding boxes
        let bbox_centers = (0..detection_count).map(|x| msg.bboxes[x].center);
        let bbox_half_sizes = (0..detection_count).map(|x| msg.bboxes[x].half_size);
        self.recording.log(
            format!("{}/boxes", channel),
            &rerun::Boxes2D::from_centers_and_half_sizes(bbox_centers, bbox_half_sizes)
                .with_class_ids(ids.clone()),
        )?;

        // Log position in world
        let world_points_rr =
            rerun::Points2D::new(world_positions.axis_iter(Axis(0)).map(|x| (x[0], x[1])));
        self.recording.log(
            "world/input_camera/detections/positions",
            &world_points_rr.with_class_ids(ids).with_radii([20.0]),
        )?;

        // Log masks
        const THRESHOLD: u8 = (0.8 * 255.0) as u8;
        let mut image_classes = ndarray::Array::<u8, _>::zeros((mask_height, mask_width).f());
        for id in 0..detection_count {
            // Log image
            let mask_i = masks.slice(s![id, .., ..]);
            for (p, m) in image_classes.iter_mut().zip(mask_i.iter()) {
                if *m >= THRESHOLD {
                    *p = (id + 1) as u8;
                }
            }
        }
        let rr_image = rerun::SegmentationImage::try_from(image_classes)?;
        self.recording.log(
            format!("{}/masks", channel),
            &rr_image.with_draw_order(1.0).with_opacity(0.7),
        )?;

        Ok(())
    }
}
