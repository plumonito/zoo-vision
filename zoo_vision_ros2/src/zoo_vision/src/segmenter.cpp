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

#include "zoo_vision/segmenter.hpp"

#include "zoo_vision/utils.hpp"

#include <ATen/core/List.h>
#include <opencv2/core.hpp>
#include <opencv2/highgui.hpp>
#include <opencv2/imgproc.hpp>
#include <rclcpp/time.hpp>
#include <sensor_msgs/image_encodings.hpp>
#include <torch/torch.h>

#include <chrono>
#include <fstream>
#include <string.h>

using namespace std::chrono_literals;
using namespace torch::indexing;

namespace zoo {

Segmenter::Segmenter(const rclcpp::NodeOptions &options) : Node("segmenter", options) {
  // Load model
  const auto modelDir = getDataPath().parent_path() / "models/camera0/segmentation";
  const std::string modelPath = modelDir / "torch.pt";

  model_ = torch::jit::load(modelPath, torch::kCUDA);

  // DEBUG print model info

  // Load anchors
  const auto anchorsPath = modelDir / "anchors.bin";
  assert(std::filesystem::exists(anchorsPath));
  const size_t anchorsByteSize = std::filesystem::file_size(anchorsPath);
  const size_t anchorCount = anchorsByteSize / 4 / sizeof(float32_t);
  std::vector<float32_t> anchors;
  anchors_.resize(anchorCount, 4);
  {
    std::ifstream f(anchorsPath, std::ios::binary);
    f.read(reinterpret_cast<char *>(anchors_.data()), anchorsByteSize);
  }

  // Subscribe to receive images from camera
  imageSubscriber_ = rclcpp::create_subscription<zoo_msgs::msg::Image12m>(
      *this, "input_camera/image", 10, [this](const zoo_msgs::msg::Image12m &msg) { this->onImage(msg); });

  // Publish detections
  detectionImagePublisher_ =
      rclcpp::create_publisher<zoo_msgs::msg::Image12m>(*this, "input_camera/detections/image", 10);
  detectionPublisher_ = rclcpp::create_publisher<zoo_msgs::msg::Detection>(*this, "input_camera/detections", 10);
}

void Segmenter::loadModel() {}

void Segmenter::onImage(const zoo_msgs::msg::Image12m &imageMsg) {

  const cv::Mat3b img = wrapMat3bFromMsg(imageMsg);
  const float inputAspect = static_cast<float>(imageMsg.width) / imageMsg.height;
  const int DETECTION_HEIGHT = 600;

  torch::IValue detectionResult;
  {
    auto detectionImageMsg = std::make_unique<zoo_msgs::msg::Image12m>();
    detectionImageMsg->header = imageMsg.header;
    setMsgString(detectionImageMsg->encoding, sensor_msgs::image_encodings::BGR8);
    detectionImageMsg->height = DETECTION_HEIGHT;
    detectionImageMsg->width = DETECTION_HEIGHT * inputAspect;
    detectionImageMsg->step = detectionImageMsg->width * 3 * sizeof(char);
    cv::Mat3b detectionImage = wrapMat3bFromMsg(*detectionImageMsg);

    cv::resize(img, detectionImage, detectionImage.size());

    // TODO: accept input as uint8
    cv::Mat3f detectionImagef;
    detectionImage.convertTo(detectionImagef, CV_32FC3, 1.0f / 255);

    torch::Tensor imageTensor =
        torch::from_blob(detectionImagef.data, {detectionImagef.rows, detectionImagef.cols, detectionImagef.channels()},
                         torch::TensorOptions().dtype(torch::kFloat32));

    imageTensor = imageTensor.permute({2, 0, 1}).to(torch::kCUDA);
    c10::List<torch::Tensor> imageList({imageTensor});

    // TorchScript models require a List[IValue] as input
    detectionResult = model_.forward({imageList});

    // Publish image to have it in sync with masks
    detectionImagePublisher_->publish(std::move(detectionImageMsg));
  }

  const auto detections = detectionResult.toTuple()->elements()[1].toList()[0].get().toGenericDict();

  // const torch::Tensor boxes = detections.at("boxes").toTensor();
  // const torch::Tensor scores = detections.at("scores").toTensor().to(torch::kCPU);

  const torch::Tensor masksGpu = detections.at("masks").toTensor();
  assert(masksGpu.dim() == 4);
  const int64_t maskCount = masksGpu.sizes()[0];
  for (int i = 0; i < maskCount; ++i) {
    const torch::Tensor maskGpu = masksGpu.index({i, 0});
    const torch::Tensor maskb = maskGpu.mul(255).clamp(0, 255).to(torch::kU8).to(torch::kCPU);

    assert(maskb.stride(1) == 1);
    const cv::Mat1b maskCv(maskb.sizes()[0], maskb.sizes()[1], maskb.data_ptr<uchar>(), maskb.stride(0) * sizeof(char));

    // cv::imwrite("test.png", maskCv);

    // Publish detection message
    auto detectionMsg = std::make_unique<zoo_msgs::msg::Detection>();
    detectionMsg->detection_id = i;
    detectionMsg->mask.header = imageMsg.header;
    copyMat1bToMsg(maskCv, detectionMsg->mask);

    detectionPublisher_->publish(std::move(detectionMsg));
  }
}

} // namespace zoo

#include "rclcpp_components/register_node_macro.hpp"

RCLCPP_COMPONENTS_REGISTER_NODE(zoo::Segmenter)