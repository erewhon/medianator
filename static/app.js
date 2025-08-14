// Global state
let currentPage = 0;
const pageSize = 20;
let currentFilter = '';
let currentSearch = '';

// Initialize the app
document.addEventListener('DOMContentLoaded', () => {
    loadStats();
    loadGallery();
    setupEventListeners();
    setupUploadArea();
});

// Load statistics
async function loadStats() {
    try {
        const [statsRes, duplicatesRes] = await Promise.all([
            fetch('/api/stats'),
            fetch('/api/duplicates/stats')
        ]);
        
        const stats = await statsRes.json();
        const duplicateStats = await duplicatesRes.json();
        
        if (stats.success) {
            document.getElementById('total-files').textContent = stats.data.total_files || 0;
            document.getElementById('image-files').textContent = stats.data.image_files || 0;
            document.getElementById('video-files').textContent = stats.data.video_files || 0;
            document.getElementById('audio-files').textContent = stats.data.audio_files || 0;
            document.getElementById('total-size').textContent = formatBytes(stats.data.total_size_bytes || 0);
        }
        
        if (duplicateStats.success) {
            document.getElementById('duplicate-groups').textContent = duplicateStats.data.duplicate_groups || 0;
        }
    } catch (error) {
        console.error('Error loading stats:', error);
    }
}

// Load media gallery
async function loadGallery() {
    const gallery = document.getElementById('gallery');
    gallery.classList.add('loading');
    
    try {
        const params = new URLSearchParams({
            limit: pageSize,
            offset: currentPage * pageSize
        });
        
        if (currentFilter) {
            params.append('media_type', currentFilter);
        }
        
        let url = '/api/media?' + params;
        if (currentSearch) {
            url = `/api/media/search?q=${encodeURIComponent(currentSearch)}`;
        }
        
        const response = await fetch(url);
        const data = await response.json();
        
        if (data.success) {
            displayMedia(data.data);
            updatePagination(data.data.length);
        }
    } catch (error) {
        console.error('Error loading gallery:', error);
        gallery.innerHTML = '<p class="error">Error loading media files</p>';
    } finally {
        gallery.classList.remove('loading');
    }
}

// Display media items
function displayMedia(items) {
    const gallery = document.getElementById('gallery');
    const isGridView = gallery.classList.contains('grid-view');
    
    if (!items || items.length === 0) {
        gallery.innerHTML = '<p style="text-align: center; color: #666;">No media files found</p>';
        return;
    }
    
    gallery.innerHTML = items.map(item => {
        const thumbnail = item.thumbnail_path ? `/api/media/${item.id}/thumbnail` : getPlaceholderIcon(item.media_type);
        
        if (isGridView) {
            return `
                <div class="media-item gallery-item" data-id="${item.id}" data-media-id="${item.id}">
                    ${item.thumbnail_path || item.media_type === 'image' ? 
                        `<img src="${item.media_type === 'image' ? `/api/media/${item.id}/image` : thumbnail}" alt="${item.file_name}" class="media-thumbnail">` :
                        `<div class="media-thumbnail" style="display: flex; align-items: center; justify-content: center; background: #f8f9fa; font-size: 3em;">${getMediaEmoji(item.media_type)}</div>`
                    }
                    <div class="media-overlay">
                        <div>${item.file_name}</div>
                    </div>
                </div>
            `;
        } else {
            return `
                <div class="media-item gallery-item" data-id="${item.id}" data-media-id="${item.id}">
                    ${item.thumbnail_path || item.media_type === 'image' ? 
                        `<img src="${item.media_type === 'image' ? `/api/media/${item.id}/image` : thumbnail}" alt="${item.file_name}" class="media-thumbnail">` :
                        `<div class="media-thumbnail" style="display: flex; align-items: center; justify-content: center; background: #f8f9fa;">${getMediaEmoji(item.media_type)}</div>`
                    }
                    <div class="media-details">
                        <div class="media-name">${item.file_name}</div>
                        <div class="media-meta">
                            ${formatBytes(item.file_size)} • ${item.media_type}
                            ${item.width && item.height ? ` • ${item.width}×${item.height}` : ''}
                        </div>
                    </div>
                    <span class="media-type-badge ${item.media_type}">${item.media_type}</span>
                </div>
            `;
        }
    }).join('');
    
    // Add click handlers
    document.querySelectorAll('.media-item').forEach(item => {
        item.addEventListener('click', () => showMediaDetail(item.dataset.id));
    });
}

// Show media detail modal
async function showMediaDetail(mediaId) {
    try {
        const [mediaRes, facesRes] = await Promise.all([
            fetch(`/api/media/${mediaId}`),
            fetch(`/api/media/${mediaId}/faces`)
        ]);
        
        const mediaData = await mediaRes.json();
        const facesData = await facesRes.json();
        
        if (mediaData.success) {
            const media = mediaData.data;
            const modal = document.getElementById('media-modal');
            modal.dataset.currentMediaId = mediaId;
            
            // Set basic info
            document.getElementById('media-filename').textContent = media.file_name;
            document.getElementById('media-path').textContent = media.file_path;
            document.getElementById('media-size').textContent = formatBytes(media.file_size);
            document.getElementById('media-type').textContent = media.media_type;
            document.getElementById('media-hash').textContent = media.file_hash;
            document.getElementById('media-created').textContent = formatDate(media.file_created_at);
            
            // Set dimensions
            if (media.width && media.height) {
                document.getElementById('media-dimensions').textContent = `${media.width} × ${media.height}`;
            } else {
                document.getElementById('media-dimensions').textContent = 'N/A';
            }
            
            // Set preview
            const imgPreview = document.getElementById('media-preview');
            const videoPreview = document.getElementById('video-preview');
            const audioPreview = document.getElementById('audio-preview');
            
            // Hide all previews first
            imgPreview.style.display = 'none';
            videoPreview.style.display = 'none';
            audioPreview.style.display = 'none';
            
            if (media.media_type === 'image') {
                imgPreview.src = `/api/media/${mediaId}/image`;
                imgPreview.style.display = 'block';
            } else if (media.media_type === 'video') {
                // Use video streaming endpoint
                videoPreview.src = `/api/media/${mediaId}/video`;
                videoPreview.style.display = 'block';
            } else if (media.media_type === 'audio') {
                audioPreview.src = `/api/media/${mediaId}/audio`;
                audioPreview.style.display = 'block';
            }
            
            // Set camera info if available
            const cameraInfo = document.getElementById('camera-info');
            if (media.camera_make || media.camera_model) {
                cameraInfo.classList.remove('hidden');
                document.getElementById('camera-make').textContent = media.camera_make || 'N/A';
                document.getElementById('camera-model').textContent = media.camera_model || 'N/A';
                document.getElementById('camera-iso').textContent = media.iso || 'N/A';
                document.getElementById('camera-aperture').textContent = media.aperture ? `f/${media.aperture}` : 'N/A';
            } else {
                cameraInfo.classList.add('hidden');
            }
            
            // Set faces if available with toggle button
            const facesSection = document.getElementById('detected-faces');
            if (facesData.success && facesData.data.length > 0) {
                facesSection.classList.remove('hidden');
                const facesCount = facesData.data.length;
                document.getElementById('faces-list').innerHTML = `
                    <p>${facesCount} face(s) detected 
                        <button id="toggle-faces-btn" class="toggle-faces-btn">👁 Show Faces</button>
                    </p>
                `;
                
                // Draw face boxes on image if it's an image
                if (media.media_type === 'image') {
                    const img = document.getElementById('media-preview');
                    if (img) {
                        // Store faces data for later use
                        img.facesData = facesData.data;
                        
                        img.onload = () => {
                            // Wait for layout to settle
                            setTimeout(() => {
                                drawFaceBoxes(facesData.data, img);
                            }, 100);
                        };
                        // If image already loaded
                        if (img.complete && img.naturalHeight !== 0) {
                            setTimeout(() => {
                                drawFaceBoxes(facesData.data, img);
                            }, 100);
                        }
                        
                        // Redraw on window resize
                        window.addEventListener('resize', () => {
                            if (img.facesData) {
                                drawFaceBoxes(img.facesData, img);
                            }
                        });
                    }
                }
                
                // Add toggle functionality
                const toggleBtn = document.getElementById('toggle-faces-btn');
                if (toggleBtn) {
                    toggleBtn.addEventListener('click', () => {
                        const faceBoxes = document.querySelector('.face-boxes-overlay');
                        if (faceBoxes) {
                            if (faceBoxes.style.display === 'none') {
                                faceBoxes.style.display = 'block';
                                toggleBtn.textContent = '👁 Hide Faces';
                            } else {
                                faceBoxes.style.display = 'none';
                                toggleBtn.textContent = '👁 Show Faces';
                            }
                        }
                    });
                }
            } else {
                facesSection.classList.add('hidden');
            }
            
            modal.classList.remove('hidden');
        }
    } catch (error) {
        console.error('Error loading media detail:', error);
    }
}

// Setup event listeners
function setupEventListeners() {
    // Search
    document.getElementById('search-btn').addEventListener('click', performSearch);
    document.getElementById('search-input').addEventListener('keypress', (e) => {
        if (e.key === 'Enter') performSearch();
    });
    
    // Filter
    document.getElementById('filter-type').addEventListener('change', (e) => {
        currentFilter = e.target.value;
        currentPage = 0;
        loadGallery();
    });
    
    // View toggle
    document.querySelectorAll('.view-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            document.querySelectorAll('.view-btn').forEach(b => b.classList.remove('active'));
            e.target.classList.add('active');
            
            const gallery = document.getElementById('gallery');
            gallery.classList.remove('grid-view', 'list-view');
            gallery.classList.add(e.target.dataset.view + '-view');
            loadGallery();
        });
    });
    
    // Pagination
    document.getElementById('prev-page').addEventListener('click', () => {
        if (currentPage > 0) {
            currentPage--;
            loadGallery();
        }
    });
    
    document.getElementById('next-page').addEventListener('click', () => {
        currentPage++;
        loadGallery();
    });
    
    // Scan button
    document.getElementById('scan-btn').addEventListener('click', () => {
        document.getElementById('scan-modal').classList.remove('hidden');
    });
    
    document.getElementById('start-scan').addEventListener('click', startScan);
    
    // Duplicates button
    document.getElementById('duplicates-btn').addEventListener('click', showDuplicates);
    
    // Modal close buttons
    document.querySelectorAll('.close').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.target.closest('.modal').classList.add('hidden');
        });
    });
    
    // Close modal on outside click
    document.querySelectorAll('.modal').forEach(modal => {
        modal.addEventListener('click', (e) => {
            if (e.target === modal) {
                modal.classList.add('hidden');
            }
        });
    });
}

// Setup upload area
function setupUploadArea() {
    const uploadArea = document.getElementById('upload-area');
    const fileInput = document.getElementById('file-input');
    
    uploadArea.addEventListener('click', () => fileInput.click());
    
    uploadArea.addEventListener('dragover', (e) => {
        e.preventDefault();
        uploadArea.classList.add('dragover');
    });
    
    uploadArea.addEventListener('dragleave', () => {
        uploadArea.classList.remove('dragover');
    });
    
    uploadArea.addEventListener('drop', (e) => {
        e.preventDefault();
        uploadArea.classList.remove('dragover');
        handleFiles(e.dataTransfer.files);
    });
    
    fileInput.addEventListener('change', (e) => {
        handleFiles(e.target.files);
    });
}

// Handle file upload
async function handleFiles(files) {
    const progressDiv = document.getElementById('upload-progress');
    const progressFill = document.getElementById('progress-fill');
    const statusText = document.getElementById('upload-status');
    
    progressDiv.classList.remove('hidden');
    
    let uploaded = 0;
    const total = files.length;
    
    for (const file of files) {
        statusText.textContent = `Uploading ${file.name}...`;
        
        const formData = new FormData();
        formData.append('file', file);
        
        try {
            const response = await fetch('/api/upload', {
                method: 'POST',
                body: formData
            });
            
            if (response.ok) {
                uploaded++;
            }
        } catch (error) {
            console.error('Upload error:', error);
        }
        
        progressFill.style.width = `${(uploaded / total) * 100}%`;
    }
    
    statusText.textContent = `Uploaded ${uploaded} of ${total} files`;
    
    setTimeout(() => {
        progressDiv.classList.add('hidden');
        loadStats();
        loadGallery();
    }, 2000);
}

// Perform search
function performSearch() {
    currentSearch = document.getElementById('search-input').value;
    currentPage = 0;
    loadGallery();
}

// Start scan
async function startScan() {
    const path = document.getElementById('scan-path').value;
    if (!path) return;
    
    const resultsDiv = document.getElementById('scan-results');
    const statusText = document.getElementById('scan-status');
    const progressFill = document.getElementById('scan-progress');
    
    resultsDiv.classList.remove('hidden');
    statusText.textContent = 'Starting scan...';
    
    try {
        const response = await fetch('/api/scan', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ path })
        });
        
        const data = await response.json();
        
        if (data.success) {
            statusText.textContent = `Scan complete! Scanned: ${data.data.files_scanned}, Added: ${data.data.files_added}, Updated: ${data.data.files_updated}`;
            progressFill.style.width = '100%';
            
            setTimeout(() => {
                document.getElementById('scan-modal').classList.add('hidden');
                loadStats();
                loadGallery();
            }, 2000);
        } else {
            statusText.textContent = `Error: ${data.error}`;
            statusText.classList.add('error');
        }
    } catch (error) {
        statusText.textContent = `Error: ${error.message}`;
        statusText.classList.add('error');
    }
}

// Show duplicates
async function showDuplicates() {
    try {
        const response = await fetch('/api/duplicates');
        const data = await response.json();
        
        if (data.success) {
            const modal = document.getElementById('duplicates-modal');
            const statsDiv = document.getElementById('duplicate-stats');
            const groupsDiv = document.getElementById('duplicate-groups');
            
            // Calculate stats
            const totalGroups = data.data.length;
            const totalWasted = data.data.reduce((sum, group) => {
                return sum + (group.total_size * (group.count - 1) / group.count);
            }, 0);
            
            statsDiv.innerHTML = `
                <p><strong>${totalGroups}</strong> duplicate groups found</p>
                <p><strong>${formatBytes(totalWasted)}</strong> of wasted space</p>
            `;
            
            // Display groups
            groupsDiv.innerHTML = data.data.slice(0, 10).map(group => `
                <div class="duplicate-group">
                    <h4>${group.count} copies (${formatBytes(group.total_size)} total)</h4>
                    <div class="duplicate-files">
                        ${group.files.map(file => `
                            <div class="duplicate-file">
                                <span class="duplicate-file-path">${file.path}</span>
                                <span class="duplicate-file-size">${formatBytes(file.size)}</span>
                            </div>
                        `).join('')}
                    </div>
                </div>
            `).join('');
            
            modal.classList.remove('hidden');
        }
    } catch (error) {
        console.error('Error loading duplicates:', error);
    }
}

// Update pagination
function updatePagination(itemCount) {
    const prevBtn = document.getElementById('prev-page');
    const nextBtn = document.getElementById('next-page');
    const pageInfo = document.getElementById('page-info');
    
    prevBtn.disabled = currentPage === 0;
    nextBtn.disabled = itemCount < pageSize;
    pageInfo.textContent = `Page ${currentPage + 1}`;
}

// Utility functions
function formatBytes(bytes) {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
}

function formatDate(dateString) {
    if (!dateString) return 'N/A';
    const date = new Date(dateString);
    return date.toLocaleDateString() + ' ' + date.toLocaleTimeString();
}

function getMediaEmoji(type) {
    switch(type) {
        case 'image': return '🖼️';
        case 'video': return '🎥';
        case 'audio': return '🎵';
        default: return '📄';
    }
}

function getPlaceholderIcon(type) {
    return `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='100' height='100'%3E%3Crect width='100' height='100' fill='%23f8f9fa'/%3E%3Ctext x='50' y='50' font-size='40' text-anchor='middle' dominant-baseline='middle'%3E${getMediaEmoji(type)}%3C/text%3E%3C/svg%3E`;
}

// Selection mode for batch processing
let selectionMode = false;
let selectedMedia = new Set();

// Initialize batch reprocess functionality
function initBatchReprocess() {
    const batchBtn = document.getElementById('batch-reprocess-btn');
    const modal = document.getElementById('batch-reprocess-modal');
    const closeBtn = document.getElementById('close-batch-modal');
    const cancelBtn = document.getElementById('cancel-batch-reprocess');
    const startBtn = document.getElementById('start-batch-reprocess');
    
    if (batchBtn) {
        batchBtn.addEventListener('click', () => {
            if (!selectionMode) {
                enterSelectionMode();
            } else {
                showBatchReprocessModal();
            }
        });
    }
    
    if (closeBtn) {
        closeBtn.addEventListener('click', () => {
            modal.classList.add('hidden');
        });
    }
    
    if (cancelBtn) {
        cancelBtn.addEventListener('click', () => {
            modal.classList.add('hidden');
            exitSelectionMode();
        });
    }
    
    if (startBtn) {
        startBtn.addEventListener('click', async () => {
            await startBatchReprocess();
            modal.classList.add('hidden');
            exitSelectionMode();
        });
    }
}

function enterSelectionMode() {
    selectionMode = true;
    selectedMedia.clear();
    
    const gallery = document.getElementById('gallery');
    const items = gallery.querySelectorAll('.gallery-item');
    
    items.forEach(item => {
        item.classList.add('selectable');
        item.addEventListener('click', toggleSelection);
    });
    
    const batchBtn = document.getElementById('batch-reprocess-btn');
    if (batchBtn) {
        batchBtn.textContent = '✅ Confirm Selection';
    }
}

function exitSelectionMode() {
    selectionMode = false;
    
    const gallery = document.getElementById('gallery');
    const items = gallery.querySelectorAll('.gallery-item');
    
    items.forEach(item => {
        item.classList.remove('selectable', 'selected');
        item.removeEventListener('click', toggleSelection);
    });
    
    selectedMedia.clear();
    
    const batchBtn = document.getElementById('batch-reprocess-btn');
    if (batchBtn) {
        batchBtn.textContent = '♻️ Batch Reprocess';
    }
}

function toggleSelection(e) {
    e.stopPropagation();
    const item = e.currentTarget;
    const mediaId = item.dataset.mediaId;
    
    if (selectedMedia.has(mediaId)) {
        selectedMedia.delete(mediaId);
        item.classList.remove('selected');
    } else {
        selectedMedia.add(mediaId);
        item.classList.add('selected');
    }
}

function showBatchReprocessModal() {
    if (selectedMedia.size === 0) {
        alert('Please select at least one media file');
        return;
    }
    
    const modal = document.getElementById('batch-reprocess-modal');
    const countSpan = document.getElementById('selected-media-count');
    
    if (countSpan) {
        countSpan.textContent = selectedMedia.size;
    }
    
    modal.classList.remove('hidden');
}

async function startBatchReprocess() {
    const reprocessFaces = document.getElementById('reprocess-faces').checked;
    const reprocessThumbnails = document.getElementById('reprocess-thumbnails').checked;
    const reprocessMetadata = document.getElementById('reprocess-metadata').checked;
    
    const mediaIds = Array.from(selectedMedia);
    
    try {
        const response = await fetch('/api/batch/reprocess', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                media_ids: mediaIds,
                reprocess_faces: reprocessFaces,
                reprocess_thumbnails: reprocessThumbnails,
                reprocess_metadata: reprocessMetadata
            })
        });
        
        const result = await response.json();
        
        if (result.success) {
            alert(result.data || 'Batch reprocessing started');
            // Reload gallery after a delay to show updated data
            setTimeout(() => loadGallery(), 2000);
        } else {
            alert('Error: ' + (result.error || 'Failed to start batch reprocessing'));
        }
    } catch (error) {
        console.error('Error starting batch reprocess:', error);
        alert('Failed to start batch reprocessing');
    }
}

// Single file reprocess
async function reprocessSingleFile(mediaId) {
    try {
        const response = await fetch(`/api/media/${mediaId}/reprocess`, {
            method: 'POST'
        });
        
        const result = await response.json();
        
        if (result.success) {
            alert(result.data || 'Reprocessing started');
            // Reload the modal data after a delay
            setTimeout(() => {
                const modal = document.getElementById('media-modal');
                if (!modal.classList.contains('hidden')) {
                    // Refresh the modal content
                    const event = new CustomEvent('refresh-media', { detail: { mediaId } });
                    document.dispatchEvent(event);
                }
            }, 2000);
        } else {
            alert('Error: ' + (result.error || 'Failed to start reprocessing'));
        }
    } catch (error) {
        console.error('Error reprocessing file:', error);
        alert('Failed to start reprocessing');
    }
}

// Face groups viewing
async function loadFaceGroups() {
    try {
        const response = await fetch('/api/faces/grouped');
        const result = await response.json();
        
        if (result.success && result.data) {
            displayFaceGroups(result.data);
        }
    } catch (error) {
        console.error('Error loading face groups:', error);
    }
}

function displayFaceGroups(groups) {
    const container = document.getElementById('face-groups-container');
    
    if (!container) return;
    
    if (groups.length === 0) {
        container.innerHTML = '<p>No face groups found. Process some images with face detection enabled.</p>';
        return;
    }
    
    container.innerHTML = groups.map(group => `
        <div class="face-group">
            <h3>${group.group_name || `Group ${group.group_id}`} (${group.face_count} faces)</h3>
            <div class="face-group-faces">
                ${group.faces.map(face => `
                    <div class="face-item" data-media-id="${face.media_file_id}" onclick="loadMediaDetail('${face.media_file_id}')">
                        <div class="face-thumbnail-container">
                            <img src="/api/faces/${face.face_id}/thumbnail" 
                                 alt="Face from ${face.file_name}"
                                 class="face-thumbnail"
                                 onerror="this.src='/api/media/${face.media_file_id}/thumbnail'">
                        </div>
                        <div class="face-info">
                            <span class="face-filename">${face.file_name}</span>
                            <span class="face-confidence">${Math.round(face.confidence * 100)}%</span>
                        </div>
                    </div>
                `).join('')}
            </div>
        </div>
    `).join('');
    
    // Add click handlers to face items
    container.querySelectorAll('.face-item').forEach(item => {
        item.addEventListener('click', () => {
            const mediaId = item.dataset.mediaId;
            if (mediaId) {
                // Load the media detail modal
                loadMediaDetail(mediaId);
            }
        });
    });
}

// Initialize face viewing
function initFaceViewing() {
    const viewFacesBtn = document.getElementById('view-faces-btn');
    const modal = document.getElementById('faces-modal');
    const closeBtn = document.getElementById('close-faces-modal');
    
    if (viewFacesBtn) {
        viewFacesBtn.addEventListener('click', async () => {
            await loadFaceGroups();
            modal.classList.remove('hidden');
        });
    }
    
    if (closeBtn) {
        closeBtn.addEventListener('click', () => {
            modal.classList.add('hidden');
        });
    }
}

// Load media detail for face viewing
async function loadMediaDetail(mediaId) {
    try {
        const response = await fetch(`/api/media/${mediaId}`);
        const result = await response.json();
        
        if (result.success && result.data) {
            showMediaDetail(result.data.id);
        }
    } catch (error) {
        console.error('Error loading media detail:', error);
    }
}

// Draw face bounding boxes on image
function drawFaceBoxes(faces, img) {
    // Remove existing overlay if any
    const existingOverlay = img.parentElement?.querySelector('.face-boxes-overlay');
    if (existingOverlay) {
        existingOverlay.remove();
    }
    
    // Get the image's position relative to its parent
    const imgRect = img.getBoundingClientRect();
    const parentRect = img.parentElement.getBoundingClientRect();
    
    // Create overlay container
    const overlay = document.createElement('div');
    overlay.className = 'face-boxes-overlay';
    overlay.style.position = 'absolute';
    overlay.style.top = (imgRect.top - parentRect.top) + 'px';
    overlay.style.left = (imgRect.left - parentRect.left) + 'px';
    overlay.style.width = img.width + 'px';
    overlay.style.height = img.height + 'px';
    overlay.style.pointerEvents = 'none';
    overlay.style.display = 'none'; // Initially hidden
    
    // Calculate scale factors based on displayed size vs natural size
    const scaleX = img.width / img.naturalWidth;
    const scaleY = img.height / img.naturalHeight;
    
    // Draw boxes for each face
    faces.forEach((face, index) => {
        const bbox = face.face_bbox.split(',').map(Number);
        if (bbox.length === 4) {
            const [x, y, width, height] = bbox;
            
            const box = document.createElement('div');
            box.className = 'face-box';
            box.style.position = 'absolute';
            box.style.left = (x * scaleX) + 'px';
            box.style.top = (y * scaleY) + 'px';
            box.style.width = (width * scaleX) + 'px';
            box.style.height = (height * scaleY) + 'px';
            box.style.border = '2px solid #00ff00';
            box.style.borderRadius = '4px';
            box.style.boxShadow = '0 0 4px rgba(0,255,0,0.5)';
            
            // Add label
            const label = document.createElement('div');
            label.className = 'face-label';
            label.style.position = 'absolute';
            label.style.top = '-20px';
            label.style.left = '0';
            label.style.background = '#00ff00';
            label.style.color = '#000';
            label.style.padding = '2px 6px';
            label.style.borderRadius = '3px';
            label.style.fontSize = '12px';
            label.style.fontWeight = 'bold';
            label.textContent = `Face ${index + 1} (${Math.round(face.confidence * 100)}%)`;
            
            box.appendChild(label);
            overlay.appendChild(box);
        }
    });
    
    // Add overlay to image container
    if (img.parentElement) {
        img.parentElement.style.position = 'relative';
        img.parentElement.appendChild(overlay);
    }
}

// Update the DOMContentLoaded event listener
document.addEventListener('DOMContentLoaded', () => {
    loadStats();
    loadGallery();
    setupEventListeners();
    setupUploadArea();
    initBatchReprocess();
    initFaceViewing();
    
    // Add reprocess button handler for single files
    const reprocessBtn = document.getElementById('reprocess-single-btn');
    if (reprocessBtn) {
        reprocessBtn.addEventListener('click', () => {
            const modal = document.getElementById('media-modal');
            const mediaId = modal.dataset.currentMediaId;
            if (mediaId) {
                reprocessSingleFile(mediaId);
            }
        });
    }
    
    // Listen for media refresh events
    document.addEventListener('refresh-media', async (e) => {
        const mediaId = e.detail.mediaId;
        if (mediaId) {
            await loadMediaDetail(mediaId);
        }
    });
});