#ifndef GAME_ENGINE_VIDEO_DEVICE_H
#define GAME_ENGINE_VIDEO_DEVICE_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stddef.h>

// Forward declarations
struct CVideoDevice;
typedef struct CVideoDevice CVideoDevice;

// Error codes
typedef enum {
    VIDEO_SUCCESS = 0,
    VIDEO_ERROR_INVALID_PARAMETER = -1,
    VIDEO_ERROR_INITIALIZATION_FAILED = -2,
    VIDEO_ERROR_RESOURCE_NOT_FOUND = -3,
    VIDEO_ERROR_OUT_OF_MEMORY = -4,
    VIDEO_ERROR_UNSUPPORTED = -5,
    VIDEO_ERROR_INTERNAL = -6
} VideoResult;

// Color formats
typedef enum {
    COLOR_FORMAT_RGBA8 = 0,
    COLOR_FORMAT_BGRA8 = 1,
    COLOR_FORMAT_RGBA16 = 2,
    COLOR_FORMAT_RGBA32_FLOAT = 3
} ColorFormat;

// VSync modes
typedef enum {
    VSYNC_DISABLED = 0,
    VSYNC_ENABLED = 1,
    VSYNC_ADAPTIVE = 2,
    VSYNC_FAST = 3
} VSyncMode;

// Vertex structure
typedef struct {
    float position[3];
    float normal[3];
    float tex_coords[2];
    float color[4];
} Vertex;

// Statistics structure
typedef struct {
    float fps;
    float frame_time_ms;
    uint64_t gpu_memory_usage;
    uint32_t draw_calls;
    uint32_t triangle_count;
    float gpu_utilization;
    uint32_t textures_loaded;
    uint32_t buffers_allocated;
} VideoStatistics;

// Core API Functions

/**
 * Create a new video device
 * @return Pointer to video device or NULL on failure
 */
CVideoDevice* video_device_create(void);

/**
 * Initialize the video device with specific parameters
 * @param device Video device handle
 * @param width Screen width in pixels
 * @param height Screen height in pixels
 * @param fullscreen 1 for fullscreen, 0 for windowed
 * @return VIDEO_SUCCESS on success, error code on failure
 */
VideoResult video_device_initialize(CVideoDevice* device, uint32_t width, uint32_t height, int fullscreen);

/**
 * Destroy the video device and free all resources
 * @param device Video device handle
 * @return VIDEO_SUCCESS on success, error code on failure
 */
VideoResult video_device_destroy(CVideoDevice* device);

/**
 * Create a texture
 * @param device Video device handle
 * @param width Texture width in pixels
 * @param height Texture height in pixels  
 * @param format Color format
 * @return Texture ID (0 = invalid)
 */
uint32_t video_device_create_texture(CVideoDevice* device, uint32_t width, uint32_t height, uint32_t format);

/**
 * Set render target texture
 * @param device Video device handle
 * @param texture_id Texture ID to render to
 * @return VIDEO_SUCCESS on success, error code on failure
 */
VideoResult video_device_set_render_target(CVideoDevice* device, uint32_t texture_id);

/**
 * Draw primitive with vertices and indices
 * @param device Video device handle
 * @param vertices Array of vertices
 * @param vertex_count Number of vertices
 * @param indices Array of indices (can be NULL)
 * @param index_count Number of indices
 * @return VIDEO_SUCCESS on success, error code on failure
 */
VideoResult video_device_draw_primitive(CVideoDevice* device, 
                                       const Vertex* vertices, uint32_t vertex_count,
                                       const uint16_t* indices, uint32_t index_count);

/**
 * Present the current frame to the screen
 * @param device Video device handle
 * @return VIDEO_SUCCESS on success, error code on failure
 */
VideoResult video_device_present(CVideoDevice* device);

/**
 * Get current device statistics
 * @param device Video device handle
 * @param stats Output statistics structure
 * @return VIDEO_SUCCESS on success, error code on failure
 */
VideoResult video_device_get_statistics(CVideoDevice* device, VideoStatistics* stats);

/**
 * Set display resolution
 * @param device Video device handle
 * @param width New width in pixels
 * @param height New height in pixels
 * @return VIDEO_SUCCESS on success, error code on failure
 */
VideoResult video_device_set_resolution(CVideoDevice* device, uint32_t width, uint32_t height);

/**
 * Toggle fullscreen mode
 * @param device Video device handle
 * @param fullscreen 1 for fullscreen, 0 for windowed
 * @return VIDEO_SUCCESS on success, error code on failure
 */
VideoResult video_device_set_fullscreen(CVideoDevice* device, int fullscreen);

/**
 * Set VSync mode
 * @param device Video device handle
 * @param vsync_mode VSync mode
 * @return VIDEO_SUCCESS on success, error code on failure
 */
VideoResult video_device_set_vsync(CVideoDevice* device, uint32_t vsync_mode);

/**
 * Get adapter name
 * @param device Video device handle
 * @return Adapter name string (must be freed with video_device_free_string)
 */
char* video_device_get_adapter_name(CVideoDevice* device);

/**
 * Free a string allocated by the video device API
 * @param string String to free
 */
void video_device_free_string(char* string);

/**
 * Get last error message (thread-local)
 * @return Error message string or NULL
 */
const char* video_device_get_last_error(void);

/**
 * Check if video device is initialized
 * @param device Video device handle
 * @return 1 if initialized, 0 if not
 */
int video_device_is_initialized(CVideoDevice* device);

/**
 * Get GPU memory usage in bytes
 * @param device Video device handle
 * @return GPU memory usage in bytes
 */
uint64_t video_device_get_gpu_memory_usage(CVideoDevice* device);

// Utility Functions

/**
 * Create a vertex from individual components
 */
Vertex create_vertex(float pos_x, float pos_y, float pos_z,
                    float norm_x, float norm_y, float norm_z,
                    float tex_u, float tex_v,
                    float color_r, float color_g, float color_b, float color_a);

/**
 * Create vertices and indices for a simple quad (2 triangles)
 * @param vertices Output array (must have space for 4 vertices)
 * @param indices Output array (must have space for 6 indices)
 * @return 0 on success, -1 on failure
 */
int create_quad_vertices(Vertex* vertices, uint16_t* indices);

#ifdef __cplusplus
}

// C++ wrapper class for easier integration
class VideoDevice {
private:
    CVideoDevice* m_device;
    bool m_initialized;

public:
    VideoDevice() : m_device(nullptr), m_initialized(false) {
        m_device = video_device_create();
    }

    ~VideoDevice() {
        if (m_device) {
            video_device_destroy(m_device);
        }
    }

    // Delete copy constructor and assignment operator
    VideoDevice(const VideoDevice&) = delete;
    VideoDevice& operator=(const VideoDevice&) = delete;

    // Move constructor and assignment operator
    VideoDevice(VideoDevice&& other) noexcept : m_device(other.m_device), m_initialized(other.m_initialized) {
        other.m_device = nullptr;
        other.m_initialized = false;
    }

    VideoDevice& operator=(VideoDevice&& other) noexcept {
        if (this != &other) {
            if (m_device) {
                video_device_destroy(m_device);
            }
            m_device = other.m_device;
            m_initialized = other.m_initialized;
            other.m_device = nullptr;
            other.m_initialized = false;
        }
        return *this;
    }

    bool Initialize(uint32_t width, uint32_t height, bool fullscreen = false) {
        if (!m_device) return false;
        VideoResult result = video_device_initialize(m_device, width, height, fullscreen ? 1 : 0);
        m_initialized = (result == VIDEO_SUCCESS);
        return m_initialized;
    }

    uint32_t CreateTexture(uint32_t width, uint32_t height, ColorFormat format = COLOR_FORMAT_RGBA8) {
        if (!m_device || !m_initialized) return 0;
        return video_device_create_texture(m_device, width, height, static_cast<uint32_t>(format));
    }

    bool SetRenderTarget(uint32_t texture_id) {
        if (!m_device || !m_initialized) return false;
        return video_device_set_render_target(m_device, texture_id) == VIDEO_SUCCESS;
    }

    bool DrawPrimitive(const Vertex* vertices, uint32_t vertex_count, 
                      const uint16_t* indices = nullptr, uint32_t index_count = 0) {
        if (!m_device || !m_initialized) return false;
        return video_device_draw_primitive(m_device, vertices, vertex_count, indices, index_count) == VIDEO_SUCCESS;
    }

    bool Present() {
        if (!m_device || !m_initialized) return false;
        return video_device_present(m_device) == VIDEO_SUCCESS;
    }

    VideoStatistics GetStatistics() {
        VideoStatistics stats = {};
        if (m_device && m_initialized) {
            video_device_get_statistics(m_device, &stats);
        }
        return stats;
    }

    bool SetResolution(uint32_t width, uint32_t height) {
        if (!m_device || !m_initialized) return false;
        return video_device_set_resolution(m_device, width, height) == VIDEO_SUCCESS;
    }

    bool SetFullscreen(bool fullscreen) {
        if (!m_device || !m_initialized) return false;
        return video_device_set_fullscreen(m_device, fullscreen ? 1 : 0) == VIDEO_SUCCESS;
    }

    bool SetVSync(VSyncMode mode) {
        if (!m_device || !m_initialized) return false;
        return video_device_set_vsync(m_device, static_cast<uint32_t>(mode)) == VIDEO_SUCCESS;
    }

    std::string GetAdapterName() {
        if (!m_device) return "";
        char* name = video_device_get_adapter_name(m_device);
        if (!name) return "";
        std::string result(name);
        video_device_free_string(name);
        return result;
    }

    bool IsInitialized() const {
        return m_initialized && m_device && video_device_is_initialized(m_device);
    }

    uint64_t GetGpuMemoryUsage() {
        if (!m_device || !m_initialized) return 0;
        return video_device_get_gpu_memory_usage(m_device);
    }

    // Operator bool for easy checking
    explicit operator bool() const {
        return IsInitialized();
    }
};

#endif // __cplusplus

#endif // GAME_ENGINE_VIDEO_DEVICE_H