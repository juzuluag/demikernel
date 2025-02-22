// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

//======================================================================================================================
// Imports
//======================================================================================================================

use crate::{
    demikernel::libos::{
        name::LibOSName,
        LibOS,
    },
    pal::{
        constants::AF_INET,
        data_structures::{
            SockAddrIn,
            Socklen,
        },
        functions::get_addr_from_sock_addr_in,
    },
    runtime::{
        fail::Fail,
        logging,
        types::{
            demi_qresult_t,
            demi_qtoken_t,
            demi_sgarray_t,
            demi_sgaseg_t,
        },
        QToken,
    },
};
use ::libc::{
    c_char,
    c_int,
    c_void,
    sockaddr,
};
use ::std::{
    cell::RefCell,
    ffi::CStr,
    mem,
    net::{
        Ipv4Addr,
        SocketAddrV4,
    },
    ptr,
    slice,
    time::{
        Duration,
        SystemTime,
    },
};

//======================================================================================================================
// DEMIKERNEL
//======================================================================================================================

/// Demikernel state.
static mut DEMIKERNEL: RefCell<Option<LibOS>> = RefCell::new(None);

//======================================================================================================================
// init
//======================================================================================================================

#[allow(unused)]
#[no_mangle]
pub extern "C" fn demi_init(argc: c_int, argv: *mut *mut c_char) -> c_int {
    logging::initialize();
    trace!("demi_init()");

    let libos_name: LibOSName = match LibOSName::from_env() {
        Ok(libos_name) => libos_name.into(),
        Err(e) => panic!("{:?}", e),
    };

    // TODO: Pass arguments to the underlying libOS.
    let libos: LibOS = match LibOS::new(libos_name) {
        Ok(libos) => libos,
        Err(e) => {
            trace!("demi_init() failed: {:?}", e);
            return -e.errno;
        },
    };

    unsafe { DEMIKERNEL = RefCell::new(Some(libos)) };

    0
}

//======================================================================================================================
// create
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_create_pipe(memqd_out: *mut c_int, name: *const libc::c_char) -> c_int {
    trace!("demi_create_pipe() memqd_out={:?}, name={:?}", memqd_out, name);

    // Convert C string to a Rust one.
    let name: &str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return libc::EINVAL,
    };

    // Issue socket operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.create_pipe(name) {
        Ok(qd) => {
            unsafe { *memqd_out = qd.into() };
            0
        },
        Err(e) => {
            trace!("demi_create_pipe() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// open
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_open_pipe(memqd_out: *mut c_int, name: *const libc::c_char) -> c_int {
    trace!("demi_open_pipe() memqd_out={:?}, name={:?}", memqd_out, name);

    // Convert C string to a Rust one.
    let name: &str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return libc::EINVAL,
    };

    // Issue socket operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.open_pipe(name) {
        Ok(qd) => {
            unsafe { *memqd_out = qd.into() };
            0
        },
        Err(e) => {
            trace!("demi_open_pipe() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// socket
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_socket(qd_out: *mut c_int, domain: c_int, socket_type: c_int, protocol: c_int) -> c_int {
    trace!("demi_socket()");

    // Issue socket operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.socket(domain, socket_type, protocol) {
        Ok(qd) => {
            unsafe { *qd_out = qd.into() };
            0
        },
        Err(e) => {
            trace!("demi_socket() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// bind
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_bind(qd: c_int, saddr: *const sockaddr, size: Socklen) -> c_int {
    trace!("demi_bind()");

    // Check if socket address is invalid.
    if saddr.is_null() {
        return libc::EINVAL;
    }

    // Check if socket address length is invalid.
    if size as usize != mem::size_of::<SockAddrIn>() {
        return libc::EINVAL;
    }

    // Get socket address.
    let endpoint: SocketAddrV4 = match sockaddr_to_socketaddrv4(saddr) {
        Ok(endpoint) => endpoint,
        Err(e) => {
            trace!("demi_bind() failed: {:?}", e);
            return e.errno;
        },
    };

    // Issue bind operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.bind(qd.into(), endpoint) {
        Ok(..) => 0,
        Err(e) => {
            trace!("demi_bind() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// listen
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_listen(sockqd: c_int, backlog: c_int) -> c_int {
    trace!("demi_listen()");

    // Check if socket backlog is invalid.
    if backlog < 1 {
        return libc::EINVAL;
    }

    // Issue listen operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.listen(sockqd.into(), backlog as usize) {
        Ok(..) => 0,
        Err(e) => {
            trace!("demi_listen() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// accept
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_accept(qtok_out: *mut demi_qtoken_t, sockqd: c_int) -> c_int {
    trace!("demi_accept()");

    // Issue accept operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| {
        unsafe {
            *qtok_out = match libos.accept(sockqd.into()) {
                Ok(qt) => qt.into(),
                Err(e) => {
                    trace!("demi_accept() failed: {:?}", e);
                    return e.errno;
                },
            }
        };
        0
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// connect
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_connect(
    qtok_out: *mut demi_qtoken_t,
    sockqd: c_int,
    saddr: *const sockaddr,
    size: Socklen,
) -> c_int {
    trace!("demi_connect()");

    // Check if socket address is invalid.
    if saddr.is_null() {
        return libc::EINVAL;
    }

    // Check if socket address length is invalid.
    if size as usize != mem::size_of::<SockAddrIn>() {
        return libc::EINVAL;
    }

    // Get socket address.
    let endpoint: SocketAddrV4 = match sockaddr_to_socketaddrv4(saddr) {
        Ok(endpoint) => endpoint,
        Err(e) => {
            trace!("demi_connect() failed: {:?}", e);
            return e.errno;
        },
    };

    // Issue connect operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.connect(sockqd.into(), endpoint) {
        Ok(qt) => {
            unsafe { *qtok_out = qt.into() };
            0
        },
        Err(e) => {
            trace!("demi_connect() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// close
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_close(qd: c_int) -> c_int {
    trace!("demi_close()");

    // Issue close operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.close(qd.into()) {
        Ok(..) => 0,
        Err(e) => {
            trace!("demi_close() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// pushto
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_pushto(
    qtok_out: *mut demi_qtoken_t,
    sockqd: c_int,
    sga: *const demi_sgarray_t,
    saddr: *const sockaddr,
    size: Socklen,
) -> c_int {
    trace!("demi_pushto()");

    // Check if scatter-gather array is invalid.
    if sga.is_null() {
        return libc::EINVAL;
    }

    // Check if socket address is invalid.
    if saddr.is_null() {
        return libc::EINVAL;
    }

    // Check if socket address length is invalid.
    if size as usize != mem::size_of::<SockAddrIn>() {
        return libc::EINVAL;
    }

    let sga: &demi_sgarray_t = unsafe { &*sga };

    // Get socket address.
    let endpoint: SocketAddrV4 = match sockaddr_to_socketaddrv4(saddr) {
        Ok(endpoint) => endpoint,
        Err(e) => {
            trace!("demi_pushto() failed: {:?}", e);
            return e.errno;
        },
    };

    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.pushto(sockqd.into(), sga, endpoint) {
        Ok(qt) => {
            unsafe { *qtok_out = qt.into() };
            0
        },
        Err(e) => {
            trace!("demi_pushto() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// push
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_push(qtok_out: *mut demi_qtoken_t, qd: c_int, sga: *const demi_sgarray_t) -> c_int {
    trace!("demi_push()");

    // Check if scatter-gather array is invalid.
    if sga.is_null() {
        return libc::EINVAL;
    }

    let sga: &demi_sgarray_t = unsafe { &*sga };

    // Issue push operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.push(qd.into(), sga) {
        Ok(qt) => {
            unsafe { *qtok_out = qt.into() };
            0
        },
        Err(e) => {
            trace!("demi_push() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// pop
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_pop(qtok_out: *mut demi_qtoken_t, qd: c_int) -> c_int {
    trace!("demi_pop()");

    // Issue pop operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.pop(qd.into()) {
        Ok(qt) => {
            unsafe { *qtok_out = qt.into() };
            0
        },
        Err(e) => {
            trace!("demi_pop() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// timedwait
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_timedwait(
    qr_out: *mut demi_qresult_t,
    qt: demi_qtoken_t,
    abstime: *const libc::timespec,
) -> c_int {
    trace!("demi_timedwait() {:?} {:?} {:?}", qr_out, qt, abstime);

    // Check for invalid timeout.
    if abstime.is_null() {
        warn!("abstime is a null pointer");
        return libc::EINVAL;
    }

    // Convert timespec to SystemTime.
    let abstime: Option<SystemTime> = {
        let timeout: Duration = Duration::from_nanos(
            unsafe { (*abstime).tv_sec } as u64 * 1_000_000_000_ + unsafe { (*abstime).tv_nsec } as u64,
        );
        match SystemTime::UNIX_EPOCH.checked_add(timeout) {
            Some(abstime) => Some(abstime),
            None => Some(SystemTime::now()),
        }
    };

    // Issue operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.timedwait(qt.into(), abstime) {
        Ok(r) => {
            if !qr_out.is_null() {
                unsafe { *qr_out = r };
            }
            0
        },
        Err(e) => {
            trace!("demi_timedwait() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// wait
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_wait(qr_out: *mut demi_qresult_t, qt: demi_qtoken_t, timeout: *const libc::timespec) -> c_int {
    trace!("demi_wait() {:?} {:?} {:?}", qr_out, qt, timeout);

    // Convert timespec to Duration.
    let duration: Option<Duration> = if timeout.is_null() {
        None
    } else {
        // Safety: We have to trust that our user is providing a valid timeout pointer for us to dereference.
        Some(unsafe { Duration::new((*timeout).tv_sec as u64, (*timeout).tv_nsec as u32) })
    };

    // Issue wait operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.wait(qt.into(), duration) {
        Ok(r) => {
            if !qr_out.is_null() {
                unsafe { *qr_out = r };
            }
            0
        },
        Err(e) => {
            trace!("demi_wait() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// wait_any
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_wait_any(
    qr_out: *mut demi_qresult_t,
    ready_offset: *mut c_int,
    qts: *mut demi_qtoken_t,
    num_qts: c_int,
    timeout: *const libc::timespec,
) -> c_int {
    trace!(
        "demi_wait_any() {:?} {:?} {:?} {:?} {:?}",
        qr_out,
        ready_offset,
        qts,
        num_qts,
        timeout
    );

    // Check arguments.
    if num_qts < 0 {
        return libc::EINVAL;
    }

    // Get queue tokens.
    let qts: Vec<QToken> = {
        let raw_qts: &[u64] = unsafe { slice::from_raw_parts(qts, num_qts as usize) };
        raw_qts.iter().map(|i| QToken::from(*i)).collect()
    };

    // Convert timespec to Duration.
    let duration: Option<Duration> = if timeout.is_null() {
        None
    } else {
        // Safety: We have to trust that our user is providing a valid timeout pointer for us to dereference.
        Some(unsafe { Duration::new((*timeout).tv_sec as u64, (*timeout).tv_nsec as u32) })
    };

    // Issue wait_any operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.wait_any(&qts, duration) {
        Ok((ix, qr)) => {
            unsafe {
                *qr_out = qr;
                *ready_offset = ix as c_int;
            }
            0
        },
        Err(e) => {
            trace!("demi_wait_any() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// sgaalloc
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_sgaalloc(size: libc::size_t) -> demi_sgarray_t {
    trace!("demi_sgaalloc()");

    let null_sga: demi_sgarray_t = {
        demi_sgarray_t {
            sga_buf: ptr::null_mut() as *mut _,
            sga_numsegs: 0,
            sga_segs: [demi_sgaseg_t {
                sgaseg_buf: ptr::null_mut() as *mut c_void,
                sgaseg_len: 0,
            }; 1],
            sga_addr: unsafe { mem::zeroed() },
        }
    };

    // Issue sgaalloc operation.
    let ret: Result<demi_sgarray_t, Fail> = do_syscall(|libos| -> demi_sgarray_t {
        match libos.sgaalloc(size) {
            Ok(sga) => sga,
            Err(e) => {
                trace!("demi_sgaalloc() failed: {:?}", e);
                null_sga
            },
        }
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => {
            trace!("demi_sgaalloc() failed: {:?}", e);
            null_sga
        },
    }
}

//======================================================================================================================
// sgafree
//======================================================================================================================

#[no_mangle]
pub extern "C" fn demi_sgafree(sga: *mut demi_sgarray_t) -> c_int {
    trace!("demi_sgfree()");

    // Check if scatter-gather array is invalid.
    if sga.is_null() {
        return libc::EINVAL;
    }

    // Issue sgafree operation.
    let ret: Result<i32, Fail> = do_syscall(|libos| match libos.sgafree(unsafe { *sga }) {
        Ok(()) => 0,
        Err(e) => {
            trace!("demi_sgafree() failed: {:?}", e);
            e.errno
        },
    });

    match ret {
        Ok(ret) => ret,
        Err(e) => e.errno,
    }
}

//======================================================================================================================
// getsockname
//======================================================================================================================

#[allow(unused)]
#[no_mangle]
pub extern "C" fn demi_getsockname(qd: c_int, saddr: *mut sockaddr, size: *mut Socklen) -> c_int {
    // TODO: Implement this system call.
    libc::ENOSYS
}

//======================================================================================================================
// setsockopt
//======================================================================================================================

#[allow(unused)]
#[no_mangle]
pub extern "C" fn demi_setsockopt(
    qd: c_int,
    level: c_int,
    optname: c_int,
    optval: *const c_void,
    optlen: Socklen,
) -> c_int {
    // TODO: Implement this system call.
    libc::ENOSYS
}

//======================================================================================================================
// getsockopt
//======================================================================================================================

#[allow(unused)]
#[no_mangle]
pub extern "C" fn demi_getsockopt(
    qd: c_int,
    level: c_int,
    optname: c_int,
    optval: *mut c_void,
    optlen: *mut Socklen,
) -> c_int {
    // TODO: Implement this system call.
    libc::ENOSYS
}

//======================================================================================================================
// Standalone Functions
//======================================================================================================================

/// Issues a system call.
fn do_syscall<T>(f: impl FnOnce(&mut LibOS) -> T) -> Result<T, Fail> {
    match unsafe { DEMIKERNEL.try_borrow_mut() } {
        Ok(mut libos) => match libos.as_mut() {
            Some(libos) => Ok(f(libos)),
            None => Err(Fail::new(libc::ENOSYS, "Demikernel is not initialized")),
        },
        Err(_) => Err(Fail::new(libc::EBUSY, "Demikernel is busy")),
    }
}

/// Converts a [sockaddr] into a [SocketAddrV4].
fn sockaddr_to_socketaddrv4(saddr: *const sockaddr) -> Result<SocketAddrV4, Fail> {
    // TODO: Change the logic bellow and rename this function once we support V6 addresses as well.
    let sin: SockAddrIn = unsafe { *mem::transmute::<*const sockaddr, *const SockAddrIn>(saddr) };
    if sin.sin_family != AF_INET as u16 {
        return Err(Fail::new(libc::ENOTSUP, "communication domain not supported"));
    };
    let addr: Ipv4Addr = Ipv4Addr::from(u32::from_be(get_addr_from_sock_addr_in(&sin)));
    let port: u16 = u16::from_be(sin.sin_port);
    Ok(SocketAddrV4::new(addr, port))
}

#[test]
fn test_sockaddr_to_socketaddrv4() {
    // TODO: assign something meaningful to sa_family and check it once we support V6 addresses as well.

    // SocketAddrV4: 127.0.0.1:80
    let saddr: libc::sockaddr = {
        sockaddr {
            sa_family: AF_INET as u16,
            sa_data: [0, 80, 127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        }
    };
    match sockaddr_to_socketaddrv4(&saddr) {
        Ok(addr) => {
            assert_eq!(addr.port(), 80);
            assert_eq!(addr.ip(), &Ipv4Addr::new(127, 0, 0, 1));
        },
        _ => panic!("failed to convert"),
    }
}
