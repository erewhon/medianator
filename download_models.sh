#!/bin/bash

# Download pre-trained face detection models

echo "Downloading face detection models..."

# Create models directory
mkdir -p models

cd models

# Download YuNet face detection model (lightweight and accurate)
echo "Downloading YuNet face detection model..."
if [ ! -f "face_detection_yunet_2023mar.onnx" ]; then
    curl -L -o face_detection_yunet_2023mar.onnx \
        "https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx"
    echo "YuNet model downloaded"
else
    echo "YuNet model already exists"
fi

# Alternative: Download OpenCV's Caffe model (more compatible)
echo "Downloading OpenCV DNN face detection model..."
if [ ! -f "opencv_face_detector.caffemodel" ]; then
    curl -L -o opencv_face_detector.caffemodel \
        "https://github.com/opencv/opencv_3rdparty/raw/dnn_samples_face_detector_20170830/res10_300x300_ssd_iter_140000.caffemodel"
    echo "Caffe model downloaded"
else
    echo "Caffe model already exists"
fi

if [ ! -f "opencv_face_detector.prototxt" ]; then
    curl -L -o opencv_face_detector.prototxt \
        "https://raw.githubusercontent.com/opencv/opencv/master/samples/dnn/face_detector/deploy.prototxt"
    echo "Prototxt config downloaded"
else
    echo "Prototxt config already exists"
fi

# Download Haar Cascade as fallback
echo "Downloading Haar Cascade face detection model..."
if [ ! -f "haarcascade_frontalface_default.xml" ]; then
    curl -L -o haarcascade_frontalface_default.xml \
        "https://raw.githubusercontent.com/opencv/opencv/master/data/haarcascades/haarcascade_frontalface_default.xml"
    echo "Haar Cascade model downloaded"
else
    echo "Haar Cascade model already exists"
fi

cd ..

echo ""
echo "Models downloaded successfully to ./models/"
echo "Available models:"
echo "  - YuNet ONNX model (recommended, most accurate)"
echo "  - OpenCV DNN Caffe model (good compatibility)"
echo "  - Haar Cascade XML (fallback, fastest)"
echo ""
echo "The face detector will automatically use the best available model."