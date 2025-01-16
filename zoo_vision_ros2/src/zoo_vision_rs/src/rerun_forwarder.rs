use anyhow::{Error, Result};
use ndarray::prelude::*;
use rerun::{
    demo_util::grid,
    external::{glam, ndarray},
};
use std::path::Path;

const MAX_DETECTION_COUNT: i32 = 5;

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
        use rerun::external::ndarray::ArrayView;
        let data_view = unsafe {
            ArrayView::from_shape_ptr(
                (msg.height as usize, msg.width as usize, 3),
                msg.data.as_ptr(),
            )
        };
        let rr_image =
            rerun::Image::from_color_model_and_tensor(rerun::ColorModel::BGR, data_view).unwrap();

        let time_ns = nanosec_from_ros(&msg.header.stamp);
        self.recording.set_time_sequence("ros_time", time_ns);
        self.recording
            .log(channel, &rr_image.with_draw_order(-1.0))?;

        // Clear out detections
        self.recording.set_time_sequence("ros_time", time_ns - 1);

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
        let id = msg.detection_id as i32;
        if id >= MAX_DETECTION_COUNT {
            return Ok(());
        }

        // Map message data to image array
        let data_view: ArrayBase<ndarray::ViewRepr<&u8>, Ix2> = unsafe {
            ArrayView::from_shape_ptr(
                (msg.mask.height as usize, msg.mask.width as usize),
                msg.mask.data.as_ptr(),
            )
        };

        // Set rerun time
        self.recording
            .set_time_sequence("ros_time", nanosec_from_ros(&msg.mask.header.stamp));

        // Log image
        // Create an rgba image
        let mut image_rgb = ndarray::Array::<u8, _>::zeros(
            (msg.mask.height as usize, msg.mask.width as usize, 3).f(),
        );
        let color_idx = (id % 3) as usize;
        data_view.assign_to(&mut image_rgb.index_axis_mut(Axis(2), color_idx));

        let rr_image =
            rerun::Image::from_color_model_and_tensor(rerun::ColorModel::RGB, image_rgb).unwrap();
        self.recording.log(
            format!("{}/{}/mask", channel, id),
            &rr_image.with_draw_order(id as f32).with_opacity(0.3),
        )?;

        // Log bounding box
        self.recording.log(
            format!("{}/{}/box", channel, id),
            &rerun::Boxes2D::from_centers_and_half_sizes(
                [(msg.bbox.center[0], msg.bbox.center[1])],
                [(msg.bbox.half_size[0], msg.bbox.half_size[1])],
            ),
        )?;

        // Log position in world
        let world_points_rr =
            rerun::Points2D::new([(msg.world_position[0], msg.world_position[1])]);
        let mut color = [0, 0, 0];
        color[color_idx] = 255;
        let color_rr = rerun::Color::from_unmultiplied_rgba(color[0], color[1], color[2], 76);
        self.recording.log(
            format!("world/input_camera/detections/{}", id),
            &world_points_rr.with_colors([color_rr]).with_radii([20.0]),
        )?;
        // // println!("Test from forwarder, mask id={}", unsafe {
        //     std::str::from_utf8_unchecked(msg.header.frame_id.data.as_slice())
        // });
        Ok(())
    }
}
