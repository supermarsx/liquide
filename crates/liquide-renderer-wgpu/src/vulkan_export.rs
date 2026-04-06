//! Vulkan DMA-BUF export for zero-copy GPU-to-encoder paths (Linux only).
//!
//! Creates Vulkan images with external memory export capability, allowing the
//! rendered frame to be imported directly by VAAPI without CPU copies.
//!
//! All Vulkan symbols are loaded at runtime via `dlopen` / `dlsym` so the
//! binary compiles on any platform and runs without link-time Vulkan deps.

#![allow(non_camel_case_types)]

#[cfg(target_os = "linux")]
mod inner {
    use std::ffi::c_void;
    use std::sync::OnceLock;

    // -----------------------------------------------------------------------
    // Vulkan C types (from vulkan_core.h)
    // -----------------------------------------------------------------------

    pub type VkInstance = u64;
    pub type VkPhysicalDevice = u64;
    pub type VkDevice = u64;
    pub type VkQueue = u64;
    pub type VkImage = u64;
    pub type VkDeviceMemory = u64;
    pub type VkResult = i32;
    pub type VkBool32 = u32;

    pub const VK_SUCCESS: VkResult = 0;
    pub const VK_NULL_HANDLE: u64 = 0;

    // Structure types (VkStructureType)
    const VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO: u32 = 1;
    const VK_STRUCTURE_TYPE_APPLICATION_INFO: u32 = 0;
    const VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO: u32 = 3;
    const VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO: u32 = 2;
    const VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO: u32 = 14;
    const VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO: u32 = 5;
    const VK_STRUCTURE_TYPE_EXPORT_MEMORY_ALLOCATE_INFO: u32 = 1000072002;
    const VK_STRUCTURE_TYPE_EXTERNAL_MEMORY_IMAGE_CREATE_INFO: u32 = 1000072001;
    const VK_STRUCTURE_TYPE_MEMORY_GET_FD_INFO_KHR: u32 = 1000074002;
    const VK_STRUCTURE_TYPE_MEMORY_FD_PROPERTIES_KHR: u32 = 1000074001;

    // VkExternalMemoryHandleTypeFlagBits
    const VK_EXTERNAL_MEMORY_HANDLE_TYPE_DMA_BUF_BIT_EXT: u32 = 0x0000_0200;

    // VkImageType
    const VK_IMAGE_TYPE_2D: u32 = 1;

    // VkFormat: BGRA 8-bit unorm (matches our framebuffer layout)
    const VK_FORMAT_B8G8R8A8_UNORM: u32 = 44;

    // VkImageTiling
    const VK_IMAGE_TILING_LINEAR: u32 = 0;

    // VkImageUsageFlags
    const VK_IMAGE_USAGE_TRANSFER_SRC_BIT: u32 = 0x0000_0001;
    const VK_IMAGE_USAGE_TRANSFER_DST_BIT: u32 = 0x0000_0002;

    // VkSharingMode
    const VK_SHARING_MODE_EXCLUSIVE: u32 = 0;

    // VkImageLayout
    const VK_IMAGE_LAYOUT_UNDEFINED: u32 = 0;

    // VkImageAspectFlags
    const VK_IMAGE_ASPECT_COLOR_BIT: u32 = 0x0000_0001;

    // VkMemoryPropertyFlags
    const VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT: u32 = 0x0000_0002;

    // -----------------------------------------------------------------------
    // Vulkan structures (minimal, repr C)
    // -----------------------------------------------------------------------

    #[repr(C)]
    struct VkApplicationInfo {
        s_type: u32,
        p_next: *const c_void,
        p_application_name: *const u8,
        application_version: u32,
        p_engine_name: *const u8,
        engine_version: u32,
        api_version: u32,
    }

    #[repr(C)]
    struct VkInstanceCreateInfo {
        s_type: u32,
        p_next: *const c_void,
        flags: u32,
        p_application_info: *const VkApplicationInfo,
        enabled_layer_count: u32,
        pp_enabled_layer_names: *const *const u8,
        enabled_extension_count: u32,
        pp_enabled_extension_names: *const *const u8,
    }

    #[repr(C)]
    struct VkDeviceQueueCreateInfo {
        s_type: u32,
        p_next: *const c_void,
        flags: u32,
        queue_family_index: u32,
        queue_count: u32,
        p_queue_priorities: *const f32,
    }

    #[repr(C)]
    struct VkDeviceCreateInfo {
        s_type: u32,
        p_next: *const c_void,
        flags: u32,
        queue_create_info_count: u32,
        p_queue_create_infos: *const VkDeviceQueueCreateInfo,
        enabled_layer_count: u32,
        pp_enabled_layer_names: *const *const u8,
        enabled_extension_count: u32,
        pp_enabled_extension_names: *const *const u8,
        p_enabled_features: *const c_void,
    }

    #[repr(C)]
    struct VkExtensionProperties {
        extension_name: [u8; 256],
        spec_version: u32,
    }

    #[repr(C)]
    struct VkExternalMemoryImageCreateInfo {
        s_type: u32,
        p_next: *const c_void,
        handle_types: u32,
    }

    #[repr(C)]
    struct VkImageCreateInfo {
        s_type: u32,
        p_next: *const c_void,
        flags: u32,
        image_type: u32,
        format: u32,
        extent_width: u32,
        extent_height: u32,
        extent_depth: u32,
        mip_levels: u32,
        array_layers: u32,
        samples: u32, // VK_SAMPLE_COUNT_1_BIT = 1
        tiling: u32,
        usage: u32,
        sharing_mode: u32,
        queue_family_index_count: u32,
        p_queue_family_indices: *const u32,
        initial_layout: u32,
    }

    #[repr(C)]
    struct VkMemoryRequirements {
        size: u64,
        alignment: u64,
        memory_type_bits: u32,
    }

    #[repr(C)]
    struct VkExportMemoryAllocateInfo {
        s_type: u32,
        p_next: *const c_void,
        handle_types: u32,
    }

    #[repr(C)]
    struct VkMemoryAllocateInfo {
        s_type: u32,
        p_next: *const c_void,
        allocation_size: u64,
        memory_type_index: u32,
    }

    #[repr(C)]
    struct VkMemoryGetFdInfoKHR {
        s_type: u32,
        p_next: *const c_void,
        memory: VkDeviceMemory,
        handle_type: u32,
    }

    #[repr(C)]
    struct VkImageSubresource {
        aspect_mask: u32,
        mip_level: u32,
        array_layer: u32,
    }

    #[repr(C)]
    struct VkSubresourceLayout {
        offset: u64,
        size: u64,
        row_pitch: u64,
        array_pitch: u64,
        depth_pitch: u64,
    }

    #[repr(C)]
    struct VkPhysicalDeviceMemoryProperties {
        memory_type_count: u32,
        memory_types: [VkMemoryType; 32],
        memory_heap_count: u32,
        memory_heaps: [VkMemoryHeap; 16],
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct VkMemoryType {
        property_flags: u32,
        heap_index: u32,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct VkMemoryHeap {
        size: u64,
        flags: u32,
    }

    // -----------------------------------------------------------------------
    // Dynamically-loaded Vulkan function table
    // -----------------------------------------------------------------------

    type PFN_vkVoidFunction = *mut c_void;

    type PFN_vkGetInstanceProcAddr =
        unsafe extern "C" fn(VkInstance, *const u8) -> PFN_vkVoidFunction;
    type PFN_vkCreateInstance =
        unsafe extern "C" fn(*const VkInstanceCreateInfo, *const c_void, *mut VkInstance) -> VkResult;
    type PFN_vkDestroyInstance =
        unsafe extern "C" fn(VkInstance, *const c_void);
    type PFN_vkEnumeratePhysicalDevices =
        unsafe extern "C" fn(VkInstance, *mut u32, *mut VkPhysicalDevice) -> VkResult;
    type PFN_vkEnumerateDeviceExtensionProperties =
        unsafe extern "C" fn(VkPhysicalDevice, *const u8, *mut u32, *mut VkExtensionProperties) -> VkResult;
    type PFN_vkCreateDevice =
        unsafe extern "C" fn(VkPhysicalDevice, *const VkDeviceCreateInfo, *const c_void, *mut VkDevice) -> VkResult;
    type PFN_vkDestroyDevice =
        unsafe extern "C" fn(VkDevice, *const c_void);
    type PFN_vkGetDeviceQueue =
        unsafe extern "C" fn(VkDevice, u32, u32, *mut VkQueue);
    type PFN_vkCreateImage =
        unsafe extern "C" fn(VkDevice, *const VkImageCreateInfo, *const c_void, *mut VkImage) -> VkResult;
    type PFN_vkDestroyImage =
        unsafe extern "C" fn(VkDevice, VkImage, *const c_void);
    type PFN_vkGetImageMemoryRequirements =
        unsafe extern "C" fn(VkDevice, VkImage, *mut VkMemoryRequirements);
    type PFN_vkAllocateMemory =
        unsafe extern "C" fn(VkDevice, *const VkMemoryAllocateInfo, *const c_void, *mut VkDeviceMemory) -> VkResult;
    type PFN_vkFreeMemory =
        unsafe extern "C" fn(VkDevice, VkDeviceMemory, *const c_void);
    type PFN_vkBindImageMemory =
        unsafe extern "C" fn(VkDevice, VkImage, VkDeviceMemory, u64) -> VkResult;
    type PFN_vkGetImageSubresourceLayout =
        unsafe extern "C" fn(VkDevice, VkImage, *const VkImageSubresource, *mut VkSubresourceLayout);
    type PFN_vkGetPhysicalDeviceMemoryProperties =
        unsafe extern "C" fn(VkPhysicalDevice, *mut VkPhysicalDeviceMemoryProperties);
    // Extension: VK_KHR_external_memory_fd
    type PFN_vkGetMemoryFdKHR =
        unsafe extern "C" fn(VkDevice, *const VkMemoryGetFdInfoKHR, *mut i32) -> VkResult;
    // Device-level GetDeviceProcAddr for extension functions
    type PFN_vkGetDeviceProcAddr =
        unsafe extern "C" fn(VkDevice, *const u8) -> PFN_vkVoidFunction;

    struct VkLib {
        _handle: *mut c_void,
        get_instance_proc_addr: PFN_vkGetInstanceProcAddr,
    }

    // Safety: VkLib holds a dlopen handle and a function pointer; both are
    // just addresses and are safe to share across threads.
    unsafe impl Send for VkLib {}
    unsafe impl Sync for VkLib {}

    static VK_LIB: OnceLock<Option<VkLib>> = OnceLock::new();

    impl VkLib {
        fn load() -> Option<&'static VkLib> {
            VK_LIB
                .get_or_init(|| Self::try_load())
                .as_ref()
        }

        fn try_load() -> Option<VkLib> {
            extern "C" {
                fn dlopen(filename: *const u8, flags: i32) -> *mut c_void;
                fn dlsym(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
            }

            const RTLD_NOW: i32 = 0x0002;
            const RTLD_LOCAL: i32 = 0;

            let handle = unsafe {
                dlopen(b"libvulkan.so.1\0".as_ptr(), RTLD_NOW | RTLD_LOCAL)
            };
            if handle.is_null() {
                return None;
            }

            let get_instance_proc_addr: PFN_vkGetInstanceProcAddr = unsafe {
                let p = dlsym(handle, b"vkGetInstanceProcAddr\0".as_ptr());
                if p.is_null() {
                    return None;
                }
                std::mem::transmute(p)
            };

            Some(VkLib {
                _handle: handle,
                get_instance_proc_addr,
            })
        }

        /// Resolve an instance-level (or global) Vulkan function.
        unsafe fn get_proc(&self, instance: VkInstance, name: &[u8]) -> *mut c_void {
            // name must be null-terminated.
            unsafe { (self.get_instance_proc_addr)(instance, name.as_ptr()) }
        }
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// A GPU texture that can be exported as a DMA-BUF file descriptor.
    pub struct ExportableImage {
        pub width: u32,
        pub height: u32,
        /// The DMA-BUF file descriptor (owned). Caller must `close()` when done.
        pub dmabuf_fd: i32,
        /// Row stride in bytes.
        pub stride: u32,
        /// Total allocation size in bytes.
        pub size: u64,
        // Internal Vulkan handles for cleanup.
        image: VkImage,
        memory: VkDeviceMemory,
    }

    /// Vulkan DMA-BUF exporter.
    ///
    /// Manages a Vulkan instance + device with `VK_KHR_external_memory_fd` and
    /// `VK_EXT_external_memory_dma_buf` enabled, allowing GPU images to be
    /// exported as DMA-BUF file descriptors for zero-copy handoff to VAAPI.
    pub struct VulkanExporter {
        instance: VkInstance,
        device: VkDevice,
        physical_device: VkPhysicalDevice,
        _queue: VkQueue,
        supports_dmabuf: bool,

        // Cached function pointers (device-level)
        fn_create_image: PFN_vkCreateImage,
        fn_destroy_image: PFN_vkDestroyImage,
        fn_get_image_memory_requirements: PFN_vkGetImageMemoryRequirements,
        fn_allocate_memory: PFN_vkAllocateMemory,
        fn_free_memory: PFN_vkFreeMemory,
        fn_bind_image_memory: PFN_vkBindImageMemory,
        fn_get_image_subresource_layout: PFN_vkGetImageSubresourceLayout,
        fn_get_physical_device_memory_properties: PFN_vkGetPhysicalDeviceMemoryProperties,
        fn_get_memory_fd_khr: PFN_vkGetMemoryFdKHR,
        fn_destroy_device: PFN_vkDestroyDevice,
        fn_destroy_instance: PFN_vkDestroyInstance,
    }

    // Safety: All fields are Vulkan handles (u64 integers) or function pointers.
    // Vulkan itself requires external synchronization on a per-object basis, and
    // we only access the device from one thread at a time.
    unsafe impl Send for VulkanExporter {}

    impl VulkanExporter {
        /// Try to create a Vulkan DMA-BUF exporter.
        ///
        /// Returns `None` if Vulkan is not available, or if the required
        /// extensions (`VK_KHR_external_memory_fd`, `VK_EXT_external_memory_dma_buf`)
        /// are not supported by any physical device.
        pub fn new() -> Option<Self> {
            let lib = VkLib::load()?;

            // --- resolve global functions ---
            let vk_create_instance: PFN_vkCreateInstance = unsafe {
                std::mem::transmute(lib.get_proc(VK_NULL_HANDLE, b"vkCreateInstance\0"))
            };
            if (vk_create_instance as *const c_void).is_null() {
                return None;
            }

            // --- create instance ---
            let app_info = VkApplicationInfo {
                s_type: VK_STRUCTURE_TYPE_APPLICATION_INFO,
                p_next: std::ptr::null(),
                p_application_name: b"liquide-dmabuf-export\0".as_ptr(),
                application_version: 1,
                p_engine_name: b"liquide\0".as_ptr(),
                engine_version: 1,
                api_version: (1 << 22) | (1 << 12), // Vulkan 1.1
            };

            // We need VK_KHR_external_memory_capabilities at instance level.
            let instance_exts: [*const u8; 1] = [
                b"VK_KHR_external_memory_capabilities\0".as_ptr(),
            ];

            let create_info = VkInstanceCreateInfo {
                s_type: VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: 0,
                p_application_info: &app_info,
                enabled_layer_count: 0,
                pp_enabled_layer_names: std::ptr::null(),
                enabled_extension_count: instance_exts.len() as u32,
                pp_enabled_extension_names: instance_exts.as_ptr(),
            };

            let mut instance: VkInstance = VK_NULL_HANDLE;
            let res = unsafe { vk_create_instance(&create_info, std::ptr::null(), &mut instance) };
            if res != VK_SUCCESS || instance == VK_NULL_HANDLE {
                return None;
            }

            // --- resolve instance-level functions ---
            macro_rules! ifn {
                ($name:literal) => {{
                    let p = unsafe { lib.get_proc(instance, concat!($name, "\0").as_bytes().as_ptr() as *const u8) };
                    // We build the name with a trailing nul above via concat.
                    // Actually, get_proc expects the slice to be nul-terminated.
                    // Let's use a byte literal directly.
                    if p.is_null() {
                        // Cleanup instance on failure.
                        let destroy: PFN_vkDestroyInstance = unsafe {
                            std::mem::transmute(lib.get_proc(instance, b"vkDestroyInstance\0"))
                        };
                        if !(destroy as *const c_void).is_null() {
                            unsafe { destroy(instance, std::ptr::null()) };
                        }
                        return None;
                    }
                    unsafe { std::mem::transmute(p) }
                }};
            }

            // We need a cleaner approach for function resolution. Let's load
            // each function with proper null-terminated byte strings.
            let fn_enumerate_physical_devices: PFN_vkEnumeratePhysicalDevices = unsafe {
                let p = lib.get_proc(instance, b"vkEnumeratePhysicalDevices\0");
                if p.is_null() {
                    let d: PFN_vkDestroyInstance = std::mem::transmute(lib.get_proc(instance, b"vkDestroyInstance\0"));
                    if !(d as *const c_void).is_null() { d(instance, std::ptr::null()); }
                    return None;
                }
                std::mem::transmute(p)
            };

            let fn_enumerate_device_ext_props: PFN_vkEnumerateDeviceExtensionProperties = unsafe {
                let p = lib.get_proc(instance, b"vkEnumerateDeviceExtensionProperties\0");
                if p.is_null() {
                    let d: PFN_vkDestroyInstance = std::mem::transmute(lib.get_proc(instance, b"vkDestroyInstance\0"));
                    if !(d as *const c_void).is_null() { d(instance, std::ptr::null()); }
                    return None;
                }
                std::mem::transmute(p)
            };

            let fn_create_device: PFN_vkCreateDevice = unsafe {
                let p = lib.get_proc(instance, b"vkCreateDevice\0");
                if p.is_null() {
                    let d: PFN_vkDestroyInstance = std::mem::transmute(lib.get_proc(instance, b"vkDestroyInstance\0"));
                    if !(d as *const c_void).is_null() { d(instance, std::ptr::null()); }
                    return None;
                }
                std::mem::transmute(p)
            };

            let fn_get_device_proc_addr: PFN_vkGetDeviceProcAddr = unsafe {
                let p = lib.get_proc(instance, b"vkGetDeviceProcAddr\0");
                if p.is_null() {
                    let d: PFN_vkDestroyInstance = std::mem::transmute(lib.get_proc(instance, b"vkDestroyInstance\0"));
                    if !(d as *const c_void).is_null() { d(instance, std::ptr::null()); }
                    return None;
                }
                std::mem::transmute(p)
            };

            let fn_destroy_instance: PFN_vkDestroyInstance = unsafe {
                let p = lib.get_proc(instance, b"vkDestroyInstance\0");
                if p.is_null() { return None; }
                std::mem::transmute(p)
            };

            let fn_get_device_queue: PFN_vkGetDeviceQueue = unsafe {
                let p = lib.get_proc(instance, b"vkGetDeviceQueue\0");
                if p.is_null() {
                    unsafe { fn_destroy_instance(instance, std::ptr::null()) };
                    return None;
                }
                std::mem::transmute(p)
            };

            let fn_get_phys_mem_props: PFN_vkGetPhysicalDeviceMemoryProperties = unsafe {
                let p = lib.get_proc(instance, b"vkGetPhysicalDeviceMemoryProperties\0");
                if p.is_null() {
                    unsafe { fn_destroy_instance(instance, std::ptr::null()) };
                    return None;
                }
                std::mem::transmute(p)
            };

            // --- enumerate physical devices ---
            let mut count: u32 = 0;
            let res = unsafe {
                fn_enumerate_physical_devices(instance, &mut count, std::ptr::null_mut())
            };
            if res != VK_SUCCESS || count == 0 {
                unsafe { fn_destroy_instance(instance, std::ptr::null()) };
                return None;
            }

            let mut physical_devices = vec![VK_NULL_HANDLE; count as usize];
            let res = unsafe {
                fn_enumerate_physical_devices(instance, &mut count, physical_devices.as_mut_ptr())
            };
            if res != VK_SUCCESS {
                unsafe { fn_destroy_instance(instance, std::ptr::null()) };
                return None;
            }

            // --- find a device that supports the DMA-BUF export extensions ---
            let required_exts: [&[u8]; 2] = [
                b"VK_KHR_external_memory_fd",
                b"VK_EXT_external_memory_dma_buf",
            ];

            let mut chosen_phys = VK_NULL_HANDLE;
            for &phys in &physical_devices {
                let mut ext_count: u32 = 0;
                let r = unsafe {
                    fn_enumerate_device_ext_props(phys, std::ptr::null(), &mut ext_count, std::ptr::null_mut())
                };
                if r != VK_SUCCESS || ext_count == 0 {
                    continue;
                }

                let mut exts = vec![
                    VkExtensionProperties {
                        extension_name: [0u8; 256],
                        spec_version: 0,
                    };
                    ext_count as usize
                ];
                let r = unsafe {
                    fn_enumerate_device_ext_props(phys, std::ptr::null(), &mut ext_count, exts.as_mut_ptr())
                };
                if r != VK_SUCCESS {
                    continue;
                }

                let mut found = [false; 2];
                for ext in &exts[..ext_count as usize] {
                    for (i, req) in required_exts.iter().enumerate() {
                        if ext.extension_name.starts_with(req)
                            && ext.extension_name[req.len()] == 0
                        {
                            found[i] = true;
                        }
                    }
                }
                if found.iter().all(|f| *f) {
                    chosen_phys = phys;
                    break;
                }
            }

            if chosen_phys == VK_NULL_HANDLE {
                unsafe { fn_destroy_instance(instance, std::ptr::null()) };
                return None;
            }

            // --- create logical device with the extensions ---
            let queue_priority: f32 = 1.0;
            let queue_ci = VkDeviceQueueCreateInfo {
                s_type: VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: 0,
                queue_family_index: 0,
                queue_count: 1,
                p_queue_priorities: &queue_priority,
            };

            let device_ext_names: [*const u8; 2] = [
                b"VK_KHR_external_memory_fd\0".as_ptr(),
                b"VK_EXT_external_memory_dma_buf\0".as_ptr(),
            ];

            let device_ci = VkDeviceCreateInfo {
                s_type: VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: 0,
                queue_create_info_count: 1,
                p_queue_create_infos: &queue_ci,
                enabled_layer_count: 0,
                pp_enabled_layer_names: std::ptr::null(),
                enabled_extension_count: device_ext_names.len() as u32,
                pp_enabled_extension_names: device_ext_names.as_ptr(),
                p_enabled_features: std::ptr::null(),
            };

            let mut device: VkDevice = VK_NULL_HANDLE;
            let res = unsafe {
                fn_create_device(chosen_phys, &device_ci, std::ptr::null(), &mut device)
            };
            if res != VK_SUCCESS || device == VK_NULL_HANDLE {
                unsafe { fn_destroy_instance(instance, std::ptr::null()) };
                return None;
            }

            let mut queue: VkQueue = VK_NULL_HANDLE;
            unsafe { fn_get_device_queue(device, 0, 0, &mut queue) };

            // --- resolve device-level functions ---
            macro_rules! dfn {
                ($name:literal) => {{
                    let p = unsafe { fn_get_device_proc_addr(device, $name.as_ptr()) };
                    if p.is_null() {
                        let dd: PFN_vkDestroyDevice = unsafe {
                            std::mem::transmute(fn_get_device_proc_addr(device, b"vkDestroyDevice\0".as_ptr()))
                        };
                        if !(dd as *const c_void).is_null() {
                            unsafe { dd(device, std::ptr::null()) };
                        }
                        unsafe { fn_destroy_instance(instance, std::ptr::null()) };
                        return None;
                    }
                    unsafe { std::mem::transmute(p) }
                }};
            }

            let fn_create_image: PFN_vkCreateImage = dfn!(b"vkCreateImage\0");
            let fn_destroy_image: PFN_vkDestroyImage = dfn!(b"vkDestroyImage\0");
            let fn_get_image_memory_requirements: PFN_vkGetImageMemoryRequirements =
                dfn!(b"vkGetImageMemoryRequirements\0");
            let fn_allocate_memory: PFN_vkAllocateMemory = dfn!(b"vkAllocateMemory\0");
            let fn_free_memory: PFN_vkFreeMemory = dfn!(b"vkFreeMemory\0");
            let fn_bind_image_memory: PFN_vkBindImageMemory = dfn!(b"vkBindImageMemory\0");
            let fn_get_image_subresource_layout: PFN_vkGetImageSubresourceLayout =
                dfn!(b"vkGetImageSubresourceLayout\0");
            let fn_get_memory_fd_khr: PFN_vkGetMemoryFdKHR = dfn!(b"vkGetMemoryFdKHR\0");
            let fn_destroy_device: PFN_vkDestroyDevice = dfn!(b"vkDestroyDevice\0");

            log::info!("Vulkan DMA-BUF exporter initialized");

            Some(Self {
                instance,
                device,
                physical_device: chosen_phys,
                _queue: queue,
                supports_dmabuf: true,
                fn_create_image,
                fn_destroy_image,
                fn_get_image_memory_requirements,
                fn_allocate_memory,
                fn_free_memory,
                fn_bind_image_memory,
                fn_get_image_subresource_layout,
                fn_get_physical_device_memory_properties: fn_get_phys_mem_props,
                fn_get_memory_fd_khr,
                fn_destroy_device,
                fn_destroy_instance,
            })
        }

        /// Whether DMA-BUF export is available on this device.
        pub fn supports_dmabuf(&self) -> bool {
            self.supports_dmabuf
        }

        /// Find a memory type index that satisfies both the type bitmask and
        /// the required property flags.
        fn find_memory_type(&self, type_bits: u32, required_flags: u32) -> Option<u32> {
            let mut props = std::mem::MaybeUninit::<VkPhysicalDeviceMemoryProperties>::uninit();
            unsafe {
                (self.fn_get_physical_device_memory_properties)(
                    self.physical_device,
                    props.as_mut_ptr(),
                );
            }
            let props = unsafe { props.assume_init() };

            for i in 0..props.memory_type_count {
                if (type_bits & (1 << i)) != 0
                    && (props.memory_types[i as usize].property_flags & required_flags)
                        == required_flags
                {
                    return Some(i);
                }
            }
            None
        }

        /// Create a GPU image that can be exported as a DMA-BUF fd.
        ///
        /// The returned [`ExportableImage`] owns the DMA-BUF fd and Vulkan
        /// handles. Call [`destroy_image`](Self::destroy_image) to free them.
        pub fn create_exportable_image(
            &self,
            width: u32,
            height: u32,
        ) -> Option<ExportableImage> {
            if !self.supports_dmabuf {
                return None;
            }

            // -- VkExternalMemoryImageCreateInfo (chained to image create) --
            let ext_mem_info = VkExternalMemoryImageCreateInfo {
                s_type: VK_STRUCTURE_TYPE_EXTERNAL_MEMORY_IMAGE_CREATE_INFO,
                p_next: std::ptr::null(),
                handle_types: VK_EXTERNAL_MEMORY_HANDLE_TYPE_DMA_BUF_BIT_EXT,
            };

            let image_ci = VkImageCreateInfo {
                s_type: VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
                p_next: &ext_mem_info as *const _ as *const c_void,
                flags: 0,
                image_type: VK_IMAGE_TYPE_2D,
                format: VK_FORMAT_B8G8R8A8_UNORM,
                extent_width: width,
                extent_height: height,
                extent_depth: 1,
                mip_levels: 1,
                array_layers: 1,
                samples: 1, // VK_SAMPLE_COUNT_1_BIT
                tiling: VK_IMAGE_TILING_LINEAR,
                usage: VK_IMAGE_USAGE_TRANSFER_SRC_BIT | VK_IMAGE_USAGE_TRANSFER_DST_BIT,
                sharing_mode: VK_SHARING_MODE_EXCLUSIVE,
                queue_family_index_count: 0,
                p_queue_family_indices: std::ptr::null(),
                initial_layout: VK_IMAGE_LAYOUT_UNDEFINED,
            };

            let mut image: VkImage = VK_NULL_HANDLE;
            let res = unsafe {
                (self.fn_create_image)(self.device, &image_ci, std::ptr::null(), &mut image)
            };
            if res != VK_SUCCESS || image == VK_NULL_HANDLE {
                return None;
            }

            // -- query memory requirements --
            let mut mem_reqs = std::mem::MaybeUninit::<VkMemoryRequirements>::uninit();
            unsafe {
                (self.fn_get_image_memory_requirements)(self.device, image, mem_reqs.as_mut_ptr());
            }
            let mem_reqs = unsafe { mem_reqs.assume_init() };

            // Find a host-visible memory type (needed for linear tiling).
            let mem_type_index = match self.find_memory_type(
                mem_reqs.memory_type_bits,
                VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT,
            ) {
                Some(i) => i,
                None => {
                    unsafe { (self.fn_destroy_image)(self.device, image, std::ptr::null()) };
                    return None;
                }
            };

            // -- allocate with export info --
            let export_info = VkExportMemoryAllocateInfo {
                s_type: VK_STRUCTURE_TYPE_EXPORT_MEMORY_ALLOCATE_INFO,
                p_next: std::ptr::null(),
                handle_types: VK_EXTERNAL_MEMORY_HANDLE_TYPE_DMA_BUF_BIT_EXT,
            };

            let alloc_info = VkMemoryAllocateInfo {
                s_type: VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
                p_next: &export_info as *const _ as *const c_void,
                allocation_size: mem_reqs.size,
                memory_type_index: mem_type_index,
            };

            let mut memory: VkDeviceMemory = VK_NULL_HANDLE;
            let res = unsafe {
                (self.fn_allocate_memory)(self.device, &alloc_info, std::ptr::null(), &mut memory)
            };
            if res != VK_SUCCESS || memory == VK_NULL_HANDLE {
                unsafe { (self.fn_destroy_image)(self.device, image, std::ptr::null()) };
                return None;
            }

            // -- bind memory to image --
            let res = unsafe {
                (self.fn_bind_image_memory)(self.device, image, memory, 0)
            };
            if res != VK_SUCCESS {
                unsafe {
                    (self.fn_free_memory)(self.device, memory, std::ptr::null());
                    (self.fn_destroy_image)(self.device, image, std::ptr::null());
                }
                return None;
            }

            // -- get DMA-BUF fd via vkGetMemoryFdKHR --
            let fd_info = VkMemoryGetFdInfoKHR {
                s_type: VK_STRUCTURE_TYPE_MEMORY_GET_FD_INFO_KHR,
                p_next: std::ptr::null(),
                memory,
                handle_type: VK_EXTERNAL_MEMORY_HANDLE_TYPE_DMA_BUF_BIT_EXT,
            };

            let mut fd: i32 = -1;
            let res = unsafe {
                (self.fn_get_memory_fd_khr)(self.device, &fd_info, &mut fd)
            };
            if res != VK_SUCCESS || fd < 0 {
                unsafe {
                    (self.fn_free_memory)(self.device, memory, std::ptr::null());
                    (self.fn_destroy_image)(self.device, image, std::ptr::null());
                }
                return None;
            }

            // -- query stride from subresource layout --
            let subresource = VkImageSubresource {
                aspect_mask: VK_IMAGE_ASPECT_COLOR_BIT,
                mip_level: 0,
                array_layer: 0,
            };
            let mut layout = std::mem::MaybeUninit::<VkSubresourceLayout>::uninit();
            unsafe {
                (self.fn_get_image_subresource_layout)(
                    self.device,
                    image,
                    &subresource,
                    layout.as_mut_ptr(),
                );
            }
            let layout = unsafe { layout.assume_init() };

            Some(ExportableImage {
                width,
                height,
                dmabuf_fd: fd,
                stride: layout.row_pitch as u32,
                size: mem_reqs.size,
                image,
                memory,
            })
        }

        /// Destroy an exportable image, freeing its Vulkan memory and handle.
        ///
        /// The caller is responsible for closing the `dmabuf_fd` separately
        /// (it remains valid after Vulkan resources are freed until explicitly
        /// closed).
        pub fn destroy_image(&self, img: &ExportableImage) {
            unsafe {
                (self.fn_free_memory)(self.device, img.memory, std::ptr::null());
                (self.fn_destroy_image)(self.device, img.image, std::ptr::null());
            }
        }
    }

    impl Drop for VulkanExporter {
        fn drop(&mut self) {
            unsafe {
                (self.fn_destroy_device)(self.device, std::ptr::null());
                (self.fn_destroy_instance)(self.instance, std::ptr::null());
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub use inner::*;

// ---------------------------------------------------------------------------
// Stubs for non-Linux platforms
// ---------------------------------------------------------------------------

/// Stub exporter on non-Linux platforms (always unavailable).
#[cfg(not(target_os = "linux"))]
pub struct VulkanExporter;

#[cfg(not(target_os = "linux"))]
impl VulkanExporter {
    /// Always returns `None` on non-Linux.
    pub fn new() -> Option<Self> {
        None
    }

    /// Always returns `false` on non-Linux.
    pub fn supports_dmabuf(&self) -> bool {
        false
    }
}
