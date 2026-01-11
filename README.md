Кэширующий сервис для изображений.

Забирает на себя всю рутину по отдаче файлов на фронт

Функционал:
Here's the rewritten backlog in English:

- [x] Support for request proxying to image sources
    - When an image is not in cache, we fetch it from the primary backend
- [x] Serve static content (images only), with input file validation support
- [x] Automatically convert images to requested format
    - Just specify the format in the URL (e.g., https://<server>/<image_id>.<extension>)
- [x] Support on-the-fly image scaling
    - Default behavior - resize to specified dimensions using `width`, `height` parameters
- [ ] ~~Also support cropping, by specifying `mode=crop` parameter~~
    - Different approach implemented instead
- [x] Automatically configure proper static content delivery
    - Caching headers (ETag)
    - Browser-side caching (TTL)
    - Correct MIME types
- [x] In-memory caching
- [x] Disk caching?
    - ~~Cache by hashing all parameters into filenames,~~ implemented fjall embedded database instead (more efficient than direct file operations)
    - Enables fast disk data retrieval
- [x] Support for preloading images (via auth token)
- [ ] Ready Docker image and usage examples in compose
- [ ] Swagger/OpenAPI documentation
- [ ] Clear and documented error messages
- [x] Flexible configuration via .env
    - Compression level settings
    - x Cache size settings (by image count, by variation count)
    - x Enable/disable disk caching
- [ ] Clear README with quick start and all configuration explanations
    - Separate explanations for disk caching
- [ ] Support S3 as backend for persistent file storage
    - Note: Might not be optimal; on-the-fly cache processing likely faster
    - Also implies S3 as proxy source backend (if primary sources have issues)
- [ ] Benchmarks for different operational modes
- [x] Switch to https://github.com/Cykooz/fast_image_resize for resizing
- [x] ~~Support adaptive resizing (maintain aspect ratio when only one dimension specified)~~
    - ~~Only works when explicitly passing `type=adaptive`~~
    - Crop also changed to different approach
- [ ] Support various output formats (not just WebP)
- [ ] Support Redis cache (for larger deployments)
- [ ] ~~Refactor everything to use image container from file receipt~~
- [x] Add context parameter propagation to all logs (store request ID, pass image_id)
- [x] Add aspect ratio crop support
    - If source image has different aspect ratio than requested, control behavior via parameter:
        - `ratio_policy` (resize, crop_center)
        - Calculate projected aspect ratio and crop from center, then resize
- [ ] Example server + benchmark for performance monitoring
- [ ] Direct compression support
- [ ] Clean up all TODOs from code
- [ ] ETag support for dynamic content (and conditional requests)trol behaviour via config)