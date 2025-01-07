// This file is part of zoo_vision.
//
// zoo_vision is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.
//
// zoo_vision is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
// A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// zoo_vision. If not, see <https://www.gnu.org/licenses/>.

#include "rclcpp/rclcpp.hpp"
#include <image_transport/camera_publisher.hpp>
#include <image_transport/image_transport.hpp>
#include <opencv2/highgui/highgui.hpp>

namespace zoo {
class ZooCamera : public rclcpp::Node {
public:
  explicit ZooCamera(const rclcpp::NodeOptions &options = rclcpp::NodeOptions());
  void onTimer();

  std::string videoUrl_;
  cv::VideoCapture cvStream_;
  size_t frameIndex_;

  image_transport::Publisher publisher_;
  rclcpp::TimerBase::SharedPtr timer_;
};
} // namespace zoo