#!/usr/bin/env python3
"""
Test OpenCV collage detection on the pexels image with 28 polaroids.
This script demonstrates the detection algorithms used in the Rust implementation.
"""

import cv2
import numpy as np
from pathlib import Path

def detect_by_edges(img):
    """Detect photos using edge detection."""
    photos = []
    
    # Convert to grayscale
    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    
    # Apply bilateral filter
    filtered = cv2.bilateralFilter(gray, 9, 75, 75)
    
    # Detect edges
    edges = cv2.Canny(filtered, 50, 150)
    
    # Dilate to connect broken edges
    kernel = cv2.getStructuringElement(cv2.MORPH_RECT, (3, 3))
    dilated = cv2.dilate(edges, kernel, iterations=1)
    
    # Find contours
    contours, _ = cv2.findContours(dilated, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
    
    img_area = img.shape[0] * img.shape[1]
    
    for contour in contours:
        area = cv2.contourArea(contour)
        
        # Filter by area
        if area < img_area * 0.005 or area > img_area * 0.5:
            continue
        
        # Approximate polygon
        epsilon = 0.02 * cv2.arcLength(contour, True)
        approx = cv2.approxPolyDP(contour, epsilon, True)
        
        # Check if roughly rectangular
        if 4 <= len(approx) <= 8:
            x, y, w, h = cv2.boundingRect(contour)
            aspect_ratio = w / h
            
            if 0.5 < aspect_ratio < 2.0:
                photos.append({
                    'bbox': (x, y, w, h),
                    'type': 'edge_detected',
                    'confidence': area / (w * h)
                })
    
    return photos

def detect_polaroids(img):
    """Detect polaroid-style photos with white borders."""
    photos = []
    
    # Convert to HSV
    hsv = cv2.cvtColor(img, cv2.COLOR_BGR2HSV)
    
    # Threshold for white borders
    lower_white = np.array([0, 0, 200])
    upper_white = np.array([180, 30, 255])
    white_mask = cv2.inRange(hsv, lower_white, upper_white)
    
    # Clean up mask
    kernel = cv2.getStructuringElement(cv2.MORPH_RECT, (5, 5))
    white_mask = cv2.morphologyEx(white_mask, cv2.MORPH_CLOSE, kernel)
    white_mask = cv2.morphologyEx(white_mask, cv2.MORPH_OPEN, kernel)
    
    # Find contours
    contours, _ = cv2.findContours(white_mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
    
    img_area = img.shape[0] * img.shape[1]
    
    for contour in contours:
        area = cv2.contourArea(contour)
        
        if area < img_area * 0.01 or area > img_area * 0.3:
            continue
        
        x, y, w, h = cv2.boundingRect(contour)
        aspect_ratio = w / h
        
        # Polaroid aspect ratio
        if 0.7 < aspect_ratio < 1.1:
            # Check for bottom margin
            if has_polaroid_margin(img, x, y, w, h):
                photos.append({
                    'bbox': (x, y, w, h),
                    'type': 'polaroid',
                    'confidence': 0.9
                })
    
    return photos

def has_polaroid_margin(img, x, y, w, h):
    """Check if region has polaroid-style bottom margin."""
    margin_height = h // 5
    margin_y = y + h - margin_height
    
    if margin_y + margin_height > img.shape[0]:
        return False
    
    roi = img[margin_y:margin_y+margin_height, x:x+w]
    gray_roi = cv2.cvtColor(roi, cv2.COLOR_BGR2GRAY)
    mean_brightness = np.mean(gray_roi)
    
    return mean_brightness > 200

def detect_by_adaptive_threshold(img):
    """Detect photos using adaptive thresholding."""
    photos = []
    
    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    
    # Apply adaptive threshold
    binary = cv2.adaptiveThreshold(gray, 255, cv2.ADAPTIVE_THRESH_MEAN_C, 
                                  cv2.THRESH_BINARY, 11, 2)
    
    # Find connected components
    num_labels, labels, stats, centroids = cv2.connectedComponentsWithStats(binary, 8, cv2.CV_32S)
    
    img_area = img.shape[0] * img.shape[1]
    
    for i in range(1, num_labels):  # Skip background
        area = stats[i, cv2.CC_STAT_AREA]
        x = stats[i, cv2.CC_STAT_LEFT]
        y = stats[i, cv2.CC_STAT_TOP]
        w = stats[i, cv2.CC_STAT_WIDTH]
        h = stats[i, cv2.CC_STAT_HEIGHT]
        
        if area < img_area // 200 or area > img_area // 3:
            continue
        
        aspect_ratio = w / h
        if 0.5 < aspect_ratio < 2.0:
            photos.append({
                'bbox': (x, y, w, h),
                'type': 'adaptive_threshold',
                'confidence': 0.7
            })
    
    return photos

def filter_overlapping(photos, iou_threshold=0.3):
    """Remove overlapping detections."""
    # Sort by confidence
    photos = sorted(photos, key=lambda x: x['confidence'], reverse=True)
    
    filtered = []
    for photo in photos:
        overlap = False
        for existing in filtered:
            if calculate_iou(photo['bbox'], existing['bbox']) > iou_threshold:
                overlap = True
                break
        
        if not overlap:
            filtered.append(photo)
    
    return filtered

def calculate_iou(box1, box2):
    """Calculate Intersection over Union."""
    x1 = max(box1[0], box2[0])
    y1 = max(box1[1], box2[1])
    x2 = min(box1[0] + box1[2], box2[0] + box2[2])
    y2 = min(box1[1] + box1[3], box2[1] + box2[3])
    
    if x2 < x1 or y2 < y1:
        return 0.0
    
    intersection = (x2 - x1) * (y2 - y1)
    area1 = box1[2] * box1[3]
    area2 = box2[2] * box2[3]
    union = area1 + area2 - intersection
    
    return intersection / union

def visualize_results(img, photos, title="Detected Photos"):
    """Visualize detection results."""
    result = img.copy()
    
    colors = {
        'polaroid': (0, 255, 0),      # Green
        'edge_detected': (255, 0, 0),  # Blue
        'adaptive_threshold': (0, 0, 255),  # Red
    }
    
    for i, photo in enumerate(photos):
        x, y, w, h = photo['bbox']
        color = colors.get(photo['type'], (255, 255, 0))
        
        # Draw rectangle
        cv2.rectangle(result, (x, y), (x+w, y+h), color, 2)
        
        # Add label
        label = f"{i+1}: {photo['type'][:3]}"
        cv2.putText(result, label, (x, y-5), cv2.FONT_HERSHEY_SIMPLEX, 
                   0.5, color, 1)
    
    # Add statistics
    stats_text = f"Total: {len(photos)} photos"
    cv2.putText(result, stats_text, (10, 30), cv2.FONT_HERSHEY_SIMPLEX, 
               1, (255, 255, 255), 2)
    
    return result

def main():
    # Test with pexels image
    image_path = "test_images/pexels-fotios-photos-3024995.jpg"
    
    if not Path(image_path).exists():
        print(f"Error: {image_path} not found")
        return
    
    print(f"Processing {image_path}...")
    img = cv2.imread(image_path)
    
    if img is None:
        print("Failed to load image")
        return
    
    print(f"Image size: {img.shape[1]}x{img.shape[0]}")
    
    # Run different detection methods
    print("\n1. Edge-based detection...")
    edge_photos = detect_by_edges(img)
    print(f"   Found {len(edge_photos)} photos")
    
    print("\n2. Polaroid detection...")
    polaroid_photos = detect_polaroids(img)
    print(f"   Found {len(polaroid_photos)} polaroids")
    
    print("\n3. Adaptive threshold detection...")
    threshold_photos = detect_by_adaptive_threshold(img)
    print(f"   Found {len(threshold_photos)} photos")
    
    # Combine all detections
    all_photos = edge_photos + polaroid_photos + threshold_photos
    print(f"\nTotal detections (before filtering): {len(all_photos)}")
    
    # Filter overlapping
    filtered_photos = filter_overlapping(all_photos)
    print(f"After filtering overlaps: {len(filtered_photos)} photos")
    
    # Sort by position (top-left to bottom-right)
    filtered_photos.sort(key=lambda p: (p['bbox'][1], p['bbox'][0]))
    
    # Print detection details
    print("\nDetected photos (sorted by position):")
    for i, photo in enumerate(filtered_photos):
        x, y, w, h = photo['bbox']
        print(f"  {i+1}. Type: {photo['type']:20s} Pos: ({x:4d},{y:4d}) "
              f"Size: {w:3d}x{h:3d} Conf: {photo['confidence']:.2f}")
    
    # Visualize results
    result_img = visualize_results(img, filtered_photos)
    
    # Save result
    output_path = "test_opencv_collage_result.jpg"
    cv2.imwrite(output_path, result_img)
    print(f"\nResult saved to {output_path}")
    
    # Also save individual crops
    output_dir = Path("detected_photos")
    output_dir.mkdir(exist_ok=True)
    
    for i, photo in enumerate(filtered_photos):
        x, y, w, h = photo['bbox']
        crop = img[y:y+h, x:x+w]
        crop_path = output_dir / f"photo_{i+1:02d}_{photo['type']}.jpg"
        cv2.imwrite(str(crop_path), crop)
    
    print(f"Individual crops saved to {output_dir}/")
    
    # Save a high-res version for viewing
    print(f"\nDetection complete! Found {len(filtered_photos)} photos out of expected 28 polaroids")

if __name__ == "__main__":
    main()