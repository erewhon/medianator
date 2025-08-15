// Global state
let currentPage = 1;
const pageSize = 20;
let currentFilter = '';
let currentSearch = '';
let totalPages = 1;
let websocket = null;
let reconnectInterval = null;

// Initialize the app
document.addEventListener('DOMContentLoaded', () => {
    loadStats();
    loadGallery();
    setupEventListeners();
    setupUploadArea();
    connectWebSocket();
});

// WebSocket connection
function connectWebSocket() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws`;
    
    console.log('Connecting to WebSocket:', wsUrl);
    websocket = new WebSocket(wsUrl);
    
    websocket.onopen = () => {
        console.log('WebSocket connected');
        clearInterval(reconnectInterval);
        
        // Subscribe to events
        websocket.send(JSON.stringify({
            type: 'subscribe',
            events: ['transcription', 'scan', 'media_update', 'face_detection']
        }));
    };
    
    websocket.onmessage = (event) => {
        try {
            const message = JSON.parse(event.data);
            handleWebSocketMessage(message);
        } catch (error) {
            console.error('Error parsing WebSocket message:', error);
        }
    };
    
    websocket.onerror = (error) => {
        console.error('WebSocket error:', error);
    };
    
    websocket.onclose = () => {
        console.log('WebSocket disconnected');
        websocket = null;
        
        // Attempt to reconnect after 5 seconds
        if (!reconnectInterval) {
            reconnectInterval = setInterval(() => {
                console.log('Attempting to reconnect WebSocket...');
                connectWebSocket();
            }, 5000);
        }
    };
}

// Handle WebSocket messages
function handleWebSocketMessage(message) {
    console.log('WebSocket message:', message);
    
    switch (message.type) {
        case 'connected':
            console.log('Connected with client ID:', message.client_id);
            showNotification('Connected to real-time updates', 'success');
            break;
            
        case 'transcription_progress':
            handleTranscriptionProgress(message);
            break;
            
        case 'transcription_segment':
            handleTranscriptionSegment(message);
            break;
            
        case 'scan_progress':
            handleScanProgress(message);
            break;
            
        case 'media_updated':
            handleMediaUpdate(message);
            break;
            
        case 'face_detection_progress':
            handleFaceDetectionProgress(message);
            break;
            
        case 'error':
            showNotification(`Error: ${message.message}`, 'error');
            break;
    }
}

// Handle transcription progress updates
function handleTranscriptionProgress(message) {
    const { media_id, status, progress, message: msg } = message;
    
    // Update UI if we're viewing this media
    const panel = document.getElementById('media-panel');
    if (panel && panel.dataset.currentMediaId === media_id) {
        const transcriptionSection = document.getElementById('transcription-section');
        if (transcriptionSection) {
            // Update or create progress indicator
            let progressEl = transcriptionSection.querySelector('.transcription-progress');
            if (!progressEl) {
                progressEl = document.createElement('div');
                progressEl.className = 'transcription-progress';
                transcriptionSection.insertBefore(progressEl, transcriptionSection.firstChild);
            }
            
            // Check if it's an error about Whisper not being installed
            if (status === 'error' && msg && msg.includes('Whisper is not installed')) {
                progressEl.innerHTML = `
                    <div style="background: #fff3cd; border: 1px solid #ffc107; border-radius: 8px; padding: 15px; margin: 10px 0;">
                        <h4 style="color: #856404; margin: 0 0 10px 0;">⚠️ Whisper Not Installed</h4>
                        <p style="color: #856404; margin: 0 0 10px 0;">The transcription feature requires OpenAI Whisper to be installed.</p>
                        <p style="color: #856404; margin: 0 0 10px 0;">To install Whisper, run:</p>
                        <code style="background: #f8f9fa; padding: 5px 10px; border-radius: 4px; display: block; margin: 10px 0;">
                            pip install openai-whisper
                        </code>
                        <p style="color: #856404; margin: 10px 0 0 0; font-size: 12px;">
                            Or run the included script: <code>./install_whisper.sh</code>
                        </p>
                    </div>
                `;
            } else {
                progressEl.innerHTML = `
                    <div class="progress-bar" style="margin: 10px 0;">
                        <div class="progress-fill" style="width: ${progress}%; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);"></div>
                    </div>
                    <p style="font-size: 14px; color: ${status === 'error' ? '#dc3545' : '#666'};">${msg || `Transcription ${status}...`}</p>
                `;
            }
            
            if (status === 'complete') {
                setTimeout(() => {
                    progressEl.remove();
                    // Reload transcription data
                    loadTranscriptionForMedia(media_id);
                }, 2000);
            } else if (status === 'error' && !msg.includes('Whisper is not installed')) {
                // Remove error message after 5 seconds for other errors
                setTimeout(() => {
                    progressEl.remove();
                }, 5000);
            }
        }
    }
}

// Handle transcription segment updates (for streaming)
function handleTranscriptionSegment(message) {
    const { media_id, segment } = message;
    
    // Update UI if we're viewing this media
    const panel = document.getElementById('media-panel');
    if (panel && panel.dataset.currentMediaId === media_id) {
        const transcriptionContent = document.getElementById('transcription-content');
        if (transcriptionContent) {
            // Append segment to transcription display
            const segmentEl = document.createElement('div');
            segmentEl.className = 'transcription-segment';
            segmentEl.innerHTML = `
                <span class="segment-time">[${formatTime(segment.start_time)} - ${formatTime(segment.end_time)}]</span>
                <span class="segment-text">${segment.text}</span>
            `;
            transcriptionContent.appendChild(segmentEl);
        }
    }
}

// Handle scan progress updates
function handleScanProgress(message) {
    const { path, files_scanned, files_added, files_updated } = message;
    showNotification(`Scanning: ${files_scanned} files (${files_added} new, ${files_updated} updated)`, 'info');
}

// Handle media update notifications
function handleMediaUpdate(message) {
    const { media_id, update_type } = message;
    
    // Refresh gallery if visible
    if (document.getElementById('gallery').children.length > 0) {
        // Could selectively update just the affected item
        console.log(`Media ${media_id} updated: ${update_type}`);
    }
}

// Handle face detection progress
function handleFaceDetectionProgress(message) {
    const { media_id, faces_detected } = message;
    console.log(`Face detection for ${media_id}: ${faces_detected} faces found`);
}

// Show notification
function showNotification(message, type = 'info') {
    // Create notification element
    const notification = document.createElement('div');
    notification.className = `notification notification-${type}`;
    notification.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        padding: 15px 20px;
        background: ${type === 'success' ? '#4CAF50' : type === 'error' ? '#f44336' : '#2196F3'};
        color: white;
        border-radius: 8px;
        box-shadow: 0 2px 10px rgba(0,0,0,0.2);
        z-index: 10000;
        animation: slideIn 0.3s ease;
        max-width: 300px;
    `;
    notification.textContent = message;
    
    document.body.appendChild(notification);
    
    // Remove after 5 seconds
    setTimeout(() => {
        notification.style.animation = 'slideOut 0.3s ease';
        setTimeout(() => notification.remove(), 300);
    }, 5000);
}

// Format time in seconds to mm:ss
function formatTime(seconds) {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
}

// Load transcription for a specific media
async function loadTranscriptionForMedia(mediaId) {
    try {
        const response = await fetch(`/api/transcriptions/media/${mediaId}`);
        if (response.ok) {
            const data = await response.json();
            if (data.success && data.data) {
                displayTranscription(data.data);
            }
        }
    } catch (error) {
        console.error('Error loading transcription:', error);
    }
}

// Display transcription in the UI
function displayTranscription(transcriptionData) {
    const transcriptionSection = document.getElementById('transcription-section');
    if (!transcriptionSection) return;
    
    const { transcription, segments } = transcriptionData;
    
    // Update transcription display
    let transcriptionHTML = `
        <div class="transcription-result">
            <div class="transcription-header">
                <h4>Transcription</h4>
                <button class="small-btn danger" onclick="deleteTranscription('${transcription.id}')">Delete</button>
            </div>
            <div class="transcription-text">${transcription.transcription_text}</div>
    `;
    
    if (segments && segments.length > 0) {
        transcriptionHTML += `
            <details style="margin-top: 15px;">
                <summary style="cursor: pointer; font-weight: bold;">View Segments (${segments.length})</summary>
                <div class="transcription-segments" id="transcription-content">
        `;
        
        segments.forEach(segment => {
            transcriptionHTML += `
                <div class="transcription-segment">
                    <span class="segment-time">[${formatTime(segment.start_time)} - ${formatTime(segment.end_time)}]</span>
                    <span class="segment-text">${segment.text}</span>
                </div>
            `;
        });
        
        transcriptionHTML += `
                </div>
            </details>
        `;
    }
    
    transcriptionHTML += '</div>';
    
    // Find or create result container
    let resultContainer = transcriptionSection.querySelector('.transcription-result-container');
    if (!resultContainer) {
        resultContainer = document.createElement('div');
        resultContainer.className = 'transcription-result-container';
        transcriptionSection.appendChild(resultContainer);
    }
    
    resultContainer.innerHTML = transcriptionHTML;
}

// Delete transcription
async function deleteTranscription(transcriptionId) {
    if (!confirm('Are you sure you want to delete this transcription?')) return;
    
    try {
        const response = await fetch(`/api/transcriptions/${transcriptionId}`, {
            method: 'DELETE'
        });
        
        if (response.ok) {
            showNotification('Transcription deleted successfully', 'success');
            // Clear the transcription display
            const resultContainer = document.querySelector('.transcription-result-container');
            if (resultContainer) {
                resultContainer.innerHTML = '';
            }
        } else {
            showNotification('Failed to delete transcription', 'error');
        }
    } catch (error) {
        console.error('Error deleting transcription:', error);
        showNotification('Error deleting transcription', 'error');
    }
}

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
            offset: (currentPage - 1) * pageSize
        });
        
        if (currentFilter) {
            params.append('media_type', currentFilter);
        }
        
        let url = '/api/media?' + params;
        if (currentSearch) {
            url = `/api/media/search?q=${encodeURIComponent(currentSearch)}&limit=${pageSize}&offset=${(currentPage - 1) * pageSize}`;
        }
        
        const response = await fetch(url);
        const data = await response.json();
        
        if (data.success) {
            displayMedia(data.data);
            // Fetch total count for proper pagination
            await updateTotalCount();
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
        // Determine the appropriate image source
        let imageSrc;
        let useImg = false;
        
        if (item.media_type === 'image') {
            // For images, prefer thumbnail if available, otherwise use the image itself
            imageSrc = item.thumbnail_path ? `/api/media/${item.id}/thumbnail` : `/api/media/${item.id}/image`;
            useImg = true;
        } else if (item.thumbnail_path) {
            // For videos/audio with thumbnails
            imageSrc = `/api/media/${item.id}/thumbnail`;
            useImg = true;
        }
        
        if (isGridView) {
            return `
                <div class="media-item gallery-item" data-id="${item.id}" data-media-id="${item.id}">
                    ${useImg ? 
                        `<img src="${imageSrc}" alt="${item.file_name}" class="media-thumbnail" loading="lazy">` :
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
                    ${useImg ? 
                        `<img src="${imageSrc}" alt="${item.file_name}" class="media-thumbnail" loading="lazy">` :
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
    
    // Add error handlers for images
    document.querySelectorAll('.media-thumbnail').forEach(img => {
        if (img.tagName === 'IMG') {
            img.onerror = function() {
                // Replace with placeholder on error
                const parent = this.parentElement;
                const mediaType = parent.querySelector('.media-type-badge')?.textContent || 'image';
                this.style.display = 'none';
                const placeholder = document.createElement('div');
                placeholder.className = 'media-thumbnail';
                placeholder.style.cssText = 'display: flex; align-items: center; justify-content: center; background: #f8f9fa; font-size: 3em;';
                placeholder.textContent = getMediaEmoji(mediaType);
                parent.insertBefore(placeholder, this);
            };
        }
    });
}

// Update total count for pagination
async function updateTotalCount() {
    try {
        const stats = await fetch('/api/stats').then(r => r.json());
        if (stats.success) {
            const totalFiles = stats.data.total_files || 0;
            totalPages = Math.max(1, Math.ceil(totalFiles / pageSize));
        }
    } catch (error) {
        console.error('Error fetching total count:', error);
    }
}

// Show media detail panel
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
            const panel = document.getElementById('media-panel');
            const overlay = document.getElementById('panel-overlay');
            panel.dataset.currentMediaId = mediaId;
            
            // Set basic info
            document.getElementById('media-filename').textContent = media.file_name;
            document.getElementById('media-path').textContent = media.file_path;
            document.getElementById('media-size').textContent = formatBytes(media.file_size);
            document.getElementById('media-type').textContent = media.media_type;
            document.getElementById('media-hash').textContent = media.file_hash;
            document.getElementById('media-created').textContent = formatDate(media.file_created_at);
            
            // Check if this is a sub-image and show parent info
            const parentInfo = document.getElementById('parent-image-info');
            if (media.parent_id && media.is_sub_image) {
                try {
                    const parentRes = await fetch(`/api/media/${media.parent_id}`);
                    const parentData = await parentRes.json();
                    
                    if (parentData.success) {
                        const parent = parentData.data;
                        parentInfo.classList.remove('hidden');
                        document.getElementById('parent-thumbnail').src = `/api/media/${media.parent_id}/image`;
                        document.getElementById('parent-filename').textContent = parent.file_name;
                        document.getElementById('parent-image-link').onclick = (e) => {
                            e.preventDefault();
                            closePanel();
                            setTimeout(() => showMediaDetail(media.parent_id), 300);
                        };
                    }
                } catch (error) {
                    console.error('Error loading parent image:', error);
                }
            } else {
                parentInfo.classList.add('hidden');
            }
            
            // Load user description and tags
            document.getElementById('media-user-description').value = media.user_description || '';
            document.getElementById('media-user-tags').value = media.user_tags ? 
                JSON.parse(media.user_tags).join(', ') : '';
            
            // Show/hide conversion controls based on media type
            const imageConversion = document.getElementById('image-conversion');
            const videoConversion = document.getElementById('video-conversion');
            const audioConversion = document.getElementById('audio-conversion');
            
            imageConversion.classList.add('hidden');
            videoConversion.classList.add('hidden');
            audioConversion.classList.add('hidden');
            
            if (media.media_type === 'image') {
                imageConversion.classList.remove('hidden');
            } else if (media.media_type === 'video') {
                videoConversion.classList.remove('hidden');
            } else if (media.media_type === 'audio') {
                audioConversion.classList.remove('hidden');
            }
            
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
            
            // Initialize transcription for audio/video files
            initTranscription(mediaId, media.media_type);
            
            // Show detection actions based on media type
            const detectionActions = document.getElementById('detection-actions');
            const detectScenesBtn = document.getElementById('detect-scenes-btn');
            const classifyPhotoBtn = document.getElementById('classify-photo-btn');
            const detectObjectsBtn = document.getElementById('detect-objects-btn');
            
            if (detectionActions) {
                if (media.media_type === 'video') {
                    detectionActions.classList.remove('hidden');
                    if (detectScenesBtn) detectScenesBtn.style.display = 'inline-block';
                    if (classifyPhotoBtn) classifyPhotoBtn.style.display = 'none';
                    if (detectObjectsBtn) detectObjectsBtn.style.display = 'inline-block';
                } else if (media.media_type === 'image') {
                    detectionActions.classList.remove('hidden');
                    if (detectScenesBtn) detectScenesBtn.style.display = 'none';
                    if (classifyPhotoBtn) classifyPhotoBtn.style.display = 'inline-block';
                    if (detectObjectsBtn) detectObjectsBtn.style.display = 'inline-block';
                } else {
                    detectionActions.classList.add('hidden');
                }
            }
            
            // Show the panel
            panel.classList.add('active');
            overlay.classList.remove('hidden');
        }
    } catch (error) {
        console.error('Error loading media detail:', error);
    }
}

// Save media info
async function saveMediaInfo() {
    const panel = document.getElementById('media-panel');
    const mediaId = panel.dataset.currentMediaId;
    
    if (!mediaId) return;
    
    const description = document.getElementById('media-user-description').value;
    const tagsInput = document.getElementById('media-user-tags').value;
    const tags = tagsInput ? tagsInput.split(',').map(t => t.trim()).filter(t => t) : [];
    
    try {
        const response = await fetch(`/api/media/${mediaId}/metadata`, {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                user_description: description,
                user_tags: JSON.stringify(tags)
            })
        });
        
        const data = await response.json();
        
        if (data.success) {
            // Show success feedback
            const btn = document.getElementById('save-media-info-btn');
            const originalText = btn.textContent;
            btn.textContent = '✓ Saved!';
            btn.style.background = '#28a745';
            
            setTimeout(() => {
                btn.textContent = originalText;
                btn.style.background = '';
            }, 2000);
        } else {
            alert('Error saving: ' + data.error);
        }
    } catch (error) {
        console.error('Error saving media info:', error);
        alert('Failed to save changes');
    }
}

// Close panel function
function closePanel() {
    const panel = document.getElementById('media-panel');
    const overlay = document.getElementById('panel-overlay');
    panel.classList.remove('active');
    overlay.classList.add('hidden');
}

// Helper function to safely add event listeners
function addEventListenerIfExists(elementId, event, handler) {
    const element = document.getElementById(elementId);
    if (element) {
        element.addEventListener(event, handler);
    }
    return element;
}

// Setup event listeners
function setupEventListeners() {
    // Search
    addEventListenerIfExists('search-btn', 'click', performSearch);
    addEventListenerIfExists('search-input', 'keypress', (e) => {
        if (e.key === 'Enter') performSearch();
    });
    
    // Filter
    addEventListenerIfExists('filter-type', 'change', (e) => {
        currentFilter = e.target.value;
        currentPage = 1;
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
    addEventListenerIfExists('prev-page', 'click', () => {
        if (currentPage > 1) {
            currentPage--;
            loadGallery();
        }
    });
    
    addEventListenerIfExists('next-page', 'click', () => {
        if (currentPage < totalPages) {
            currentPage++;
            loadGallery();
        }
    });
    
    // Scan button
    addEventListenerIfExists('scan-btn', 'click', () => {
        const modal = document.getElementById('scan-modal');
        if (modal) modal.classList.remove('hidden');
    });
    
    addEventListenerIfExists('start-scan', 'click', startScan);
    
    // Duplicates button
    addEventListenerIfExists('duplicates-btn', 'click', showDuplicates);
    
    // Auto Albums button
    addEventListenerIfExists('auto-albums-btn', 'click', handleAutoAlbums);
    
    // Detection buttons
    addEventListenerIfExists('detect-scenes-btn', 'click', () => {
        const panel = document.getElementById('media-panel');
        const mediaId = panel?.dataset.currentMediaId;
        if (mediaId) detectScenes(mediaId);
    });
    
    addEventListenerIfExists('classify-photo-btn', 'click', () => {
        const panel = document.getElementById('media-panel');
        const mediaId = panel?.dataset.currentMediaId;
        if (mediaId) classifyPhoto(mediaId);
    });
    
    addEventListenerIfExists('detect-objects-btn', 'click', () => {
        const panel = document.getElementById('media-panel');
        const mediaId = panel?.dataset.currentMediaId;
        if (mediaId) detectObjects(mediaId);
    });
    
    // Archive selected duplicates
    addEventListenerIfExists('archive-selected-duplicates', 'click', archiveDuplicates);
    
    // Stories - these elements may not exist in the main gallery page
    addEventListenerIfExists('stories-btn', 'click', showStories);
    addEventListenerIfExists('create-story-btn', 'click', () => {
        const modal = document.getElementById('create-story-modal');
        if (modal) modal.classList.remove('hidden');
    });
    addEventListenerIfExists('create-story-form', 'submit', createStory);
    addEventListenerIfExists('cancel-create-story', 'click', () => {
        const modal = document.getElementById('create-story-modal');
        if (modal) modal.classList.add('hidden');
    });
    addEventListenerIfExists('add-to-story-btn', 'click', showAddToStory);
    addEventListenerIfExists('confirm-add-to-story', 'click', addToStory);
    addEventListenerIfExists('cancel-add-to-story', 'click', () => {
        const modal = document.getElementById('add-to-story-modal');
        if (modal) modal.classList.add('hidden');
    });
    
    // Panel close
    addEventListenerIfExists('close-panel', 'click', closePanel);
    addEventListenerIfExists('panel-overlay', 'click', closePanel);
    
    // Save media info
    addEventListenerIfExists('save-media-info-btn', 'click', saveMediaInfo);
    
    // Conversion buttons
    addEventListenerIfExists('convert-image-btn', 'click', () => convertMedia('image'));
    addEventListenerIfExists('convert-video-btn', 'click', () => convertMedia('video'));
    addEventListenerIfExists('convert-audio-btn', 'click', () => convertMedia('audio'));
    
    // Modal close buttons - specific handlers for each modal
    document.querySelectorAll('.modal .close').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.target.closest('.modal').classList.add('hidden');
        });
    });
    
    // Additional close handlers for story modals
    const closeHandlers = [
        ['close-stories-modal', 'stories-modal'],
        ['close-create-story-modal', 'create-story-modal'],
        ['close-add-to-story-modal', 'add-to-story-modal'],
        ['close-story-view-modal', 'story-view-modal']
    ];
    
    closeHandlers.forEach(([btnId, modalId]) => {
        const btn = document.getElementById(btnId);
        if (btn) {
            btn.addEventListener('click', () => {
                document.getElementById(modalId).classList.add('hidden');
            });
        }
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
    currentPage = 1;
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
            
            // Display groups with selection checkboxes
            groupsDiv.innerHTML = data.data.slice(0, 10).map((group, groupIndex) => `
                <div class="duplicate-group" data-hash="${group.hash}">
                    <h4>${group.count} copies (${formatBytes(group.total_size)} total)</h4>
                    <p class="duplicate-hint">Keep the first file, archive the rest:</p>
                    <div class="duplicate-files">
                        ${group.files.map((file, fileIndex) => `
                            <div class="duplicate-file">
                                <label>
                                    <input type="checkbox" class="duplicate-checkbox" 
                                           data-file-id="${file.id}" 
                                           data-file-path="${file.path}"
                                           ${fileIndex > 0 ? 'checked' : ''}>
                                    <span class="duplicate-file-path">${file.path}</span>
                                    <span class="duplicate-file-size">${formatBytes(file.size)}</span>
                                    ${fileIndex === 0 ? '<span class="keep-badge">KEEP</span>' : ''}
                                </label>
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

// Convert media to different format
async function convertMedia(type) {
    const panel = document.getElementById('media-panel');
    const mediaId = panel.dataset.currentMediaId;
    
    if (!mediaId) return;
    
    let format, options = {};
    
    if (type === 'image') {
        format = document.getElementById('image-format').value;
        if (!format) {
            alert('Please select a format');
            return;
        }
        options.quality = parseInt(document.getElementById('image-quality').value);
    } else if (type === 'video') {
        format = document.getElementById('video-format').value;
        if (!format) {
            alert('Please select a format');
            return;
        }
        const resolution = document.getElementById('video-resolution').value;
        if (resolution) {
            options.resolution = resolution;
        }
    } else if (type === 'audio') {
        format = document.getElementById('audio-format').value;
        if (!format) {
            alert('Please select a format');
            return;
        }
        options.bitrate = parseInt(document.getElementById('audio-bitrate').value);
    }
    
    // Show progress
    const progressDiv = document.getElementById('conversion-progress');
    const progressFill = document.getElementById('conversion-progress-fill');
    const statusText = document.getElementById('conversion-status');
    
    progressDiv.classList.remove('hidden');
    progressFill.style.width = '0%';
    statusText.textContent = 'Starting conversion...';
    
    try {
        const response = await fetch(`/api/media/${mediaId}/convert`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                format,
                options
            })
        });
        
        if (!response.ok) {
            throw new Error('Conversion failed');
        }
        
        // Get the converted file as a blob
        const blob = await response.blob();
        
        // Get filename from Content-Disposition header
        const contentDisposition = response.headers.get('Content-Disposition');
        let filename = `converted.${format}`;
        if (contentDisposition) {
            const matches = /filename="(.+)"/.exec(contentDisposition);
            if (matches) {
                filename = matches[1];
            }
        }
        
        // Create download link
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        window.URL.revokeObjectURL(url);
        
        progressFill.style.width = '100%';
        statusText.textContent = 'Conversion complete! Download started.';
        
        setTimeout(() => {
            progressDiv.classList.add('hidden');
        }, 3000);
        
    } catch (error) {
        console.error('Conversion error:', error);
        statusText.textContent = 'Conversion failed. Please try again.';
        statusText.style.color = 'red';
        
        setTimeout(() => {
            progressDiv.classList.add('hidden');
            statusText.style.color = '';
        }, 3000);
    }
}

// Archive selected duplicates
async function archiveDuplicates() {
    const selectedFiles = [];
    document.querySelectorAll('.duplicate-checkbox:checked').forEach(checkbox => {
        selectedFiles.push({
            id: checkbox.dataset.fileId,
            path: checkbox.dataset.filePath
        });
    });
    
    if (selectedFiles.length === 0) {
        alert('No files selected for archiving');
        return;
    }
    
    if (!confirm(`Archive ${selectedFiles.length} duplicate files?\n\nFiles will be moved to an 'archive' directory and removed from the database.`)) {
        return;
    }
    
    try {
        // Create archive directory and move files
        const response = await fetch('/api/duplicates/archive', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ files: selectedFiles })
        });
        
        const data = await response.json();
        
        if (data.success) {
            alert(`Successfully archived ${data.data.archived_count} files to ${data.data.archive_path}`);
            document.getElementById('duplicates-modal').classList.add('hidden');
            loadStats();
            loadGallery();
        } else {
            alert(`Error archiving files: ${data.error}`);
        }
    } catch (error) {
        console.error('Error archiving duplicates:', error);
        alert('Failed to archive duplicates. See console for details.');
    }
}

// Update pagination
function updatePagination(itemCount) {
    const prevBtn = document.getElementById('prev-page');
    const nextBtn = document.getElementById('next-page');
    const pageInfo = document.getElementById('page-info');
    
    prevBtn.disabled = currentPage === 1;
    nextBtn.disabled = currentPage >= totalPages || itemCount < pageSize;
    pageInfo.textContent = `Page ${currentPage} of ${totalPages}`;
}

// Show stories modal
async function showStories() {
    const modal = document.getElementById('stories-modal');
    modal.classList.remove('hidden');
    await loadStories();
}

// Load stories
async function loadStories() {
    try {
        const response = await fetch('/api/stories');
        const data = await response.json();
        
        if (data.success) {
            const storiesList = document.getElementById('stories-list');
            
            if (data.data.length === 0) {
                storiesList.innerHTML = '<p style="text-align: center; color: #666;">No stories yet. Create your first story!</p>';
            } else {
                storiesList.innerHTML = data.data.map(story => `
                    <div class="story-card" data-story-id="${story.id}">
                        <h3>${story.name}</h3>
                        <p>${story.description || 'No description'}</p>
                        <div class="story-meta">
                            <span>${story.item_count || 0} items</span>
                            <span>${formatDate(story.created_at)}</span>
                        </div>
                    </div>
                `).join('');
                
                // Add click handlers
                document.querySelectorAll('.story-card').forEach(card => {
                    card.addEventListener('click', () => viewStory(card.dataset.storyId));
                });
            }
        }
    } catch (error) {
        console.error('Error loading stories:', error);
    }
}

// Create story
async function createStory(e) {
    e.preventDefault();
    
    const name = document.getElementById('story-name').value;
    const description = document.getElementById('story-description').value;
    
    try {
        const response = await fetch('/api/stories', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ name, description })
        });
        
        const data = await response.json();
        
        if (data.success) {
            document.getElementById('create-story-modal').classList.add('hidden');
            document.getElementById('create-story-form').reset();
            await loadStories();
        } else {
            alert('Error creating story: ' + data.error);
        }
    } catch (error) {
        console.error('Error creating story:', error);
        alert('Failed to create story');
    }
}

// View story
async function viewStory(storyId) {
    try {
        const response = await fetch(`/api/stories/${storyId}`);
        const data = await response.json();
        
        if (data.success) {
            const story = data.data;
            document.getElementById('story-view-name').textContent = story.story.name;
            document.getElementById('story-view-description').textContent = story.story.description || '';
            
            const itemsDiv = document.getElementById('story-items');
            if (story.items.length === 0) {
                itemsDiv.innerHTML = '<p style="text-align: center; color: #666;">No items in this story yet.</p>';
            } else {
                itemsDiv.innerHTML = story.items.map(item => {
                    let mediaContent = '';
                    if (item.media_type === 'image') {
                        mediaContent = `<img src="/api/media/${item.id}/image" alt="${item.file_name}">`;
                    } else if (item.media_type === 'video') {
                        mediaContent = `<video src="/api/media/${item.id}/video" controls></video>`;
                    } else if (item.media_type === 'audio') {
                        mediaContent = `<div style="padding: 50px 10px; text-align: center;">🎵<br>${item.file_name}</div>`;
                    }
                    
                    return `
                        <div class="story-item" data-item-id="${item.id}">
                            ${mediaContent}
                            <div class="item-caption">${item.caption || item.file_name}</div>
                            <button class="remove-item" data-story-id="${storyId}" data-media-id="${item.id}">×</button>
                        </div>
                    `;
                }).join('');
                
                // Add remove handlers
                document.querySelectorAll('.remove-item').forEach(btn => {
                    btn.addEventListener('click', (e) => {
                        e.stopPropagation();
                        removeFromStory(btn.dataset.storyId, btn.dataset.mediaId);
                    });
                });
            }
            
            // Set up story action buttons
            document.getElementById('delete-story-btn').onclick = () => deleteStory(storyId);
            
            document.getElementById('story-view-modal').classList.remove('hidden');
        }
    } catch (error) {
        console.error('Error viewing story:', error);
    }
}

// Show add to story modal
async function showAddToStory() {
    const panel = document.getElementById('media-panel');
    const mediaId = panel.dataset.currentMediaId;
    
    if (!mediaId) return;
    
    try {
        const response = await fetch('/api/stories');
        const data = await response.json();
        
        if (data.success) {
            const select = document.getElementById('select-story');
            select.innerHTML = '<option value="">Choose a story...</option>' + 
                data.data.map(story => `<option value="${story.id}">${story.name}</option>`).join('');
            
            document.getElementById('add-to-story-modal').classList.remove('hidden');
        }
    } catch (error) {
        console.error('Error loading stories:', error);
    }
}

// Add to story
async function addToStory() {
    const panel = document.getElementById('media-panel');
    const mediaId = panel.dataset.currentMediaId;
    const storyId = document.getElementById('select-story').value;
    const caption = document.getElementById('item-caption').value;
    
    if (!storyId) {
        alert('Please select a story');
        return;
    }
    
    try {
        const response = await fetch(`/api/stories/${storyId}/items`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                media_file_id: mediaId,
                caption
            })
        });
        
        const data = await response.json();
        
        if (data.success) {
            document.getElementById('add-to-story-modal').classList.add('hidden');
            document.getElementById('item-caption').value = '';
            alert('Added to story successfully!');
        } else {
            alert('Error adding to story: ' + data.error);
        }
    } catch (error) {
        console.error('Error adding to story:', error);
        alert('Failed to add to story');
    }
}

// Remove from story
async function removeFromStory(storyId, mediaId) {
    if (!confirm('Remove this item from the story?')) return;
    
    try {
        const response = await fetch(`/api/stories/${storyId}/items/${mediaId}`, {
            method: 'DELETE'
        });
        
        const data = await response.json();
        
        if (data.success) {
            await viewStory(storyId);
        } else {
            alert('Error removing item: ' + data.error);
        }
    } catch (error) {
        console.error('Error removing item:', error);
    }
}

// Delete story
async function deleteStory(storyId) {
    if (!confirm('Are you sure you want to delete this story? This cannot be undone.')) return;
    
    try {
        const response = await fetch(`/api/stories/${storyId}`, {
            method: 'DELETE'
        });
        
        const data = await response.json();
        
        if (data.success) {
            document.getElementById('story-view-modal').classList.add('hidden');
            await loadStories();
        } else {
            alert('Error deleting story: ' + data.error);
        }
    } catch (error) {
        console.error('Error deleting story:', error);
    }
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

// Transcription Functions
async function initTranscription(mediaId, mediaType) {
    const transcriptionSection = document.getElementById('transcription-section');
    
    // Only show for audio and video files
    if (mediaType === 'audio' || mediaType === 'video') {
        transcriptionSection.classList.remove('hidden');
        
        // Check if transcription already exists
        try {
            const response = await fetch(`/api/transcriptions/media/${mediaId}`);
            if (response.ok) {
                const data = await response.json();
                if (data.success && data.data) {
                    displayTranscription(data.data);
                } else {
                    // Show controls to generate transcription
                    showTranscriptionControls();
                }
            } else if (response.status === 404) {
                // No transcription exists, show controls
                showTranscriptionControls();
            }
        } catch (error) {
            console.error('Error checking transcription:', error);
            showTranscriptionControls();
        }
    } else {
        transcriptionSection.classList.add('hidden');
    }
}

function showTranscriptionControls() {
    document.getElementById('transcription-controls').classList.remove('hidden');
    document.getElementById('transcription-progress').classList.add('hidden');
    document.getElementById('transcription-result').classList.add('hidden');
}

function displayTranscription(data) {
    const { transcription, segments } = data;
    
    document.getElementById('transcription-controls').classList.add('hidden');
    document.getElementById('transcription-progress').classList.add('hidden');
    document.getElementById('transcription-result').classList.remove('hidden');
    
    // Display language if detected
    if (transcription.language) {
        document.getElementById('transcription-language-detected').textContent = 
            `Language: ${transcription.language.toUpperCase()}`;
    }
    
    // Display main transcription text
    document.getElementById('transcription-text').textContent = 
        transcription.transcription_text || 'No transcription text available';
    
    // Display segments if available
    if (segments && segments.length > 0) {
        const segmentsList = document.getElementById('segments-list');
        segmentsList.innerHTML = segments.map(seg => {
            const timestamp = `${formatTime(seg.start_time)} - ${formatTime(seg.end_time)}`;
            return `
                <div class="segment-item">
                    <div class="segment-timestamp">${timestamp}</div>
                    ${seg.speaker ? `<div class="segment-speaker">${seg.speaker}</div>` : ''}
                    <div class="segment-text">${seg.text}</div>
                </div>
            `;
        }).join('');
    }
    
    // Store transcription ID for deletion
    document.getElementById('transcription-result').dataset.transcriptionId = transcription.id;
}

function formatTime(seconds) {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
}

async function generateTranscription() {
    const mediaPanel = document.getElementById('media-panel');
    const mediaId = mediaPanel.dataset.currentMediaId;
    
    if (!mediaId) return;
    
    const language = document.getElementById('transcription-language').value;
    const enableDiarization = document.getElementById('enable-speaker-diarization').checked;
    
    // Show progress
    document.getElementById('transcription-controls').classList.add('hidden');
    document.getElementById('transcription-progress').classList.remove('hidden');
    document.getElementById('transcription-status').textContent = 'Transcribing...';
    
    try {
        const response = await fetch('/api/transcribe', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                media_file_id: mediaId,
                language: language || null,
                enable_speaker_diarization: enableDiarization
            })
        });
        
        const data = await response.json();
        
        if (data.success) {
            displayTranscription(data.data);
        } else {
            alert('Transcription failed: ' + (data.error || 'Unknown error'));
            showTranscriptionControls();
        }
    } catch (error) {
        console.error('Error generating transcription:', error);
        alert('Failed to generate transcription');
        showTranscriptionControls();
    } finally {
        document.getElementById('transcription-progress').classList.add('hidden');
    }
}

async function deleteTranscription() {
    const transcriptionResult = document.getElementById('transcription-result');
    const transcriptionId = transcriptionResult.dataset.transcriptionId;
    
    if (!transcriptionId) return;
    
    if (!confirm('Are you sure you want to delete this transcription?')) return;
    
    try {
        const response = await fetch(`/api/transcriptions/${transcriptionId}`, {
            method: 'DELETE'
        });
        
        if (response.ok) {
            showTranscriptionControls();
            transcriptionResult.dataset.transcriptionId = '';
        } else {
            alert('Failed to delete transcription');
        }
    } catch (error) {
        console.error('Error deleting transcription:', error);
        alert('Failed to delete transcription');
    }
}

function toggleTranscriptionSegments() {
    const segments = document.getElementById('transcription-segments');
    const button = document.getElementById('toggle-segments-btn');
    
    if (segments.classList.contains('hidden')) {
        segments.classList.remove('hidden');
        button.textContent = 'Hide Detailed Segments';
    } else {
        segments.classList.add('hidden');
        button.textContent = 'Show Detailed Segments';
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
    
    // Add transcription event handlers
    const transcribeBtn = document.getElementById('transcribe-btn');
    if (transcribeBtn) {
        transcribeBtn.addEventListener('click', generateTranscription);
    }
    
    const deleteTranscriptionBtn = document.getElementById('delete-transcription-btn');
    if (deleteTranscriptionBtn) {
        deleteTranscriptionBtn.addEventListener('click', deleteTranscription);
    }
    
    const toggleSegmentsBtn = document.getElementById('toggle-segments-btn');
    if (toggleSegmentsBtn) {
        toggleSegmentsBtn.addEventListener('click', toggleTranscriptionSegments);
    }
    
    // Listen for media refresh events
    document.addEventListener('refresh-media', async (e) => {
        const mediaId = e.detail.mediaId;
        if (mediaId) {
            await loadMediaDetail(mediaId);
        }
    });
});

// Scene Detection for Videos
async function detectScenes(mediaId) {
    try {
        showNotification('Detecting scenes...', 'info');
        
        const response = await fetch(`/api/media/${mediaId}/detect-scenes`, {
            method: 'POST'
        });
        
        const data = await response.json();
        
        if (data.success) {
            displayScenes(data.data);
            showNotification(`Detected ${data.data.length} scenes`, 'success');
        } else {
            showNotification('Scene detection failed: ' + data.error, 'error');
        }
    } catch (error) {
        console.error('Scene detection error:', error);
        showNotification('Scene detection failed', 'error');
    }
}

function displayScenes(scenes) {
    const detectionResults = document.getElementById('detection-results');
    if (!detectionResults) return;
    
    let scenesHtml = '<div class="scenes-container"><h4>🎬 Detected Scenes</h4>';
    
    if (scenes.length === 0) {
        scenesHtml += '<p>No scenes detected</p>';
    } else {
        scenes.forEach((scene, index) => {
            scenesHtml += `
                <div class="scene-item">
                    <div class="scene-header">
                        <span class="scene-number">Scene ${scene.scene_number}</span>
                        <span class="scene-duration">${formatTime(scene.start_time)} - ${formatTime(scene.end_time)}</span>
                    </div>
                    <div class="scene-details">
                        <span>Duration: ${scene.duration.toFixed(1)}s</span>
                        <span>Frames: ${scene.start_frame} - ${scene.end_frame}</span>
                        <span>Confidence: ${(scene.confidence * 100).toFixed(0)}%</span>
                    </div>
                    ${scene.keyframe_path ? `<img src="${scene.keyframe_path}" class="scene-keyframe" />` : ''}
                </div>
            `;
        });
    }
    
    scenesHtml += '</div>';
    
    detectionResults.innerHTML = scenesHtml;
    detectionResults.classList.remove('hidden');
}

function displayPhotoClassification(classification) {
    const detectionResults = document.getElementById('detection-results');
    if (!detectionResults) return;
    
    let classHtml = '<div class="classification-container"><h4>🏷️ Photo Classification</h4>';
    
    classHtml += `
        <div class="classification-main">
            <p><strong>Primary Category:</strong> ${classification.primary_category}</p>
            ${classification.scene_type ? `<p><strong>Scene Type:</strong> ${classification.scene_type}</p>` : ''}
        </div>
    `;
    
    // Categories
    if (classification.categories) {
        const categories = JSON.parse(classification.categories);
        classHtml += '<div class="classification-categories">';
        categories.forEach(cat => {
            classHtml += `<span class="category-badge">${cat.name} (${(cat.confidence * 100).toFixed(0)}%)</span>`;
        });
        classHtml += '</div>';
    }
    
    // Tags
    if (classification.tags) {
        const tags = JSON.parse(classification.tags);
        classHtml += '<div class="classification-tags">';
        tags.forEach(tag => {
            classHtml += `<span class="tag">${tag}</span>`;
        });
        classHtml += '</div>';
    }
    
    // Dominant colors
    if (classification.dominant_colors) {
        const colors = JSON.parse(classification.dominant_colors);
        classHtml += '<div class="color-palette">';
        colors.forEach(color => {
            classHtml += `<span class="color-swatch" style="background-color: ${color}" title="${color}"></span>`;
        });
        classHtml += '</div>';
    }
    
    classHtml += '</div>';
    
    detectionResults.innerHTML = classHtml;
    detectionResults.classList.remove('hidden');
}

// Photo Classification
async function classifyPhoto(mediaId) {
    try {
        showNotification('Classifying photo...', 'info');
        
        const response = await fetch(`/api/media/${mediaId}/classify`, {
            method: 'POST'
        });
        
        const data = await response.json();
        
        if (data.success) {
            displayClassification(data.data);
            showNotification('Photo classified successfully', 'success');
        } else {
            showNotification('Classification failed: ' + data.error, 'error');
        }
    } catch (error) {
        console.error('Classification error:', error);
        showNotification('Classification failed', 'error');
    }
}

function displayClassification(classification) {
    const detailContent = document.querySelector('.detail-content');
    if (!detailContent) return;
    
    const categories = JSON.parse(classification.categories || '[]');
    const tags = JSON.parse(classification.tags || '[]');
    const colors = JSON.parse(classification.dominant_colors || '[]');
    
    let classHtml = `
        <div class="classification-container">
            <h3>🏷️ Photo Classification</h3>
            <div class="classification-primary">
                <strong>Category:</strong> ${classification.primary_category}
            </div>
            ${classification.scene_type ? `<div><strong>Scene:</strong> ${classification.scene_type}</div>` : ''}
            <div class="classification-badges">
                ${classification.is_screenshot ? '<span class="badge badge-screenshot">Screenshot</span>' : ''}
                ${classification.is_document ? '<span class="badge badge-document">Document</span>' : ''}
                ${classification.has_text ? '<span class="badge badge-text">Contains Text</span>' : ''}
            </div>
            ${tags.length > 0 ? `
                <div class="classification-tags">
                    <strong>Tags:</strong> ${tags.map(tag => `<span class="tag">${tag}</span>`).join(' ')}
                </div>
            ` : ''}
            ${colors.length > 0 ? `
                <div class="classification-colors">
                    <strong>Colors:</strong>
                    <div class="color-palette">
                        ${colors.map(color => `<span class="color-swatch" style="background-color: ${color}"></span>`).join('')}
                    </div>
                </div>
            ` : ''}
        </div>
    `;
    
    // Add or update classification section
    let classSection = detailContent.querySelector('.classification-container');
    if (classSection) {
        classSection.outerHTML = classHtml;
    } else {
        detailContent.insertAdjacentHTML('beforeend', classHtml);
    }
}

// Object Detection
async function detectObjects(mediaId) {
    try {
        showNotification('Detecting objects...', 'info');
        
        const response = await fetch(`/api/media/${mediaId}/detect-objects`, {
            method: 'POST'
        });
        
        const data = await response.json();
        
        if (data.success) {
            displayDetectedObjects(data.data);
            showNotification(`Detected ${data.data.length} objects`, 'success');
        } else {
            showNotification('Object detection failed: ' + data.error, 'error');
        }
    } catch (error) {
        console.error('Object detection error:', error);
        showNotification('Object detection failed', 'error');
    }
}

function displayDetectedObjects(objects) {
    const detectionResults = document.getElementById('detection-results');
    if (!detectionResults) return;
    
    let objectsHtml = '<div class="objects-container"><h4>🔍 Detected Objects</h4>';
    
    if (objects.length === 0) {
        objectsHtml += '<p>No objects detected</p>';
    } else {
        objectsHtml += '<div class="objects-list">';
        objects.forEach(obj => {
            objectsHtml += `
                <div class="object-item">
                    <span class="object-class">${obj.class_name}</span>
                    <span class="object-confidence">${(obj.confidence * 100).toFixed(0)}%</span>
                </div>
            `;
        });
        objectsHtml += '</div>';
    }
    
    objectsHtml += '</div>';
    
    detectionResults.innerHTML = objectsHtml;
    detectionResults.classList.remove('hidden');
}

// Auto Albums functions
async function handleAutoAlbums() {
    try {
        // First generate albums
        showNotification('Generating smart albums...', 'info');
        const generateResponse = await fetch('/api/auto-albums/generate', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                min_media_count: 3,
                confidence_threshold: 0.7,
                album_types: ['category', 'scene', 'object']
            })
        });
        
        const generateData = await generateResponse.json();
        if (generateData.success) {
            showNotification(`Generated ${generateData.data.total_albums_created} smart albums`, 'success');
            
            // Now display the albums
            await displayAutoAlbums();
        } else {
            showNotification('Failed to generate albums', 'error');
        }
    } catch (error) {
        console.error('Auto album error:', error);
        showNotification('Failed to process auto albums', 'error');
    }
}

async function displayAutoAlbums() {
    try {
        const response = await fetch('/api/auto-albums');
        const data = await response.json();
        
        if (data.success) {
            const gallery = document.getElementById('gallery');
            gallery.innerHTML = '<h2>📸 Smart Albums</h2>';
            
            if (data.data.length === 0) {
                gallery.innerHTML += '<p>No albums created yet. Process some photos first!</p>';
                return;
            }
            
            const albumsGrid = document.createElement('div');
            albumsGrid.className = 'albums-grid';
            
            data.data.forEach(album => {
                const albumCard = document.createElement('div');
                albumCard.className = 'album-card';
                albumCard.onclick = () => viewAlbumMedia(album.id, album.album_name);
                
                albumCard.innerHTML = `
                    <div class="album-cover">
                        ${album.cover_media_id ? 
                            `<img src="/api/media/${album.cover_media_id}/thumbnail" alt="${album.album_name}">` : 
                            '<div class="album-placeholder">📸</div>'}
                    </div>
                    <div class="album-info">
                        <h3>${album.album_name}</h3>
                        <p class="album-type">${album.album_type}</p>
                        <p class="album-count">${album.media_count} items</p>
                    </div>
                `;
                
                albumsGrid.appendChild(albumCard);
            });
            
            gallery.appendChild(albumsGrid);
            
            // Add back button
            const backButton = document.createElement('button');
            backButton.className = 'action-btn';
            backButton.textContent = '← Back to Gallery';
            backButton.onclick = () => loadMedia();
            gallery.insertBefore(backButton, gallery.firstChild);
        }
    } catch (error) {
        console.error('Error loading auto albums:', error);
        showNotification('Failed to load albums', 'error');
    }
}

async function viewAlbumMedia(albumId, albumName) {
    try {
        const response = await fetch(`/api/auto-albums/${albumId}/media`);
        const data = await response.json();
        
        if (data.success) {
            const gallery = document.getElementById('gallery');
            gallery.innerHTML = `<h2>📸 ${albumName}</h2>`;
            
            // Add back button
            const backButton = document.createElement('button');
            backButton.className = 'action-btn';
            backButton.textContent = '← Back to Albums';
            backButton.onclick = () => displayAutoAlbums();
            gallery.appendChild(backButton);
            
            if (data.data.length === 0) {
                gallery.innerHTML += '<p>No media in this album</p>';
                return;
            }
            
            const mediaGrid = document.createElement('div');
            mediaGrid.className = 'gallery grid-view';
            
            data.data.forEach(media => {
                const mediaItem = createMediaItem(media);
                mediaGrid.appendChild(mediaItem);
            });
            
            gallery.appendChild(mediaGrid);
        }
    } catch (error) {
        console.error('Error loading album media:', error);
        showNotification('Failed to load album media', 'error');
    }
}