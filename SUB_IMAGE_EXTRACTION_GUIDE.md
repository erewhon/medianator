# Sub-Image Extraction Guide

## Overview
The sub-image extraction feature in Medianator is designed to automatically detect and extract individual photos from composite images, such as:
- Scanned photo album pages
- Photo collages with multiple distinct images
- Contact sheets
- Photo montages with clear boundaries between images

## What It Does
- Detects rectangular regions with clear boundaries (using edge detection)
- Extracts each detected region as a separate image file
- Maintains parent-child relationships in the database
- Copies metadata from parent to extracted sub-images
- Runs face detection on each extracted sub-image

## What It Doesn't Do
- **Does NOT** extract individual faces from a single photograph
- **Does NOT** split up continuous photographs with multiple people
- **Does NOT** work on images without clear boundaries between sub-images

## How It Works

### Detection Algorithm
1. Applies edge detection (Sobel filters) to find boundaries
2. Identifies rectangular regions with strong edges
3. Filters regions by:
   - Minimum size (100x100 pixels)
   - Aspect ratio (0.3 to 3.0)
   - Must not be the entire image

### When Sub-Images Are Extracted
- During initial media scanning (if `SUB_IMAGES_DIR` is set)
- During image reprocessing

## Configuration

Set the environment variable to enable:
```bash
export SUB_IMAGES_DIR="sub_images"
```

## Examples

### Works Well With:
✅ Scanned photo album pages with multiple photos
✅ Photo collages with distinct borders
✅ Contact sheets from photo shoots
✅ Montages with clear separations

### Doesn't Work With:
❌ Single photographs with multiple people
❌ Continuous panoramic images
❌ Images without clear boundaries
❌ Regular photos (use face detection instead)

## Face Detection vs Sub-Image Extraction

| Feature | Face Detection | Sub-Image Extraction |
|---------|---------------|---------------------|
| Purpose | Find faces in any image | Extract separate photos from composites |
| Works on | Any photograph | Composite images only |
| Output | Face coordinates | New image files |
| Use case | Identifying people | Processing scanned albums |

## Testing Sub-Image Detection

Use the provided test script to visualize what the algorithm sees:
```bash
python3 test_sub_image_detection.py /path/to/image.jpg
```

This will create an analysis image showing:
- Original image
- Edge detection result
- Edge magnitude
- Detected regions (if any)

## Troubleshooting

### No sub-images detected?
1. Check if the image is actually a composite (multiple distinct photos)
2. Verify clear boundaries exist between images
3. Ensure individual images are at least 100x100 pixels
4. Try adjusting scanner parameters if needed

### Too many false detections?
- Usually caused by textured backgrounds or patterns
- The algorithm may detect noise as boundaries
- Consider manual extraction for problematic images

## Important Notes

- Sub-image extraction is **complementary** to face detection
- Both features can work together:
  - Sub-images are extracted first
  - Face detection runs on both parent and extracted sub-images
- For single photos with multiple faces, only face detection is needed
- For photo albums, both features are useful