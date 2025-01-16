mod rerun_forwarder;
use libc::c_char;
use rerun_forwarder::RerunForwarder;
use std::ffi::CStr;

const ZOO_VISION_OK: u32 = 0;
const ZOO_VISION_ERROR: u32 = 1;

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn zoo_rs_init(client: *mut *mut RerunForwarder) -> u32 {
    let result = RerunForwarder::new();
    match result {
        Ok(c) => unsafe {
            *client = Box::into_raw(Box::new(c));
            ZOO_VISION_OK
        },
        Err(e) => {
            println!(
                "zoo_vision_rs: Error creating RerunForwarder!\nError: {}",
                e
            );
            ZOO_VISION_ERROR
        }
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn zoo_rs_test_me(p_client: *mut RerunForwarder, frame_id: *const c_char) -> u32 {
    if p_client.is_null() {
        return ZOO_VISION_ERROR;
    }

    let client: &mut RerunForwarder = unsafe { &mut *p_client };

    let frame_id = (unsafe { CStr::from_ptr(frame_id) }).to_str().unwrap();

    let result = client.test_me(frame_id);
    match result {
        Ok(_) => ZOO_VISION_OK,
        Err(e) => {
            println!("zoo_vision_rs: Error logging!\nError: {}", e);
            ZOO_VISION_ERROR
        }
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn zoo_rs_image_callback(
    p_client: *mut RerunForwarder,
    p_channel: *const c_char,
    msg: *const zoo_msgs::msg::rmw::Image12m,
) -> u32 {
    if p_client.is_null() {
        return ZOO_VISION_ERROR;
    }

    let client: &mut RerunForwarder = unsafe { &mut *p_client };
    let channel = unsafe { CStr::from_ptr(p_channel) }.to_str().unwrap();
    let msg = unsafe { &*msg };

    let result = client.image_callback(channel, msg);
    match result {
        Ok(_) => ZOO_VISION_OK,
        Err(e) => {
            println!("zoo_vision_rs: Error logging!\nError: {}", e);
            ZOO_VISION_ERROR
        }
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn zoo_rs_detection_callback(
    p_client: *mut RerunForwarder,
    p_channel: *const c_char,
    msg: *const zoo_msgs::msg::rmw::Detection,
) -> u32 {
    if p_client.is_null() {
        return ZOO_VISION_ERROR;
    }

    let client: &mut RerunForwarder = unsafe { &mut *p_client };
    let channel = unsafe { CStr::from_ptr(p_channel) }.to_str().unwrap();
    let msg = unsafe { &*msg };

    let result = client.detection_callback(channel, msg);
    match result {
        Ok(_) => ZOO_VISION_OK,
        Err(e) => {
            println!("zoo_vision_rs: Error logging!\nError: {}", e);
            ZOO_VISION_ERROR
        }
    }
}
