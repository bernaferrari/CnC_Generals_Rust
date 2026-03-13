//! COM utility functions mirroring WWLib `WWCOMUtil.h` / `WWCOMUtil.cpp`.
//!
//! Provides utilities for interacting with COM (Component Object Model) interfaces,
//! particularly IDispatch-based property access and method invocation.
//!
//! Note: The original C++ implementation uses Windows COM APIs directly. This Rust
//! port provides the interface and structure, but the actual COM interop is
//! platform-specific. On Windows, these would use the `windows` crate or similar.

/// COM operation result type.
/// Maps to Windows HRESULT values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComResult(pub i32);

impl ComResult {
    /// Operation succeeded.
    pub const S_OK: ComResult = ComResult(0);
    /// Operation failed with unspecified error.
    pub const E_FAIL: ComResult = ComResult(-2147467259); // 0x80004005
    /// Invalid argument.
    pub const E_INVALIDARG: ComResult = ComResult(-2147024809); // 0x80070057
    /// Interface not supported.
    pub const E_NOINTERFACE: ComResult = ComResult(-2147467262); // 0x80004002

    /// Check if the result indicates success.
    pub fn succeeded(self) -> bool {
        self.0 >= 0
    }

    /// Check if the result indicates failure.
    pub fn failed(self) -> bool {
        self.0 < 0
    }
}

/// Variant type for COM property values.
/// Simplified representation for cross-platform use.
#[derive(Clone, Debug, PartialEq)]
pub enum Variant {
    /// Empty/uninitialized variant.
    Empty,
    /// 32-bit integer value.
    I32(i32),
    /// 32-bit unsigned integer value.
    U32(u32),
    /// 64-bit integer value.
    I64(i64),
    /// 64-bit unsigned integer value.
    U64(u64),
    /// 32-bit floating point value.
    F32(f32),
    /// 64-bit floating point value.
    F64(f64),
    /// Boolean value.
    Bool(bool),
    /// String value.
    String(String),
    /// Raw bytes.
    Bytes(Vec<u8>),
}

impl Default for Variant {
    fn default() -> Self {
        Variant::Empty
    }
}

/// Trait for objects that support IDispatch-like property and method access.
///
/// This provides a cross-platform abstraction for COM IDispatch interfaces.
pub trait Dispatch {
    /// Get a property by name.
    fn get_property(&self, name: &str) -> Result<Variant, ComResult>;

    /// Set a property by name.
    fn put_property(&self, name: &str, value: Variant) -> Result<(), ComResult>;

    /// Invoke a method by name with parameters.
    fn invoke_method(&self, name: &str, params: &[Variant]) -> Result<Variant, ComResult>;
}

/// Invoke PropertyGet on IDispatch interface.
///
/// Matches C++ `Dispatch_GetProperty(IDispatch* object, const OLECHAR* propName, VARIANT* result)`.
///
/// # Arguments
///
/// * `object` - Dispatch interface reference.
/// * `prop_name` - Name of property to get.
///
/// # Returns
///
/// The property value, or an error code.
pub fn dispatch_get_property(object: &dyn Dispatch, prop_name: &str) -> Result<Variant, ComResult> {
    object.get_property(prop_name)
}

/// Invoke PropertyPut on IDispatch interface.
///
/// Matches C++ `Dispatch_PutProperty(IDispatch* object, const OLECHAR* propName, VARIANT* propValue)`.
///
/// # Arguments
///
/// * `object` - Dispatch interface reference.
/// * `prop_name` - Name of property to set.
/// * `prop_value` - Value to set.
pub fn dispatch_put_property(
    object: &dyn Dispatch,
    prop_name: &str,
    prop_value: Variant,
) -> Result<(), ComResult> {
    object.put_property(prop_name, prop_value)
}

/// Invoke Method on IDispatch interface.
///
/// Matches C++ `Dispatch_InvokeMethod(IDispatch* object, const OLECHAR* methodName, DISPPARAMS* params, VARIANT* result)`.
///
/// # Arguments
///
/// * `object` - Dispatch interface reference.
/// * `method_name` - Name of method to invoke.
/// * `params` - Method parameters.
///
/// # Returns
///
/// The method result, or an error code.
pub fn dispatch_invoke_method(
    object: &dyn Dispatch,
    method_name: &str,
    params: &[Variant],
) -> Result<Variant, ComResult> {
    object.invoke_method(method_name, params)
}

/// Register COM in-process DLL server.
///
/// Matches C++ `bool RegisterCOMServer(const char* dllName)`.
///
/// On Windows, this would load the DLL and call DllRegisterServer.
/// On other platforms, this is a no-op returning false.
///
/// # Arguments
///
/// * `_dll_name` - Name of DLL to register.
///
/// # Returns
///
/// True if operation successful.
pub fn register_com_server(_dll_name: &str) -> bool {
    // Windows-specific: LoadLibrary + GetProcAddress("DllRegisterServer")
    // On non-Windows platforms, COM is not available
    #[cfg(target_os = "windows")]
    {
        // Would use windows crate for LoadLibraryW / GetProcAddress
        // This is a placeholder for the actual Windows implementation
        false
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Unregister COM in-process DLL server.
///
/// Matches C++ `bool UnregisterCOMServer(const char* dllName)`.
///
/// On Windows, this would load the DLL and call DllUnregisterServer.
/// On other platforms, this is a no-op returning false.
///
/// # Arguments
///
/// * `_dll_name` - Name of DLL to unregister.
///
/// # Returns
///
/// True if operation successful.
pub fn unregister_com_server(_dll_name: &str) -> bool {
    // Windows-specific: LoadLibrary + GetProcAddress("DllUnregisterServer")
    // On non-Windows platforms, COM is not available
    #[cfg(target_os = "windows")]
    {
        // Would use windows crate for LoadLibraryW / GetProcAddress
        // This is a placeholder for the actual Windows implementation
        false
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDispatch {
        properties: std::collections::HashMap<String, Variant>,
        methods_called: std::cell::RefCell<Vec<String>>,
    }

    impl MockDispatch {
        fn new() -> Self {
            let mut props = std::collections::HashMap::new();
            props.insert("TestProp".to_string(), Variant::I32(42));
            MockDispatch {
                properties: props,
                methods_called: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl Dispatch for MockDispatch {
        fn get_property(&self, name: &str) -> Result<Variant, ComResult> {
            self.properties.get(name).cloned().ok_or(ComResult::E_FAIL)
        }

        fn put_property(&self, _name: &str, _value: Variant) -> Result<(), ComResult> {
            Ok(())
        }

        fn invoke_method(&self, name: &str, _params: &[Variant]) -> Result<Variant, ComResult> {
            self.methods_called.borrow_mut().push(name.to_string());
            Ok(Variant::I32(0))
        }
    }

    #[test]
    fn test_com_result_success() {
        assert!(ComResult::S_OK.succeeded());
        assert!(!ComResult::S_OK.failed());
    }

    #[test]
    fn test_com_result_failure() {
        assert!(ComResult::E_FAIL.failed());
        assert!(!ComResult::E_FAIL.succeeded());
    }

    #[test]
    fn test_variant_default() {
        let v = Variant::default();
        assert_eq!(v, Variant::Empty);
    }

    #[test]
    fn test_dispatch_get_property() {
        let mock = MockDispatch::new();
        let result = dispatch_get_property(&mock, "TestProp");
        assert_eq!(result.unwrap(), Variant::I32(42));
    }

    #[test]
    fn test_dispatch_get_property_not_found() {
        let mock = MockDispatch::new();
        let result = dispatch_get_property(&mock, "NonExistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_dispatch_invoke_method() {
        let mock = MockDispatch::new();
        let result = dispatch_invoke_method(&mock, "TestMethod", &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_register_com_server() {
        // Should return false on non-Windows, or on Windows without the DLL
        assert!(!register_com_server("nonexistent.dll"));
    }

    #[test]
    fn test_unregister_com_server() {
        assert!(!unregister_com_server("nonexistent.dll"));
    }
}
