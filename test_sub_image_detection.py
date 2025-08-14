#!/usr/bin/env python3
"""
Test script to visualize what the sub-image detection algorithm sees.
This helps understand why certain images do or don't get sub-images extracted.
"""

import cv2
import numpy as np
import sys
import os

def detect_edges(image_path):
    """
    Simulate the edge detection used in the Rust sub-image extractor.
    """
    # Read the image
    img = cv2.imread(image_path)
    if img is None:
        print(f"Error: Could not read image {image_path}")
        return
    
    # Convert to grayscale
    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    
    # Apply Gaussian blur to reduce noise
    blurred = cv2.GaussianBlur(gray, (5, 5), 0)
    
    # Apply Sobel edge detection
    sobelx = cv2.Sobel(blurred, cv2.CV_64F, 1, 0, ksize=3)
    sobely = cv2.Sobel(blurred, cv2.CV_64F, 0, 1, ksize=3)
    
    # Calculate magnitude
    magnitude = np.sqrt(sobelx**2 + sobely**2)
    magnitude = np.uint8(np.clip(magnitude, 0, 255))
    
    # Threshold the edges (similar to edge_threshold in Rust code)
    edge_threshold = 30  # Same as in Rust code
    _, edges = cv2.threshold(magnitude, edge_threshold, 255, cv2.THRESH_BINARY)
    
    # Find contours (potential sub-image regions)
    contours, _ = cv2.findContours(edges, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
    
    # Filter contours by size and aspect ratio
    min_region_size = 100  # Same as in Rust code
    min_aspect_ratio = 0.3
    max_aspect_ratio = 3.0
    
    valid_regions = []
    for contour in contours:
        x, y, w, h = cv2.boundingRect(contour)
        
        # Check size
        if w >= min_region_size and h >= min_region_size:
            # Check aspect ratio
            aspect_ratio = w / h
            if min_aspect_ratio <= aspect_ratio <= max_aspect_ratio:
                # Check if region is not too large (not the whole image)
                if w < img.shape[1] * 0.9 and h < img.shape[0] * 0.9:
                    valid_regions.append((x, y, w, h))
    
    # Create visualization
    vis_img = img.copy()
    edges_colored = cv2.cvtColor(edges, cv2.COLOR_GRAY2BGR)
    
    # Draw valid regions on the original image
    for (x, y, w, h) in valid_regions:
        cv2.rectangle(vis_img, (x, y), (x+w, y+h), (0, 255, 0), 2)
        cv2.putText(vis_img, f"{w}x{h}", (x, y-5), 
                   cv2.FONT_HERSHEY_SIMPLEX, 0.5, (0, 255, 0), 1)
    
    # Create output montage
    height = img.shape[0]
    width = img.shape[1]
    
    # Resize all images to same size for montage
    montage = np.zeros((height * 2, width * 2, 3), dtype=np.uint8)
    
    # Top left: Original image
    montage[0:height, 0:width] = img
    cv2.putText(montage, "Original", (10, 30), 
               cv2.FONT_HERSHEY_SIMPLEX, 1, (255, 255, 255), 2)
    
    # Top right: Edge detection
    montage[0:height, width:width*2] = edges_colored
    cv2.putText(montage, "Edges Detected", (width+10, 30), 
               cv2.FONT_HERSHEY_SIMPLEX, 1, (255, 255, 255), 2)
    
    # Bottom left: Magnitude
    magnitude_colored = cv2.cvtColor(magnitude, cv2.COLOR_GRAY2BGR)
    montage[height:height*2, 0:width] = magnitude_colored
    cv2.putText(montage, "Edge Magnitude", (10, height+30), 
               cv2.FONT_HERSHEY_SIMPLEX, 1, (255, 255, 255), 2)
    
    # Bottom right: Detected regions
    montage[height:height*2, width:width*2] = vis_img
    cv2.putText(montage, f"Detected Regions: {len(valid_regions)}", 
               (width+10, height+30), 
               cv2.FONT_HERSHEY_SIMPLEX, 1, (255, 255, 255), 2)
    
    # Save the visualization
    output_path = image_path.replace('.', '_edge_analysis.')
    cv2.imwrite(output_path, montage)
    
    print(f"Edge detection analysis saved to: {output_path}")
    print(f"Found {len(valid_regions)} potential sub-image regions")
    
    if len(valid_regions) == 0:
        print("\nNo sub-images detected. This could be because:")
        print("1. The image is a single photograph (not a composite/collage)")
        print("2. The boundaries between sub-images are not clear enough")
        print("3. The detected regions are too small or have invalid aspect ratios")
        print("\nFor single photos with multiple faces, face detection will still work,")
        print("but sub-image extraction is meant for composite images like photo albums.")
    else:
        print("\nDetected regions:")
        for i, (x, y, w, h) in enumerate(valid_regions):
            print(f"  Region {i+1}: Position ({x}, {y}), Size {w}x{h}")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python test_sub_image_detection.py <image_path>")
        sys.exit(1)
    
    image_path = sys.argv[1]
    if not os.path.exists(image_path):
        print(f"Error: Image file not found: {image_path}")
        sys.exit(1)
    
    detect_edges(image_path)