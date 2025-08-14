#!/bin/bash

# Create models directory if it doesn't exist
mkdir -p models

echo "Downloading OpenCV Haar Cascade models..."

# Download the face detection models
curl -L https://raw.githubusercontent.com/opencv/opencv/master/data/haarcascades/haarcascade_frontalface_default.xml -o models/haarcascade_frontalface_default.xml
curl -L https://raw.githubusercontent.com/opencv/opencv/master/data/haarcascades/haarcascade_frontalface_alt.xml -o models/haarcascade_frontalface_alt.xml
curl -L https://raw.githubusercontent.com/opencv/opencv/master/data/haarcascades/haarcascade_frontalface_alt2.xml -o models/haarcascade_frontalface_alt2.xml

echo "Downloaded models to models/ directory"
ls -la models/*.xml