//! The floating-point environment of the host while the guest runs. The bits of FPSCR for the
//! rounding mode, flush-to-zero and default NaN sit at the same positions as in the FPCR of
//! AArch64, and the cumulative exception bits at the same positions as in its FPSR, so the host
//! computes the floating-point instructions with the semantics of the guest.

/// AHP, DN, FZ and RMode.
#[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
const CONTROL_MASK: u32 = 0x07C0_0000;
/// IDC, IXC, UFC, OFC, DZC and IOC.
#[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
const CUMULATIVE_MASK: u32 = 0x0000_009F;

pub struct HostEnvironment {
    #[cfg(target_arch = "aarch64")]
    fpcr: u64,
    #[cfg(target_arch = "aarch64")]
    fpsr: u64,
}

#[cfg(target_arch = "aarch64")]
mod imp {
    use std::arch::asm;

    pub fn read_fpcr() -> u64 {
        let value: u64;
        unsafe { asm!("mrs {}, fpcr", out(reg) value, options(nomem, nostack, preserves_flags)) };
        value
    }

    pub fn write_fpcr(value: u64) {
        unsafe { asm!("msr fpcr, {}", in(reg) value, options(nomem, nostack, preserves_flags)) };
    }

    pub fn read_fpsr() -> u64 {
        let value: u64;
        unsafe { asm!("mrs {}, fpsr", out(reg) value, options(nomem, nostack, preserves_flags)) };
        value
    }

    pub fn write_fpsr(value: u64) {
        unsafe { asm!("msr fpsr, {}", in(reg) value, options(nomem, nostack, preserves_flags)) };
    }
}

/// Switches the host to the environment of the guest and returns the one of the host.
pub fn enter_guest(fpscr: u32) -> HostEnvironment {
    #[cfg(target_arch = "aarch64")]
    {
        let host = HostEnvironment {
            fpcr: imp::read_fpcr(),
            fpsr: imp::read_fpsr(),
        };
        imp::write_fpcr((host.fpcr & !(CONTROL_MASK as u64)) | (fpscr & CONTROL_MASK) as u64);
        imp::write_fpsr(0);
        host
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = fpscr;
        HostEnvironment {}
    }
}

/// Moves the exceptions that the guest raised into its FPSCR and restores the host.
pub fn leave_guest(host: HostEnvironment, fpscr: &mut u32) {
    #[cfg(target_arch = "aarch64")]
    {
        *fpscr |= imp::read_fpsr() as u32 & CUMULATIVE_MASK;
        imp::write_fpcr(host.fpcr);
        imp::write_fpsr(host.fpsr);
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = (host, fpscr);
    }
}

/// Moves the exceptions raised so far into FPSCR without leaving the guest, for VMRS.
pub fn collect_exceptions(fpscr: &mut u32) {
    #[cfg(target_arch = "aarch64")]
    {
        *fpscr |= imp::read_fpsr() as u32 & CUMULATIVE_MASK;
        imp::write_fpsr(0);
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = fpscr;
    }
}

/// Applies a new FPSCR that the guest wrote with VMSR.
pub fn apply_control(fpscr: u32) {
    #[cfg(target_arch = "aarch64")]
    {
        let fpcr = imp::read_fpcr();
        imp::write_fpcr((fpcr & !(CONTROL_MASK as u64)) | (fpscr & CONTROL_MASK) as u64);
        imp::write_fpsr(0);
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = fpscr;
    }
}
