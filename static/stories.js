// Stories page functionality
let allStories = [];
let filteredStories = [];

// Initialize page
document.addEventListener('DOMContentLoaded', () => {
    loadStories();
    setupEventListeners();
});

// Setup event listeners
function setupEventListeners() {
    // Create story button
    document.getElementById('create-story-btn').addEventListener('click', () => {
        document.getElementById('create-story-modal').classList.remove('hidden');
    });
    
    // Create story form
    document.getElementById('create-story-form').addEventListener('submit', createStory);
    
    // Cancel create story
    document.getElementById('cancel-create-story').addEventListener('click', () => {
        document.getElementById('create-story-modal').classList.add('hidden');
    });
    
    // Search
    document.getElementById('story-search').addEventListener('input', (e) => {
        searchStories(e.target.value);
    });
    
    // Modal close buttons
    document.querySelectorAll('.modal .close').forEach(btn => {
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

// Load all stories
async function loadStories() {
    try {
        const response = await fetch('/api/stories');
        const data = await response.json();
        
        if (data.success) {
            allStories = data.data;
            filteredStories = allStories;
            displayStories();
        }
    } catch (error) {
        console.error('Error loading stories:', error);
    }
}

// Display stories
function displayStories() {
    const grid = document.getElementById('stories-grid');
    const emptyState = document.getElementById('empty-state');
    
    if (filteredStories.length === 0) {
        grid.innerHTML = '';
        emptyState.classList.remove('hidden');
    } else {
        emptyState.classList.add('hidden');
        grid.innerHTML = filteredStories.map(story => createStoryCard(story)).join('');
        
        // Add click handlers
        document.querySelectorAll('.story-card-large').forEach(card => {
            card.addEventListener('click', () => viewStory(card.dataset.storyId));
        });
    }
}

// Create story card HTML
function createStoryCard(story) {
    return `
        <div class="story-card-large" data-story-id="${story.id}">
            <div class="story-cover">
                ${story.cover_image_id ? 
                    `<img src="/api/media/${story.cover_image_id}/thumbnail" alt="Cover">` : 
                    '📚'
                }
            </div>
            <div class="story-card-content">
                <h3>${story.name}</h3>
                <p>${story.description || 'No description'}</p>
                <div class="story-stats">
                    <span>📅 ${formatDate(story.created_at)}</span>
                    <span>${story.item_count || 0} items</span>
                </div>
            </div>
        </div>
    `;
}

// Search stories
function searchStories(query) {
    if (!query) {
        filteredStories = allStories;
    } else {
        const lowerQuery = query.toLowerCase();
        filteredStories = allStories.filter(story => 
            story.name.toLowerCase().includes(lowerQuery) ||
            (story.description && story.description.toLowerCase().includes(lowerQuery))
        );
    }
    displayStories();
}

// Create new story
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
            
            // Show success message
            showNotification('Story created successfully!');
        } else {
            alert('Error creating story: ' + data.error);
        }
    } catch (error) {
        console.error('Error creating story:', error);
        alert('Failed to create story');
    }
}

// View story details
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
                itemsDiv.innerHTML = '<p style="text-align: center; color: #666; padding: 40px;">No items in this story yet. Add media from the gallery.</p>';
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

// Remove item from story
async function removeFromStory(storyId, mediaId) {
    if (!confirm('Remove this item from the story?')) return;
    
    try {
        const response = await fetch(`/api/stories/${storyId}/items/${mediaId}`, {
            method: 'DELETE'
        });
        
        const data = await response.json();
        
        if (data.success) {
            await viewStory(storyId);
            showNotification('Item removed from story');
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
            showNotification('Story deleted');
        } else {
            alert('Error deleting story: ' + data.error);
        }
    } catch (error) {
        console.error('Error deleting story:', error);
    }
}

// Show notification
function showNotification(message) {
    // Create notification element if it doesn't exist
    let notification = document.getElementById('notification');
    if (!notification) {
        notification = document.createElement('div');
        notification.id = 'notification';
        notification.style.cssText = `
            position: fixed;
            top: 20px;
            right: 20px;
            background: #28a745;
            color: white;
            padding: 15px 25px;
            border-radius: 5px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
            z-index: 10000;
            transition: opacity 0.3s;
        `;
        document.body.appendChild(notification);
    }
    
    notification.textContent = message;
    notification.style.opacity = '1';
    notification.style.display = 'block';
    
    setTimeout(() => {
        notification.style.opacity = '0';
        setTimeout(() => {
            notification.style.display = 'none';
        }, 300);
    }, 3000);
}

// Format date utility
function formatDate(dateString) {
    if (!dateString) return 'N/A';
    const date = new Date(dateString);
    return date.toLocaleDateString();
}