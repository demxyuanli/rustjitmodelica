//! Minimal FMI 2.0 Co-Simulation host (libloading).

use super::parse::{ensure_binary, FmuDescription};
use libloading::Library;
use std::ffi::{c_char, c_void, CString};
use std::path::Path;

pub type Fmi2Status = i32;
pub type Fmi2Component = *mut c_void;

const FMI2_OK: Fmi2Status = 0;

#[repr(C)]
struct Fmi2CallbackFunctions {
    logger: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const c_char,
            i32,
            *const c_char,
            *const c_char,
        ),
    >,
    allocate_memory: Option<unsafe extern "C" fn(usize, usize) -> *mut c_void>,
    free_memory: Option<unsafe extern "C" fn(*mut c_void)>,
    step_finished: Option<unsafe extern "C" fn(*mut c_void, Fmi2Status)>,
    component_environment: *mut c_void,
}

unsafe extern "C" fn alloc_mem(nobj: usize, size: usize) -> *mut c_void {
    let total = nobj.saturating_mul(size);
    if total == 0 {
        return std::ptr::null_mut();
    }
    let layout = match std::alloc::Layout::from_size_align(total, 8) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    std::alloc::alloc_zeroed(layout) as *mut c_void
}

unsafe extern "C" fn free_mem(_obj: *mut c_void) {
    // FMI free without size; skip (process-lifetime allocations for host MVP).
}

type InstantiateFn = unsafe extern "C" fn(
    *const c_char,
    i32,
    *const c_char,
    *const c_char,
    *const Fmi2CallbackFunctions,
    i32,
    i32,
) -> Fmi2Component;
type SetupExperimentFn =
    unsafe extern "C" fn(Fmi2Component, i32, f64, f64, i32, f64) -> Fmi2Status;
type VoidStatusFn = unsafe extern "C" fn(Fmi2Component) -> Fmi2Status;
type SetRealFn =
    unsafe extern "C" fn(Fmi2Component, *const u32, usize, *const f64) -> Fmi2Status;
type GetRealFn =
    unsafe extern "C" fn(Fmi2Component, *const u32, usize, *mut f64) -> Fmi2Status;
type DoStepFn = unsafe extern "C" fn(Fmi2Component, f64, f64, i32) -> Fmi2Status;
type FreeInstanceFn = unsafe extern "C" fn(Fmi2Component);

pub struct FmuInstance {
    _lib: Library,
    component: Fmi2Component,
    setup_experiment: SetupExperimentFn,
    enter_init: VoidStatusFn,
    exit_init: VoidStatusFn,
    set_real: SetRealFn,
    get_real: GetRealFn,
    do_step: DoStepFn,
    terminate: VoidStatusFn,
    free_instance: FreeInstanceFn,
    pub desc: FmuDescription,
    pub inputs: Vec<super::parse::FmuScalar>,
    pub outputs: Vec<super::parse::FmuScalar>,
    pub parameters: Vec<super::parse::FmuScalar>,
}

impl FmuInstance {
    pub fn load(asset_dir: &Path, desc: FmuDescription) -> Result<Self, String> {
        let bin = ensure_binary(asset_dir, &desc.model_identifier)?;
        let lib =
            unsafe { Library::new(&bin) }.map_err(|e| format!("load {}: {e}", bin.display()))?;

        let prefix = desc.model_identifier.clone();
        let resolve = |name: &str| -> Result<*const (), String> {
            let mangled = format!("{prefix}_{name}");
            unsafe {
                if let Ok(sym) = lib.get::<*const ()>(mangled.as_bytes()) {
                    return Ok(*sym);
                }
                // Some toolchains export undecorated names.
                if let Ok(sym) = lib.get::<*const ()>(name.as_bytes()) {
                    return Ok(*sym);
                }
            }
            Err(format!("symbol not found: {mangled} / {name}"))
        };

        let instantiate: InstantiateFn = unsafe { std::mem::transmute(resolve("fmi2Instantiate")?) };
        let setup_experiment: SetupExperimentFn =
            unsafe { std::mem::transmute(resolve("fmi2SetupExperiment")?) };
        let enter_init: VoidStatusFn =
            unsafe { std::mem::transmute(resolve("fmi2EnterInitializationMode")?) };
        let exit_init: VoidStatusFn =
            unsafe { std::mem::transmute(resolve("fmi2ExitInitializationMode")?) };
        let set_real: SetRealFn = unsafe { std::mem::transmute(resolve("fmi2SetReal")?) };
        let get_real: GetRealFn = unsafe { std::mem::transmute(resolve("fmi2GetReal")?) };
        let do_step: DoStepFn = unsafe { std::mem::transmute(resolve("fmi2DoStep")?) };
        let terminate: VoidStatusFn = unsafe { std::mem::transmute(resolve("fmi2Terminate")?) };
        let free_instance: FreeInstanceFn =
            unsafe { std::mem::transmute(resolve("fmi2FreeInstance")?) };

        let callbacks = Fmi2CallbackFunctions {
            logger: None,
            allocate_memory: Some(alloc_mem),
            free_memory: Some(free_mem),
            step_finished: None,
            component_environment: std::ptr::null_mut(),
        };

        let instance_name = CString::new("studio_fmu").map_err(|e| e.to_string())?;
        let guid = CString::new(desc.guid.as_str()).map_err(|e| e.to_string())?;
        let resource = {
            let res = asset_dir.join("resources");
            let loc = if res.exists() {
                format!(
                    "file:///{}",
                    res.display().to_string().replace('\\', "/")
                )
            } else {
                format!(
                    "file:///{}",
                    asset_dir.display().to_string().replace('\\', "/")
                )
            };
            CString::new(loc).map_err(|e| e.to_string())?
        };

        let component = unsafe {
            instantiate(
                instance_name.as_ptr(),
                1, // fmi2CoSimulation
                guid.as_ptr(),
                resource.as_ptr(),
                &callbacks,
                0,
                0,
            )
        };
        if component.is_null() {
            return Err("fmi2Instantiate returned null".into());
        }

        let inputs: Vec<_> = desc
            .scalars
            .iter()
            .filter(|s| s.causality == "input")
            .cloned()
            .collect();
        let outputs: Vec<_> = desc
            .scalars
            .iter()
            .filter(|s| s.causality == "output")
            .cloned()
            .collect();
        let parameters: Vec<_> = desc
            .scalars
            .iter()
            .filter(|s| s.causality == "parameter")
            .cloned()
            .collect();

        Ok(Self {
            _lib: lib,
            component,
            setup_experiment,
            enter_init,
            exit_init,
            set_real,
            get_real,
            do_step,
            terminate,
            free_instance,
            desc,
            inputs,
            outputs,
            parameters,
        })
    }

    pub fn setup(&self, start: f64, stop: f64, tol: f64) -> Result<(), String> {
        check(
            unsafe { (self.setup_experiment)(self.component, 1, tol, start, 1, stop) },
            "SetupExperiment",
        )?;
        check(
            unsafe { (self.enter_init)(self.component) },
            "EnterInitializationMode",
        )?;
        Ok(())
    }

    pub fn set_parameters_from_map(&self, values: &[(u32, f64)]) -> Result<(), String> {
        if values.is_empty() {
            return Ok(());
        }
        let vrs: Vec<u32> = values.iter().map(|(vr, _)| *vr).collect();
        let vals: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
        check(
            unsafe {
                (self.set_real)(self.component, vrs.as_ptr(), vrs.len(), vals.as_ptr())
            },
            "SetReal(params)",
        )
    }

    pub fn exit_initialization(&self) -> Result<(), String> {
        check(
            unsafe { (self.exit_init)(self.component) },
            "ExitInitializationMode",
        )
    }

    pub fn set_inputs(&self, values: &[(u32, f64)]) -> Result<(), String> {
        if values.is_empty() {
            return Ok(());
        }
        let vrs: Vec<u32> = values.iter().map(|(vr, _)| *vr).collect();
        let vals: Vec<f64> = values.iter().map(|(_, v)| *v).collect();
        check(
            unsafe {
                (self.set_real)(self.component, vrs.as_ptr(), vrs.len(), vals.as_ptr())
            },
            "SetReal(inputs)",
        )
    }

    pub fn get_outputs(&self) -> Result<Vec<(u32, f64)>, String> {
        if self.outputs.is_empty() {
            return Ok(Vec::new());
        }
        let vrs: Vec<u32> = self.outputs.iter().map(|s| s.value_reference).collect();
        let mut vals = vec![0.0; vrs.len()];
        check(
            unsafe {
                (self.get_real)(
                    self.component,
                    vrs.as_ptr(),
                    vrs.len(),
                    vals.as_mut_ptr(),
                )
            },
            "GetReal(outputs)",
        )?;
        Ok(vrs.into_iter().zip(vals).collect())
    }

    pub fn do_step(&self, t: f64, dt: f64) -> Result<(), String> {
        check(
            unsafe { (self.do_step)(self.component, t, dt, 1) },
            "DoStep",
        )
    }
}

impl Drop for FmuInstance {
    fn drop(&mut self) {
        if !self.component.is_null() {
            unsafe {
                let _ = (self.terminate)(self.component);
                (self.free_instance)(self.component);
            }
        }
    }
}

fn check(status: Fmi2Status, what: &str) -> Result<(), String> {
    if status == FMI2_OK {
        Ok(())
    } else {
        Err(format!("FMI {what} status={status}"))
    }
}
