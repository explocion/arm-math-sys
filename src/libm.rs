#[no_mangle]
pub extern "C" fn expf(x: f32) -> f32 {
    libm::expf(x)
}

#[no_mangle]
pub extern "C" fn log(x: f64) -> f64 {
    libm::log(x)
}

#[no_mangle]
pub extern "C" fn logf(x: f32) -> f32 {
    libm::logf(x)
}

#[no_mangle]
pub extern "C" fn powf(x: f32, y: f32) -> f32 {
    libm::powf(x, y)
}

#[no_mangle]
pub extern "C" fn sqrtf(x: f32) -> f32 {
    libm::sqrtf(x)
}

#[no_mangle]
pub extern "C" fn tanhf(x: f32) -> f32 {
    libm::tanhf(x)
}
