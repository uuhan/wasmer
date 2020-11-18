use crate::exports::Exports;
use crate::externals::Extern;
use crate::module::Module;
use crate::store::Store;
use crate::{HostEnvInitError, LinkError, RuntimeError};
use std::fmt;
use thiserror::Error;
use wasmer_engine::Resolver;
use wasmer_vm::{InstanceHandle, VMContext};

/// A WebAssembly Instance is a stateful, executable
/// instance of a WebAssembly [`Module`].
///
/// Instance objects contain all the exported WebAssembly
/// functions, memories, tables and globals that allow
/// interacting with WebAssembly.
///
/// Spec: https://webassembly.github.io/spec/core/exec/runtime.html#module-instances
#[derive(Clone)]
pub struct Instance {
    handle: InstanceHandle,
    module: Module,
    /// The exports for an instance.
    pub exports: Exports,
}

#[cfg(test)]
mod send_test {
    use super::*;

    fn is_send<T: Send>() -> bool {
        true
    }
    #[test]
    fn instance_is_send() {
        assert!(is_send::<Instance>());
    }
}

/// An error while instantiating a module.
///
/// This is not a common WebAssembly error, however
/// we need to differentiate from a `LinkError` (an error
/// that happens while linking, on instantiation), a
/// Trap that occurs when calling the WebAssembly module
/// start function, and an error when initializing the user's
/// host environments.
#[derive(Error, Debug)]
pub enum InstantiationError {
    /// A linking ocurred during instantiation.
    #[error(transparent)]
    Link(LinkError),

    /// A runtime error occured while invoking the start function
    #[error(transparent)]
    Start(RuntimeError),

    /// Error occurred when initializing the host environment.
    #[error(transparent)]
    HostEnvInitialization(HostEnvInitError),
}

impl From<wasmer_engine::InstantiationError> for InstantiationError {
    fn from(other: wasmer_engine::InstantiationError) -> Self {
        match other {
            wasmer_engine::InstantiationError::Link(e) => Self::Link(e),
            wasmer_engine::InstantiationError::Start(e) => Self::Start(e),
        }
    }
}

impl From<HostEnvInitError> for InstantiationError {
    fn from(other: HostEnvInitError) -> Self {
        Self::HostEnvInitialization(other)
    }
}

impl Instance {
    /// Creates a new `Instance` from a WebAssembly [`Module`] and a
    /// set of imports resolved by the [`Resolver`].
    ///
    /// The resolver can be anything that implements the [`Resolver`] trait,
    /// so you can plug custom resolution for the imports, if you wish not
    /// to use [`ImportObject`].
    ///
    /// The [`ImportObject`] is the easiest way to provide imports to the instance.
    ///
    /// [`ImportObject`]: crate::ImportObject
    ///
    /// ```
    /// # use wasmer::{imports, Store, Module, Global, Value, Instance};
    /// # fn main() -> anyhow::Result<()> {
    /// let store = Store::default();
    /// let module = Module::new(&store, "(module)")?;
    /// let imports = imports!{
    ///   "host" => {
    ///     "var" => Global::new(&store, Value::I32(2))
    ///   }
    /// };
    /// let instance = Instance::new(&module, &imports)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Errors
    ///
    /// The function can return [`InstantiationError`]s.
    ///
    /// Those are, as defined by the spec:
    ///  * Link errors that happen when plugging the imports into the instance
    ///  * Runtime errors that happen when running the module `start` function.
    pub fn new(module: &Module, resolver: &dyn Resolver) -> Result<Self, InstantiationError> {
        let store = module.store();

        let handle = module.instantiate(resolver)?;

        let exports = module
            .exports()
            .map(|export| {
                let name = export.name().to_string();
                let export = handle.lookup(&name).expect("export");
                let extern_ = Extern::from_export(store, export);
                (name, extern_)
            })
            .collect::<Exports>();

        let mut instance = Self {
            handle,
            module: module.clone(),
            exports,
        };

        unsafe {
            instance
                .handle
                .initialize_host_envs::<HostEnvInitError>(&instance as *const _ as *const _)?;
        }

        Ok(instance)
    }

    /// Gets the [`Module`] associated with this instance.
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Returns the [`Store`] where the `Instance` belongs.
    pub fn store(&self) -> &Store {
        self.module.store()
    }

    #[doc(hidden)]
    pub fn vmctx_ptr(&self) -> *mut VMContext {
        self.handle.vmctx_ptr()
    }
}

impl fmt::Debug for Instance {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Instance")
            .field("exports", &self.exports)
            .finish()
    }
}
