#!/usr/bin/env python3
"""
Extracts individual polaroid photos from a grid layout using contour detection.
"""

import cv2
import numpy as np
import json
import sys

def extract_polaroids(image_path):
    # Read the image
    img = cv2.imread(image_path)
    if img is None:
        return []
    
    # Convert to grayscale
    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    
    # Apply bilateral filter to reduce noise while keeping edges sharp
    filtered = cv2.bilateralFilter(gray, 9, 75, 75)
    
    # Apply adaptive thresholding to get binary image
    thresh = cv2.adaptiveThreshold(filtered, 255, cv2.ADAPTIVE_THRESH_GAUSSIAN_C, 
                                   cv2.THRESH_BINARY, 11, 2)
    
    # Apply morphological operations to clean up the image
    kernel = np.ones((5,5), np.uint8)
    morph = cv2.morphologyEx(thresh, cv2.MORPH_CLOSE, kernel)
    morph = cv2.morphologyEx(morph, cv2.MORPH_OPEN, kernel)
    
    # Find contours
    contours, _ = cv2.findContours(morph, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
    
    # Filter contours to find rectangular regions (polaroids)
    rectangles = []
    img_area = img.shape[0] * img.shape[1]
    
    for contour in contours:
        # Get bounding rectangle
        x, y, w, h = cv2.boundingRect(contour)
        
        # Calculate area and aspect ratio
        area = w * h
        aspect_ratio = w / h if h > 0 else 0
        
        # Filter based on size and aspect ratio
        # Polaroids are typically squarish to slightly rectangular
        if (area > img_area * 0.005 and  # At least 0.5% of image
            area < img_area * 0.5 and      # At most 50% of image
            0.5 < aspect_ratio < 2.0 and   # Reasonable aspect ratio
            w > 100 and h > 100):          # Minimum size
            
            # Check if the contour is approximately rectangular
            epsilon = 0.02 * cv2.arcLength(contour, True)
            approx = cv2.approxPolyDP(contour, epsilon, True)
            
            # A rectangle should have 4 vertices
            if len(approx) >= 4 and len(approx) <= 8:
                rectangles.append({
                    'x': int(x),
                    'y': int(y),
                    'width': int(w),
                    'height': int(h),
                    'confidence': float(len(approx) == 4)  # Higher confidence for perfect rectangles
                })
    
    # Sort by position (top to bottom, left to right)
    rectangles.sort(key=lambda r: (r['y'] // 100, r['x']))
    
    return rectangles

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(json.dumps([]))
        sys.exit(0)
    
    image_path = sys.argv[1]
    rectangles = extract_polaroids(image_path)
    print(json.dumps(rectangles))